# Database Encryption via SQLCipher — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add SQLCipher database encryption so that when a user sets a password (onboarding or later), the database file is encrypted at rest.

**Architecture:** Switch rusqlite from `bundled` to `bundled-sqlcipher`. Store the runtime password in a global `Mutex<Option<String>>` in db.rs — `get_connection()` reads it internally so zero call-site changes are needed. Detect encrypted databases at startup via a probe query; prompt for password with `rpassword`. Provide `nigel password set/change/remove` CLI + dashboard TUI for managing encryption on existing databases.

**Tech Stack:** rusqlite (bundled-sqlcipher), rpassword, zeroize (already present), ratatui (for TUI password screen)

**Design doc:** `docs/plans/2026-03-01-database-encryption-design.md`

**PR note:** Issue #66 requires the PR be created as a DRAFT.

---

### Task 1: Switch to bundled-sqlcipher and add rpassword

**Files:**
- Modify: `Cargo.toml:14` (rusqlite features), add rpassword dep

**Step 1: Update Cargo.toml**

In `Cargo.toml`, change line 14 from:
```toml
rusqlite = { version = "0.31", features = ["bundled", "backup"] }
```
to:
```toml
rusqlite = { version = "0.31", features = ["bundled-sqlcipher", "backup"] }
```

Add `rpassword` dependency (after the existing deps):
```toml
rpassword = "7"
```

**Step 2: Verify it builds**

Run: `cargo build 2>&1 | tail -20`
Expected: Compiles successfully. SQLCipher builds from vendored source (may take longer than usual on first build).

**Step 3: Run existing tests**

Run: `cargo test`
Expected: All 173 tests pass — SQLCipher is backwards-compatible with plain SQLite databases.

**Step 4: Commit**

```
feat: switch rusqlite to bundled-sqlcipher for encryption support

Add rpassword dependency for masked password input.
Ref #66
```

---

### Task 2: Add global password and PRAGMA key to get_connection

**Files:**
- Modify: `src/db.rs:1-5` (imports), `src/db.rs:125-129` (get_connection)
- Test: `src/db.rs` (existing test module, around line 163)

**Step 1: Write failing tests**

Add to the `#[cfg(test)]` module in `src/db.rs` (after existing tests):

```rust
#[test]
fn test_encrypted_db_cannot_open_without_password() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("encrypted.db");

    // Create encrypted DB
    set_db_password(Some("secret".into()));
    let conn = get_connection(&db_path).unwrap();
    init_db(&conn).unwrap();
    drop(conn);

    // Try opening without password — should fail
    set_db_password(None);
    let conn = get_connection(&db_path).unwrap();
    let result = conn.execute_batch("SELECT count(*) FROM sqlite_master;");
    assert!(result.is_err());
}

#[test]
fn test_encrypted_db_opens_with_correct_password() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("encrypted.db");

    set_db_password(Some("secret".into()));
    let conn = get_connection(&db_path).unwrap();
    init_db(&conn).unwrap();
    drop(conn);

    // Reopen with same password
    set_db_password(Some("secret".into()));
    let conn = get_connection(&db_path).unwrap();
    let result: i64 = conn
        .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
        .unwrap();
    assert!(result > 0);
}

#[test]
fn test_unencrypted_db_works_without_password() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("plain.db");

    set_db_password(None);
    let conn = get_connection(&db_path).unwrap();
    init_db(&conn).unwrap();
    drop(conn);

    set_db_password(None);
    let conn = get_connection(&db_path).unwrap();
    let result: i64 = conn
        .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
        .unwrap();
    assert!(result > 0);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib db::tests::test_encrypted`
Expected: FAIL — `set_db_password` doesn't exist yet.

**Step 3: Add global password state and update get_connection**

At the top of `src/db.rs`, update imports (line 1):
```rust
use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::Result;

static DB_PASSWORD: Mutex<Option<String>> = Mutex::new(None);

/// Set the global database password. Call before `get_connection()`.
pub fn set_db_password(password: Option<String>) {
    *DB_PASSWORD.lock().unwrap() = password;
}

/// Read the current global database password.
pub fn get_db_password() -> Option<String> {
    DB_PASSWORD.lock().unwrap().clone()
}
```

Update `get_connection` (currently lines 125-129):
```rust
pub fn get_connection(db_path: &Path) -> Result<Connection> {
    let password = get_db_password();
    open_connection(db_path, password.as_deref())
}

/// Open a connection with an explicit password (bypasses global state).
/// Used by backup, password management, and tests.
pub fn open_connection(db_path: &Path, password: Option<&str>) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    if let Some(pw) = password {
        conn.pragma_update(None, "key", pw)?;
    }
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}
```

**Step 4: Run tests**

Run: `cargo test --lib db::tests`
Expected: All db tests pass (old + new).

**Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass. Existing tests use `get_connection` with no password set, which should behave identically to before.

Note: Tests that call `set_db_password` modify global state. If tests run in parallel and interfere, add cleanup (`set_db_password(None)`) at the start of each encryption test. Alternatively, use `open_connection` directly in tests to avoid global state.

**Step 6: Commit**

```
feat: add PRAGMA key support to get_connection for SQLCipher

Adds global password via Mutex<Option<String>> in db.rs.
get_connection() reads it internally — no call-site changes needed.
Exposes open_connection() for explicit password use in backups/tests.
Ref #66
```

---

### Task 3: Add encryption detection helper

**Files:**
- Modify: `src/db.rs` (add `is_encrypted` function)

**Step 1: Write failing test**

Add to db.rs test module:

```rust
#[test]
fn test_is_encrypted_returns_false_for_plain_db() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("plain.db");
    set_db_password(None);
    let conn = get_connection(&db_path).unwrap();
    init_db(&conn).unwrap();
    drop(conn);
    assert!(!is_encrypted(&db_path).unwrap());
}

#[test]
fn test_is_encrypted_returns_true_for_encrypted_db() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("encrypted.db");
    set_db_password(Some("secret".into()));
    let conn = get_connection(&db_path).unwrap();
    init_db(&conn).unwrap();
    drop(conn);
    set_db_password(None);
    assert!(is_encrypted(&db_path).unwrap());
}

#[test]
fn test_is_encrypted_returns_false_for_nonexistent_db() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("nope.db");
    assert!(!is_encrypted(&db_path).unwrap());
}
```

**Step 2: Run to verify failure**

Run: `cargo test --lib db::tests::test_is_encrypted`
Expected: FAIL — `is_encrypted` not defined.

**Step 3: Implement is_encrypted**

Add to `src/db.rs` after `open_connection`:

```rust
/// Check whether a database file is encrypted (requires a password to open).
/// Returns false for nonexistent files (they will be created fresh).
pub fn is_encrypted(db_path: &Path) -> Result<bool> {
    if !db_path.exists() {
        return Ok(false);
    }
    let conn = Connection::open(db_path)?;
    match conn.execute_batch("SELECT count(*) FROM sqlite_master;") {
        Ok(_) => Ok(false),
        Err(e) if e.to_string().contains("not a database") => Ok(true),
        Err(e) => Err(e.into()),
    }
}
```

**Step 4: Run tests**

Run: `cargo test --lib db::tests::test_is_encrypted`
Expected: All 3 pass.

**Step 5: Commit**

```
feat: add is_encrypted() helper to detect encrypted databases

Probes the database with a no-key connection and checks for
SQLITE_NOTADB error. Ref #66
```

---

### Task 4: Add password prompt for CLI subcommands

**Files:**
- Modify: `src/main.rs` (add prompt before dispatch)
- Modify: `src/db.rs` (add prompt helper)

**Step 1: Add prompt_password_if_needed to db.rs**

Add to `src/db.rs`:

```rust
/// If the database is encrypted, prompt the user for a password (up to 3 attempts).
/// Sets the global password on success. Returns an error after 3 failures.
pub fn prompt_password_if_needed(db_path: &Path) -> Result<()> {
    if !is_encrypted(db_path)? {
        return Ok(());
    }
    for attempt in 1..=3 {
        let pw = rpassword::prompt_password("Database password: ")
            .map_err(|e| crate::error::NigelError::Other(e.to_string()))?;
        set_db_password(Some(pw));
        // Verify the password works
        let conn = get_connection(db_path)?;
        match conn.execute_batch("SELECT count(*) FROM sqlite_master;") {
            Ok(_) => return Ok(()),
            Err(_) => {
                set_db_password(None);
                if attempt < 3 {
                    eprintln!("Wrong password. Try again ({}/3).", attempt);
                }
            }
        }
    }
    Err(crate::error::NigelError::Other(
        "Failed to unlock database after 3 attempts. The database may be corrupted.".into(),
    ))
}
```

Add `use rpassword;` is not needed — the function call uses the full path `rpassword::prompt_password`.

**Step 2: Check error type**

Open `src/error.rs` and verify there's an `Other(String)` variant or similar on `NigelError`. If not, add one or use whichever variant handles arbitrary error messages. Adjust the error construction in the function above to match.

**Step 3: Wire into main.rs dispatch**

In `src/main.rs`, in the `dispatch` function (around line 43), add a password check at the top before the match:

```rust
fn dispatch(command: Commands) -> Result<()> {
    // Prompt for password if database is encrypted (skip for init/demo which may create new DBs)
    if !matches!(command, Commands::Init { .. } | Commands::Demo) {
        let data_dir = crate::settings::get_data_dir();
        let db_path = data_dir.join("nigel.db");
        if db_path.exists() {
            crate::db::prompt_password_if_needed(&db_path)?;
        }
    }

    match command {
        // ... existing arms unchanged
    }
}
```

**Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles. No test for this step (interactive prompt is hard to unit-test).

**Step 5: Commit**

```
feat: prompt for password on CLI launch when database is encrypted

Adds prompt_password_if_needed() which detects encryption and prompts
via rpassword with 3 attempts. Wired into main.rs dispatch for all
subcommands except init/demo. Ref #66
```

---

### Task 5: Wire onboarding password through dashboard

**Files:**
- Modify: `src/cli/dashboard.rs:799-820` (onboarding result handling + connection)

**Step 1: Pass onboarding password to global state**

In `src/cli/dashboard.rs`, in the `run()` function, after the onboarding result is consumed (around line 809), add password handling before the connection is opened:

Find the block (approximately lines 798-810):
```rust
if let Some(result) = super::onboarding::run()? {
    let mut settings = load_settings();
    if !result.user_name.is_empty() {
        settings.user_name = result.user_name;
    }
    save_settings(&settings)?;

    if !result.company_name.is_empty() {
        onboarding_company = Some(result.company_name);
    }
    post_setup_action = Some(result.action);
}
```

Add password handling inside the `if let Some(result)` block, after `post_setup_action`:
```rust
    if let Some(ref pw) = result.password {
        crate::db::set_db_password(Some(pw.clone()));
    }
```

**Step 2: Add encryption detection for returning users**

Before the connection is opened at line 820, add a password prompt for returning users (not first run):

Find the line (approximately 820):
```rust
let conn = crate::db::get_connection(&data_dir.join("nigel.db"))?;
```

Add before it:
```rust
if !is_first_run {
    let db_path = data_dir.join("nigel.db");
    if db_path.exists() {
        crate::db::prompt_password_if_needed(&db_path)?;
    }
}
```

**Step 3: Build and verify**

Run: `cargo build`
Expected: Compiles.

**Step 4: Run full test suite**

Run: `cargo test`
Expected: All tests pass (onboarding tests don't exercise the dashboard wiring).

**Step 5: Commit**

```
feat: wire onboarding password to database encryption

Sets global DB password from OnboardingResult before first connection.
Adds encryption detection + prompt for returning users. Ref #66
```

---

### Task 6: Update backup to preserve encryption

**Files:**
- Modify: `src/cli/backup.rs:11-19` (snapshot function)

**Step 1: Write failing test**

Add a test file or add to backup.rs (if it has a test module). If no test module exists, add one:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db, is_encrypted, open_connection, set_db_password};

    #[test]
    fn test_snapshot_preserves_encryption() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.db");
        let dst_path = dir.path().join("backup.db");

        // Create encrypted source
        set_db_password(Some("secret".into()));
        let conn = get_connection(&src_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Snapshot
        let src_conn = get_connection(&src_path).unwrap();
        snapshot(&src_conn, &dst_path).unwrap();
        drop(src_conn);

        // Verify backup is also encrypted
        set_db_password(None);
        assert!(is_encrypted(&dst_path).unwrap());

        // Verify backup opens with same password
        let conn = open_connection(&dst_path, Some("secret")).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_snapshot_plain_stays_plain() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.db");
        let dst_path = dir.path().join("backup.db");

        set_db_password(None);
        let conn = get_connection(&src_path).unwrap();
        init_db(&conn).unwrap();

        snapshot(&conn, &dst_path).unwrap();
        drop(conn);

        assert!(!is_encrypted(&dst_path).unwrap());
    }
}
```

**Step 2: Run to verify failure**

Run: `cargo test --lib cli::backup::tests`
Expected: `test_snapshot_preserves_encryption` FAILS — destination is not encrypted.

**Step 3: Update snapshot to apply password**

In `src/cli/backup.rs`, update the `snapshot` function. Currently line 15:
```rust
let mut dest_conn = rusqlite::Connection::open(dest_path)?;
```

Replace the `snapshot` function:
```rust
pub fn snapshot(conn: &rusqlite::Connection, dest_path: &Path) -> Result<()> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let password = crate::db::get_db_password();
    let mut dest_conn = rusqlite::Connection::open(dest_path)?;
    if let Some(ref pw) = password {
        dest_conn.pragma_update(None, "key", pw)?;
    }
    let backup = Backup::new(conn, &mut dest_conn)?;
    backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
    Ok(())
}
```

**Step 4: Run tests**

Run: `cargo test --lib cli::backup::tests`
Expected: Both tests pass.

**Step 5: Run full suite**

Run: `cargo test`
Expected: All tests pass.

**Step 6: Commit**

```
feat: preserve encryption state in backups and snapshots

snapshot() now applies the global DB password to the destination
connection, so encrypted databases produce encrypted backups. Ref #66
```

---

### Task 7: Add nigel password CLI subcommand

**Files:**
- Create: `src/cli/password.rs`
- Modify: `src/cli/mod.rs` (add module + Commands variant)
- Modify: `src/main.rs` (add dispatch arm)

**Step 1: Add the subcommand to cli/mod.rs**

Add `pub mod password;` to the module declarations at the top of `src/cli/mod.rs`.

Add a `PasswordCommand` enum and a `Password` variant to `Commands`:

```rust
#[derive(Subcommand)]
pub enum PasswordCommand {
    /// Set a password on an unencrypted database
    Set,
    /// Change the password on an encrypted database
    Change,
    /// Remove the password (decrypt the database)
    Remove,
}
```

Add to the `Commands` enum (before the closing brace):
```rust
    /// Manage database password
    Password {
        #[command(subcommand)]
        command: PasswordCommand,
    },
```

**Step 2: Create src/cli/password.rs**

```rust
use std::path::Path;

use rusqlite::backup::Backup;

use crate::db::{get_db_password, is_encrypted, open_connection, set_db_password};
use crate::error::Result;
use crate::settings::get_data_dir;

/// Encrypt an unencrypted database with a new password.
fn encrypt_database(db_path: &Path, new_password: &str) -> Result<()> {
    let src = open_connection(db_path, None)?;
    let tmp_path = db_path.with_extension("db.encrypting");
    let mut dst = open_connection(&tmp_path, Some(new_password))?;
    let backup = Backup::new(&src, &mut dst)?;
    backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
    drop(backup);
    drop(src);
    drop(dst);
    // Remove WAL/SHM files from source before replacing
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    std::fs::rename(&tmp_path, db_path)?;
    let _ = std::fs::remove_file(tmp_path.with_extension("db.encrypting-wal"));
    let _ = std::fs::remove_file(tmp_path.with_extension("db.encrypting-shm"));
    Ok(())
}

/// Decrypt an encrypted database (remove password).
fn decrypt_database(db_path: &Path, current_password: &str) -> Result<()> {
    let src = open_connection(db_path, Some(current_password))?;
    let tmp_path = db_path.with_extension("db.decrypting");
    let mut dst = open_connection(&tmp_path, None)?;
    let backup = Backup::new(&src, &mut dst)?;
    backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
    drop(backup);
    drop(src);
    drop(dst);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    std::fs::rename(&tmp_path, db_path)?;
    let _ = std::fs::remove_file(tmp_path.with_extension("db.decrypting-wal"));
    let _ = std::fs::remove_file(tmp_path.with_extension("db.decrypting-shm"));
    Ok(())
}

/// Change the password on an already-encrypted database.
fn rekey_database(db_path: &Path, current_password: &str, new_password: &str) -> Result<()> {
    let conn = open_connection(db_path, Some(current_password))?;
    // Verify current password is correct
    conn.execute_batch("SELECT count(*) FROM sqlite_master;")?;
    conn.pragma_update(None, "rekey", new_password)?;
    Ok(())
}

fn prompt(msg: &str) -> Result<String> {
    rpassword::prompt_password(msg)
        .map_err(|e| crate::error::NigelError::Other(e.to_string()))
}

fn prompt_and_confirm(msg: &str) -> Result<String> {
    let pw1 = prompt(msg)?;
    let pw2 = prompt("Confirm password: ")?;
    if pw1.trim() != pw2.trim() {
        return Err(crate::error::NigelError::Other(
            "Passwords do not match.".into(),
        ));
    }
    Ok(pw1.trim().to_string())
}

pub fn run_set() -> Result<()> {
    let db_path = get_data_dir().join("nigel.db");
    if is_encrypted(&db_path)? {
        eprintln!("Database is already encrypted. Use `nigel password change` instead.");
        return Ok(());
    }
    let new_pw = prompt_and_confirm("New password: ")?;
    if new_pw.is_empty() {
        eprintln!("Password cannot be empty.");
        return Ok(());
    }
    encrypt_database(&db_path, &new_pw)?;
    set_db_password(Some(new_pw));
    println!("Database encrypted successfully.");
    Ok(())
}

pub fn run_change() -> Result<()> {
    let db_path = get_data_dir().join("nigel.db");
    if !is_encrypted(&db_path)? {
        eprintln!("Database is not encrypted. Use `nigel password set` instead.");
        return Ok(());
    }
    let current_pw = prompt("Current password: ")?;
    let new_pw = prompt_and_confirm("New password: ")?;
    if new_pw.is_empty() {
        eprintln!("New password cannot be empty. Use `nigel password remove` to decrypt.");
        return Ok(());
    }
    rekey_database(&db_path, current_pw.trim(), &new_pw)?;
    set_db_password(Some(new_pw));
    println!("Password changed successfully.");
    Ok(())
}

pub fn run_remove() -> Result<()> {
    let db_path = get_data_dir().join("nigel.db");
    if !is_encrypted(&db_path)? {
        eprintln!("Database is not already encrypted.");
        return Ok(());
    }
    let current_pw = prompt("Current password: ")?;
    decrypt_database(&db_path, current_pw.trim())?;
    set_db_password(None);
    println!("Database decrypted successfully. Password removed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db, set_db_password};

    #[test]
    fn test_encrypt_then_decrypt_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create plain DB
        set_db_password(None);
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        assert!(!is_encrypted(&db_path).unwrap());

        // Encrypt it
        encrypt_database(&db_path, "mypass").unwrap();
        assert!(is_encrypted(&db_path).unwrap());

        // Verify data survived
        let conn = open_connection(&db_path, Some("mypass")).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count > 0);
        drop(conn);

        // Decrypt it
        decrypt_database(&db_path, "mypass").unwrap();
        assert!(!is_encrypted(&db_path).unwrap());

        // Verify data survived
        set_db_password(None);
        let conn = get_connection(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_rekey_changes_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create encrypted DB
        set_db_password(Some("old".into()));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Change password
        rekey_database(&db_path, "old", "new").unwrap();

        // Old password should fail
        let conn = open_connection(&db_path, Some("old")).unwrap();
        assert!(conn
            .execute_batch("SELECT count(*) FROM sqlite_master;")
            .is_err());

        // New password should work
        let conn = open_connection(&db_path, Some("new")).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_encrypt_wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        set_db_password(Some("correct".into()));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Try opening with wrong password
        let conn = open_connection(&db_path, Some("wrong")).unwrap();
        assert!(conn
            .execute_batch("SELECT count(*) FROM sqlite_master;")
            .is_err());
    }
}
```

**Step 3: Add dispatch in main.rs**

In `src/main.rs`, add the password command dispatch arm:

```rust
Commands::Password { command } => match command {
    cli::PasswordCommand::Set => cli::password::run_set(),
    cli::PasswordCommand::Change => cli::password::run_change(),
    cli::PasswordCommand::Remove => cli::password::run_remove(),
},
```

Also skip password detection for the Password command itself in the dispatch guard:

```rust
if !matches!(command, Commands::Init { .. } | Commands::Demo | Commands::Password { .. }) {
```

**Step 4: Run tests**

Run: `cargo test --lib cli::password::tests`
Expected: All 3 tests pass.

**Step 5: Run full suite**

Run: `cargo test`
Expected: All tests pass.

**Step 6: Commit**

```
feat: add nigel password set/change/remove subcommand

Supports encrypting unencrypted DBs (via backup + rekey),
changing passwords (via PRAGMA rekey), and decrypting
(via backup to plain). Ref #66
```

---

### Task 8: Add dashboard password manager TUI screen

**Files:**
- Create: `src/cli/password_manager.rs`
- Modify: `src/cli/mod.rs` (add module)
- Modify: `src/cli/dashboard.rs` (add menu item + screen variant + dispatch)

**Step 1: Add module to cli/mod.rs**

Add `pub mod password_manager;` to the module declarations.

**Step 2: Create password_manager.rs**

Build an inline TUI screen following the pattern of `reconcile_manager.rs` (form-style with status messages). The screen should:
- Detect current encryption state
- Show appropriate options: Set (if plain), Change/Remove (if encrypted)
- Use masked text input fields (reuse the pattern from onboarding)
- Show success/error messages inline

```rust
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::time::Duration;
use zeroize::Zeroize;

use crate::db;
use crate::error::Result;
use crate::settings::get_data_dir;
use crate::tui::{FOOTER_STYLE, HEADER_STYLE, SELECTED_STYLE};

enum Action {
    Set,
    Change,
    Remove,
}

const ACTIONS_ENCRYPTED: &[(&str, Action)] = &[
    ("Change password", Action::Change),
    ("Remove password", Action::Remove),
];

const ACTIONS_PLAIN: &[(&str, Action)] = &[("Set password", Action::Set)];

enum Phase {
    Menu,
    InputCurrent,
    InputNew,
    InputConfirm,
    Result(String, bool), // message, is_success
}

pub struct PasswordManager {
    encrypted: bool,
    selection: usize,
    phase: Phase,
    current_pw: String,
    new_pw: String,
    confirm_pw: String,
    cursor: usize,
    chosen_action: Option<Action>,
}

impl PasswordManager {
    pub fn new() -> Result<Self> {
        let db_path = get_data_dir().join("nigel.db");
        let encrypted = db::is_encrypted(&db_path)?;
        Ok(Self {
            encrypted,
            selection: 0,
            phase: Phase::Menu,
            current_pw: String::new(),
            new_pw: String::new(),
            confirm_pw: String::new(),
            cursor: 0,
            chosen_action: None,
        })
    }

    pub fn is_done(&self) -> bool {
        false // never auto-closes; user presses Esc
    }

    fn actions(&self) -> &[(&str, Action)] {
        if self.encrypted {
            ACTIONS_ENCRYPTED
        } else {
            ACTIONS_PLAIN
        }
    }

    fn active_input(&self) -> &str {
        match self.phase {
            Phase::InputCurrent => &self.current_pw,
            Phase::InputNew => &self.new_pw,
            Phase::InputConfirm => &self.confirm_pw,
            _ => "",
        }
    }

    fn active_input_mut(&mut self) -> &mut String {
        match self.phase {
            Phase::InputCurrent => &mut self.current_pw,
            Phase::InputNew => &mut self.new_pw,
            Phase::InputConfirm => &mut self.confirm_pw,
            _ => unreachable!(),
        }
    }

    fn execute(&mut self) {
        let db_path = get_data_dir().join("nigel.db");
        let result = match self.chosen_action {
            Some(Action::Set) => {
                if self.new_pw.trim() != self.confirm_pw.trim() {
                    Err("Passwords do not match.".to_string())
                } else if self.new_pw.trim().is_empty() {
                    Err("Password cannot be empty.".to_string())
                } else {
                    self.do_encrypt(&db_path)
                }
            }
            Some(Action::Change) => {
                if self.new_pw.trim() != self.confirm_pw.trim() {
                    Err("Passwords do not match.".to_string())
                } else if self.new_pw.trim().is_empty() {
                    Err("New password cannot be empty.".to_string())
                } else {
                    self.do_change(&db_path)
                }
            }
            Some(Action::Remove) => self.do_remove(&db_path),
            None => Err("No action selected.".to_string()),
        };

        match result {
            Ok(msg) => {
                self.encrypted = !self.encrypted || matches!(self.chosen_action, Some(Action::Change));
                self.phase = Phase::Result(msg, true);
            }
            Err(msg) => {
                self.phase = Phase::Result(msg, false);
            }
        }
    }

    fn do_encrypt(&self, db_path: &std::path::Path) -> std::result::Result<String, String> {
        // Use open_connection to encrypt via backup
        let src = db::open_connection(db_path, None).map_err(|e| e.to_string())?;
        let tmp = db_path.with_extension("db.encrypting");
        let pw = self.new_pw.trim();
        let mut dst = db::open_connection(&tmp, Some(pw)).map_err(|e| e.to_string())?;
        let backup = rusqlite::backup::Backup::new(&src, &mut dst).map_err(|e| e.to_string())?;
        backup
            .run_to_completion(100, Duration::from_millis(10), None)
            .map_err(|e| e.to_string())?;
        drop(backup);
        drop(src);
        drop(dst);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
        std::fs::rename(&tmp, db_path).map_err(|e| e.to_string())?;
        db::set_db_password(Some(pw.to_string()));
        Ok("Database encrypted successfully.".into())
    }

    fn do_change(&self, db_path: &std::path::Path) -> std::result::Result<String, String> {
        let conn = db::open_connection(db_path, Some(self.current_pw.trim()))
            .map_err(|e| e.to_string())?;
        conn.execute_batch("SELECT count(*) FROM sqlite_master;")
            .map_err(|_| "Current password is incorrect.".to_string())?;
        conn.pragma_update(None, "rekey", self.new_pw.trim())
            .map_err(|e| e.to_string())?;
        db::set_db_password(Some(self.new_pw.trim().to_string()));
        Ok("Password changed successfully.".into())
    }

    fn do_remove(&self, db_path: &std::path::Path) -> std::result::Result<String, String> {
        let src = db::open_connection(db_path, Some(self.current_pw.trim()))
            .map_err(|e| e.to_string())?;
        src.execute_batch("SELECT count(*) FROM sqlite_master;")
            .map_err(|_| "Current password is incorrect.".to_string())?;
        let tmp = db_path.with_extension("db.decrypting");
        let mut dst = db::open_connection(&tmp, None).map_err(|e| e.to_string())?;
        let backup = rusqlite::backup::Backup::new(&src, &mut dst).map_err(|e| e.to_string())?;
        backup
            .run_to_completion(100, Duration::from_millis(10), None)
            .map_err(|e| e.to_string())?;
        drop(backup);
        drop(src);
        drop(dst);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
        std::fs::rename(&tmp, db_path).map_err(|e| e.to_string())?;
        db::set_db_password(None);
        Ok("Database decrypted. Password removed.".into())
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let [_top, title_area, _gap, content_area, _gap2, hints_area, _bottom] =
            Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(6),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(area);

        let status = if self.encrypted {
            "Database is encrypted"
        } else {
            "Database is not encrypted"
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("Password Management  ({status})"),
                HEADER_STYLE,
            ))
            .alignment(ratatui::layout::Alignment::Center),
            title_area,
        );

        let form_width = 50u16.min(area.width.saturating_sub(4));
        let form_x = area.x + (area.width.saturating_sub(form_width)) / 2;
        let centered = Rect::new(form_x, content_area.y, form_width, content_area.height);

        match &self.phase {
            Phase::Menu => self.draw_menu(frame, centered),
            Phase::InputCurrent => self.draw_input(frame, centered, "Current password:", &self.current_pw),
            Phase::InputNew => self.draw_input(frame, centered, "New password:", &self.new_pw),
            Phase::InputConfirm => self.draw_input(frame, centered, "Confirm password:", &self.confirm_pw),
            Phase::Result(msg, success) => {
                let color = if *success { Color::Green } else { Color::Red };
                frame.render_widget(
                    Paragraph::new(Span::styled(msg.as_str(), Style::default().fg(color)))
                        .alignment(ratatui::layout::Alignment::Center),
                    centered,
                );
            }
        }

        let hints = match &self.phase {
            Phase::Menu => "Enter=select  Esc=back",
            Phase::InputCurrent | Phase::InputNew | Phase::InputConfirm => "Enter=confirm  Esc=cancel",
            Phase::Result(_, _) => "Esc=back",
        };
        frame.render_widget(
            Paragraph::new(format!(" {hints}"))
                .style(FOOTER_STYLE)
                .alignment(ratatui::layout::Alignment::Center),
            hints_area,
        );
    }

    fn draw_menu(&self, frame: &mut Frame, area: Rect) {
        let actions = self.actions();
        let lines: Vec<Line> = actions
            .iter()
            .enumerate()
            .map(|(i, (label, _))| {
                let marker = if i == self.selection { ">" } else { " " };
                let style = if i == self.selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!(" {marker} {label}"), style))
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn draw_input(&self, frame: &mut Frame, area: Rect, label: &str, value: &str) {
        let [label_area, input_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(Span::styled(label, Style::default().add_modifier(Modifier::BOLD))),
            label_area,
        );

        let masked: String = "\u{25cf}".repeat(value.chars().count());
        let cursor_display = format!("{masked}\u{2588}");
        let width = input_area.width as usize;
        let padded = format!("{:<width$}", cursor_display, width = width);
        frame.render_widget(
            Paragraph::new(Span::styled(padded, SELECTED_STYLE)),
            input_area,
        );
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match &self.phase {
            Phase::Menu => self.handle_menu_key(code),
            Phase::InputCurrent | Phase::InputNew | Phase::InputConfirm => {
                self.handle_input_key(code)
            }
            Phase::Result(_, _) => {
                if code == KeyCode::Esc || code == KeyCode::Enter {
                    self.reset();
                }
                false
            }
        }
    }

    fn handle_menu_key(&mut self, code: KeyCode) -> bool {
        let actions = self.actions();
        match code {
            KeyCode::Esc => return true, // close screen
            KeyCode::Up => self.selection = self.selection.saturating_sub(1),
            KeyCode::Down => {
                self.selection = (self.selection + 1).min(actions.len().saturating_sub(1))
            }
            KeyCode::Enter => {
                let action = match actions.get(self.selection) {
                    Some((_, Action::Set)) => Action::Set,
                    Some((_, Action::Change)) => Action::Change,
                    Some((_, Action::Remove)) => Action::Remove,
                    None => return false,
                };
                self.chosen_action = Some(match action {
                    Action::Set => Action::Set,
                    Action::Change => Action::Change,
                    Action::Remove => Action::Remove,
                });
                // Determine first input phase
                match action {
                    Action::Set => self.phase = Phase::InputNew,
                    Action::Change | Action::Remove => self.phase = Phase::InputCurrent,
                }
            }
            _ => {}
        }
        false
    }

    fn handle_input_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc => {
                self.reset();
                return false;
            }
            KeyCode::Enter => {
                // Advance to next phase
                match (&self.phase, &self.chosen_action) {
                    (Phase::InputCurrent, Some(Action::Change)) => {
                        self.phase = Phase::InputNew;
                        self.cursor = 0;
                    }
                    (Phase::InputCurrent, Some(Action::Remove)) => {
                        self.execute();
                    }
                    (Phase::InputNew, _) => {
                        self.phase = Phase::InputConfirm;
                        self.cursor = 0;
                    }
                    (Phase::InputConfirm, _) => {
                        self.execute();
                    }
                    _ => {}
                }
            }
            KeyCode::Char(c) => {
                self.active_input_mut().push(c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.active_input_mut().pop();
                    self.cursor -= 1;
                }
            }
            _ => {}
        }
        false
    }

    fn reset(&mut self) {
        self.current_pw.zeroize();
        self.new_pw.zeroize();
        self.confirm_pw.zeroize();
        self.cursor = 0;
        self.selection = 0;
        self.chosen_action = None;
        self.phase = Phase::Menu;
    }
}

impl Drop for PasswordManager {
    fn drop(&mut self) {
        self.current_pw.zeroize();
        self.new_pw.zeroize();
        self.confirm_pw.zeroize();
    }
}
```

**Step 3: Add to dashboard**

In `src/cli/dashboard.rs`:

1. Add to `MENU_ITEMS` (before Snake):
```rust
("[p] Password management", 'p'),
```

2. Update `MENU_LEFT_COUNT` if needed (count of items in left column).

3. Add variant to `DashboardScreen`:
```rust
Password(super::password_manager::PasswordManager),
```

4. In the key-handling match for Home screen, add:
```rust
'p' => {
    match super::password_manager::PasswordManager::new() {
        Ok(mgr) => self.screen = DashboardScreen::Password(mgr),
        Err(e) => { /* show error */ }
    }
}
```

5. In the draw match, add:
```rust
DashboardScreen::Password(ref mgr) => mgr.draw(frame, area),
```

6. In the key-handling match for the Password screen:
```rust
DashboardScreen::Password(ref mut mgr) => {
    if mgr.handle_key(key.code) {
        self.screen = DashboardScreen::Home;
    }
}
```

**Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles.

**Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 6: Commit**

```
feat: add password management to dashboard TUI

Adds [p] Password menu item with inline TUI screen for
set/change/remove operations. Ref #66
```

---

### Task 9: Update documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

**Step 1: Update CLAUDE.md**

Update the following sections:

- **Architecture > Database:** Add mention of SQLCipher encryption, `PRAGMA key`
- **Architecture > Settings:** Note that password is runtime-only (not stored)
- **Commands:** Add `nigel password set/change/remove` examples
- **Dashboard:** Add `p=Password` to the shortcut list
- **Key Design Constraints:** Add constraint about password never persisted to disk, demo always unencrypted
- **Project Structure:** Add `password.rs` and `password_manager.rs`
- **Modules > db.rs:** Mention `set_db_password`, `get_db_password`, `open_connection`, `is_encrypted`, `prompt_password_if_needed`

**Step 2: Update README.md**

Add encryption section covering:
- Optional password during onboarding
- `nigel password` subcommand usage
- Dashboard password management
- Backups preserve encryption state

**Step 3: Run tests one final time**

Run: `cargo test`
Expected: All tests pass.

**Step 4: Commit**

```
docs: update CLAUDE.md and README.md for database encryption

Documents SQLCipher integration, password management CLI and TUI,
encryption-related design constraints. Ref #66
```

---

### Final: Run full verification

**Step 1: Full test suite**

Run: `cargo test`
Expected: All tests pass (original 173 + new encryption tests).

**Step 2: Build release**

Run: `cargo build --release`
Expected: Clean build.

**Step 3: Build without default features**

Run: `cargo build --no-default-features`
Expected: Builds (SQLCipher is not feature-gated — always present).
