//! `GET /api/categories` — the active chart of accounts. Writes land here with
//! task 31.6.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::cli::categories::{self, CategoryRow};

use super::super::error::ApiResult;
use super::super::state::AppState;
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new().route("/categories", get(list))
}

async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<CategoryRow>>> {
    Ok(Json(with_conn(&state, categories::list_categories).await?))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;

    #[tokio::test]
    async fn categories_list_matches_the_data_layer_and_hides_inactive_rows() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/categories", &token).await;
        let expected =
            serde_json::to_value(super::categories::list_categories(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert!(!rows.is_empty());
        for key in ["categoryType", "taxLine", "formLine"] {
            assert!(rows[0].get(key).is_some(), "missing {key} in {rows:?}");
        }

        conn.execute(
            "UPDATE categories SET is_active = 0 WHERE name = 'Client Services'",
            [],
        )
        .unwrap();
        let after = ok_json(&app, "/api/categories", &token).await;
        assert!(
            !after
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["name"] == "Client Services"),
            "a soft-deleted category should not be listed: {after}"
        );
    }
}
