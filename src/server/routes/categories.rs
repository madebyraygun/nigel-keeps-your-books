//! The chart of accounts: `GET /api/categories` plus create, edit, and
//! soft-delete.
//!
//! Editing is a genuine partial update — the client sends the fields it is
//! changing and the current row supplies the rest, so two screens editing
//! different fields cannot blank each other's work. `taxLine` and `formLine`
//! accept `null` to clear them.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Deserialize;

use crate::cli::categories::{self, CategoryRow};

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::{double_option, with_conn, Deleted};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/categories", get(list).post(create))
        .route("/categories/{id}", patch(update).delete(remove))
}

async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<CategoryRow>>> {
    Ok(Json(with_conn(&state, categories::list_categories).await?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCategory {
    name: String,
    category_type: String,
    tax_line: Option<String>,
    form_line: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewCategory>,
) -> ApiResult<(StatusCode, Json<CategoryRow>)> {
    let category = with_conn(&state, move |conn| {
        let id = categories::add_category(
            conn,
            &new.name,
            &new.category_type,
            new.tax_line.as_deref(),
            new.form_line.as_deref(),
        )?;
        categories::get_category(conn, id)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(category)))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPatch {
    name: Option<String>,
    category_type: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    tax_line: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    form_line: Option<Option<String>>,
}

impl CategoryPatch {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.category_type.is_none()
            && self.tax_line.is_none()
            && self.form_line.is_none()
    }
}

async fn update(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(patch): ApiJson<CategoryPatch>,
) -> ApiResult<Json<CategoryRow>> {
    if patch.is_empty() {
        return Err(ApiError::bad_request(
            "Nothing to update — provide at least one of `name`, `categoryType`, `taxLine`, or `formLine`.",
        ));
    }

    let category = with_conn(&state, move |conn| {
        let current = categories::get_category(conn, id)?;
        let name = patch.name.unwrap_or(current.name);
        let category_type = patch.category_type.unwrap_or(current.category_type);
        // `Some(None)` means the client asked for the field to be cleared;
        // absent means keep what is there.
        let tax_line = patch.tax_line.unwrap_or(current.tax_line);
        let form_line = patch.form_line.unwrap_or(current.form_line);

        categories::update_category(
            conn,
            id,
            &name,
            &category_type,
            tax_line.as_deref(),
            form_line.as_deref(),
        )?;
        categories::get_category(conn, id)
    })
    .await?;
    Ok(Json(category))
}

async fn remove(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Deleted>> {
    with_conn(&state, move |conn| categories::delete_category(conn, id)).await?;
    Ok(Deleted::new(id))
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
    async fn categories_list_matches_the_data_layer_and_hides_inactive_rows() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/categories", &token).await;
        let expected =
            serde_json::to_value(super::categories::list_categories(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert!(rows.len() >= 29, "the seeded chart of accounts");
        for key in ["categoryType", "taxLine", "formLine"] {
            assert!(rows[0].get(key).is_some(), "missing {key}");
        }
    }

    #[tokio::test]
    async fn a_category_can_be_created_edited_field_by_field_and_soft_deleted() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, created) = post_json(
            &app,
            "/api/categories",
            &token,
            &serde_json::json!({
                "name": "Continuing Education",
                "categoryType": "expense",
                "taxLine": "Other deductions",
                "formLine": "1120S-19",
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["id"].as_i64().expect("an id");
        assert_eq!(created["formLine"], "1120S-19");

        // A one-field patch leaves everything else alone.
        let (status, renamed) = patch_json(
            &app,
            &format!("/api/categories/{id}"),
            &token,
            &serde_json::json!({ "name": "Training" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{renamed}");
        assert_eq!(renamed["name"], "Training");
        assert_eq!(renamed["categoryType"], "expense");
        assert_eq!(renamed["taxLine"], "Other deductions");
        assert_eq!(renamed["formLine"], "1120S-19");

        // An explicit null clears a field.
        let (status, cleared) = patch_json(
            &app,
            &format!("/api/categories/{id}"),
            &token,
            &serde_json::json!({ "formLine": null }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{cleared}");
        assert_eq!(cleared["formLine"], serde_json::Value::Null);
        assert_eq!(cleared["taxLine"], "Other deductions");

        let (status, deleted) = delete_json(&app, &format!("/api/categories/{id}"), &token).await;
        assert_eq!(status, StatusCode::OK, "{deleted}");

        let listed = ok_json(&app, "/api/categories", &token).await;
        assert!(
            !listed.as_array().unwrap().iter().any(|c| c["id"] == id),
            "a soft-deleted category is off the list"
        );
        // Soft delete: the row is still there, just inactive.
        let conn = crate::db::open_connection(&db_path, None).expect("open db");
        let active: i64 = conn
            .query_row(
                "SELECT is_active FROM categories WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active, 0);
    }

    #[tokio::test]
    async fn category_guardrails_answer_with_their_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // In use by transactions.
        let in_use = category_id(&db_path, "Client Services");
        let (status, body) = delete_json(&app, &format!("/api/categories/{in_use}"), &token).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "has_transactions");
        assert_eq!(body["error"]["details"]["count"], 3);

        // Referenced by an active rule but no transactions: a fresh category
        // with only a rule pointing at it.
        let (_, created) = post_json(
            &app,
            "/api/categories",
            &token,
            &serde_json::json!({ "name": "Rule Only", "categoryType": "expense" }),
        )
        .await;
        let rule_only = created["id"].as_i64().unwrap();
        let (status, rule) = post_json(
            &app,
            "/api/rules",
            &token,
            &serde_json::json!({ "pattern": "NOTHING MATCHES THIS", "categoryId": rule_only }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{rule}");

        let (status, body) =
            delete_json(&app, &format!("/api/categories/{rule_only}"), &token).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "has_active_rules");
        assert_eq!(body["error"]["details"]["count"], 1);

        // Duplicate name.
        let (status, body) = post_json(
            &app,
            "/api/categories",
            &token,
            &serde_json::json!({ "name": "Rule Only", "categoryType": "expense" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "duplicate_name");
    }

    #[tokio::test]
    async fn bad_category_requests_are_rejected() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = category_id(&db_path, "Office Expense");

        let cases: [(&str, serde_json::Value, StatusCode); 4] = [
            (
                "/api/categories",
                serde_json::json!({ "name": "Nope", "categoryType": "revenue" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                "/api/categories",
                serde_json::json!({ "name": "  ", "categoryType": "expense" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                "/api/categories/999999",
                serde_json::json!({ "name": "Ghost" }),
                StatusCode::NOT_FOUND,
            ),
            (
                "/api/categories/999999",
                serde_json::json!({}),
                StatusCode::BAD_REQUEST,
            ),
        ];

        for (uri, body, expected) in cases {
            let (status, json) = if uri.ends_with("/categories") {
                post_json(&app, uri, &token, &body).await
            } else {
                patch_json(&app, uri, &token, &body).await
            };
            assert_eq!(status, expected, "{uri} with {body} gave {json}");
        }

        // An empty patch on a real category is still nothing to do.
        let (status, json) = patch_json(
            &app,
            &format!("/api/categories/{id}"),
            &token,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    }
}
