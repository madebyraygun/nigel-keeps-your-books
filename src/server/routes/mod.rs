//! API route modules — one file per domain as endpoints land.
//!
//! Routes fall into two groups. `/ping`, `/status`, and `/unlock` answer while
//! the database is still encrypted and locked; everything else needs the key.
//! The locked guard is layered over the whole `/api` router and exempts that
//! short list by path, so a new endpoint is guarded by default — forgetting to
//! mount it in the right place cannot expose a locked database.

pub mod accounts;
pub mod categories;
pub mod clients;
pub mod exports;
pub mod imports;
pub mod reconcile;
pub mod reports;
pub mod review;
pub mod rules;
pub mod settings;
pub mod status;
pub mod transactions;

use axum::http::Uri;
use axum::routing::get;
use axum::{middleware, Json, Router};
use rusqlite::Connection;
use serde::{Deserialize, Deserializer, Serialize};

use super::error::{ApiError, ApiResult};
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
    let router = Router::new()
        .merge(reports::routes())
        .merge(exports::routes())
        .merge(accounts::routes())
        .merge(categories::routes())
        .merge(rules::routes())
        .merge(imports::routes())
        .merge(transactions::routes())
        .merge(review::routes())
        .merge(reconcile::routes())
        .merge(settings::routes())
        .merge(clients::routes());

    // A route that exists only to prove `api_router` — the assembly every
    // endpoint is mounted into — actually applies the guard, without pinning
    // that proof to whichever real endpoint happens to exist.
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

/// Run a database read on the blocking pool. rusqlite is synchronous and a
/// `Connection` is not `Send`-shared, so each request opens its own — WAL and
/// `busy_timeout` handle the concurrency, and there is no pool to size.
///
/// `db::get_connection` reads the process-global password itself, so an
/// unlocked database needs no plumbing through here.
///
/// The `db_gate` read guard is held for the life of the connection. Encrypting,
/// decrypting and switching data directories take the write side, so none of
/// them can rewrite the file underneath a live connection. **A handler that
/// opens a connection without coming through here must take
/// `state.db_gate.read().await` itself, and must read `state.db_path()` after
/// it rather than before** — `routes::imports`, `routes::status` and
/// `routes::settings` all do.
pub(super) async fn with_conn<T, F>(state: &AppState, work: F) -> ApiResult<T>
where
    F: FnOnce(&Connection) -> crate::error::Result<T> + Send + 'static,
    T: Send + 'static,
{
    with_conn_api(state, move |conn| work(conn).map_err(ApiError::from)).await
}

/// [`with_conn`] for work that already speaks in API errors — an export asked
/// for a format this build cannot render owes the caller a `501`, which is a
/// distinction `NigelError` has no way to carry.
pub(super) async fn with_conn_api<T, F>(state: &AppState, work: F) -> ApiResult<T>
where
    F: FnOnce(&Connection) -> ApiResult<T> + Send + 'static,
    T: Send + 'static,
{
    // The guard comes first: the path is read under it, because a data-directory
    // switch holds the write side and a path captured before the wait belongs to
    // the database this request is no longer serving.
    let _gate = state.db_gate.read().await;
    let db_path = state.db_path();
    tokio::task::spawn_blocking(move || -> ApiResult<T> {
        let conn = crate::db::get_connection(&db_path)?;
        work(&conn)
    })
    .await
    .map_err(ApiError::internal)?
}

/// Filtering by an account name that does not exist is a wrong question, not an
/// empty answer: `get_register` and `list_reconciliations` would both report a
/// typo as "nothing here".
pub(super) fn ensure_account_exists(conn: &Connection, name: &str) -> crate::error::Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
        [name],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(crate::error::NigelError::UnknownAccount(name.to_string()))
    }
}

/// Re-answer a data-layer `NotFound` with the name of the thing this route was
/// looking up.
///
/// The data layer says only that something was not found, which is right for a
/// terminal and not enough for a handler that resolves both an invoice and its
/// client: a client branching on the status alone has to guess which one is
/// missing. Everything else passes through untouched.
pub(super) fn not_found_because(err: crate::error::NigelError, reason: &str) -> ApiError {
    match err {
        crate::error::NigelError::NotFound(message) => {
            ApiError::not_found_because(message, reason)
        }
        other => ApiError::from(other),
    }
}

/// What a delete answers with. A body rather than a bare `204` so a client can
/// decode every response the same way.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Deleted {
    id: i64,
    deleted: bool,
}

impl Deleted {
    pub(crate) fn new(id: i64) -> Json<Self> {
        Json(Self { id, deleted: true })
    }
}

/// Tell "field absent" from "field explicitly null" in a PATCH body.
///
/// A plain `Option<T>` collapses the two, which is exactly the distinction a
/// partial update needs: absent means leave it alone, `null` means clear it.
/// Used with `#[serde(default, deserialize_with = "double_option")]`.
pub(crate) fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
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
