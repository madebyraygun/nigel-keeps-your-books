//! Where a browser upload waits between being sent and being imported.
//!
//! The importers all take a `&Path` — they open the file more than once, and
//! two of them dispatch on its extension — so an upload has to reach the disk
//! before it can be parsed. Each one gets its own directory named for its id:
//!
//! ```text
//! <data_dir>/tmp/uploads/<32 hex chars>/<the user's filename>
//! ```
//!
//! The directory carries the id so the file can keep the name the user sent,
//! which is the name `import_file` records in the `imports` table and the name
//! the import history then shows. Nothing here is authoritative state: an
//! upload is a file on disk with an mtime, and anything older than an hour is
//! collected.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rand::RngCore;

use crate::error::Result;
use crate::settings::{restrict_dir_permissions, restrict_file_permissions};

/// Uploads are collected an hour after they land. Long enough that a slow
/// preview-then-confirm never loses its file, short enough that a browser tab
/// closed mid-import does not leave a statement lying around.
pub const MAX_AGE: Duration = Duration::from_secs(60 * 60);

/// The ceiling on an upload. Bank statements are tens of kilobytes; this is
/// three orders of magnitude of headroom and still small enough to hold in
/// memory while it is written.
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// What the importers can read. The extension is not decoration: Gusto's
/// detector refuses anything not named `.xlsx`, and calamine picks its reader
/// from the extension, so this is also what the stored file must keep.
pub const ALLOWED_EXTENSIONS: [&str; 3] = ["csv", "xlsx", "xls"];

const ID_BYTES: usize = 16;
const MAX_FILENAME_BYTES: usize = 100;

/// A file waiting to be imported.
#[derive(Debug, Clone)]
pub struct StoredUpload {
    pub id: String,
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
}

/// The spool directory for the database the server actually opened — not
/// `settings.json`, which another process is free to repoint mid-run.
pub fn uploads_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("tmp")
        .join("uploads")
}

pub fn new_id() -> String {
    let mut bytes = [0u8; ID_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Reduce whatever the browser sent to a name that is safe to join onto a path
/// and that an importer can still read.
///
/// Errors carry the message the client is shown.
pub fn sanitize_filename(raw: &str) -> std::result::Result<String, String> {
    // `file_name` drops every directory component, `..` among them.
    let base = Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .trim();
    if base.is_empty() || base == "." || base == ".." {
        return Err("The uploaded file has no usable filename.".to_string());
    }

    let (stem, extension) = match base.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, ext.to_ascii_lowercase()),
        _ => return Err(unsupported_extension(base)),
    };
    if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(unsupported_extension(base));
    }

    let mut safe: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // A leading dot would make the stored file hidden, and a stem of nothing
    // but separators leaves no name at all.
    while safe.starts_with('.') {
        safe.remove(0);
    }
    if safe.is_empty() {
        safe.push_str("upload");
    }
    safe.truncate(MAX_FILENAME_BYTES.saturating_sub(extension.len() + 1));

    Ok(format!("{safe}.{extension}"))
}

fn unsupported_extension(name: &str) -> String {
    let types = ALLOWED_EXTENSIONS
        .iter()
        .map(|ext| format!(".{ext}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("'{name}' is not a supported file type — upload one of: {types}.")
}

/// Write an upload to its own directory and lock both down to the owner.
pub fn store(dir: &Path, filename: &str, bytes: &[u8]) -> Result<StoredUpload> {
    std::fs::create_dir_all(dir)?;
    restrict_dir_permissions(dir)?;

    let id = new_id();
    let upload_dir = dir.join(&id);
    std::fs::create_dir_all(&upload_dir)?;
    restrict_dir_permissions(&upload_dir)?;

    let path = upload_dir.join(filename);
    let mut file = std::fs::File::create(&path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    restrict_file_permissions(&path)?;

    Ok(StoredUpload {
        id,
        path,
        filename: filename.to_string(),
        size: bytes.len() as u64,
    })
}

/// Find a stored upload by id, or nothing if it never existed or has been
/// collected. Ids that are not the shape this module hands out are rejected
/// before the filesystem is touched, since the id is joined onto a path.
pub fn resolve(dir: &Path, id: &str) -> Option<StoredUpload> {
    if !is_valid_id(id) {
        return None;
    }
    let upload_dir = dir.join(id);
    let entry = std::fs::read_dir(&upload_dir)
        .ok()?
        .flatten()
        .find(|entry| entry.path().is_file())?;
    let path = entry.path();

    Some(StoredUpload {
        id: id.to_string(),
        filename: path.file_name()?.to_str()?.to_string(),
        size: entry.metadata().ok()?.len(),
        path,
    })
}

/// Drop an upload once it has been imported. Best effort: a file we cannot
/// remove is a temp file, and the hourly sweep will try again.
pub fn delete(dir: &Path, id: &str) {
    if is_valid_id(id) {
        let _ = std::fs::remove_dir_all(dir.join(id));
    }
}

/// Remove uploads older than `max_age`. Runs at startup and before every new
/// upload, so an abandoned file never outlives the hour by much.
pub fn purge_stale(dir: &Path, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .map(|modified| now.duration_since(modified).is_ok_and(|age| age > max_age))
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Lowercase because that is what `hex::encode` produces, and because a
/// case-insensitive filesystem would otherwise resolve an id this module never
/// handed out.
fn is_valid_id(id: &str) -> bool {
    id.len() == ID_BYTES * 2
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spool() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tmp/uploads");
        (dir, path)
    }

    #[test]
    fn uploads_live_beside_the_database_the_server_opened() {
        let dir = uploads_dir(Path::new("/books/nigel.db"));
        assert_eq!(dir, PathBuf::from("/books/tmp/uploads"));
    }

    #[test]
    fn ids_are_thirty_two_hex_characters_and_do_not_repeat() {
        let (a, b) = (new_id(), new_id());
        assert_eq!(a.len(), ID_BYTES * 2);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn sanitizing_keeps_a_reasonable_name_intact() {
        assert_eq!(sanitize_filename("jan-2025.csv").unwrap(), "jan-2025.csv");
        assert_eq!(
            sanitize_filename("Payroll_Q1.XLSX").unwrap(),
            "Payroll_Q1.xlsx"
        );
    }

    #[test]
    fn sanitizing_strips_directories_and_traversal() {
        assert_eq!(
            sanitize_filename("../../etc/passwd.csv").unwrap(),
            "passwd.csv"
        );
        assert_eq!(
            sanitize_filename("/tmp/statement.csv").unwrap(),
            "statement.csv"
        );
        assert!(sanitize_filename("../..").is_err());
        assert!(sanitize_filename("").is_err());
    }

    #[test]
    fn sanitizing_replaces_hostile_characters_and_hidden_names() {
        assert_eq!(
            sanitize_filename("my state;ment $1.csv").unwrap(),
            "my_state_ment__1.csv"
        );
        assert_eq!(sanitize_filename(".hidden.csv").unwrap(), "hidden.csv");
        assert_eq!(sanitize_filename("...csv").unwrap(), "upload.csv");
    }

    #[test]
    fn sanitizing_rejects_types_no_importer_can_read() {
        for name in ["notes.txt", "report.pdf", "statement", "archive.csv.gz"] {
            assert!(
                sanitize_filename(name).is_err(),
                "{name} should not be accepted"
            );
        }
    }

    #[test]
    fn sanitizing_truncates_a_long_name_but_keeps_the_extension() {
        let name = format!("{}.csv", "a".repeat(500));
        let safe = sanitize_filename(&name).unwrap();
        assert!(safe.len() <= MAX_FILENAME_BYTES, "{}", safe.len());
        assert!(safe.ends_with(".csv"));
    }

    #[test]
    fn storing_then_resolving_round_trips() {
        let (_dir, spool) = spool();
        let stored = store(&spool, "jan.csv", b"date,desc\n").unwrap();

        assert_eq!(stored.filename, "jan.csv");
        assert_eq!(stored.size, 10);
        assert!(stored.path.ends_with("jan.csv"));
        assert_eq!(std::fs::read(&stored.path).unwrap(), b"date,desc\n");

        let found = resolve(&spool, &stored.id).expect("resolves");
        assert_eq!(found.path, stored.path);
        assert_eq!(found.filename, "jan.csv");
        assert_eq!(found.size, 10);
    }

    #[cfg(unix)]
    #[test]
    fn stored_uploads_are_readable_only_by_their_owner() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, spool) = spool();
        let stored = store(&spool, "jan.csv", b"x").unwrap();
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;

        assert_eq!(mode(&stored.path), 0o600);
        assert_eq!(mode(&spool.join(&stored.id)), 0o700);
        assert_eq!(mode(&spool), 0o700);
    }

    #[test]
    fn resolving_rejects_ids_that_are_not_ours() {
        let (_dir, spool) = spool();
        store(&spool, "jan.csv", b"x").unwrap();

        for id in [
            "",
            "../..",
            "not-hex-at-all",
            &"a".repeat(31),
            "A".repeat(32).as_str(),
        ] {
            assert!(resolve(&spool, id).is_none(), "{id} should not resolve");
        }
        assert!(resolve(&spool, &new_id()).is_none());
    }

    /// The id guard has to do the rejecting on its own: every case above is one
    /// the filesystem would also turn away, so a guard that accepted them all
    /// would still pass that test.
    #[test]
    fn the_id_guard_rejects_before_the_filesystem_is_consulted() {
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("../.."));
        assert!(!is_valid_id(&"a".repeat(31)));
        assert!(!is_valid_id(&"a".repeat(33)));
        assert!(!is_valid_id(&"g".repeat(32)));
        // Uppercase hex is not what `new_id` hands out, and a case-insensitive
        // filesystem would resolve it to a real upload.
        assert!(!is_valid_id(&"A".repeat(32)));
        assert!(is_valid_id(&new_id()));
    }

    #[test]
    fn an_uppercase_spelling_of_a_real_id_does_not_resolve() {
        let (_dir, spool) = spool();
        let stored = store(&spool, "jan.csv", b"x").unwrap();

        assert!(resolve(&spool, &stored.id).is_some());
        assert!(resolve(&spool, &stored.id.to_uppercase()).is_none());
    }

    #[test]
    fn deleting_removes_the_whole_upload() {
        let (_dir, spool) = spool();
        let stored = store(&spool, "jan.csv", b"x").unwrap();

        delete(&spool, &stored.id);

        assert!(resolve(&spool, &stored.id).is_none());
        assert!(!spool.join(&stored.id).exists());
        assert_eq!(std::fs::read_dir(&spool).unwrap().count(), 0);
    }

    #[test]
    fn purging_collects_the_old_and_spares_the_new() {
        let (_dir, spool) = spool();
        let fresh = store(&spool, "fresh.csv", b"x").unwrap();
        let stale = store(&spool, "stale.csv", b"x").unwrap();

        let long_ago = SystemTime::now() - Duration::from_secs(2 * 60 * 60);
        std::fs::File::open(spool.join(&stale.id))
            .unwrap()
            .set_modified(long_ago)
            .unwrap();

        purge_stale(&spool, MAX_AGE);

        assert!(resolve(&spool, &fresh.id).is_some());
        assert!(resolve(&spool, &stale.id).is_none());
    }

    #[test]
    fn purging_a_directory_that_is_not_there_is_not_an_error() {
        let (_dir, spool) = spool();
        purge_stale(&spool, MAX_AGE);
    }
}
