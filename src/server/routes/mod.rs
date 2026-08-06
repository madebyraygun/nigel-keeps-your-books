//! API route modules — one file per domain as endpoints land (31.4 onward).
//!
//! Today this is just the placeholder `GET /api/ping` plus the JSON 404 that
//! keeps unknown `/api` paths from falling through to the SPA shell.

use axum::http::Uri;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use super::error::ApiError;
use super::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/ping", get(ping))
        .fallback(api_not_found)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PingResponse {
    ok: bool,
    version: &'static str,
}

async fn ping() -> Json<PingResponse> {
    Json(PingResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn api_not_found(uri: Uri) -> ApiError {
    // Nesting strips the mount point, so put it back for a message that matches
    // what the caller actually requested.
    ApiError::not_found(format!("No API endpoint at /api{}", uri.path()))
}
