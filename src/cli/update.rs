use std::io::Write;

use crate::error::{NigelError, Result};
use crate::settings::{load_settings, save_settings};

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/madebyraygun/nigel-keeps-your-books/releases/latest";

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const TIMESTAMP_FMT: &str = "%Y-%m-%dT%H:%M:%S";

/// Minimum acceptable binary size (1 MB) to guard against truncated downloads.
const MIN_BINARY_SIZE: usize = 1_000_000;

/// Information about an available update.
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

/// Build an HTTP client with the given timeout.
fn http_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .user_agent(format!("nigel/{CURRENT_VERSION}"))
        .build()
        .map_err(|e| NigelError::Other(format!("HTTP client error: {e}")))
}

/// Returns the expected release asset name for the current platform.
pub fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => Some("nigel-universal-apple-darwin"),
        ("linux", "x86_64") => Some("nigel-x86_64-unknown-linux-gnu"),
        ("windows", "x86_64") => Some("nigel-x86_64-pc-windows-msvc.exe"),
        _ => None,
    }
}

/// Check the GitHub Releases API for a newer version.
/// Returns `Some(UpdateInfo)` if a newer version is available, `None` otherwise.
pub fn check_for_update() -> Result<Option<UpdateInfo>> {
    let client = http_client(5)?;

    let resp: serde_json::Value = client
        .get(GITHUB_API_URL)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| NigelError::Other(format!("Update check failed: {e}")))?
        .json()
        .map_err(|e| NigelError::Other(format!("Invalid response: {e}")))?;

    let tag = resp["tag_name"]
        .as_str()
        .ok_or_else(|| NigelError::Other("Missing tag_name in release".into()))?;

    let remote_version = tag.strip_prefix('v').unwrap_or(tag);

    if !is_newer(remote_version, CURRENT_VERSION) {
        return Ok(None);
    }

    let asset = asset_name()
        .ok_or_else(|| NigelError::Other("Unsupported platform for auto-update".into()))?;

    let download_url = resp["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a["name"].as_str() == Some(asset))
                .and_then(|a| a["browser_download_url"].as_str())
        })
        .ok_or_else(|| NigelError::Other(format!("Release asset '{asset}' not found")))?
        .to_string();

    Ok(Some(UpdateInfo {
        version: remote_version.to_string(),
        download_url,
    }))
}

/// Download the binary from `url` and replace the current executable.
fn download_and_install(url: &str) -> Result<()> {
    println!("Downloading...");
    let client = http_client(120)?;

    let bytes = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| NigelError::Other(format!("Download failed: {e}")))?
        .bytes()
        .map_err(|e| NigelError::Other(format!("Download failed: {e}")))?;

    if bytes.len() < MIN_BINARY_SIZE {
        return Err(NigelError::Other(format!(
            "Downloaded file too small ({} bytes, minimum {}). Aborting.",
            bytes.len(),
            MIN_BINARY_SIZE
        )));
    }

    // Write to a temp file with a unique name, then atomically replace
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("nigel-update-{}", std::process::id()));
    std::fs::write(&tmp_path, &bytes)?;

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    println!("Installing ({} bytes)...", bytes.len());
    self_replace::self_replace(&tmp_path)
        .map_err(|e| NigelError::Other(format!("Failed to replace binary: {e}")))?;

    // Clean up temp file (best effort)
    let _ = std::fs::remove_file(&tmp_path);

    Ok(())
}

/// The `nigel update` CLI command.
pub fn run() -> Result<()> {
    println!("Checking for updates...");
    match check_for_update() {
        Ok(Some(info)) => {
            print!(
                "Nigel v{} is available (current: v{CURRENT_VERSION}). Install? [Y/n] ",
                info.version
            );
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();
            if input.is_empty() || input == "y" || input == "yes" {
                download_and_install(&info.download_url)?;
                println!(
                    "Updated to v{}. Restart nigel to use the new version.",
                    info.version
                );
            } else {
                println!("Update cancelled.");
            }
        }
        Ok(None) => {
            println!("You're on the latest version (v{CURRENT_VERSION}).");
        }
        Err(e) => {
            return Err(e);
        }
    }
    Ok(())
}

/// Non-blocking check on launch: respects cooldown and opt-out setting.
/// Returns a notification message if an update is available, or None.
pub fn check_and_notify() -> Option<String> {
    let mut settings = load_settings();

    if !settings.update_check {
        return None;
    }

    let now = chrono::Local::now().naive_local();

    // Check cooldown (24 hours)
    if let Some(ref last_check) = settings.last_update_check {
        if let Ok(last) = chrono::NaiveDateTime::parse_from_str(last_check, TIMESTAMP_FMT) {
            if now.signed_duration_since(last) < chrono::Duration::hours(24) {
                return None;
            }
        }
    }

    // Update the timestamp regardless of check result.
    // If we can't persist, skip the check to avoid hammering the API on every launch.
    settings.last_update_check = Some(now.format(TIMESTAMP_FMT).to_string());
    if save_settings(&settings).is_err() {
        return None;
    }

    // Attempt the check, silently returning None on any error
    let info = check_for_update().ok()??;
    Some(format!(
        "A new version of Nigel is available: v{}. Run `nigel update` to install.",
        info.version
    ))
}

/// Compare two semver strings. Returns true if `remote` is newer than `current`.
pub fn is_newer(remote: &str, current: &str) -> bool {
    let remote_ver = semver::Version::parse(remote).ok();
    let current_ver = semver::Version::parse(current).ok();
    match (remote_ver, current_ver) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_name_returns_some() {
        // On any supported CI/dev platform this should return Some
        let name = asset_name();
        assert!(
            name.is_some(),
            "asset_name() returned None on this platform"
        );
        let name = name.unwrap();
        assert!(name.starts_with("nigel-"));
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("1.1.0", "1.0.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn test_is_newer_with_prerelease() {
        // Pre-release versions are lower than their release counterparts
        assert!(!is_newer("1.0.1-beta.1", "1.0.1"));
        assert!(is_newer("1.0.1", "1.0.1-beta.1"));
    }

    #[test]
    fn test_is_newer_invalid_version() {
        assert!(!is_newer("not-a-version", "1.0.0"));
        assert!(!is_newer("1.0.0", "not-a-version"));
    }

    #[test]
    fn test_current_version_is_valid_semver() {
        assert!(
            semver::Version::parse(CURRENT_VERSION).is_ok(),
            "CARGO_PKG_VERSION is not valid semver: {CURRENT_VERSION}"
        );
    }

    #[test]
    fn test_cooldown_within_24h() {
        // Simulate a recent check timestamp
        let now = chrono::Local::now().naive_local();
        let recent = now - chrono::Duration::hours(1);
        let timestamp = recent.format(TIMESTAMP_FMT).to_string();

        // Parse and check the cooldown logic
        let last = chrono::NaiveDateTime::parse_from_str(&timestamp, TIMESTAMP_FMT).unwrap();
        let duration = now.signed_duration_since(last);
        assert!(duration < chrono::Duration::hours(24));
    }

    #[test]
    fn test_cooldown_expired() {
        let now = chrono::Local::now().naive_local();
        let old = now - chrono::Duration::hours(25);
        let timestamp = old.format(TIMESTAMP_FMT).to_string();

        let last = chrono::NaiveDateTime::parse_from_str(&timestamp, TIMESTAMP_FMT).unwrap();
        let duration = now.signed_duration_since(last);
        assert!(duration >= chrono::Duration::hours(24));
    }
}
