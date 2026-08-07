//! Settings: the business name, the auto-update toggle, the active data
//! directory, and the database password.
//!
//! Parity target is `cli/settings_manager.rs` plus `nigel load`. The four
//! password and data-directory operations rewrite the database *file* rather
//! than rows in it, so they take the `db_gate` write guard: `encrypt_database`
//! and `decrypt_database` finish with a rename and drop the `-wal`/`-shm`
//! sidecars, which a live connection elsewhere would not survive.
//!
//! Every route here sits behind the locked guard. `company-name` plainly needs
//! the key; `settings/app` does not, but nothing on the unlock screen reads it,
//! and exempting a route to serve a screen that does not exist is how a guard
//! rots. `password/change` and `password/remove` carry the current password in
//! the body and would technically work while locked — exempting them would hand
//! over a password oracle that bypasses the unlock screen's throttle entirely.

use std::path::PathBuf;

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::cli::password;
use crate::db;
use crate::settings::{self, Settings};

use super::super::error::{ApiError, ApiResult};
use super::super::extract::ApiJson;
use super::super::secret::Secret;
use super::super::state::AppState;
use super::status::{current_status, StatusResponse};
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings/app", get(get_app).put(put_app))
        .route("/settings/company-name", put(put_company_name))
        .route("/settings/data-dir", post(post_data_dir))
        .route("/settings/password/set", post(post_password_set))
        .route("/settings/password/change", post(post_password_change))
        .route("/settings/password/remove", post(post_password_remove))
}

// ---------------------------------------------------------------------------
// application settings (settings.json)
// ---------------------------------------------------------------------------

/// The settings.json fields the web UI shows.
///
/// Hand-written rather than serializing [`Settings`], which has no
/// `rename_all` and would put snake_case on the wire — the one casing rule this
/// API documents. There is deliberately no `dataDir` here: `/api/status`
/// already reports the directory the server actually opened, and two sources
/// for one value is how they drift.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    user_name: String,
    update_check: bool,
    last_update_check: Option<String>,
}

impl From<Settings> for AppSettings {
    fn from(settings: Settings) -> Self {
        Self {
            user_name: settings.user_name,
            update_check: settings.update_check,
            last_update_check: settings.last_update_check,
        }
    }
}

async fn get_app() -> Json<AppSettings> {
    Json(settings::load_settings().into())
}

/// `updateCheck` is the only field the web UI may write. The user name is set
/// during onboarding and the last-check timestamp is the updater's bookkeeping;
/// neither is the settings screen's to change.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    update_check: bool,
}

async fn put_app(ApiJson(patch): ApiJson<AppSettingsPatch>) -> ApiResult<Json<AppSettings>> {
    let mut settings = settings::load_settings();
    settings.update_check = patch.update_check;
    settings::save_settings(&settings)?;
    Ok(Json(settings.into()))
}

// ---------------------------------------------------------------------------
// company name (database metadata)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyNameRequest {
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyNameResponse {
    company_name: String,
}

/// The name is trimmed, and an empty one is allowed: clearing the business name
/// is what the TUI's settings screen does with an empty field, and refusing it
/// here would make the web UI the only place you cannot undo a typo.
async fn put_company_name(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<CompanyNameRequest>,
) -> ApiResult<Json<CompanyNameResponse>> {
    let name = request.name.trim().to_string();
    let stored = name.clone();
    with_conn(&state, move |conn| {
        db::set_metadata(conn, "company_name", &stored)
    })
    .await?;
    Ok(Json(CompanyNameResponse { company_name: name }))
}

// ---------------------------------------------------------------------------
// data directory
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirRequest {
    path: String,
}

/// Switch the database this server serves, mirroring `nigel load` and then
/// doing the three things a *running* server additionally has to.
///
/// Rewriting settings.json alone would leave every later request reading the
/// old database under the new directory's name. So the path this process holds
/// is rebound too, the password global is cleared (an encrypted target must
/// come up locked rather than inherit the previous database's key), and the
/// failed-attempt budget is reset, because that budget belongs to a database
/// rather than to a process.
async fn post_data_dir(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<DataDirRequest>,
) -> ApiResult<Json<StatusResponse>> {
    if request.path.trim().is_empty() {
        return Err(ApiError::bad_request("A data directory path is required."));
    }

    let resolved = PathBuf::from(settings::shellexpand_path(request.path.trim()));
    let db_path = resolved.join("nigel.db");
    if !db_path.exists() {
        return Err(ApiError::bad_request(format!(
            "No database found at {}. Run `nigel init --data-dir {}` to create one.",
            db_path.display(),
            resolved.display()
        )));
    }

    {
        let _gate = state.db_gate.write().await;

        // Everything that can fail happens before anything is committed. A
        // target this build cannot open must leave the switch un-made: rebinding
        // first and discovering the problem afterwards would strand the server
        // on a database it cannot serve, and settings.json would send it back
        // there on the next start.
        if !db::is_encrypted(&db_path)? {
            // Opened with no password explicitly rather than through
            // `get_connection`: the process global still holds the *current*
            // database's key at this point, and applying it to a plaintext file
            // would fail.
            let path = db_path.clone();
            tokio::task::spawn_blocking(move || -> ApiResult<()> {
                // The same pre-flight `nigel serve` runs at startup, so a target
                // written by an older version is migrated before the first read.
                // An encrypted one is skipped; unlocking migrates it.
                let conn = db::open_connection(&path, None)?;
                db::init_db(&conn).map_err(ApiError::internal)
            })
            .await
            .map_err(ApiError::internal)??;
        }

        let mut stored = settings::load_settings();
        stored.data_dir = resolved.to_string_lossy().to_string();
        settings::save_settings(&stored)?;

        db::set_db_password(None);
        state.set_db_path(db_path);
        state.unlock.reset();
    }

    Ok(Json(current_status(&state).await?))
}

// ---------------------------------------------------------------------------
// database password
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPasswordRequest {
    new_password: Secret,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    current_password: Secret,
    new_password: Secret,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovePasswordRequest {
    current_password: Secret,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordStateResponse {
    encrypted: bool,
    locked: bool,
}

/// Decode a password body without letting the rejection quote it.
///
/// The same reasoning `POST /api/unlock` documents: serde's type-mismatch
/// messages include the offending value, and `ApiJson`'s rejection path passes
/// that text straight through. These three routes carry passwords, so they get
/// a fixed message instead of whatever the deserializer wanted to say.
fn password_body<T>(body: Result<Json<T>, JsonRejection>, shape: &str) -> ApiResult<T> {
    body.map(|Json(value)| value)
        .map_err(|_| ApiError::bad_request(format!("Expected a JSON body of the form {shape}.")))
}

/// Encrypt a plaintext database.
async fn post_password_set(
    State(state): State<AppState>,
    body: Result<Json<SetPasswordRequest>, JsonRejection>,
) -> ApiResult<Json<PasswordStateResponse>> {
    let request = password_body(body, "{\"newPassword\": \"...\"}")?;
    let new_password = trimmed_new_password(&request.new_password)?;

    // Taken before the path is read, so a data-directory switch that lands
    // between the two cannot leave this encrypting the database it replaced.
    let _gate = state.db_gate.write().await;
    let db_path = state.db_path();
    if db::is_encrypted(&db_path)? {
        return Err(already_encrypted());
    }
    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        password::encrypt_database(&db_path, &new_password)?;
        // Only after the file is genuinely encrypted: the reverse order leaves
        // this process holding a key for a database that is still plaintext.
        db::set_db_password(Some(new_password));
        Ok(())
    })
    .await
    .map_err(ApiError::internal)??;

    Ok(Json(PasswordStateResponse {
        encrypted: true,
        locked: false,
    }))
}

/// Change the password on an encrypted database.
async fn post_password_change(
    State(state): State<AppState>,
    body: Result<Json<ChangePasswordRequest>, JsonRejection>,
) -> ApiResult<Json<PasswordStateResponse>> {
    let request = password_body(
        body,
        "{\"currentPassword\": \"...\", \"newPassword\": \"...\"}",
    )?;
    let new_password = trimmed_new_password(&request.new_password)?;
    let current = request.current_password.expose().trim().to_string();

    let _gate = state.db_gate.write().await;
    let db_path = state.db_path();
    if !db::is_encrypted(&db_path)? {
        return Err(not_encrypted());
    }
    verify_current_password(&state, &db_path, &current).await?;

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        password::rekey_database(&db_path, &current, &new_password)?;
        db::set_db_password(Some(new_password));
        Ok(())
    })
    .await
    .map_err(ApiError::internal)??;

    Ok(Json(PasswordStateResponse {
        encrypted: true,
        locked: false,
    }))
}

/// Decrypt a database, removing its password.
async fn post_password_remove(
    State(state): State<AppState>,
    body: Result<Json<RemovePasswordRequest>, JsonRejection>,
) -> ApiResult<Json<PasswordStateResponse>> {
    let request = password_body(body, "{\"currentPassword\": \"...\"}")?;
    let current = request.current_password.expose().trim().to_string();

    let _gate = state.db_gate.write().await;
    let db_path = state.db_path();
    if !db::is_encrypted(&db_path)? {
        return Err(not_encrypted());
    }
    verify_current_password(&state, &db_path, &current).await?;

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        password::decrypt_database(&db_path, &current)?;
        db::set_db_password(None);
        Ok(())
    })
    .await
    .map_err(ApiError::internal)??;

    Ok(Json(PasswordStateResponse {
        encrypted: false,
        locked: false,
    }))
}

/// Check the supplied current password, throttled by the same gate the unlock
/// endpoint uses.
///
/// Sharing that counter is the point: guessing a password through the change
/// endpoint has to cost exactly what guessing it through unlock costs, or the
/// throttle is decoration. Validating up front also keeps a wrong password a
/// clean `401` instead of the `NotADatabase` failure `rekey_database` would
/// raise a moment later.
///
/// **The caller holds `db_gate`.** Both callers go on to rewrite the database
/// file, so they hold the write side across the check and the rewrite together
/// — taking a read guard here would deadlock against it, and releasing between
/// the two would let a data-directory switch land in the middle.
async fn verify_current_password(
    state: &AppState,
    db_path: &std::path::Path,
    current: &str,
) -> ApiResult<()> {
    let path = db_path.to_path_buf();
    let candidate = current.to_string();
    let valid = tokio::task::spawn_blocking(move || db::validate_password(&path, &candidate))
        .await
        .map_err(ApiError::internal)?
        // The error text is dropped: the key is applied as literal SQL in a
        // `PRAGMA key` statement, which rusqlite prints back in its message.
        .map_err(|_| ApiError::internal("Couldn't open the database to check the password."))?;
    if valid {
        state.unlock.reset();
        return Ok(());
    }

    let (attempts_remaining, delay) = state.unlock.record_failure();
    tokio::time::sleep(delay).await;
    Err(ApiError::invalid_password(
        attempts_remaining,
        delay.as_millis(),
    ))
}

/// Trim as `prompt_and_confirm` and the TUI's password screen both do, so a
/// password set from the web can always be typed back in from the terminal.
///
/// Control characters are refused for the same reason. `encrypt_database` binds
/// the key as a parameter and so accepts anything, but every later open applies
/// it through `PRAGMA key = '…'` as literal SQL, which cannot tokenize an
/// embedded NUL. A password containing one encrypts the database and then locks
/// its owner out permanently: it cannot be typed at a terminal prompt, and it
/// cannot survive `NIGEL_DB_PASSWORD` either, since a NUL terminates the value.
fn trimmed_new_password(secret: &Secret) -> ApiResult<String> {
    let trimmed = secret.expose().trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "The password cannot be empty. Use `password/remove` to decrypt the database.",
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "The password cannot contain control characters.",
        ));
    }
    Ok(trimmed.to_string())
}

fn already_encrypted() -> ApiError {
    ApiError::conflict(
        "This database is already encrypted. Change the password instead.",
        serde_json::json!({ "reason": "already_encrypted" }),
    )
}

fn not_encrypted() -> ApiError {
    ApiError::conflict(
        "This database is not encrypted. Set a password first.",
        serde_json::json!({ "reason": "not_encrypted" }),
    )
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;
    use serde_json::json;

    /// Every test here writes settings.json, so every one needs the redirect.
    fn fixture() -> (TempConfig, tempfile::TempDir, std::path::PathBuf) {
        let config = TempConfig::new();
        let (dir, db_path) = seeded_db();
        (config, dir, db_path)
    }

    // -- company name -------------------------------------------------------

    #[tokio::test]
    async fn company_name_round_trips_through_status() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let (status, body) = put_json(
            &app,
            "/api/settings/company-name",
            &token,
            &json!({ "name": "  Raygun LLC  " }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["companyName"], "Raygun LLC", "not trimmed");

        assert_eq!(status_json(&app, &token).await["companyName"], "Raygun LLC");
    }

    #[tokio::test]
    async fn an_empty_company_name_clears_it() {
        // Parity with the TUI settings screen, where blanking the field clears
        // the name rather than being refused.
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        put_json(
            &app,
            "/api/settings/company-name",
            &token,
            &json!({ "name": "Raygun LLC" }),
        )
        .await;
        let (status, body) = put_json(
            &app,
            "/api/settings/company-name",
            &token,
            &json!({ "name": "" }),
        )
        .await;

        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["companyName"], "");
        assert_eq!(status_json(&app, &token).await["companyName"], "");
    }

    // -- application settings ----------------------------------------------

    #[tokio::test]
    async fn app_settings_report_the_defaults_then_the_edit() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let before = ok_json(&app, "/api/settings/app", &token).await;
        assert_eq!(before["updateCheck"], true, "default");

        let (status, body) = put_json(
            &app,
            "/api/settings/app",
            &token,
            &json!({ "updateCheck": false }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["updateCheck"], false);

        let after = ok_json(&app, "/api/settings/app", &token).await;
        assert_eq!(after["updateCheck"], false, "did not survive a reread");
    }

    #[tokio::test]
    async fn app_settings_ignore_fields_the_web_may_not_write() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let (status, body) = put_json(
            &app,
            "/api/settings/app",
            &token,
            &json!({ "updateCheck": true, "userName": "someone else" }),
        )
        .await;

        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["userName"], "", "userName is not web-editable");
    }

    // -- data directory -----------------------------------------------------

    #[tokio::test]
    async fn switching_data_dir_rebinds_the_running_server() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        // A second database with an account the first one does not have: the
        // whole point is proving later reads land on the new file, not just
        // that settings.json was rewritten.
        let (_other_dir, other_db) = temp_db();
        let conn = crate::db::open_connection(&other_db, None).expect("open db");
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Second Books', 'checking')",
            [],
        )
        .expect("account");
        drop(conn);
        let other_root = other_db.parent().unwrap().to_path_buf();
        // The route resolves the path it is handed, so what comes back is the
        // resolved one — on macOS the tempdir's `/var` is a symlink into
        // `/private/var`, and the two spellings are not equal as strings.
        let resolved = std::fs::canonicalize(&other_root)
            .expect("canonicalize")
            .to_string_lossy()
            .to_string();

        let (status, body) = post_json(
            &app,
            "/api/settings/data-dir",
            &token,
            &json!({ "path": other_root.to_string_lossy() }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["dataDir"], resolved);

        let accounts = ok_json(&app, "/api/accounts", &token).await;
        let names: Vec<&str> = accounts
            .as_array()
            .expect("array")
            .iter()
            .map(|a| a["name"].as_str().expect("name"))
            .collect();
        assert_eq!(names, ["Second Books"], "still reading the old database");

        assert_eq!(
            crate::settings::load_settings().data_dir,
            resolved,
            "settings.json was not rewritten"
        );
    }

    #[tokio::test]
    async fn switching_to_an_encrypted_database_relocks_the_server() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let (_other_dir, other_db) = temp_db();
        encrypt(&other_db);
        let other_root = other_db.parent().unwrap().to_path_buf();

        let (status, body) = post_json(
            &app,
            "/api/settings/data-dir",
            &token,
            &json!({ "path": other_root.to_string_lossy() }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["encrypted"], true);
        assert_eq!(body["locked"], true, "inherited the old database's key");

        let (status, _) = get_json(&app, "/api/accounts", &token).await;
        assert_eq!(status, StatusCode::LOCKED);

        // ...and the target's own password opens it.
        let (status, _) = post_json(
            &app,
            "/api/unlock",
            &token,
            &json!({ "password": PASSWORD }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn switching_to_a_directory_with_no_database_changes_nothing() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        let empty = tempfile::tempdir().expect("tempdir");

        let (status, body) = post_json(
            &app,
            "/api/settings/data-dir",
            &token,
            &json!({ "path": empty.path().to_string_lossy() }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");

        // The original database is still the one being served.
        assert_eq!(
            status_json(&app, &token).await["dataDir"],
            db_path.parent().unwrap().to_string_lossy().to_string()
        );
    }

    #[tokio::test]
    async fn a_target_that_cannot_be_opened_leaves_the_switch_un_made() {
        // The failure has to happen after the exists() pre-check and inside the
        // part that used to commit first: a rebind that fails halfway would
        // strand the server on a database it cannot serve, and settings.json
        // would send it back there on the next start.
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        let before = crate::settings::load_settings().data_dir;

        let broken = tempfile::tempdir().expect("tempdir");
        // A directory where the database file should be: it exists, so the
        // pre-check passes, and every attempt to read it fails.
        std::fs::create_dir(broken.path().join("nigel.db")).expect("mkdir");

        let (status, body) = post_json(
            &app,
            "/api/settings/data-dir",
            &token,
            &json!({ "path": broken.path().to_string_lossy() }),
        )
        .await;
        assert!(!status.is_success(), "switch reported success: {body}");

        assert_eq!(
            status_json(&app, &token).await["dataDir"],
            db_path.parent().unwrap().to_string_lossy().to_string(),
            "the server was rebound by a switch that failed"
        );
        assert_eq!(
            crate::settings::load_settings().data_dir,
            before,
            "settings.json was rewritten by a switch that failed"
        );
        // ...and the original database is still readable.
        let (status, _) = get_json(&app, "/api/accounts", &token).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn an_empty_data_dir_path_is_refused() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/settings/data-dir",
            &token,
            &json!({ "path": "  " }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    // -- password -----------------------------------------------------------

    #[tokio::test]
    async fn setting_a_password_encrypts_and_stays_unlocked() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/settings/password/set",
            &token,
            &json!({ "newPassword": "hunter2" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["encrypted"], true);
        assert_eq!(body["locked"], false);

        let status_body = status_json(&app, &token).await;
        assert_eq!(status_body["encrypted"], true);
        assert_eq!(status_body["locked"], false);

        // The process kept the key, so reads keep working without an unlock.
        let (status, _) = get_json(&app, "/api/accounts", &token).await;
        assert_eq!(status, StatusCode::OK);

        assert!(crate::db::validate_password(&db_path, "hunter2").expect("validate"));
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn setting_a_password_on_an_encrypted_database_is_a_conflict() {
        let (_config, _dir, db_path) = fixture();
        encrypt(&db_path);
        let (app, token) = app_for(&db_path);
        crate::db::set_db_password(Some(PASSWORD.to_string()));

        let (status, body) = post_json(
            &app,
            "/api/settings/password/set",
            &token,
            &json!({ "newPassword": "hunter2" }),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "already_encrypted");
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn an_empty_new_password_is_refused() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/settings/password/set",
            &token,
            &json!({ "newPassword": "   " }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(!crate::db::is_encrypted(&db_path).expect("probe"));
    }

    /// A password SQLCipher accepts but `PRAGMA key = '…'` cannot tokenize
    /// encrypts the database and then locks its owner out of it for good.
    #[tokio::test]
    async fn a_new_password_holding_a_control_character_is_refused() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        for password in ["ab\u{0}cd", "line\nbreak", "tab\there"] {
            let (status, body) = post_json(
                &app,
                "/api/settings/password/set",
                &token,
                &json!({ "newPassword": password }),
            )
            .await;

            assert_eq!(status, StatusCode::BAD_REQUEST, "{password:?}: {body}");
            assert!(
                !crate::db::is_encrypted(&db_path).expect("probe"),
                "{password:?} encrypted the database"
            );
        }
    }

    #[tokio::test]
    async fn changing_the_password_swaps_which_one_opens_the_database() {
        let (_config, _dir, db_path) = fixture();
        encrypt(&db_path);
        let (app, token) = app_for(&db_path);
        crate::db::set_db_password(Some(PASSWORD.to_string()));

        let (status, body) = post_json(
            &app,
            "/api/settings/password/change",
            &token,
            &json!({ "currentPassword": PASSWORD, "newPassword": "a new one" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");

        assert!(crate::db::validate_password(&db_path, "a new one").expect("validate"));
        assert!(!crate::db::validate_password(&db_path, PASSWORD).expect("validate"));
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn a_wrong_current_password_is_refused_and_costs_an_attempt() {
        let (_config, _dir, db_path) = fixture();
        encrypt(&db_path);
        let (app, token) = app_for(&db_path);
        crate::db::set_db_password(Some(PASSWORD.to_string()));

        let (status, body) = post_json(
            &app,
            "/api/settings/password/change",
            &token,
            &json!({ "currentPassword": "not it", "newPassword": "a new one" }),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(body["error"]["code"], "invalid_password");
        assert_eq!(
            body["error"]["details"]["attemptsRemaining"], 2,
            "shares the unlock budget"
        );
        // The old password still opens the database: nothing was rekeyed.
        assert!(crate::db::validate_password(&db_path, PASSWORD).expect("validate"));
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn removing_the_password_decrypts_the_database() {
        let (_config, _dir, db_path) = fixture();
        encrypt(&db_path);
        let (app, token) = app_for(&db_path);
        crate::db::set_db_password(Some(PASSWORD.to_string()));

        let (status, body) = post_json(
            &app,
            "/api/settings/password/remove",
            &token,
            &json!({ "currentPassword": PASSWORD }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["encrypted"], false);

        assert!(!crate::db::is_encrypted(&db_path).expect("probe"));
        assert!(crate::db::get_db_password().is_none(), "key still held");

        let status_body = status_json(&app, &token).await;
        assert_eq!(status_body["encrypted"], false);
        assert_eq!(status_body["locked"], false);

        let (status, _) = get_json(&app, "/api/accounts", &token).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn changing_or_removing_on_a_plaintext_database_is_a_conflict() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        for (uri, body) in [
            (
                "/api/settings/password/change",
                json!({ "currentPassword": "x", "newPassword": "y" }),
            ),
            (
                "/api/settings/password/remove",
                json!({ "currentPassword": "x" }),
            ),
        ] {
            let (status, response) = post_json(&app, uri, &token, &body).await;
            assert_eq!(status, StatusCode::CONFLICT, "{uri}: {response}");
            assert_eq!(
                response["error"]["details"]["reason"], "not_encrypted",
                "for {uri}"
            );
        }
    }

    #[tokio::test]
    async fn a_malformed_password_body_is_refused_without_quoting_it() {
        // ApiJson's rejection echoes the deserializer's message, which quotes
        // the offending value. These routes carry passwords, so they decode by
        // hand — the same reasoning /api/unlock documents.
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        for uri in [
            "/api/settings/password/set",
            "/api/settings/password/change",
            "/api/settings/password/remove",
        ] {
            let (status, body) = post_json(&app, uri, &token, &json!({ "oops": "s3cret" })).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
            assert!(
                !body.to_string().contains("s3cret"),
                "{uri} echoed the body: {body}"
            );
        }
    }

    #[tokio::test]
    async fn a_password_never_appears_in_a_response() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let (_, body) = post_json(
            &app,
            "/api/settings/password/set",
            &token,
            &json!({ "newPassword": "swordfish" }),
        )
        .await;
        assert!(!body.to_string().contains("swordfish"), "leaked in {body}");

        crate::db::set_db_password(None);
    }
}
