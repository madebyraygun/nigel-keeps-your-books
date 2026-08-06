//! The review queue: what still needs a category, and applying or taking back
//! a decision.
//!
//! `apply` and `undo` are a matched pair. Undo is what makes the SPA's back
//! button honest — it re-flags the transaction and deletes the rule the apply
//! created, so stepping backwards leaves no trace of the decision.

use axum::routing::{get, post};
use axum::{extract::State, Json, Router};
use serde::{Deserialize, Serialize};

use crate::cli::categories;
use crate::reports::{self, RegisterRow};
use crate::reviewer::{self, FlaggedTxn};

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/review/queue", get(queue))
        .route("/review/{id}", get(one))
        .route("/review/{id}/apply", post(apply))
        .route("/review/{id}/undo", post(undo))
}

async fn queue(State(state): State<AppState>) -> ApiResult<Json<Vec<FlaggedTxn>>> {
    Ok(Json(
        with_conn(&state, reviewer::get_flagged_transactions).await?,
    ))
}

/// Re-review by id, the web equivalent of `nigel review --id`. Answers with the
/// full register row rather than the queue's summary: by the time a client asks
/// for one transaction it wants the category and vendor too.
async fn one(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<RegisterRow>> {
    Ok(Json(
        with_conn(&state, move |conn| reports::get_register_row(conn, id)).await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRequest {
    category_id: i64,
    vendor: Option<String>,
    #[serde(default)]
    create_rule: bool,
    rule_pattern: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResponse {
    transaction_id: i64,
    /// The rule this decision created, if it created one. Hand it back to
    /// `undo` to take both apart together.
    rule_id: Option<i64>,
}

async fn apply(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(request): ApiJson<ApplyRequest>,
) -> ApiResult<Json<ApplyResponse>> {
    // `apply_review` quietly creates no rule when the pattern is missing, which
    // over HTTP would look like a rule that vanished.
    if request.create_rule
        && request
            .rule_pattern
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err(ApiError::bad_request(
            "`rulePattern` is required when `createRule` is true.",
        ));
    }

    let rule_id = with_conn(&state, move |conn| {
        reports::get_register_row(conn, id)?;
        categories::ensure_category_exists(conn, request.category_id)?;
        reviewer::apply_review(
            conn,
            id,
            request.category_id,
            request.vendor.as_deref(),
            request.create_rule,
            request.rule_pattern.as_deref(),
        )
    })
    .await?;

    Ok(Json(ApplyResponse {
        transaction_id: id,
        rule_id,
    }))
}

/// The body is required even though every field is optional: `{}` is a
/// perfectly good "just put the transaction back".
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoRequest {
    /// The rule the matching apply created. Omit it and only the transaction is
    /// restored.
    rule_id: Option<i64>,
}

async fn undo(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(request): ApiJson<UndoRequest>,
) -> ApiResult<Json<RegisterRow>> {
    let row = with_conn(&state, move |conn| {
        reports::get_register_row(conn, id)?;
        reviewer::undo_review(conn, id, request.rule_id)?;
        reports::get_register_row(conn, id)
    })
    .await?;

    Ok(Json(row))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;

    fn category_id(db_path: &std::path::Path, name: &str) -> i64 {
        let conn = crate::db::open_connection(db_path, None).expect("open db");
        conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |row| {
            row.get(0)
        })
        .expect("category")
    }

    #[tokio::test]
    async fn the_queue_holds_what_still_needs_a_decision() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/review/queue", &token).await;
        let expected =
            serde_json::to_value(crate::reviewer::get_flagged_transactions(&conn).unwrap())
                .unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["description"], "UNKNOWN VENDOR 8812");
        assert_eq!(rows[0]["accountName"], "BofA Credit Card");
    }

    #[tokio::test]
    async fn one_transaction_comes_back_as_a_register_row() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = ok_json(&app, "/api/review/queue", &token).await[0]["id"]
            .as_i64()
            .unwrap();

        let row = ok_json(&app, &format!("/api/review/{id}"), &token).await;
        assert_eq!(row["id"], id);
        assert_eq!(row["isFlagged"], true);
        assert_eq!(row["categoryId"], serde_json::Value::Null);

        let (status, body) = get_json(&app, "/api/review/999999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    }

    #[tokio::test]
    async fn apply_then_undo_leaves_no_trace() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = ok_json(&app, "/api/review/queue", &token).await[0]["id"]
            .as_i64()
            .unwrap();
        let fees = category_id(&db_path, "Bank & Merchant Fees");

        let (status, applied) = post_json(
            &app,
            &format!("/api/review/{id}/apply"),
            &token,
            &serde_json::json!({
                "categoryId": fees,
                "vendor": "Mystery Co",
                "createRule": true,
                "rulePattern": "UNKNOWN VENDOR",
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{applied}");
        let rule_id = applied["ruleId"].as_i64().expect("a rule was created");
        assert_eq!(applied["transactionId"], id);

        // Categorized, unflagged, and the rule is live.
        let row = ok_json(&app, &format!("/api/review/{id}"), &token).await;
        assert_eq!(row["categoryId"], fees);
        assert_eq!(row["vendor"], "Mystery Co");
        assert_eq!(row["isFlagged"], false);
        let rules = ok_json(&app, "/api/rules", &token).await;
        assert!(rules.as_array().unwrap().iter().any(|r| r["id"] == rule_id));

        let (status, restored) = post_json(
            &app,
            &format!("/api/review/{id}/undo"),
            &token,
            &serde_json::json!({ "ruleId": rule_id }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{restored}");
        assert_eq!(restored["isFlagged"], true);
        assert_eq!(restored["categoryId"], serde_json::Value::Null);
        assert_eq!(restored["vendor"], serde_json::Value::Null);

        // The rule is gone outright, not deactivated: undo_review deletes it.
        let rules = ok_json(&app, "/api/rules", &token).await;
        assert!(!rules.as_array().unwrap().iter().any(|r| r["id"] == rule_id));
        let conn = crate::db::open_connection(&db_path, None).expect("open db");
        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM rules WHERE id = ?1",
                [rule_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn apply_rejects_what_it_cannot_carry_out() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = ok_json(&app, "/api/review/queue", &token).await[0]["id"]
            .as_i64()
            .unwrap();
        let fees = category_id(&db_path, "Bank & Merchant Fees");

        let cases = [
            (
                format!("/api/review/{id}/apply"),
                serde_json::json!({ "categoryId": fees, "createRule": true }),
                StatusCode::BAD_REQUEST,
            ),
            (
                format!("/api/review/{id}/apply"),
                serde_json::json!({ "categoryId": 99_999 }),
                StatusCode::NOT_FOUND,
            ),
            (
                format!("/api/review/{id}/apply"),
                serde_json::json!({ "vendor": "No category at all" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                "/api/review/999999/apply".to_string(),
                serde_json::json!({ "categoryId": fees }),
                StatusCode::NOT_FOUND,
            ),
            (
                "/api/review/999999/undo".to_string(),
                serde_json::json!({}),
                StatusCode::NOT_FOUND,
            ),
        ];

        for (uri, body, expected) in cases {
            let (status, json) = post_json(&app, &uri, &token, &body).await;
            assert_eq!(status, expected, "POST {uri} with {body} gave {json}");
            assert!(json["error"]["code"].is_string(), "envelope for {uri}");
        }
    }

    #[tokio::test]
    async fn undo_without_a_rule_id_still_restores_the_transaction() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = ok_json(&app, "/api/review/queue", &token).await[0]["id"]
            .as_i64()
            .unwrap();
        let fees = category_id(&db_path, "Bank & Merchant Fees");

        let (status, applied) = post_json(
            &app,
            &format!("/api/review/{id}/apply"),
            &token,
            &serde_json::json!({ "categoryId": fees }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{applied}");
        assert_eq!(applied["ruleId"], serde_json::Value::Null);

        let (status, restored) = post_json(
            &app,
            &format!("/api/review/{id}/undo"),
            &token,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{restored}");
        assert_eq!(restored["isFlagged"], true);
    }
}
