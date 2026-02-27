use std::path::PathBuf;

use crate::db::{get_connection, init_db};
use crate::error::Result;
use crate::settings::{load_settings, save_settings, Settings};

pub fn run(data_dir: Option<String>) -> Result<()> {
    let mut settings = load_settings();
    let defaults = Settings::default();

    if let Some(dir) = data_dir {
        settings.data_dir = shellexpand_path(&dir);
    } else if settings.data_dir == defaults.data_dir && settings.company_name == defaults.company_name {
        // First run — prompt for data dir
        let default = &settings.data_dir;
        println!("Data directory [{}]: ", default);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let chosen = input.trim();
        if !chosen.is_empty() {
            settings.data_dir = shellexpand_path(chosen);
        }
    }

    save_settings(&settings)?;

    let resolved = PathBuf::from(&settings.data_dir);
    std::fs::create_dir_all(&resolved)?;
    std::fs::create_dir_all(resolved.join("imports"))?;
    std::fs::create_dir_all(resolved.join("exports"))?;

    let conn = get_connection(&resolved.join("nigel.db"))?;
    init_db(&conn)?;

    println!("Initialized nigel at {}", resolved.display());
    Ok(())
}

fn shellexpand_path(path: &str) -> String {
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
