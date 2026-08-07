//! Accounts: `GET /api/accounts` plus create, rename, and delete.
//!
//! Delete is a hard delete guarded by a transaction count, exactly as the CLI
//! and the TUI do it — a blocked delete answers `409` with the count, so the
//! client can say how many transactions are in the way.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Deserialize;

use crate::cli::accounts;
use crate::models::Account;

use super::super::error::ApiResult;
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::{with_conn, Deleted};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/accounts", get(list).post(create))
        .route("/accounts/{id}", patch(rename).delete(remove))
}

async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<Account>>> {
    Ok(Json(with_conn(&state, accounts::list_accounts).await?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccount {
    name: String,
    account_type: String,
    institution: Option<String>,
    last_four: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewAccount>,
) -> ApiResult<(StatusCode, Json<Account>)> {
    let account = with_conn(&state, move |conn| {
        let id = accounts::add_account(
            conn,
            &new.name,
            &new.account_type,
            new.institution.as_deref(),
            new.last_four.as_deref(),
        )?;
        accounts::get_account(conn, id)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(account)))
}

/// Renaming is the only edit the data layer offers; institution and last four
/// are set when the account is created.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPatch {
    name: String,
}

async fn rename(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(patch): ApiJson<AccountPatch>,
) -> ApiResult<Json<Account>> {
    let account = with_conn(&state, move |conn| {
        accounts::rename_account(conn, id, &patch.name)?;
        accounts::get_account(conn, id)
    })
    .await?;
    Ok(Json(account))
}

async fn remove(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Deleted>> {
    with_conn(&state, move |conn| accounts::delete_account(conn, id)).await?;
    Ok(Deleted::new(id))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;

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

    #[tokio::test]
    async fn an_account_can_be_created_renamed_and_deleted() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, created) = post_json(
            &app,
            "/api/accounts",
            &token,
            &serde_json::json!({
                "name": "Ally Savings",
                "accountType": "checking",
                "institution": "Ally",
                "lastFour": "9876",
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["id"].as_i64().expect("an id");
        assert_eq!(created["institution"], "Ally");
        assert_eq!(created["lastFour"], "9876");

        let (status, renamed) = patch_json(
            &app,
            &format!("/api/accounts/{id}"),
            &token,
            &serde_json::json!({ "name": "Ally Checking" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{renamed}");
        assert_eq!(renamed["name"], "Ally Checking");
        // Untouched fields survive a rename.
        assert_eq!(renamed["institution"], "Ally");

        let (status, deleted) = delete_json(&app, &format!("/api/accounts/{id}"), &token).await;
        assert_eq!(status, StatusCode::OK, "{deleted}");
        assert_eq!(deleted["deleted"], true);
        assert_eq!(deleted["id"], id);

        let names: Vec<String> = ok_json(&app, "/api/accounts", &token)
            .await
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].as_str().unwrap().to_string())
            .collect();
        assert!(!names.contains(&"Ally Checking".to_string()));
    }

    #[tokio::test]
    async fn a_duplicate_name_is_a_conflict_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/accounts",
            &token,
            &serde_json::json!({ "name": "BofA Checking", "accountType": "checking" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["code"], "conflict");
        assert_eq!(body["error"]["details"]["reason"], "duplicate_name");
        assert_eq!(body["error"]["details"]["name"], "BofA Checking");
    }

    #[tokio::test]
    async fn deleting_an_account_with_transactions_is_blocked_with_a_count() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let id = ok_json(&app, "/api/accounts", &token).await[0]["id"]
            .as_i64()
            .unwrap();

        let (status, body) = delete_json(&app, &format!("/api/accounts/{id}"), &token).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "has_transactions");
        assert_eq!(body["error"]["details"]["count"], 5);
        assert!(body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Cannot delete"));
    }

    #[tokio::test]
    async fn bad_accounts_requests_are_rejected() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/accounts",
            &token,
            &serde_json::json!({ "name": "Brokerage", "accountType": "brokerage" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");

        let (status, body) = patch_json(
            &app,
            "/api/accounts/999999",
            &token,
            &serde_json::json!({ "name": "Ghost" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

        let (status, body) = delete_json(&app, "/api/accounts/999999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    }
}
