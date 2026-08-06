//! `GET /api/accounts`. Writes land here with task 31.6.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::cli::accounts;
use crate::models::Account;

use super::super::error::ApiResult;
use super::super::state::AppState;
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new().route("/accounts", get(list))
}

async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<Account>>> {
    Ok(Json(with_conn(&state, accounts::list_accounts).await?))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;

    #[tokio::test]
    async fn accounts_list_matches_the_data_layer() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/accounts", &token).await;
        let expected =
            serde_json::to_value(super::accounts::list_accounts(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["name"], "BofA Checking");
        for key in ["accountType", "lastFour"] {
            assert!(rows[0].get(key).is_some(), "missing {key} in {rows:?}");
        }
    }
}
