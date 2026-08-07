//! Server state and the encrypted-database gate: `GET /api/status`,
//! `POST /api/unlock`, and the middleware that keeps every data route shut
//! until the key arrives.
//!
//! This is the web equivalent of the splash screen's password prompt. Unlock is
//! process-wide because the password lives in the process-global
//! `db::set_db_password` mutex, so a server run serves exactly one database —
//! the same assumption every CLI subcommand makes.

use std::path::Path;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::db;

use super::super::error::{ApiError, ApiResult};
use super::super::secret::Secret;
use super::super::state::AppState;

/// Routes that work while the database is still locked.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/status", get(status))
        .route("/unlock", post(unlock))
}

/// The only `/api` paths that answer while the database is still locked. Paths
/// are as seen inside the `/api` nest, which strips the mount point.
const UNGATED_PATHS: [&str; 3] = ["/ping", "/status", "/unlock"];

/// Refuse anything that needs the database key until the database is unlocked.
///
/// Layered over the whole `/api` router rather than a hand-picked subtree: a
/// route added anywhere is guarded unless it is named above, so the failure
/// mode of forgetting is a locked-out endpoint, not a leaked database.
pub async fn locked_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if UNGATED_PATHS.contains(&req.uri().path()) {
        return next.run(req).await;
    }

    match state.is_locked() {
        Ok(true) => ApiError::locked().into_response(),
        Ok(false) => next.run(req).await,
        Err(err) => err.into_response(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusResponse {
    initialized: bool,
    encrypted: bool,
    locked: bool,
    company_name: Option<String>,
    version: &'static str,
    data_dir: String,
    pdf_export: bool,
    update_available: Option<String>,
}

/// Everything `GET /api/status` reports, computed fresh.
///
/// Shared with the data-directory switch, which answers with the status of the
/// database it just moved to rather than with a second shape that says the same
/// thing.
pub(crate) async fn current_status(state: &AppState) -> ApiResult<StatusResponse> {
    // Everything below describes one database, so the guard is taken before the
    // path is read: a data-directory switch landing partway through would
    // otherwise report one database's name under another's path.
    let _gate = state.db_gate.read().await;
    let db_path = state.db_path();
    let initialized = db_path.exists();
    let encrypted = db::is_encrypted(&db_path)?;
    let locked = encrypted && db::get_db_password().is_none();

    // Reading the company name means reading the database, which needs the key.
    let company_name = if initialized && !locked {
        let path = db_path.clone();
        tokio::task::spawn_blocking(move || -> ApiResult<Option<String>> {
            let conn = db::get_connection(&path)?;
            Ok(db::get_metadata(&conn, "company_name"))
        })
        .await
        .map_err(ApiError::internal)??
    } else {
        None
    };

    Ok(StatusResponse {
        initialized,
        encrypted,
        locked,
        company_name,
        version: env!("CARGO_PKG_VERSION"),
        // Named from the database the server actually opened rather than from
        // settings.json, which another process is free to repoint mid-run.
        data_dir: db_path
            .parent()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default(),
        // The export links are plain anchors, so the SPA has to know before it
        // clicks: a build without the feature would otherwise save the `501`
        // envelope to a file called something.pdf.
        pdf_export: state.features.pdf,
        // Filled in by the background check `serve` starts, so this is `None`
        // until GitHub answers — and stays `None` when the user has opted out
        // or the 24-hour cooldown has not elapsed.
        update_available: state.update_available(),
    })
}

async fn status(State(state): State<AppState>) -> ApiResult<Json<StatusResponse>> {
    Ok(Json(current_status(&state).await?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnlockRequest {
    password: Secret,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnlockResponse {
    locked: bool,
}

async fn unlock(
    State(state): State<AppState>,
    body: Result<Json<UnlockRequest>, JsonRejection>,
) -> ApiResult<Json<UnlockResponse>> {
    // The rejection's own text is discarded: serde's type-mismatch messages
    // quote the offending value, which here would be the password.
    let Json(request) = body.map_err(|_| {
        ApiError::bad_request("Expected a JSON body of the form {\"password\": \"...\"}.")
    })?;

    if !db::is_encrypted(&state.db_path())? {
        return Err(ApiError::bad_request(
            "This database is not encrypted — no password is needed.",
        ));
    }

    // Already unlocked: answer the question that was asked ("is it locked?")
    // rather than re-validating. This endpoint is not a password checker.
    if db::get_db_password().is_some() {
        return Ok(Json(UnlockResponse { locked: false }));
    }

    // try_unlock opens a connection and migrates: the file must not be swapped
    // out from under it, and the path is read under the guard so the key is
    // never applied to a database the switch has already moved on from.
    let unlocked = {
        let _gate = state.db_gate.read().await;
        let db_path = state.db_path();
        tokio::task::spawn_blocking(move || try_unlock(&db_path, request.password.expose()))
            .await
            .map_err(ApiError::internal)??
    };

    if unlocked {
        state.unlock.reset();
        return Ok(Json(UnlockResponse { locked: false }));
    }

    let (attempts_remaining, delay) = state.unlock.record_failure();
    // Slow repeated guessing down in the server, not the client. `tokio::time`
    // yields the worker; a thread sleep would stall the whole runtime.
    tokio::time::sleep(delay).await;
    Err(ApiError::invalid_password(
        attempts_remaining,
        delay.as_millis(),
    ))
}

/// Validate the password and, on success, adopt it for this process and run the
/// migrations that `nigel serve` had to defer while the database was locked.
///
/// Returns `Ok(false)` for a wrong password; every other failure is an error.
fn try_unlock(db_path: &Path, password: &str) -> ApiResult<bool> {
    if !db::validate_password(db_path, password).map_err(|_| open_failed())? {
        return Ok(false);
    }

    db::set_db_password(Some(password.to_string()));
    let conn = match db::get_connection(db_path) {
        Ok(conn) => conn,
        Err(_) => {
            db::set_db_password(None);
            return Err(open_failed());
        }
    };
    if let Err(err) = db::init_db(&conn) {
        // A half-unlocked process — key accepted, schema not migrated — would
        // let every later request run against a database this build cannot
        // read correctly.
        db::set_db_password(None);
        return Err(ApiError::internal(err));
    }

    Ok(true)
}

/// The error text from opening the database is deliberately dropped: the key is
/// applied with `PRAGMA key = '<password>'`, and rusqlite renders that pragma
/// as literal SQL which `Error::SqlInputError` prints back in its message.
fn open_failed() -> ApiError {
    ApiError::internal("Couldn't open the database to check the password.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_request_debug_redacts_the_password() {
        let request: UnlockRequest =
            serde_json::from_str(r#"{"password": "hunter2"}"#).expect("json");
        let rendered = format!("{request:?}");
        assert!(!rendered.contains("hunter2"), "leaked in {rendered}");
        assert!(rendered.contains("<redacted>"), "got {rendered}");
    }
}
