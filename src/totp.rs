use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::Path;
use zeroize::Zeroize;

use crate::db;
use crate::error::Result;

const KEYRING_SERVICE: &str = "nigel";

/// Build the keyring entry for a given database path.
fn keychain_entry(db_path: &Path) -> Result<keyring::Entry> {
    let canonical = db_path
        .canonicalize()
        .unwrap_or_else(|_| db_path.to_path_buf());
    let user = format!("totp:{}", canonical.display());
    keyring::Entry::new(KEYRING_SERVICE, &user)
        .map_err(|e| crate::error::NigelError::Other(format!("Keychain error: {e}")))
}

/// Generate a new TOTP secret. Returns (base32_secret, otpauth_uri).
pub fn generate_secret(issuer: &str, account: &str) -> Result<(String, String)> {
    use totp_rs::{Algorithm, Secret, TOTP};
    let secret = Secret::generate_secret();
    let secret_bytes = secret.to_bytes().map_err(|e| {
        crate::error::NigelError::Other(format!("Failed to generate TOTP secret: {e}"))
    })?;
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        Some(issuer.to_string()),
        account.to_string(),
    )
    .map_err(|e| crate::error::NigelError::Other(format!("Failed to create TOTP: {e}")))?;
    let base32 = secret.to_encoded().to_string();
    let uri = totp.get_url();
    Ok((base32, uri))
}

/// Verify a 6-digit TOTP code against a base32-encoded secret.
pub fn verify_code(base32_secret: &str, code: &str) -> bool {
    use std::time::SystemTime;
    use totp_rs::{Algorithm, Secret, TOTP};
    let secret = match Secret::Encoded(base32_secret.to_string()).to_bytes() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let totp = match TOTP::new(Algorithm::SHA1, 6, 1, 30, secret, None, String::new()) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    totp.check(code, time)
}

/// Check if TOTP is enabled for this database.
pub fn is_enabled(conn: &Connection) -> bool {
    db::get_metadata(conn, "totp_enabled")
        .map(|v| v == "true")
        .unwrap_or(false)
}

/// Get the TOTP secret from the OS keychain.
pub fn get_secret(db_path: &Path) -> Result<String> {
    let entry = keychain_entry(db_path)?;
    entry.get_password().map_err(|e| {
        crate::error::NigelError::Other(format!("Could not read TOTP secret from keychain: {e}"))
    })
}

/// Enable TOTP: store secret in OS keychain, set metadata flag.
pub fn enable(conn: &Connection, db_path: &Path, secret: &str) -> Result<()> {
    let entry = keychain_entry(db_path)?;
    entry.set_password(secret).map_err(|e| {
        crate::error::NigelError::Other(format!("Could not store TOTP secret in keychain: {e}"))
    })?;
    db::set_metadata(conn, "totp_enabled", "true")?;
    Ok(())
}

/// Unambiguous charset for recovery codes (no 0/O/1/I/L).
const RECOVERY_CHARSET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

/// Generate 8 single-use recovery codes in `XXXX-XXXX` format.
/// Returns (plaintext codes, JSON string of SHA-256 hashes).
pub fn generate_recovery_codes() -> (Vec<String>, String) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut codes = Vec::with_capacity(8);
    let mut hashes = Vec::with_capacity(8);

    for _ in 0..8 {
        let code: String = (0..8)
            .map(|i| {
                let idx = rng.gen_range(0..RECOVERY_CHARSET.len());
                let c = RECOVERY_CHARSET[idx] as char;
                if i == 4 {
                    format!("-{c}")
                } else {
                    format!("{c}")
                }
            })
            .collect();
        hashes.push(hash_code(&code));
        codes.push(code);
    }

    let json = serde_json::to_string(&hashes).unwrap();
    (codes, json)
}

/// SHA-256 hash a recovery code (strips hyphens, uppercases).
pub fn hash_code(code: &str) -> String {
    let normalized: String = code
        .chars()
        .filter(|c| *c != '-')
        .collect::<String>()
        .to_uppercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify a recovery code against the stored hashed JSON array.
/// Returns the index of the matching hash, or None.
pub fn verify_recovery_code(code: &str, hashed_json: &str) -> Option<usize> {
    let hashes: Vec<String> = serde_json::from_str(hashed_json).ok()?;
    let candidate = hash_code(code);
    hashes.iter().position(|h| h == &candidate)
}

/// Store hashed recovery codes in metadata.
pub fn store_recovery_codes(conn: &Connection, hashed_json: &str) -> Result<()> {
    db::set_metadata(conn, "totp_recovery_codes", hashed_json)
}

/// Read hashed recovery codes from metadata.
pub fn get_recovery_codes(conn: &Connection) -> Option<String> {
    db::get_metadata(conn, "totp_recovery_codes")
}

/// Remove a spent recovery code by index (set to empty string in array).
pub fn spend_recovery_code(conn: &Connection, index: usize) -> Result<()> {
    if let Some(json) = get_recovery_codes(conn) {
        if let Ok(mut hashes) = serde_json::from_str::<Vec<String>>(&json) {
            if index < hashes.len() {
                hashes[index] = String::new();
                let updated = serde_json::to_string(&hashes).unwrap();
                db::set_metadata(conn, "totp_recovery_codes", &updated)?;
            }
        }
    }
    Ok(())
}

/// Count remaining (non-empty) recovery codes.
pub fn remaining_recovery_codes(conn: &Connection) -> usize {
    get_recovery_codes(conn)
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .map(|hashes| hashes.iter().filter(|h| !h.is_empty()).count())
        .unwrap_or(0)
}

/// Delete all recovery codes from metadata.
pub fn clear_recovery_codes(conn: &Connection) {
    let _ = conn.execute("DELETE FROM metadata WHERE key = 'totp_recovery_codes'", []);
}

/// Disable TOTP: remove secret from OS keychain, clear metadata flag and recovery codes.
pub fn disable(conn: &Connection, db_path: &Path) -> Result<()> {
    let entry = keychain_entry(db_path)?;
    let _ = entry.delete_credential(); // Ignore "not found" errors
    clear_recovery_codes(conn);
    conn.execute("DELETE FROM metadata WHERE key = 'totp_enabled'", [])
        .map_err(crate::error::NigelError::Db)?;
    Ok(())
}

/// CLI helper: if TOTP is enabled, prompt for code via stdin (up to 3 attempts).
/// Falls back to recovery code if keychain is unavailable or 3 TOTP attempts fail.
pub fn prompt_totp_if_needed(db_path: &Path) -> Result<()> {
    let conn = db::get_connection(db_path)?;
    if !is_enabled(&conn) {
        return Ok(());
    }
    match get_secret(db_path) {
        Ok(mut secret) => {
            let result = (|| {
                for attempt in 1..=3 {
                    let code = rpassword::prompt_password("Authenticator code: ")
                        .map_err(|e| crate::error::NigelError::Other(e.to_string()))?;
                    if verify_code(&secret, code.trim()) {
                        return Ok(());
                    }
                    if attempt < 3 {
                        eprintln!("Invalid code. Try again ({attempt}/3).");
                    }
                }
                // 3 failures — offer recovery code
                eprintln!("3 failed attempts. You can enter a recovery code instead.");
                prompt_recovery_code(&conn, db_path)
            })();
            secret.zeroize();
            result
        }
        Err(_) => {
            eprintln!(
                "No 2FA secret found on this machine. \
                 Use a recovery code or `nigel password totp-recover` to restore access."
            );
            prompt_recovery_code(&conn, db_path)
        }
    }
}

/// Prompt for a single recovery code, verify it, spend it, and disable TOTP.
fn prompt_recovery_code(conn: &Connection, db_path: &Path) -> Result<()> {
    let hashed_json = match get_recovery_codes(conn) {
        Some(json) => json,
        None => {
            return Err(crate::error::NigelError::Other(
                "No recovery codes found. Use `nigel password totp-recover` from a machine with the keychain.".into(),
            ));
        }
    };
    let remaining = remaining_recovery_codes(conn);
    eprintln!("Recovery codes remaining: {remaining}");
    let code = rpassword::prompt_password("Recovery code: ")
        .map_err(|e| crate::error::NigelError::Other(e.to_string()))?;
    match verify_recovery_code(code.trim(), &hashed_json) {
        Some(index) => {
            spend_recovery_code(conn, index)?;
            disable(conn, db_path)?;
            eprintln!("Recovery code accepted. 2FA has been disabled.");
            Ok(())
        }
        None => Err(crate::error::NigelError::Other(
            "Invalid recovery code.".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_secret_returns_base32_and_uri() {
        let (base32, uri) = generate_secret("Nigel", "test@example.com").unwrap();
        assert!(!base32.is_empty());
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains("Nigel"));
    }

    #[test]
    fn verify_code_with_valid_code() {
        use std::time::SystemTime;
        use totp_rs::{Algorithm, Secret, TOTP};
        let secret = Secret::generate_secret();
        let base32 = secret.to_encoded().to_string();
        let secret_bytes = secret.to_bytes().unwrap();
        let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret_bytes, None, String::new()).unwrap();
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let code = totp.generate(time);
        assert!(verify_code(&base32, &code));
    }

    #[test]
    fn verify_code_rejects_wrong_code() {
        let (base32, _) = generate_secret("Nigel", "test").unwrap();
        assert!(!verify_code(&base32, "000000"));
    }

    #[test]
    fn verify_code_rejects_invalid_secret() {
        assert!(!verify_code("not-valid-base32!!!", "123456"));
    }

    #[test]
    fn is_enabled_false_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("test.db")).unwrap();
        db::init_db(&conn).unwrap();
        assert!(!is_enabled(&conn));
    }

    #[test]
    fn is_enabled_true_when_flag_set() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("test.db")).unwrap();
        db::init_db(&conn).unwrap();
        db::set_metadata(&conn, "totp_enabled", "true").unwrap();
        assert!(is_enabled(&conn));
    }

    #[test]
    fn generate_recovery_codes_format_and_count() {
        let (codes, json) = generate_recovery_codes();
        assert_eq!(codes.len(), 8);
        for code in &codes {
            assert_eq!(code.len(), 9); // XXXX-XXXX = 9 chars
            assert_eq!(&code[4..5], "-");
            // No ambiguous chars
            for c in code.chars().filter(|c| *c != '-') {
                assert!(
                    RECOVERY_CHARSET.contains(&(c as u8)),
                    "unexpected char: {c}"
                );
            }
        }
        let hashes: Vec<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(hashes.len(), 8);
    }

    #[test]
    fn verify_recovery_code_valid() {
        let (codes, json) = generate_recovery_codes();
        let idx = verify_recovery_code(&codes[3], &json);
        assert_eq!(idx, Some(3));
    }

    #[test]
    fn verify_recovery_code_case_insensitive() {
        let (codes, json) = generate_recovery_codes();
        let lower = codes[0].to_lowercase();
        let idx = verify_recovery_code(&lower, &json);
        assert_eq!(idx, Some(0));
    }

    #[test]
    fn verify_recovery_code_strips_hyphens() {
        let (codes, json) = generate_recovery_codes();
        let no_hyphen = codes[1].replace('-', "");
        let idx = verify_recovery_code(&no_hyphen, &json);
        assert_eq!(idx, Some(1));
    }

    #[test]
    fn verify_recovery_code_invalid() {
        let (_, json) = generate_recovery_codes();
        assert!(verify_recovery_code("ZZZZ-ZZZZ", &json).is_none());
    }

    #[test]
    fn spend_reduces_remaining() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("test.db")).unwrap();
        db::init_db(&conn).unwrap();
        let (_, json) = generate_recovery_codes();
        store_recovery_codes(&conn, &json).unwrap();
        assert_eq!(remaining_recovery_codes(&conn), 8);
        spend_recovery_code(&conn, 2).unwrap();
        assert_eq!(remaining_recovery_codes(&conn), 7);
    }

    #[test]
    fn clear_recovery_codes_removes_all() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("test.db")).unwrap();
        db::init_db(&conn).unwrap();
        let (_, json) = generate_recovery_codes();
        store_recovery_codes(&conn, &json).unwrap();
        assert_eq!(remaining_recovery_codes(&conn), 8);
        clear_recovery_codes(&conn);
        assert_eq!(remaining_recovery_codes(&conn), 0);
    }

    #[test]
    fn hash_code_strips_hyphens_and_uppercases() {
        let h1 = hash_code("ABCD-EFGH");
        let h2 = hash_code("abcdefgh");
        assert_eq!(h1, h2);
    }

    #[test]
    fn is_enabled_false_when_flag_not_true() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("test.db")).unwrap();
        db::init_db(&conn).unwrap();
        db::set_metadata(&conn, "totp_enabled", "false").unwrap();
        assert!(!is_enabled(&conn));
    }
}
