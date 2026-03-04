use rusqlite::Connection;
use std::path::Path;

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
    let totp = match TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret,
        None,
        String::new(),
    ) {
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
        crate::error::NigelError::Other(format!(
            "Could not read TOTP secret from keychain: {e}"
        ))
    })
}

/// Enable TOTP: store secret in OS keychain, set metadata flag.
pub fn enable(conn: &Connection, db_path: &Path, secret: &str) -> Result<()> {
    let entry = keychain_entry(db_path)?;
    entry.set_password(secret).map_err(|e| {
        crate::error::NigelError::Other(format!(
            "Could not store TOTP secret in keychain: {e}"
        ))
    })?;
    db::set_metadata(conn, "totp_enabled", "true")?;
    Ok(())
}

/// Disable TOTP: remove secret from OS keychain, clear metadata flag.
pub fn disable(conn: &Connection, db_path: &Path) -> Result<()> {
    let entry = keychain_entry(db_path)?;
    let _ = entry.delete_credential(); // Ignore "not found" errors
    conn.execute("DELETE FROM metadata WHERE key = 'totp_enabled'", [])
        .map_err(crate::error::NigelError::Db)?;
    Ok(())
}

/// CLI helper: if TOTP is enabled, prompt for code via stdin (up to 3 attempts).
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
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret_bytes,
            None,
            String::new(),
        )
        .unwrap();
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
