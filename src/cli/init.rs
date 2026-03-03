use std::path::PathBuf;

use crate::db::{get_connection, init_db};
use crate::error::Result;
use crate::settings::{
    load_settings, restrict_dir_permissions, save_settings, shellexpand_path, Settings,
};

pub fn run(data_dir: Option<String>) -> Result<()> {
    let mut settings = load_settings();
    let defaults = Settings::default();

    if let Some(dir) = data_dir {
        settings.data_dir = shellexpand_path(&dir);
    } else if settings.data_dir == defaults.data_dir && settings.user_name == defaults.user_name {
        // First run — prompt for data dir
        let default = &settings.data_dir;
        println!("Data directory [{}]: ", default);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let chosen = input.trim();
        if !chosen.is_empty() {
            settings.data_dir = shellexpand_path(chosen);
        }
    }

    save_settings(&settings)?;

    let resolved = PathBuf::from(&settings.data_dir);
    std::fs::create_dir_all(&resolved)?;
    restrict_dir_permissions(&resolved)?;
    let exports_dir = resolved.join("exports");
    std::fs::create_dir_all(&exports_dir)?;
    restrict_dir_permissions(&exports_dir)?;

    let conn = get_connection(&resolved.join("nigel.db"))?;
    init_db(&conn)?;

    println!("Initialized nigel at {}", resolved.display());
    Ok(())
}
