//! Import history and saved CSV column mappings: `GET /api/imports`,
//! `GET /api/csv-profiles`, and undoing one import by id. Running an import is
//! task 31.7.

use axum::extract::State;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Serialize;

use crate::cli::undo::{self, ImportListItem};
use crate::error::NigelError;
use crate::importer::{self, CsvProfile};

use super::super::error::ApiResult;
use super::super::extract::ApiPath;
use super::super::state::AppState;
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/imports", get(list_imports))
        .route("/imports/{id}", delete(undo_import))
        .route("/csv-profiles", get(list_csv_profiles))
}

/// What undoing an import removed. The count is the number that matters — the
/// import record itself going away is implied.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoneImport {
    id: i64,
    deleted_transactions: usize,
}

/// Roll back one import: its transactions and its record, exactly what
/// `nigel undo` does to the most recent one.
async fn undo_import(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<UndoneImport>> {
    let deleted_transactions = with_conn(&state, move |conn| {
        // `delete_import` reports a missing import as zero rows deleted, which
        // over HTTP would read as a successful undo of nothing.
        if !undo::import_exists(conn, id)? {
            return Err(NigelError::NotFound(format!("No import with ID {id}")));
        }
        undo::delete_import(conn, id)
    })
    .await?;

    Ok(Json(UndoneImport {
        id,
        deleted_transactions,
    }))
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
    use axum::http::StatusCode;

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
    async fn undoing_an_import_removes_its_transactions() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let imports = ok_json(&app, "/api/imports", &token).await;
        let loaded = imports
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["filename"] == "jan-2025.csv")
            .expect("the seeded import");
        let id = loaded["id"].as_i64().unwrap();
        assert_eq!(loaded["transactionCount"], 8);

        let (status, body) = delete_json(&app, &format!("/api/imports/{id}"), &token).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["deletedTransactions"], 8);
        assert_eq!(body["id"], id);

        let register = ok_json(&app, "/api/reports/register", &token).await;
        assert!(register["report"]["rows"].as_array().unwrap().is_empty());
        let remaining = ok_json(&app, "/api/imports", &token).await;
        assert_eq!(remaining.as_array().unwrap().len(), 1);

        // Undoing it twice is a 404, not a cheerful zero.
        let (status, body) = delete_json(&app, &format!("/api/imports/{id}"), &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["code"], "not_found");
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
