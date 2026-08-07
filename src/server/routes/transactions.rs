//! Transaction edits: `PATCH /api/transactions/:id`, plus `POST /api/categorize`
//! for the bulk pass the rules engine makes over everything uncategorized.

use axum::routing::{patch, post};
use axum::{extract::State, Json, Router};
use serde::Deserialize;

use crate::categorizer::{self, CategorizeResult};
use crate::cli::categories::ensure_category_exists;
use crate::reports::{self, RegisterRow};
use crate::reviewer;

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::{double_option, with_conn};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/transactions/{id}", patch(update))
        .route("/categorize", post(categorize))
}

/// The three fields the register can edit. Every one is optional; `vendor: null`
/// clears the vendor, while `categoryId: null` is rejected rather than silently
/// ignored — there is no "uncategorize" edit, only review undo.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPatch {
    #[serde(default, deserialize_with = "double_option")]
    category_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "double_option")]
    vendor: Option<Option<String>>,
    flag: Option<bool>,
}

async fn update(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(patch): ApiJson<TransactionPatch>,
) -> ApiResult<Json<RegisterRow>> {
    let category_id = match patch.category_id {
        Some(None) => {
            return Err(ApiError::bad_request(
                "`categoryId` cannot be null; omit it to leave the category alone.",
            ))
        }
        Some(Some(id)) => Some(id),
        None => None,
    };

    if category_id.is_none() && patch.vendor.is_none() && patch.flag.is_none() {
        return Err(ApiError::bad_request(
            "Nothing to update — provide at least one of `categoryId`, `vendor`, or `flag`.",
        ));
    }

    let row = with_conn(&state, move |conn| {
        // One transaction so a patch that fails halfway leaves nothing behind.
        let tx = conn.unchecked_transaction()?;
        reports::get_register_row(&tx, id)?;

        if let Some(category_id) = category_id {
            ensure_category_exists(&tx, category_id)?;
            reviewer::update_transaction_category(&tx, id, category_id)?;
        }
        if let Some(ref vendor) = patch.vendor {
            reviewer::update_transaction_vendor(&tx, id, vendor.as_deref())?;
        }
        if let Some(flag) = patch.flag {
            reviewer::set_transaction_flag(&tx, id, flag)?;
        }

        let row = reports::get_register_row(&tx, id)?;
        tx.commit()?;
        Ok(row)
    })
    .await?;

    Ok(Json(row))
}

async fn categorize(State(state): State<AppState>) -> ApiResult<Json<CategorizeResult>> {
    Ok(Json(
        with_conn(&state, categorizer::categorize_transactions).await?,
    ))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;

    /// The seeded flagged transaction, which starts uncategorized.
    async fn flagged_id(app: &axum::Router, token: &str) -> i64 {
        let queue = ok_json(app, "/api/review/queue", token).await;
        queue[0]["id"].as_i64().expect("a flagged transaction")
    }

    fn category_id(db_path: &std::path::Path, name: &str) -> i64 {
        let conn = crate::db::open_connection(db_path, None).expect("open db");
        conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |row| {
            row.get(0)
        })
        .expect("category")
    }

    #[tokio::test]
    async fn a_patch_applies_every_field_and_answers_with_the_row() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = flagged_id(&app, &token).await;
        let software = category_id(&db_path, "Software & Subscriptions");

        let body = serde_json::json!({
            "categoryId": software,
            "vendor": "Adobe",
            "flag": false,
        });
        let (status, row) =
            patch_json(&app, &format!("/api/transactions/{id}"), &token, &body).await;
        assert_eq!(status, StatusCode::OK, "{row}");
        assert_eq!(row["categoryId"], software);
        assert_eq!(row["vendor"], "Adobe");
        assert_eq!(row["isFlagged"], false);
        assert_eq!(row["category"], "Software & Subscriptions");

        // And it stuck: the register agrees.
        let register = ok_json(&app, "/api/reports/register", &token).await;
        let stored = register["report"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .expect("row");
        assert_eq!(stored["vendor"], "Adobe");
    }

    #[tokio::test]
    async fn a_null_vendor_clears_it() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let register = ok_json(&app, "/api/reports/register", &token).await;
        let id = register["report"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["vendor"] == "Adobe")
            .expect("a row with a vendor")["id"]
            .as_i64()
            .unwrap();

        let (status, row) = patch_json(
            &app,
            &format!("/api/transactions/{id}"),
            &token,
            &serde_json::json!({ "vendor": null }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{row}");
        assert_eq!(row["vendor"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn setting_the_flag_is_idempotent() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = flagged_id(&app, &token).await;
        let uri = format!("/api/transactions/{id}");

        for expected in [true, true, false, false] {
            let (status, row) =
                patch_json(&app, &uri, &token, &serde_json::json!({ "flag": expected })).await;
            assert_eq!(status, StatusCode::OK, "{row}");
            assert_eq!(row["isFlagged"], expected, "flag should settle, not toggle");
        }
    }

    #[tokio::test]
    async fn a_patch_that_cannot_be_honoured_is_rejected() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = flagged_id(&app, &token).await;
        let uri = format!("/api/transactions/{id}");

        let cases = [
            (
                serde_json::json!({}),
                StatusCode::BAD_REQUEST,
                "bad_request",
            ),
            (
                serde_json::json!({ "categoryId": null }),
                StatusCode::BAD_REQUEST,
                "bad_request",
            ),
            (
                serde_json::json!({ "categoryId": 99_999 }),
                StatusCode::NOT_FOUND,
                "not_found",
            ),
        ];
        for (body, expected_status, code) in cases {
            let (status, json) = patch_json(&app, &uri, &token, &body).await;
            assert_eq!(status, expected_status, "for {body}: {json}");
            assert_eq!(json["error"]["code"], code, "for {body}");
        }

        let (status, json) = patch_json(
            &app,
            "/api/transactions/424242",
            &token,
            &serde_json::json!({ "flag": true }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");

        let (status, json) = patch_json(
            &app,
            "/api/transactions/not-a-number",
            &token,
            &serde_json::json!({ "flag": true }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        assert_eq!(json["error"]["code"], "bad_request");
    }

    #[tokio::test]
    async fn a_failed_patch_changes_nothing() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = flagged_id(&app, &token).await;

        // The category is bad, so the vendor and flag beside it must not land.
        let (status, _) = patch_json(
            &app,
            &format!("/api/transactions/{id}"),
            &token,
            &serde_json::json!({ "categoryId": 99_999, "vendor": "Ghost", "flag": false }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let queue = ok_json(&app, "/api/review/queue", &token).await;
        assert_eq!(queue[0]["id"], id, "still flagged");
        let row = ok_json(&app, &format!("/api/review/{id}"), &token).await;
        assert_eq!(row["vendor"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn categorize_applies_the_rules_and_reports_what_it_did() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // The seeded flagged row matches nothing; add a rule that catches it.
        let fees = category_id(&db_path, "Bank & Merchant Fees");
        let (status, rule) = post_json(
            &app,
            "/api/rules",
            &token,
            &serde_json::json!({ "pattern": "UNKNOWN VENDOR", "categoryId": fees }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{rule}");

        let (status, result) =
            post_json(&app, "/api/categorize", &token, &serde_json::json!({})).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["categorized"], 1);
        assert_eq!(result["stillFlagged"], 0);

        let queue = ok_json(&app, "/api/review/queue", &token).await;
        assert!(queue.as_array().unwrap().is_empty(), "queue drained");
    }
}
