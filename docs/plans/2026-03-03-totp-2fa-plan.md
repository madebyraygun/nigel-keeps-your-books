# TOTP Two-Factor Authentication Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add optional TOTP 2FA with OS keychain secret storage to Nigel's encrypted database protection.

**Architecture:** New `src/totp.rs` module (feature-gated behind `totp`) encapsulates all TOTP logic. Secrets stored in OS keychain via `keyring` crate. `totp_enabled` flag in metadata table. Integration points: splash screen, onboarding, password manager, settings manager, CLI subcommands, and `prompt_password_if_needed()`.

**Tech Stack:** `totp-rs` (TOTP generation/verification), `keyring` (cross-platform keychain), `zeroize` (already present)

---

### Task 1: Add dependencies and feature flag

**Files:**
- Modify: `Cargo.toml:7-10` (features section), `Cargo.toml:12-35` (dependencies)

**Step 1: Add totp feature flag and dependencies to Cargo.toml**

In the `[features]` section, add:
```toml
totp = ["dep:totp-rs", "dep:keyring"]
```

Add `totp` to the `default` list:
```toml
default = ["gusto", "pdf", "totp"]
```

In the `[dependencies]` section, add:
```toml
totp-rs = { version = "5", features = ["rand"], optional = true }
keyring = { version = "3", optional = true }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Verify it compiles without the feature**

Run: `cargo check --no-default-features`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Add totp-rs and keyring dependencies with totp feature flag"
```

---

### Task 2: Create the TOTP module with core logic

**Files:**
- Create: `src/totp.rs`
- Modify: `src/main.rs:1-17` (add mod declaration)

**Step 1: Write the tests for the TOTP module**

Create `src/totp.rs` with tests first. The module needs:
- `generate_secret()` — returns base32 secret + otpauth URI
- `verify_code()` — validates a 6-digit code against a secret
- `is_enabled()` — checks metadata table for `totp_enabled` flag
- `enable()` — stores secret in keychain + sets metadata flag
- `disable()` — removes secret from keychain + clears metadata flag
- `get_secret()` — retrieves secret from keychain
- `keychain_entry()` — builds the keyring Entry for a DB path

```rust
#[cfg(feature = "totp")]
use rusqlite::Connection;
#[cfg(feature = "totp")]
use std::path::Path;

#[cfg(feature = "totp")]
use crate::db;
#[cfg(feature = "totp")]
use crate::error::Result;

#[cfg(feature = "totp")]
const KEYRING_SERVICE: &str = "nigel";

/// Build the keyring entry for a given database path.
/// Uses the canonical path to handle symlinks and relative paths.
#[cfg(feature = "totp")]
fn keychain_entry(db_path: &Path) -> Result<keyring::Entry> {
    let canonical = db_path
        .canonicalize()
        .unwrap_or_else(|_| db_path.to_path_buf());
    let user = format!("totp:{}", canonical.display());
    keyring::Entry::new(KEYRING_SERVICE, &user)
        .map_err(|e| crate::error::NigelError::Other(format!("Keychain error: {e}")))
}

/// Generate a new TOTP secret. Returns (base32_secret, otpauth_uri).
#[cfg(feature = "totp")]
pub fn generate_secret(issuer: &str, account: &str) -> Result<(String, String)> {
    use totp_rs::{Algorithm, Secret, TOTP};

    let secret = Secret::generate_secret();
    let secret_bytes = secret.to_bytes().map_err(|e| {
        crate::error::NigelError::Other(format!("Failed to generate TOTP secret: {e}"))
    })?;
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret_bytes, Some(issuer.to_string()), account.to_string())
        .map_err(|e| crate::error::NigelError::Other(format!("Failed to create TOTP: {e}")))?;
    let base32 = secret.to_encoded().to_string();
    let uri = totp.get_url();
    Ok((base32, uri))
}

/// Verify a 6-digit TOTP code against a base32-encoded secret.
#[cfg(feature = "totp")]
pub fn verify_code(base32_secret: &str, code: &str) -> bool {
    use totp_rs::{Algorithm, Secret, TOTP};
    use std::time::SystemTime;

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

/// Check if TOTP is enabled for this database (checks metadata flag).
#[cfg(feature = "totp")]
pub fn is_enabled(conn: &Connection) -> bool {
    db::get_metadata(conn, "totp_enabled")
        .map(|v| v == "true")
        .unwrap_or(false)
}

/// Get the TOTP secret from the OS keychain.
#[cfg(feature = "totp")]
pub fn get_secret(db_path: &Path) -> Result<String> {
    let entry = keychain_entry(db_path)?;
    entry
        .get_password()
        .map_err(|e| crate::error::NigelError::Other(format!("Could not read TOTP secret from keychain: {e}")))
}

/// Enable TOTP: store secret in OS keychain, set metadata flag in database.
#[cfg(feature = "totp")]
pub fn enable(conn: &Connection, db_path: &Path, secret: &str) -> Result<()> {
    let entry = keychain_entry(db_path)?;
    entry
        .set_password(secret)
        .map_err(|e| crate::error::NigelError::Other(format!("Could not store TOTP secret in keychain: {e}")))?;
    db::set_metadata(conn, "totp_enabled", "true")?;
    Ok(())
}

/// Disable TOTP: remove secret from OS keychain, clear metadata flag.
#[cfg(feature = "totp")]
pub fn disable(conn: &Connection, db_path: &Path) -> Result<()> {
    let entry = keychain_entry(db_path)?;
    // Ignore "not found" errors when deleting — may already be gone
    let _ = entry.delete_credential();
    conn.execute("DELETE FROM metadata WHERE key = 'totp_enabled'", [])
        .map_err(crate::error::NigelError::Db)?;
    Ok(())
}

/// CLI helper: if TOTP is enabled, prompt for code via stdin (up to 3 attempts).
/// Must be called after password has been set and DB is accessible.
#[cfg(feature = "totp")]
pub fn prompt_totp_if_needed(db_path: &Path) -> Result<()> {
    let conn = db::get_connection(db_path)?;
    if !is_enabled(&conn) {
        return Ok(());
    }
    let secret = get_secret(db_path)?;
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
    Err(crate::error::NigelError::Other(
        "Failed TOTP verification after 3 attempts.".into(),
    ))
}

#[cfg(test)]
#[cfg(feature = "totp")]
mod tests {
    use super::*;

    #[test]
    fn generate_secret_returns_base32_and_uri() {
        let (base32, uri) = generate_secret("Nigel", "test@example.com").unwrap();
        assert!(!base32.is_empty());
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains("Nigel"));
        assert!(uri.contains("test%40example.com") || uri.contains("test@example.com"));
    }

    #[test]
    fn verify_code_with_valid_code() {
        use totp_rs::{Algorithm, Secret, TOTP};
        use std::time::SystemTime;

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
    fn is_enabled_false_when_flag_not_true() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::get_connection(&dir.path().join("test.db")).unwrap();
        db::init_db(&conn).unwrap();
        db::set_metadata(&conn, "totp_enabled", "false").unwrap();
        assert!(!is_enabled(&conn));
    }
}
```

**Step 2: Add mod declaration to main.rs**

In `src/main.rs`, after line 11 (`mod pdf;`), add:
```rust
#[cfg(feature = "totp")]
mod totp;
```

**Step 3: Run the tests**

Run: `cargo test totp --features totp`
Expected: all TOTP unit tests pass (keychain tests skipped since enable/disable require OS keychain access)

**Step 4: Verify it compiles without the feature**

Run: `cargo check --no-default-features`
Expected: compiles — all TOTP code gated behind `#[cfg(feature = "totp")]`

**Step 5: Commit**

```bash
git add src/totp.rs src/main.rs
git commit -m "Add TOTP module with secret generation, verification, and keychain storage"
```

---

### Task 3: Add CLI subcommands for TOTP enable/disable

**Files:**
- Modify: `src/cli/mod.rs:173-181` (PasswordCommand enum)
- Modify: `src/cli/password.rs` (add run_totp_enable, run_totp_disable)
- Modify: `src/main.rs:198-202` (dispatch PasswordCommand)

**Step 1: Add PasswordCommand variants**

In `src/cli/mod.rs`, add two new variants to `PasswordCommand` (after `Remove`):

```rust
#[derive(Subcommand)]
pub enum PasswordCommand {
    /// Set a password on an unencrypted database.
    Set,
    /// Change the password on an encrypted database.
    Change,
    /// Remove the password (decrypt the database).
    Remove,
    /// Enable TOTP two-factor authentication (requires encrypted database).
    #[cfg(feature = "totp")]
    TotpEnable,
    /// Disable TOTP two-factor authentication.
    #[cfg(feature = "totp")]
    TotpDisable,
}
```

**Step 2: Add run_totp_enable and run_totp_disable to password.rs**

In `src/cli/password.rs`, add:

```rust
#[cfg(feature = "totp")]
pub fn run_totp_enable() -> Result<()> {
    let db_path = get_data_dir().join("nigel.db");
    if !is_encrypted(&db_path)? {
        eprintln!("Database must be encrypted before enabling 2FA. Use `nigel password set` first.");
        return Ok(());
    }

    // Prompt for password to open the DB
    crate::db::prompt_password_if_needed(&db_path)?;
    let conn = crate::db::get_connection(&db_path)?;

    if crate::totp::is_enabled(&conn) {
        eprintln!("TOTP 2FA is already enabled. Use `nigel password totp-disable` to remove it first.");
        return Ok(());
    }

    let company = crate::db::get_metadata(&conn, "company_name").unwrap_or_else(|| "Nigel".to_string());
    let (base32, _uri) = crate::totp::generate_secret(&company, "database")?;

    println!("\nTOTP Secret (add to your authenticator app):\n");
    println!("  {base32}\n");
    println!("Enter the 6-digit code from your authenticator to verify:");

    let code = rpassword::prompt_password("Code: ")
        .map_err(|e| crate::error::NigelError::Other(e.to_string()))?;

    if !crate::totp::verify_code(&base32, code.trim()) {
        eprintln!("Invalid code. 2FA was not enabled.");
        return Ok(());
    }

    crate::totp::enable(&conn, &db_path, &base32)?;
    println!("Two-factor authentication enabled.");
    Ok(())
}

#[cfg(feature = "totp")]
pub fn run_totp_disable() -> Result<()> {
    let db_path = get_data_dir().join("nigel.db");
    if !is_encrypted(&db_path)? {
        eprintln!("Database is not encrypted.");
        return Ok(());
    }

    crate::db::prompt_password_if_needed(&db_path)?;
    let conn = crate::db::get_connection(&db_path)?;

    if !crate::totp::is_enabled(&conn) {
        eprintln!("TOTP 2FA is not enabled.");
        return Ok(());
    }

    let secret = crate::totp::get_secret(&db_path)?;
    let code = rpassword::prompt_password("Enter authenticator code to disable 2FA: ")
        .map_err(|e| crate::error::NigelError::Other(e.to_string()))?;

    if !crate::totp::verify_code(&secret, code.trim()) {
        eprintln!("Invalid code. 2FA was not disabled.");
        return Ok(());
    }

    crate::totp::disable(&conn, &db_path)?;
    println!("Two-factor authentication disabled.");
    Ok(())
}
```

**Step 3: Wire up dispatch in main.rs**

In `src/main.rs`, update the `Commands::Password` match arm to add:

```rust
Commands::Password { command } => match command {
    PasswordCommand::Set => cli::password::run_set(),
    PasswordCommand::Change => cli::password::run_change(),
    PasswordCommand::Remove => cli::password::run_remove(),
    #[cfg(feature = "totp")]
    PasswordCommand::TotpEnable => cli::password::run_totp_enable(),
    #[cfg(feature = "totp")]
    PasswordCommand::TotpDisable => cli::password::run_totp_disable(),
},
```

**Step 4: Verify compilation**

Run: `cargo check`
Expected: compiles

Run: `cargo check --no-default-features`
Expected: compiles (TOTP variants gated)

**Step 5: Commit**

```bash
git add src/cli/mod.rs src/cli/password.rs src/main.rs
git commit -m "Add nigel password totp-enable and totp-disable CLI subcommands"
```

---

### Task 4: Extend prompt_password_if_needed with TOTP

**Files:**
- Modify: `src/db.rs:411-435` (prompt_password_if_needed function)

**Step 1: Add TOTP prompt after successful password**

After the successful password validation in `prompt_password_if_needed()`, add a TOTP check. Update the function:

```rust
pub fn prompt_password_if_needed(db_path: &Path) -> Result<()> {
    if !is_encrypted(db_path)? {
        return Ok(());
    }
    for attempt in 1..=3 {
        let pw = rpassword::prompt_password("Database password: ")
            .map_err(|e| crate::error::NigelError::Other(e.to_string()))?;
        set_db_password(Some(pw));
        match get_connection(db_path) {
            Ok(_) => {
                // Password accepted — now check TOTP
                #[cfg(feature = "totp")]
                crate::totp::prompt_totp_if_needed(db_path)?;
                return Ok(());
            }
            Err(_) => {
                set_db_password(None);
                if attempt < 3 {
                    eprintln!("Wrong password. Try again ({attempt}/3).");
                }
            }
        }
    }
    Err(crate::error::NigelError::Other(
        "Failed to unlock database after 3 attempts.".into(),
    ))
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: compiles

Run: `cargo check --no-default-features`
Expected: compiles (TOTP call gated)

**Step 3: Commit**

```bash
git add src/db.rs
git commit -m "Extend prompt_password_if_needed to prompt for TOTP when enabled"
```

---

### Task 5: Add TOTP input phase to splash screen

**Files:**
- Modify: `src/cli/splash.rs`

**Step 1: Add TOTP fields to Splash struct**

Add new fields to the `Splash` struct (after `attempts: u8` at line 36):

```rust
// TOTP mode fields
#[cfg(feature = "totp")]
totp_mode: bool,
#[cfg(feature = "totp")]
totp_input: String,
#[cfg(feature = "totp")]
totp_attempts: u8,
#[cfg(feature = "totp")]
totp_secret: Option<String>,
```

Initialize them in both `new()` and `new_with_password()`:
```rust
#[cfg(feature = "totp")]
totp_mode: false,
#[cfg(feature = "totp")]
totp_input: String::new(),
#[cfg(feature = "totp")]
totp_attempts: 0,
#[cfg(feature = "totp")]
totp_secret: None,
```

Update `Drop` impl to zeroize `totp_input` and `totp_secret`:
```rust
impl Drop for Splash {
    fn drop(&mut self) {
        self.password.zeroize();
        #[cfg(feature = "totp")]
        {
            self.totp_input.zeroize();
            if let Some(ref mut s) = self.totp_secret {
                s.zeroize();
            }
        }
    }
}
```

**Step 2: Add TOTP transition after password success**

In `try_password()`, after `Ok(true)` and before returning, check if TOTP is enabled. Replace the `Ok(true)` arm:

```rust
Ok(true) => {
    crate::db::set_db_password(Some(self.password.clone()));
    #[cfg(feature = "totp")]
    {
        // Check if TOTP is enabled — need temporary connection
        let conn = crate::db::get_connection(db_path).ok();
        if let Some(ref conn) = conn {
            if crate::totp::is_enabled(conn) {
                // Fetch secret from keychain
                match crate::totp::get_secret(db_path) {
                    Ok(secret) => {
                        self.totp_mode = true;
                        self.totp_secret = Some(secret);
                        self.error_msg = None;
                        return Ok(false); // Don't close yet — need TOTP
                    }
                    Err(e) => {
                        self.error_msg = Some(format!("Keychain error: {e}"));
                        crate::db::set_db_password(None);
                        return Ok(false);
                    }
                }
                drop(conn);
            }
        }
                    }
    return Ok(true);
}
```

**Step 3: Add try_totp method**

```rust
#[cfg(feature = "totp")]
fn try_totp(&mut self) -> Result<bool> {
    if let Some(ref secret) = self.totp_secret {
        if crate::totp::verify_code(secret, self.totp_input.trim()) {
            return Ok(true);
        }
        self.totp_attempts += 1;
        if self.totp_attempts >= MAX_PASSWORD_ATTEMPTS {
            self.error_msg = Some("Failed TOTP verification after 3 attempts.".into());
        } else {
            self.error_msg = Some(format!(
                "Invalid code. Try again ({}/3).",
                self.totp_attempts
            ));
        }
        self.totp_input.clear();
    }
    Ok(false)
}
```

**Step 4: Add TOTP input drawing**

```rust
#[cfg(feature = "totp")]
fn draw_totp_input(&self, frame: &mut Frame, area: Rect) {
    let form_width = 40u16.min(area.width.saturating_sub(4));
    let form_x = area.x + (area.width.saturating_sub(form_width)) / 2;
    let centered = Rect::new(form_x, area.y, form_width, area.height);

    let [label_area, input_area, _gap, error_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(centered);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter authenticator code:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        label_area,
    );

    let cursor_display = format!("{}\u{2588}", self.totp_input);
    let width = input_area.width as usize;
    let padded = format!("{:<width$}", cursor_display, width = width);
    frame.render_widget(
        Paragraph::new(Span::styled(padded, tui::SELECTED_STYLE))
            .alignment(Alignment::Left),
        input_area,
    );

    if let Some(ref msg) = self.error_msg {
        frame.render_widget(
            Paragraph::new(Span::styled(msg.as_str(), Style::default().fg(Color::Red)))
                .alignment(Alignment::Center),
            error_area,
        );
    }
}
```

**Step 5: Update draw() to show TOTP input when in totp_mode**

In the `draw()` method, in the `password_mode` branch, after `self.draw_password_input(frame, pw_area);` (line 121), add:

```rust
#[cfg(feature = "totp")]
if self.totp_mode {
    self.draw_totp_input(frame, pw_area);
    return; // Skip password input rendering
}
```

Actually, the cleaner approach is to check `totp_mode` first in the password_mode rendering block and call `draw_totp_input` instead of `draw_password_input` when in TOTP mode.

**Step 6: Update key handling in run_with_password**

In `run_with_password()`, in the `KeyCode::Enter` handler, add TOTP handling before the password logic:

```rust
KeyCode::Enter => {
    #[cfg(feature = "totp")]
    if splash.totp_mode {
        if splash.totp_input.is_empty() {
            continue;
        }
        match splash.try_totp() {
            Ok(true) => break Ok(()),
            Ok(false) => {
                if splash.totp_attempts >= MAX_PASSWORD_ATTEMPTS {
                    break Err(crate::error::NigelError::Other(
                        "Failed TOTP verification after 3 attempts.".into(),
                    ));
                }
            }
            Err(e) => break Err(e),
        }
        continue;
    }
    // ... existing password handling
}
```

And for `KeyCode::Char(c)` and `KeyCode::Backspace`, add TOTP input handling:

```rust
KeyCode::Char(c) => {
    #[cfg(feature = "totp")]
    if splash.totp_mode {
        if c.is_ascii_digit() && splash.totp_input.len() < 6 {
            splash.totp_input.push(c);
        }
        continue;
    }
    splash.password.push(c);
    splash.cursor += 1;
}
KeyCode::Backspace => {
    #[cfg(feature = "totp")]
    if splash.totp_mode {
        splash.totp_input.pop();
        continue;
    }
    if splash.cursor > 0 {
        splash.password.pop();
        splash.cursor -= 1;
    }
}
```

**Step 7: Add tests**

```rust
#[cfg(feature = "totp")]
#[test]
fn totp_mode_fields_initialized() {
    let splash = Splash::new_with_password(Path::new("/tmp/test.db"));
    assert!(!splash.totp_mode);
    assert!(splash.totp_input.is_empty());
    assert_eq!(splash.totp_attempts, 0);
    assert!(splash.totp_secret.is_none());
}
```

**Step 8: Verify compilation and tests**

Run: `cargo check`
Run: `cargo test splash --features totp`
Expected: all tests pass

**Step 9: Commit**

```bash
git add src/cli/splash.rs
git commit -m "Add TOTP input phase to splash screen after password success"
```

---

### Task 6: Add TOTP setup screen to onboarding

**Files:**
- Modify: `src/cli/onboarding.rs`

**Step 1: Add TotpSetup screen variant and fields**

Add to `Screen` enum:
```rust
enum Screen {
    NameInput,
    ConfirmPassword,
    #[cfg(feature = "totp")]
    TotpSetup,
    ActionPicker,
}
```

Add `totp_secret` to `OnboardingResult`:
```rust
pub struct OnboardingResult {
    pub user_name: String,
    pub company_name: String,
    pub password: Option<String>,
    #[cfg(feature = "totp")]
    pub totp_secret: Option<String>,
    pub action: PostSetupAction,
}
```

Add TOTP fields to `Onboarding` struct:
```rust
#[cfg(feature = "totp")]
totp_secret: String,
#[cfg(feature = "totp")]
totp_code: String,
#[cfg(feature = "totp")]
totp_cursor: usize,
#[cfg(feature = "totp")]
totp_error: bool,
```

Initialize in `new()`:
```rust
#[cfg(feature = "totp")]
totp_secret: String::new(),
#[cfg(feature = "totp")]
totp_code: String::new(),
#[cfg(feature = "totp")]
totp_cursor: 0,
#[cfg(feature = "totp")]
totp_error: false,
```

Update `Drop` impl:
```rust
impl Drop for Onboarding {
    fn drop(&mut self) {
        self.password.zeroize();
        self.confirm_password.zeroize();
        #[cfg(feature = "totp")]
        {
            self.totp_secret.zeroize();
            self.totp_code.zeroize();
        }
    }
}
```

**Step 2: Add screen transition from ConfirmPassword to TotpSetup**

In the `run()` function's `StepResult::NextScreen` handling for `Screen::ConfirmPassword`, change:

```rust
Screen::ConfirmPassword => {
    #[cfg(feature = "totp")]
    {
        // Generate TOTP secret and show setup screen
        match crate::totp::generate_secret("Nigel", &onboarding.company_name) {
            Ok((base32, _uri)) => {
                onboarding.totp_secret = base32;
                onboarding.totp_code.clear();
                onboarding.totp_cursor = 0;
                onboarding.totp_error = false;
                onboarding.screen = Screen::TotpSetup;
            }
            Err(_) => {
                // If secret generation fails, skip TOTP setup
                onboarding.screen = Screen::ActionPicker;
            }
        }
    }
    #[cfg(not(feature = "totp"))]
    {
        onboarding.screen = Screen::ActionPicker;
    }
}
```

**Step 3: Add handle_totp_key and draw_totp_setup methods**

```rust
#[cfg(feature = "totp")]
fn handle_totp_key(&mut self, code: KeyCode) -> StepResult {
    match code {
        KeyCode::Enter => {
            if crate::totp::verify_code(&self.totp_secret, self.totp_code.trim()) {
                self.totp_error = false;
                StepResult::NextScreen
            } else {
                self.totp_error = true;
                self.totp_code.zeroize();
                self.totp_cursor = 0;
                StepResult::Continue
            }
        }
        KeyCode::Esc => {
            // Skip 2FA setup
            self.totp_secret.zeroize();
            StepResult::NextScreen
        }
        KeyCode::Char(c) => {
            if c.is_ascii_digit() && self.totp_code.len() < 6 {
                self.totp_error = false;
                self.totp_code.push(c);
                self.totp_cursor += 1;
            }
            StepResult::Continue
        }
        KeyCode::Backspace => {
            if self.totp_cursor > 0 {
                self.totp_code.pop();
                self.totp_cursor -= 1;
                self.totp_error = false;
            }
            StepResult::Continue
        }
        _ => StepResult::Continue,
    }
}
```

```rust
#[cfg(feature = "totp")]
fn draw_totp_setup(&self, frame: &mut Frame, area: Rect) {
    let logo_height = LOGO.len() as u16;
    let error_height = if self.totp_error { 1 } else { 0 };
    let [_top_pad, logo_area, _gap1, title_area, _gap2, secret_label_area, secret_area, _gap3, code_label_area, code_area, error_area, _gap4, hints_area, _bottom_pad, version_area] =
        Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(logo_height),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(error_height),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

    effects::render_logo(self.phase, frame, logo_area);

    frame.render_widget(
        Paragraph::new(Span::styled("Set up Two-Factor Authentication", HEADER_STYLE))
            .alignment(ratatui::layout::Alignment::Center),
        title_area,
    );

    let form_width = 50u16.min(area.width.saturating_sub(4));
    let form_x = area.x + (area.width.saturating_sub(form_width)) / 2;

    // Secret label
    let secret_label_rect = Rect::new(form_x, secret_label_area.y, form_width, 1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Add this secret to your authenticator app:",
            Style::default().fg(Color::DarkGray),
        )),
        secret_label_rect,
    );

    // Secret value (copyable)
    let secret_rect = Rect::new(form_x, secret_area.y, form_width, 1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("  {}", self.totp_secret),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        secret_rect,
    );

    // Code input label
    let code_label_rect = Rect::new(form_x, code_label_area.y, form_width, 1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter the 6-digit code to verify:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        code_label_rect,
    );

    // Code input
    let code_rect = Rect::new(form_x, code_area.y, form_width, 1);
    let cursor_display = format!("{}\u{2588}", self.totp_code);
    let padded = format!("{:<width$}", cursor_display, width = form_width as usize);
    frame.render_widget(
        Paragraph::new(Span::styled(padded, SELECTED_STYLE)),
        code_rect,
    );

    // Error message
    if self.totp_error {
        let error_rect = Rect::new(form_x, error_area.y, form_width, 1);
        frame.render_widget(
            Paragraph::new(Span::styled(
                "Invalid code. Try again.",
                Style::default().fg(Color::Red),
            )),
            error_rect,
        );
    }

    frame.render_widget(
        Paragraph::new(" Enter=verify  Esc=skip 2FA")
            .style(FOOTER_STYLE)
            .alignment(ratatui::layout::Alignment::Center),
        hints_area,
    );

    tui::render_version(frame, version_area);
}
```

**Step 4: Wire up draw and key handling in the main event loop**

In `draw()`, add:
```rust
#[cfg(feature = "totp")]
Screen::TotpSetup => self.draw_totp_setup(frame, area),
```

In the `run()` event loop key dispatch, add:
```rust
#[cfg(feature = "totp")]
Screen::TotpSetup => onboarding.handle_totp_key(key.code),
```

In the `StepResult::NextScreen` handler, add for `TotpSetup`:
```rust
#[cfg(feature = "totp")]
Screen::TotpSetup => {
    onboarding.screen = Screen::ActionPicker;
}
```

In the `StepResult::Finish` handler, include `totp_secret`:
```rust
#[cfg(feature = "totp")]
totp_secret: if onboarding.totp_secret.is_empty() {
    None
} else {
    Some(onboarding.totp_secret.clone())
},
```

**Step 5: Update the caller (dashboard.rs) to store TOTP secret**

The caller that uses `OnboardingResult` needs to store the TOTP secret in the keychain after database creation. Find where `OnboardingResult` is consumed and add:

```rust
#[cfg(feature = "totp")]
if let Some(ref totp_secret) = result.totp_secret {
    if let Ok(conn) = crate::db::get_connection(&db_path) {
        let _ = crate::totp::enable(&conn, &db_path, totp_secret);
    }
}
```

**Step 6: Add tests**

```rust
#[cfg(feature = "totp")]
#[test]
fn totp_setup_verify_advances() {
    let mut ob = make_onboarding();
    ob.screen = Screen::TotpSetup;
    let (base32, _) = crate::totp::generate_secret("Test", "test").unwrap();
    ob.totp_secret = base32.clone();

    // Generate a valid code
    use totp_rs::{Algorithm, Secret, TOTP};
    use std::time::SystemTime;
    let secret_bytes = Secret::Encoded(base32).to_bytes().unwrap();
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret_bytes, None, String::new()).unwrap();
    let time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    let code = totp.generate(time);

    for c in code.chars() {
        ob.handle_totp_key(KeyCode::Char(c));
    }
    let result = ob.handle_totp_key(KeyCode::Enter);
    assert!(matches!(result, StepResult::NextScreen));
}

#[cfg(feature = "totp")]
#[test]
fn totp_setup_esc_skips() {
    let mut ob = make_onboarding();
    ob.screen = Screen::TotpSetup;
    ob.totp_secret = "JBSWY3DPEHPK3PXP".into();
    let result = ob.handle_totp_key(KeyCode::Esc);
    assert!(matches!(result, StepResult::NextScreen));
    assert!(ob.totp_secret.is_empty()); // zeroized on skip
}

#[cfg(feature = "totp")]
#[test]
fn totp_setup_wrong_code_shows_error() {
    let mut ob = make_onboarding();
    ob.screen = Screen::TotpSetup;
    ob.totp_secret = "JBSWY3DPEHPK3PXP".into();
    for c in "000000".chars() {
        ob.handle_totp_key(KeyCode::Char(c));
    }
    let result = ob.handle_totp_key(KeyCode::Enter);
    assert!(matches!(result, StepResult::Continue));
    assert!(ob.totp_error);
    assert!(ob.totp_code.is_empty());
}
```

**Step 7: Verify compilation and tests**

Run: `cargo check`
Run: `cargo test onboarding --features totp`
Expected: all tests pass

**Step 8: Commit**

```bash
git add src/cli/onboarding.rs
git commit -m "Add TOTP setup screen to onboarding flow"
```

---

### Task 7: Add 2FA enable/disable to password manager TUI

**Files:**
- Modify: `src/cli/password_manager.rs`

**Step 1: Add TOTP-related actions and phases**

Add to `Action` enum:
```rust
#[derive(Clone, Copy)]
enum Action {
    Set,
    Change,
    Remove,
    #[cfg(feature = "totp")]
    TotpEnable,
    #[cfg(feature = "totp")]
    TotpDisable,
}
```

Add to `Phase` enum:
```rust
enum Phase {
    Menu,
    InputCurrent,
    InputNew,
    InputConfirm,
    Result(String, bool),
    #[cfg(feature = "totp")]
    TotpDisplay(String), // base32 secret displayed to user
    #[cfg(feature = "totp")]
    TotpVerify,          // user enters code to verify
    #[cfg(feature = "totp")]
    TotpConfirmDisable,  // user enters code to confirm disable
}
```

Add TOTP fields to `PasswordManager`:
```rust
pub struct PasswordManager {
    // ... existing fields ...
    #[cfg(feature = "totp")]
    totp_enabled: bool,
    #[cfg(feature = "totp")]
    totp_secret: String,
    #[cfg(feature = "totp")]
    totp_code: String,
}
```

Initialize in `new()`:
```rust
#[cfg(feature = "totp")]
totp_enabled: {
    let db_path = get_data_dir().join("nigel.db");
    if encrypted {
        crate::db::get_connection(&db_path)
            .ok()
            .map(|conn| crate::totp::is_enabled(&conn))
            .unwrap_or(false)
    } else {
        false
    }
},
#[cfg(feature = "totp")]
totp_secret: String::new(),
#[cfg(feature = "totp")]
totp_code: String::new(),
```

Update `Drop` to zeroize TOTP fields:
```rust
impl Drop for PasswordManager {
    fn drop(&mut self) {
        self.current_pw.zeroize();
        self.new_pw.zeroize();
        self.confirm_pw.zeroize();
        #[cfg(feature = "totp")]
        {
            self.totp_secret.zeroize();
            self.totp_code.zeroize();
        }
    }
}
```

**Step 2: Update actions() to include TOTP items**

Update `actions()` to return context-dependent menu items including TOTP enable/disable when encrypted:

```rust
fn actions(&self) -> Vec<(&str, usize)> {
    let mut items = Vec::new();
    if self.encrypted {
        items.push(("Change password", 0));
        items.push(("Remove password", 1));
        #[cfg(feature = "totp")]
        if self.totp_enabled {
            items.push(("Disable 2FA", 4));
        } else {
            items.push(("Enable 2FA", 3));
        }
    } else {
        items.push(("Set password", 2));
    }
    items
}
```

Note: this changes `actions()` return type from `&[(&str, usize)]` to `Vec<(&str, usize)>`. Update all call sites accordingly.

**Step 3: Handle TOTP menu actions in handle_menu_key**

Add to the match in `handle_menu_key`:
```rust
#[cfg(feature = "totp")]
Some((_, 3)) => Action::TotpEnable,
#[cfg(feature = "totp")]
Some((_, 4)) => Action::TotpDisable,
```

And the phase transitions:
```rust
#[cfg(feature = "totp")]
Action::TotpEnable => {
    let company = {
        let db_path = get_data_dir().join("nigel.db");
        crate::db::get_connection(&db_path)
            .ok()
            .and_then(|conn| crate::db::get_metadata(&conn, "company_name"))
            .unwrap_or_else(|| "Nigel".to_string())
    };
    match crate::totp::generate_secret(&company, "database") {
        Ok((base32, _)) => {
            self.totp_secret = base32.clone();
            Phase::TotpDisplay(base32)
        }
        Err(e) => Phase::Result(format!("Failed to generate secret: {e}"), false),
    }
}
#[cfg(feature = "totp")]
Action::TotpDisable => Phase::TotpConfirmDisable,
```

**Step 4: Handle TOTP input phases**

Add to `handle_key()`:
```rust
#[cfg(feature = "totp")]
Phase::TotpDisplay(_) | Phase::TotpVerify | Phase::TotpConfirmDisable => {
    self.handle_totp_key(code)
}
```

Add `handle_totp_key` method:
```rust
#[cfg(feature = "totp")]
fn handle_totp_key(&mut self, code: KeyCode) -> PasswordAction {
    match code {
        KeyCode::Esc => {
            self.reset();
            return PasswordAction::Continue;
        }
        KeyCode::Enter => {
            match &self.phase {
                Phase::TotpDisplay(_) => {
                    self.totp_code.clear();
                    self.phase = Phase::TotpVerify;
                }
                Phase::TotpVerify => {
                    if crate::totp::verify_code(&self.totp_secret, self.totp_code.trim()) {
                        let db_path = get_data_dir().join("nigel.db");
                        match crate::db::get_connection(&db_path) {
                            Ok(conn) => match crate::totp::enable(&conn, &db_path, &self.totp_secret) {
                                Ok(()) => {
                                    self.totp_enabled = true;
                                    self.phase = Phase::Result("Two-factor authentication enabled.".into(), true);
                                }
                                Err(e) => {
                                    self.phase = Phase::Result(format!("Failed to enable 2FA: {e}"), false);
                                }
                            },
                            Err(e) => {
                                self.phase = Phase::Result(format!("DB error: {e}"), false);
                            }
                        }
                    } else {
                        self.phase = Phase::Result("Invalid code. 2FA was not enabled.".into(), false);
                    }
                    self.totp_code.clear();
                }
                Phase::TotpConfirmDisable => {
                    let db_path = get_data_dir().join("nigel.db");
                    let secret = match crate::totp::get_secret(&db_path) {
                        Ok(s) => s,
                        Err(e) => {
                            self.phase = Phase::Result(format!("Keychain error: {e}"), false);
                            return PasswordAction::Continue;
                        }
                    };
                    if crate::totp::verify_code(&secret, self.totp_code.trim()) {
                        match crate::db::get_connection(&db_path) {
                            Ok(conn) => match crate::totp::disable(&conn, &db_path) {
                                Ok(()) => {
                                    self.totp_enabled = false;
                                    self.phase = Phase::Result("Two-factor authentication disabled.".into(), true);
                                }
                                Err(e) => {
                                    self.phase = Phase::Result(format!("Failed to disable 2FA: {e}"), false);
                                }
                            },
                            Err(e) => {
                                self.phase = Phase::Result(format!("DB error: {e}"), false);
                            }
                        }
                    } else {
                        self.phase = Phase::Result("Invalid code. 2FA was not disabled.".into(), false);
                    }
                    self.totp_code.clear();
                }
                _ => {}
            }
        }
        KeyCode::Char(c) => {
            if matches!(self.phase, Phase::TotpVerify | Phase::TotpConfirmDisable) {
                if c.is_ascii_digit() && self.totp_code.len() < 6 {
                    self.totp_code.push(c);
                }
            }
        }
        KeyCode::Backspace => {
            if matches!(self.phase, Phase::TotpVerify | Phase::TotpConfirmDisable) {
                self.totp_code.pop();
            }
        }
        _ => {}
    }
    PasswordAction::Continue
}
```

**Step 5: Add TOTP drawing methods**

Add drawing for `TotpDisplay`, `TotpVerify`, and `TotpConfirmDisable` phases in the `draw()` method's match on `&self.phase`. These should show the secret (for enable) or a code input (for verify/disable).

**Step 6: Update reset() to clear TOTP state**

```rust
fn reset(&mut self) {
    self.current_pw.zeroize();
    self.new_pw.zeroize();
    self.confirm_pw.zeroize();
    self.cursor = 0;
    self.selection = 0;
    self.chosen_action = None;
    self.phase = Phase::Menu;
    #[cfg(feature = "totp")]
    {
        self.totp_secret.zeroize();
        self.totp_code.zeroize();
    }
}
```

**Step 7: Verify compilation and tests**

Run: `cargo check`
Expected: compiles

**Step 8: Commit**

```bash
git add src/cli/password_manager.rs
git commit -m "Add 2FA enable/disable to password manager TUI"
```

---

### Task 8: Add 2FA menu entry to settings manager

**Files:**
- Modify: `src/cli/settings_manager.rs`

**Step 1: Add TOTP menu constant and field**

Add after `MENU_PASSWORD`:
```rust
#[cfg(feature = "totp")]
const MENU_TOTP: usize = 2;
```

Add `totp_enabled` field to `SettingsManager`:
```rust
#[cfg(feature = "totp")]
totp_enabled: bool,
```

Initialize in `new()`:
```rust
#[cfg(feature = "totp")]
totp_enabled: {
    if encrypted {
        crate::totp::is_enabled(conn)
    } else {
        false
    }
},
```

**Step 2: Update Down navigation clamp**

In `handle_main_key`, change the Down clamp:
```rust
KeyCode::Down => {
    #[cfg(feature = "totp")]
    let max = if self.encrypted { MENU_TOTP } else { MENU_PASSWORD };
    #[cfg(not(feature = "totp"))]
    let max = MENU_PASSWORD;
    self.selection = (self.selection + 1).min(max);
    SettingsAction::Continue
}
```

**Step 3: Add TOTP menu entry rendering in draw_main**

After the Password section lines, add:
```rust
#[cfg(feature = "totp")]
if self.encrypted {
    lines.push(Line::from(""));
    let totp_selected = self.selection == MENU_TOTP;
    let totp_marker = if totp_selected { ">" } else { " " };
    let totp_style = if totp_selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let totp_status = if self.totp_enabled {
        "enabled"
    } else {
        "not set"
    };
    lines.push(Line::from(vec![
        Span::styled(format!(" {totp_marker} Two-Factor Auth  "), totp_style),
        Span::styled(
            format!("({totp_status})"),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
}
```

**Step 4: Handle TOTP menu selection**

In `handle_main_key`'s `KeyCode::Enter` match, add:
```rust
#[cfg(feature = "totp")]
MENU_TOTP if self.encrypted => {
    match PasswordManager::new(&self.greeting) {
        Ok(mgr) => self.screen = Screen::Password(mgr),
        Err(e) => self.set_status(format!("Could not open 2FA settings: {e}"), false),
    }
}
```

**Step 5: Refresh totp_enabled when returning from Password screen**

In the `Screen::Password(mgr)` handler in `handle_key`, after refreshing `self.encrypted`, add:
```rust
#[cfg(feature = "totp")]
{
    if self.encrypted {
        if let Ok(conn) = crate::db::get_connection(&db_path) {
            self.totp_enabled = crate::totp::is_enabled(&conn);
        }
    } else {
        self.totp_enabled = false;
    }
}
```

**Step 6: Add tests**

```rust
#[cfg(feature = "totp")]
#[test]
fn navigate_menu_with_totp() {
    let (_dir, conn) = test_db();
    let mut mgr = SettingsManager::new(&conn, "Hello").unwrap();
    // When not encrypted, TOTP menu item shouldn't appear, clamp at MENU_PASSWORD
    mgr.handle_key(KeyCode::Down, &conn);
    assert_eq!(mgr.selection, MENU_PASSWORD);
    mgr.handle_key(KeyCode::Down, &conn);
    assert_eq!(mgr.selection, MENU_PASSWORD); // clamped — not encrypted
}
```

**Step 7: Verify compilation and tests**

Run: `cargo check`
Run: `cargo test settings_manager --features totp`
Expected: all tests pass

**Step 8: Commit**

```bash
git add src/cli/settings_manager.rs
git commit -m "Add Two-Factor Auth menu entry to settings manager"
```

---

### Task 9: Wire up onboarding TOTP in dashboard

**Files:**
- Modify: `src/cli/dashboard.rs` (where OnboardingResult is consumed)

**Step 1: Find where OnboardingResult is consumed and add TOTP storage**

Search for the code that handles `OnboardingResult` after database creation. After the database is created and initialized (with the password set), add:

```rust
#[cfg(feature = "totp")]
if let Some(ref totp_secret) = result.totp_secret {
    if let Ok(conn) = crate::db::get_connection(&db_path) {
        if let Err(e) = crate::totp::enable(&conn, &db_path, totp_secret) {
            eprintln!("Warning: Could not enable 2FA: {e}");
        }
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: compiles

**Step 3: Commit**

```bash
git add src/cli/dashboard.rs
git commit -m "Wire up onboarding TOTP secret storage in dashboard"
```

---

### Task 10: Update documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

**Step 1: Update CLAUDE.md**

Add to the Architecture section under Database:
```
Optional TOTP 2FA via `totp` feature flag; TOTP secret stored in OS keychain via `keyring` crate (macOS Keychain, Windows Credential Store, Linux keyutils/Secret Service); `totp_enabled` flag in `metadata` table; `src/totp.rs` module handles secret generation, verification, and keychain operations
```

Add to Commands section:
```bash
nigel password totp-enable                        # Enable TOTP 2FA (requires encrypted database)
nigel password totp-disable                       # Disable TOTP 2FA
```

Add to Key Design Constraints:
```
- TOTP 2FA is optional (`totp` feature flag, enabled by default); secret stored in OS keychain via `keyring` (never in the database); `totp_enabled` metadata flag signals TOTP is required; device-bound — backing up/restoring DB to another machine does not carry over TOTP enrollment
```

Add `totp.rs` to the Project Structure:
```
  totp.rs               # TOTP 2FA (feature-gated behind "totp"): secret gen, verification, keychain storage
```

**Step 2: Update README.md**

Add 2FA to the Features section and update Configuration/password commands.

**Step 3: Verify no compilation regressions**

Run: `cargo check`
Run: `cargo test`
Expected: all pass

**Step 4: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "Document TOTP 2FA feature in CLAUDE.md and README.md"
```

---

### Task 11: Run full test suite and verify

**Step 1: Run all tests with TOTP feature**

Run: `cargo test --features totp`
Expected: all tests pass

**Step 2: Run all tests without TOTP feature**

Run: `cargo test --no-default-features`
Expected: all tests pass (TOTP code fully gated)

**Step 3: Run clippy**

Run: `cargo clippy --features totp`
Expected: no warnings

**Step 4: Build release**

Run: `cargo build --release`
Expected: builds successfully
