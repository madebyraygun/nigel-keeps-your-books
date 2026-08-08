use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{NigelError, Result};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub data_dir: String,
    #[serde(default)]
    pub user_name: String,
    #[serde(default = "default_true")]
    pub update_check: bool,
    #[serde(default)]
    pub last_update_check: Option<String>,
    #[serde(default)]
    pub stripe_secret_key: Option<String>,
    #[serde(default)]
    pub mailgun_api_key: Option<String>,
    #[serde(default)]
    pub mailgun_domain: Option<String>,
    #[serde(default)]
    pub from_email: Option<String>,
    #[serde(default)]
    pub r2_account_id: Option<String>,
    #[serde(default)]
    pub r2_access_key: Option<String>,
    #[serde(default)]
    pub r2_secret_key: Option<String>,
    #[serde(default)]
    pub r2_bucket: Option<String>,
    #[serde(default)]
    pub public_base_url: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir().to_string_lossy().to_string(),
            user_name: String::new(),
            update_check: true,
            last_update_check: None,
            stripe_secret_key: None,
            mailgun_api_key: None,
            mailgun_domain: None,
            from_email: None,
            r2_account_id: None,
            r2_access_key: None,
            r2_secret_key: None,
            r2_bucket: None,
            public_base_url: None,
        }
    }
}

/// Test-only redirection of the config directory.
///
/// Without it, any test that calls [`save_settings`] rewrites the developer's
/// real `~/.config/nigel/settings.json` and repoints their data directory. An
/// in-crate override is used rather than `$HOME`: `dirs::home_dir` does not
/// consult the environment on every platform, and mutating the environment is
/// process-global and unsafe in newer editions.
#[cfg(test)]
static CONFIG_DIR_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Point the config directory somewhere else and hand back what it was.
///
/// Returning the previous value is what lets a guard put it back rather than
/// clearing it: clearing exposes the real `~/.config/nigel/settings.json` to
/// whatever is still running, and the value it writes there is a temporary
/// directory that is about to be deleted.
#[cfg(test)]
pub fn set_config_dir_for_tests(dir: Option<PathBuf>) -> Option<PathBuf> {
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    std::mem::replace(&mut *CONFIG_DIR_OVERRIDE.lock().unwrap(), dir)
}

/// Redirect `~/.config/nigel` at a temporary directory for the life of the
/// guard.
///
/// Lives here rather than in the server's test helpers because the config
/// directory is this module's, and tests outside the `serve` feature need it
/// too: anything that reads [`load_settings`] otherwise answers from whatever
/// the developer's own settings.json happens to say.
#[cfg(test)]
pub struct TempConfigDir {
    _dir: tempfile::TempDir,
    previous: Option<PathBuf>,
}

#[cfg(test)]
impl TempConfigDir {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let previous = set_config_dir_for_tests(Some(dir.path().to_path_buf()));
        Self {
            _dir: dir,
            previous,
        }
    }
}

#[cfg(test)]
impl Default for TempConfigDir {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl Drop for TempConfigDir {
    /// Restores what was there rather than clearing, so a guard dropping while
    /// another is alive cannot hand the real config directory back to a test
    /// that is still writing.
    fn drop(&mut self) {
        set_config_dir_for_tests(self.previous.take());
    }
}

fn config_dir() -> PathBuf {
    #[cfg(test)]
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    if let Some(dir) = CONFIG_DIR_OVERRIDE.lock().unwrap().clone() {
        return dir;
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("nigel")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("nigel")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Settings::default()
    }
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    restrict_dir_permissions(&dir)?;
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| NigelError::Settings(e.to_string()))?;
    let path = settings_path();
    std::fs::write(&path, format!("{json}\n"))?;
    restrict_file_permissions(&path)?;
    Ok(())
}

pub fn settings_file_exists() -> bool {
    settings_path().exists()
}

/// Read and remove legacy `company_name` from settings.json if present.
/// Returns the value so it can be migrated to the DB metadata table.
pub fn migrate_company_name() -> Option<String> {
    let path = settings_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let mut raw: serde_json::Value = serde_json::from_str(&content).ok()?;
    let company = raw.as_object_mut()?.remove("company_name")?;
    let name = company.as_str()?.to_string();
    if name.is_empty() {
        return None;
    }
    // Rewrite settings without company_name
    if let Ok(json) = serde_json::to_string_pretty(&raw) {
        let _ = std::fs::write(&path, format!("{json}\n"));
    }
    Some(name)
}

/// Resolved invoicing credentials and endpoints.
pub struct InvoicingConfig {
    pub stripe_secret_key: Option<String>,
    pub mailgun_api_key: Option<String>,
    pub mailgun_domain: Option<String>,
    pub from_email: Option<String>,
    pub r2_account_id: Option<String>,
    pub r2_access_key: Option<String>,
    pub r2_secret_key: Option<String>,
    pub r2_bucket: Option<String>,
    pub public_base_url: Option<String>,
}

pub fn invoicing_config_from(s: &Settings) -> InvoicingConfig {
    invoicing_config_with(s, |name| std::env::var(name).ok())
}

/// The env lookup is injected so tests can exercise the env-wins precedence
/// without mutating the process environment.
fn invoicing_config_with(s: &Settings, env: impl Fn(&str) -> Option<String>) -> InvoicingConfig {
    let env_or = |name: &str, file_val: &Option<String>| env(name).or_else(|| file_val.clone());
    InvoicingConfig {
        stripe_secret_key: env_or("NIGEL_STRIPE_SECRET_KEY", &s.stripe_secret_key),
        mailgun_api_key: env_or("NIGEL_MAILGUN_API_KEY", &s.mailgun_api_key),
        mailgun_domain: env_or("NIGEL_MAILGUN_DOMAIN", &s.mailgun_domain),
        from_email: env_or("NIGEL_FROM_EMAIL", &s.from_email),
        r2_account_id: env_or("NIGEL_R2_ACCOUNT_ID", &s.r2_account_id),
        r2_access_key: env_or("NIGEL_R2_ACCESS_KEY", &s.r2_access_key),
        r2_secret_key: env_or("NIGEL_R2_SECRET_KEY", &s.r2_secret_key),
        r2_bucket: env_or("NIGEL_R2_BUCKET", &s.r2_bucket),
        public_base_url: env_or("NIGEL_PUBLIC_BASE_URL", &s.public_base_url),
    }
}

pub fn invoicing_config() -> InvoicingConfig {
    invoicing_config_from(&load_settings())
}

pub fn get_data_dir() -> PathBuf {
    PathBuf::from(&load_settings().data_dir)
}

pub fn shellexpand_path(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .to_string()
}

/// Restrict a file to owner-only read/write (0o600) on Unix.
/// No-op on non-Unix platforms.
pub fn restrict_file_permissions(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Restrict a directory to owner-only access (0o700) on Unix.
/// No-op on non-Unix platforms.
pub fn restrict_dir_permissions(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            data_dir: "/tmp/test".to_string(),
            user_name: "Alice".to_string(),
            update_check: true,
            last_update_check: None,
            ..Settings::default()
        };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&path, &json).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Settings = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.user_name, "Alice");
        assert_eq!(loaded.data_dir, "/tmp/test");
    }

    #[test]
    fn test_load_returns_defaults_when_missing() {
        let s = Settings::default();
        assert!(s.user_name.is_empty());
        assert!(!s.data_dir.is_empty());
    }

    #[test]
    fn test_load_merges_with_defaults() {
        let json = r#"{"data_dir": "/tmp/test", "user_name": "Bob"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.user_name, "Bob");
    }

    #[test]
    fn test_ignores_unknown_fields_from_older_versions() {
        let json = r#"{"data_dir": "/tmp/test", "user_name": "Bob", "fiscal_year_start": "07"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.user_name, "Bob");
    }

    #[test]
    fn test_save_creates_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("deep").join("nested").join("dir");
        std::fs::create_dir_all(&nested).unwrap();
        let path = nested.join("settings.json");
        let settings = Settings::default();
        let json = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&path, format!("{json}\n")).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_update_check_defaults_true() {
        let s = Settings::default();
        assert!(s.update_check);
        assert!(s.last_update_check.is_none());
    }

    #[test]
    fn test_update_check_roundtrip() {
        let settings = Settings {
            data_dir: "/tmp/test".to_string(),
            user_name: "Alice".to_string(),
            update_check: false,
            last_update_check: Some("2025-06-15T10:30:00".to_string()),
            ..Settings::default()
        };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert!(!loaded.update_check);
        assert_eq!(
            loaded.last_update_check.as_deref(),
            Some("2025-06-15T10:30:00")
        );
    }

    #[test]
    fn invoicing_config_prefers_env_over_settings() {
        let file_settings = Settings {
            data_dir: "/x".into(),
            user_name: String::new(),
            update_check: true,
            last_update_check: None,
            stripe_secret_key: Some("rk_file".into()),
            public_base_url: Some("https://file.example/i".into()),
            ..Settings::default()
        };

        let cfg = invoicing_config_with(&file_settings, |name| match name {
            "NIGEL_STRIPE_SECRET_KEY" => Some("rk_env".into()),
            "NIGEL_PUBLIC_BASE_URL" => Some("https://env.example/i".into()),
            _ => None,
        });
        assert_eq!(cfg.stripe_secret_key.as_deref(), Some("rk_env"));
        assert_eq!(
            cfg.public_base_url.as_deref(),
            Some("https://env.example/i")
        );

        let cfg2 = invoicing_config_with(&file_settings, |_| None);
        assert_eq!(cfg2.stripe_secret_key.as_deref(), Some("rk_file"));
        assert_eq!(
            cfg2.public_base_url.as_deref(),
            Some("https://file.example/i")
        );

        let cfg3 = invoicing_config_with(&Settings::default(), |_| None);
        assert_eq!(cfg3.public_base_url, None);
        assert_eq!(cfg3.mailgun_domain, None);
        assert_eq!(cfg3.from_email, None);
    }

    #[test]
    fn test_legacy_settings_get_update_check_default() {
        // Simulates loading settings.json that was created before update_check existed
        let json = r#"{"data_dir": "/tmp/test", "user_name": "Bob"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.update_check); // defaults to true
        assert!(s.last_update_check.is_none());
    }
}
