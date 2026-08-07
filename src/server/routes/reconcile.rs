//! Monthly reconciliation: `POST /api/reconcile` and the history behind it.
//!
//! Reconciling is a POST because it records the attempt — including the ones
//! that did not balance. That record is the point: it is how you know which
//! months have been checked.

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::reconciler::{self, ReconcileResult, ReconciliationRecord};

use super::super::error::ApiResult;
use super::super::extract::ApiJson;
use super::super::state::AppState;
use super::reports::parse_month;
use super::{ensure_account_exists, with_conn};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/reconcile", post(run))
        .route("/reconciliations", get(history))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileRequest {
    account: String,
    month: String,
    statement_balance: f64,
}

async fn run(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<ReconcileRequest>,
) -> ApiResult<Json<ReconcileResult>> {
    // `reconcile` builds date bounds by string concatenation, so a malformed
    // month would come back as "no transactions" rather than as the typo it is.
    parse_month(&request.month)?;

    let result = with_conn(&state, move |conn| {
        reconciler::reconcile(
            conn,
            &request.account,
            &request.month,
            request.statement_balance,
        )
    })
    .await?;
    Ok(Json(result))
}

#[derive(Debug, Default, Deserialize)]
struct HistoryQuery {
    account: Option<String>,
}

async fn history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> ApiResult<Json<Vec<ReconciliationRecord>>> {
    let records = with_conn(&state, move |conn| {
        if let Some(account) = query.account.as_deref() {
            // An unknown account is a wrong question, not an empty answer.
            ensure_account_exists(conn, account)?;
        }
        reconciler::list_reconciliations(conn, query.account.as_deref())
    })
    .await?;
    Ok(Json(records))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn reconciling_records_the_attempt_and_shows_it_in_the_history() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // February on the checking account: 5000 - 59.99 - 12.00 through the
        // end of the month.
        let (status, result) = post_json(
            &app,
            "/api/reconcile",
            &token,
            &serde_json::json!({
                "account": "BofA Checking",
                "month": "2025-02",
                "statementBalance": 4928.01,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["isReconciled"], true);
        assert_eq!(result["discrepancy"], 0.0);
        assert_eq!(result["statementBalance"], 4928.01);

        let (status, off) = post_json(
            &app,
            "/api/reconcile",
            &token,
            &serde_json::json!({
                "account": "BofA Checking",
                "month": "2025-03",
                "statementBalance": 1.00,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{off}");
        assert_eq!(off["isReconciled"], false);
        assert!(off["discrepancy"].as_f64().unwrap() > 0.0);

        let history = ok_json(&app, "/api/reconciliations", &token).await;
        let rows = history.as_array().expect("a bare array");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["month"], "2025-03", "newest month first");
        assert_eq!(rows[0]["accountName"], "BofA Checking");
        assert_eq!(rows[0]["isReconciled"], false);
        assert!(rows[0].get("calculatedBalance").is_some());

        let filtered = ok_json(
            &app,
            "/api/reconciliations?account=BofA%20Credit%20Card",
            &token,
        )
        .await;
        assert!(filtered.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reconcile_reports_the_ways_it_can_fail() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // A month with nothing in it is a conflict, not a 404: the account is
        // real and the request was well formed.
        let (status, empty) = post_json(
            &app,
            "/api/reconcile",
            &token,
            &serde_json::json!({
                "account": "BofA Checking",
                "month": "2025-07",
                "statementBalance": 0.0,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{empty}");
        assert_eq!(empty["error"]["details"]["reason"], "no_transactions");
        assert_eq!(empty["error"]["details"]["month"], "2025-07");

        let (status, unknown) = post_json(
            &app,
            "/api/reconcile",
            &token,
            &serde_json::json!({
                "account": "Nope Bank",
                "month": "2025-02",
                "statementBalance": 0.0,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{unknown}");

        let (status, bad_month) = post_json(
            &app,
            "/api/reconcile",
            &token,
            &serde_json::json!({
                "account": "BofA Checking",
                "month": "2025-13",
                "statementBalance": 0.0,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad_month}");

        let (status, missing) = post_json(
            &app,
            "/api/reconcile",
            &token,
            &serde_json::json!({ "account": "BofA Checking" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{missing}");
        assert_eq!(missing["error"]["code"], "bad_request");

        let (status, unknown_history) =
            get_json(&app, "/api/reconciliations?account=Nope%20Bank", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{unknown_history}");
    }
}
