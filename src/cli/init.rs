use std::path::PathBuf;

use crate::db::{get_connection, init_db_with_profile, Profile};
use crate::error::Result;
use crate::settings::{
    load_settings, restrict_dir_permissions, save_settings, shellexpand_path, Settings,
};

pub fn run(data_dir: Option<String>, profile: &str) -> Result<()> {
    let Some(profile) = Profile::parse(profile) else {
        return Err(crate::error::NigelError::Other(format!(
            "Unknown --profile '{profile}'. Expected 'business' or 'personal'."
        )));
    };

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

    let db_path = resolved.join("nigel.db");
    let already_seeded = db_path.exists();
    let conn = get_connection(&db_path)?;
    init_db_with_profile(&conn, profile)?;

    // The requested profile only takes effect on a fresh database; say so
    // rather than letting a personal init silently keep the business chart.
    let seeded = crate::db::get_profile(&conn);
    if already_seeded && seeded != profile {
        eprintln!(
            "Note: this database already keeps {} books; --profile {} was ignored.",
            seeded.as_str(),
            profile.as_str()
        );
    }

    println!("Initialized nigel at {}", resolved.display());
    Ok(())
}
