use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{NigelError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub data_dir: String,
    #[serde(default)]
    pub company_name: String,
    #[serde(default = "default_fiscal_year_start")]
    pub fiscal_year_start: String,
}

fn default_fiscal_year_start() -> String {
    "01".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir().to_string_lossy().to_string(),
            company_name: String::new(),
            fiscal_year_start: default_fiscal_year_start(),
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
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| NigelError::Settings(e.to_string()))?;
    std::fs::write(settings_path(), format!("{json}\n"))?;
    Ok(())
}

pub fn get_data_dir() -> PathBuf {
    PathBuf::from(&load_settings().data_dir)
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
            company_name: "Acme Corp".to_string(),
            fiscal_year_start: "07".to_string(),
        };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&path, &json).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Settings = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.company_name, "Acme Corp");
        assert_eq!(loaded.data_dir, "/tmp/test");
        assert_eq!(loaded.fiscal_year_start, "07");
    }

    #[test]
    fn test_load_returns_defaults_when_missing() {
        let s = Settings::default();
        assert!(s.company_name.is_empty());
        assert_eq!(s.fiscal_year_start, "01");
        assert!(!s.data_dir.is_empty());
    }

    #[test]
    fn test_load_merges_with_defaults() {
        let json = r#"{"data_dir": "/tmp/test", "company_name": "Test"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.fiscal_year_start, "01");
        assert_eq!(s.company_name, "Test");
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
