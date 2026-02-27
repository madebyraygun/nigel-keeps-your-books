use std::path::PathBuf;

use crate::error::{NigelError, Result};
use crate::settings::{load_settings, save_settings, shellexpand_path};

pub fn run(path: &str) -> Result<()> {
    let resolved = PathBuf::from(shellexpand_path(path));
    let db_path = resolved.join("nigel.db");

    if !db_path.exists() {
        return Err(NigelError::Settings(format!(
            "No database found at {}\nRun `nigel init --data-dir {}` to create one.",
            db_path.display(),
            resolved.display()
        )));
    }

    let mut settings = load_settings();
    settings.data_dir = resolved.to_string_lossy().to_string();
    save_settings(&settings)?;

    println!("Switched to {}", resolved.display());
    Ok(())
}
