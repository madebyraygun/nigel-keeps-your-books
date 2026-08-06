//! `GET /api/rules` — active categorization rules in match order. Writes land
//! here with task 31.6.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::cli::rules::{self, RuleRow};

use super::super::error::ApiResult;
use super::super::state::AppState;
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new().route("/rules", get(list))
}

async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<RuleRow>>> {
    Ok(Json(with_conn(&state, rules::list_rules).await?))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;

    #[tokio::test]
    async fn rules_list_is_in_match_order_and_keeps_a_null_vendor_null() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/rules", &token).await;
        let expected = serde_json::to_value(super::rules::list_rules(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 2);
        // Priority 10 before priority 0 — the order the categorizer applies.
        assert_eq!(rows[0]["pattern"], "ADOBE");
        assert_eq!(rows[0]["vendor"], "Adobe");
        assert_eq!(rows[0]["hitCount"], 3);
        assert_eq!(rows[1]["pattern"], "SERVICE FEE");
        assert_eq!(rows[1]["vendor"], serde_json::Value::Null);
        for key in ["matchType", "categoryId"] {
            assert!(rows[0].get(key).is_some(), "missing {key} in {rows:?}");
        }
    }
}
