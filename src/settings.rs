use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{NigelError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub data_dir: String,
    #[serde(default)]
    pub user_name: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir().to_string_lossy().to_string(),
            user_name: String::new(),
        }
    }
}

fn config_dir() -> PathBuf {
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
}
