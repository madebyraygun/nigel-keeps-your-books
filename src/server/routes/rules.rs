//! Categorization rules: `GET /api/rules` in match order, create, edit,
//! soft-delete, and `POST /api/rules/test` — the dry run that shows what a
//! pattern would catch before anything is saved.
//!
//! Rules address their category by **id** here. The CLI resolves a name because
//! that is what a person types; a form already knows the id.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::cli::rules::{self, NewRule, RuleRow, RuleTestResult, RuleUpdate};

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::{double_option, with_conn, Deleted};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rules", get(list).post(create))
        .route("/rules/test", post(test))
        .route("/rules/{id}", patch(update).delete(remove))
}

async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<RuleRow>>> {
    Ok(Json(with_conn(&state, rules::list_rules).await?))
}

fn default_match_type() -> String {
    "contains".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRuleRequest {
    pattern: String,
    category_id: i64,
    vendor: Option<String>,
    /// Defaults to `contains`, matching `nigel rules add`.
    #[serde(default = "default_match_type")]
    match_type: String,
    #[serde(default)]
    priority: i64,
}

async fn create(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewRuleRequest>,
) -> ApiResult<(StatusCode, Json<RuleRow>)> {
    let rule = with_conn(&state, move |conn| {
        let id = rules::add_rule(
            conn,
            NewRule {
                pattern: &new.pattern,
                category_id: new.category_id,
                vendor: new.vendor.as_deref(),
                match_type: &new.match_type,
                priority: new.priority,
            },
        )?;
        rules::get_rule(conn, id)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(rule)))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePatch {
    pattern: Option<String>,
    match_type: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    vendor: Option<Option<String>>,
    category_id: Option<i64>,
    priority: Option<i64>,
}

async fn update(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(patch): ApiJson<RulePatch>,
) -> ApiResult<Json<RuleRow>> {
    let rule = with_conn(&state, move |conn| {
        rules::update_rule(
            conn,
            id,
            &RuleUpdate {
                pattern: patch.pattern,
                match_type: patch.match_type,
                vendor: patch.vendor,
                category_id: patch.category_id,
                priority: patch.priority,
            },
        )?;
        rules::get_rule(conn, id)
    })
    .await?;
    Ok(Json(rule))
}

async fn remove(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Deleted>> {
    with_conn(&state, move |conn| rules::deactivate_rule(conn, id)).await?;
    Ok(Deleted::new(id))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRequest {
    pattern: String,
    #[serde(default = "default_match_type")]
    match_type: String,
}

/// A dry run: nothing is written, so this stays a POST only because the pattern
/// belongs in a body rather than a URL.
async fn test(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<TestRequest>,
) -> ApiResult<Json<RuleTestResult>> {
    if request.pattern.is_empty() {
        return Err(ApiError::bad_request("`pattern` is required."));
    }
    let result = with_conn(&state, move |conn| {
        rules::test_pattern(conn, &request.pattern, &request.match_type)
    })
    .await?;
    Ok(Json(result))
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

    #[tokio::test]
    async fn a_rule_can_be_created_edited_and_deactivated() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let software = category_id(&db_path, "Software & Subscriptions");
        let fees = category_id(&db_path, "Bank & Merchant Fees");

        let (status, created) = post_json(
            &app,
            "/api/rules",
            &token,
            &serde_json::json!({
                "pattern": "FIGMA",
                "categoryId": software,
                "vendor": "Figma",
                "priority": 5,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["id"].as_i64().expect("an id");
        assert_eq!(created["matchType"], "contains", "the default match type");
        assert_eq!(created["category"], "Software & Subscriptions");
        assert_eq!(created["priority"], 5);

        let (status, updated) = patch_json(
            &app,
            &format!("/api/rules/{id}"),
            &token,
            &serde_json::json!({ "priority": 20, "categoryId": fees, "vendor": null }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        assert_eq!(updated["priority"], 20);
        assert_eq!(updated["categoryId"], fees);
        assert_eq!(updated["vendor"], serde_json::Value::Null);
        assert_eq!(updated["pattern"], "FIGMA", "untouched fields survive");

        let (status, deleted) = delete_json(&app, &format!("/api/rules/{id}"), &token).await;
        assert_eq!(status, StatusCode::OK, "{deleted}");
        let listed = ok_json(&app, "/api/rules", &token).await;
        assert!(!listed.as_array().unwrap().iter().any(|r| r["id"] == id));

        // Soft delete: gone from the list, still on the books with its hits.
        let (status, again) = delete_json(&app, &format!("/api/rules/{id}"), &token).await;
        assert_eq!(status, StatusCode::CONFLICT, "{again}");
        assert_eq!(again["error"]["details"]["reason"], "already_inactive");

        let (status, patched) = patch_json(
            &app,
            &format!("/api/rules/{id}"),
            &token,
            &serde_json::json!({ "priority": 1 }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{patched}");
        assert_eq!(patched["error"]["details"]["reason"], "already_inactive");
    }

    #[tokio::test]
    async fn a_rule_pattern_must_be_one_the_categorizer_can_run() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let software = category_id(&db_path, "Software & Subscriptions");

        let cases: [(serde_json::Value, StatusCode); 4] = [
            (
                serde_json::json!({ "pattern": "X", "categoryId": software, "matchType": "fuzzy" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                serde_json::json!({ "pattern": "[unclosed", "categoryId": software, "matchType": "regex" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                serde_json::json!({ "pattern": "  ", "categoryId": software }),
                StatusCode::BAD_REQUEST,
            ),
            (
                serde_json::json!({ "pattern": "X", "categoryId": 999999 }),
                StatusCode::NOT_FOUND,
            ),
        ];

        for (body, expected) in cases {
            let (status, json) = post_json(&app, "/api/rules", &token, &body).await;
            assert_eq!(status, expected, "for {body}: {json}");
        }

        let (status, json) = patch_json(
            &app,
            "/api/rules/999999",
            &token,
            &serde_json::json!({ "priority": 1 }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
    }

    #[tokio::test]
    async fn testing_a_pattern_agrees_with_the_categorizer() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let (status, result) = post_json(
            &app,
            "/api/rules/test",
            &token,
            &serde_json::json!({ "pattern": "adobe", "matchType": "contains" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        let expected =
            serde_json::to_value(super::rules::test_pattern(&conn, "adobe", "contains").unwrap())
                .unwrap();
        assert_eq!(result, expected);

        // Case-insensitive, and the three seeded ADOBE rows collapse into one
        // description with a count.
        assert_eq!(result["total"], 3);
        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["description"], "ADOBE CREATIVE CLOUD");
        assert_eq!(matches[0]["count"], 3);

        // Nothing matched is a result, not an error.
        let (status, empty) = post_json(
            &app,
            "/api/rules/test",
            &token,
            &serde_json::json!({ "pattern": "NOTHING HERE" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{empty}");
        assert_eq!(empty["total"], 0);
        assert!(empty["matches"].as_array().unwrap().is_empty());

        // A regex that will not compile is the client's mistake.
        let (status, bad) = post_json(
            &app,
            "/api/rules/test",
            &token,
            &serde_json::json!({ "pattern": "[unclosed", "matchType": "regex" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(bad["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Invalid regex"));
    }
}
