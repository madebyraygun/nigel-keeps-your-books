//! API route modules — one file per domain as endpoints land.
//!
//! Routes fall into two groups. `/ping`, `/status`, and `/unlock` answer while
//! the database is still encrypted and locked; everything else needs the key.
//! The locked guard is layered over the whole `/api` router and exempts that
//! short list by path, so a new endpoint is guarded by default — forgetting to
//! mount it in the right place cannot expose a locked database.

pub mod status;

use axum::http::Uri;
use axum::routing::get;
use axum::{middleware, Json, Router};
use serde::Serialize;

use super::error::ApiError;
use super::state::AppState;

pub fn api_router(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/ping", get(ping))
        .merge(status::routes())
        .merge(data_router())
        .fallback(api_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            status::locked_guard,
        ))
}

/// Every route that touches the database — one merge per domain as they land.
fn data_router() -> Router<AppState> {
    let router = Router::new();

    // The guard has no real route to protect yet, so the router tests need one
    // to prove that `api_router` — the assembly later endpoints are mounted
    // into — actually applies it.
    #[cfg(test)]
    let router = router.route(
        "/_guarded_probe",
        get(|| async { Json(GuardProbe { ok: true }) }),
    );

    router
}

#[cfg(test)]
#[derive(Serialize)]
struct GuardProbe {
    ok: bool,
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
