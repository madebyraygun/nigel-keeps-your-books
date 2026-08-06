//! Import history and saved CSV column mappings: `GET /api/imports` and
//! `GET /api/csv-profiles`. Running an import is task 31.7; both endpoints are
//! read-only for now.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::cli::undo::{self, ImportListItem};
use crate::importer::{self, CsvProfile};

use super::super::error::ApiResult;
use super::super::state::AppState;
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/imports", get(list_imports))
        .route("/csv-profiles", get(list_csv_profiles))
}

async fn list_imports(State(state): State<AppState>) -> ApiResult<Json<Vec<ImportListItem>>> {
    Ok(Json(with_conn(&state, undo::list_imports).await?))
}

async fn list_csv_profiles(State(state): State<AppState>) -> ApiResult<Json<Vec<CsvProfile>>> {
    Ok(Json(with_conn(&state, importer::list_csv_profiles).await?))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;

    #[tokio::test]
    async fn import_history_is_newest_first_with_counts() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/imports", &token).await;
        let expected = serde_json::to_value(super::undo::list_imports(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["filename"], "empty.csv");
        assert_eq!(rows[0]["transactionCount"], 0);
        assert_eq!(rows[1]["filename"], "jan-2025.csv");
        assert_eq!(rows[1]["transactionCount"], 8);
        assert_eq!(rows[1]["accountName"], "BofA Checking");
    }

    #[tokio::test]
    async fn csv_profiles_carry_their_column_mapping() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/csv-profiles", &token).await;
        let expected =
            serde_json::to_value(super::importer::list_csv_profiles(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "chase");
        assert_eq!(rows[0]["config"]["amountCol"], 3);
        assert_eq!(rows[0]["config"]["dateFormat"], "%m/%d/%Y");
    }
}
