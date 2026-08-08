//! Invoicing clients: the list, one client in full, and create/edit/delete.
//!
//! The detail route is `client_summary` with serde on it — the same round trip
//! `nigel client show` makes — so the browser and the terminal print one client
//! from one query.
//!
//! The writes are the CLI's own data layer called directly: `add_client` and
//! `update_client` validate the name and refuse a duplicate themselves, and
//! `delete_client` owns the has-invoices guardrail, so this module shapes
//! requests and narrows 404s and does no rule-keeping of its own.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::invoicing::clients::{self, ClientInvoiceRow, ClientUpdate};
use crate::models::Client;

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::{double_option, not_found_because, with_conn, with_conn_api, Deleted};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/clients", get(list).post(create))
        .route("/clients/{id}", get(detail).patch(update).delete(remove))
}

/// A client's own fields flattened alongside its history, rather than nested
/// under a `client` key: a screen showing one client wants one object, and
/// `ClientSummary`'s shape exists for the CLI's benefit.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientDetail {
    #[serde(flatten)]
    client: Client,
    invoices: Vec<ClientInvoiceRow>,
    outstanding: f64,
}

async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<Client>>> {
    Ok(Json(with_conn(&state, clients::list_clients).await?))
}

async fn detail(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<ClientDetail>> {
    let summary = with_conn_api(&state, move |conn| {
        clients::client_summary(conn, id).map_err(|e| not_found_because(e, "client_not_found"))
    })
    .await?;
    Ok(Json(ClientDetail {
        client: summary.client,
        invoices: summary.invoices,
        outstanding: summary.outstanding,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewClientRequest {
    name: String,
    email: Option<String>,
    billing_address: Option<String>,
    notes: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewClientRequest>,
) -> ApiResult<(StatusCode, Json<Client>)> {
    let client = with_conn(&state, move |conn| {
        let id = clients::add_client(
            conn,
            &new.name,
            new.email.as_deref(),
            new.billing_address.as_deref(),
            new.notes.as_deref(),
        )?;
        clients::get_client(conn, id)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(client)))
}

/// The three nullable columns are `double_option`: absent leaves them alone,
/// `null` clears them. `name` is `NOT NULL`, so it can be renamed and never
/// cleared — which is exactly what a plain `Option` expresses.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientPatch {
    name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    billing_address: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    notes: Option<Option<String>>,
}

/// Field for field: the request body *is* the update struct, with no
/// translation layer to keep in step.
impl From<ClientPatch> for ClientUpdate {
    fn from(patch: ClientPatch) -> Self {
        Self {
            name: patch.name,
            email: patch.email,
            billing_address: patch.billing_address,
            notes: patch.notes,
        }
    }
}

async fn update(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(patch): ApiJson<ClientPatch>,
) -> ApiResult<Json<Client>> {
    let update = ClientUpdate::from(patch);
    // `update_client` refuses an empty update too, as an `Invalid`; naming the
    // fields here is what makes the 400 useful to whoever sent `{}`.
    if update.is_empty() {
        return Err(ApiError::bad_request(
            "Nothing to update — provide at least one of `name`, `email`, `billingAddress`, or `notes`.",
        ));
    }

    let client = with_conn_api(&state, move |conn| {
        clients::update_client(conn, id, &update)
            .map_err(|e| not_found_because(e, "client_not_found"))?;
        Ok(clients::get_client(conn, id)?)
    })
    .await?;
    Ok(Json(client))
}

async fn remove(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Deleted>> {
    with_conn_api(&state, move |conn| {
        clients::delete_client(conn, id).map_err(|e| not_found_because(e, "client_not_found"))
    })
    .await?;
    Ok(Deleted::new(id))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn clients_list_matches_the_data_layer() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/clients", &token).await;
        let expected =
            serde_json::to_value(crate::invoicing::clients::list_clients(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 3);
        // By name, which is the order `list_clients` promises.
        assert_eq!(rows[0]["name"], "Acme Co");
        assert!(rows[0].get("billingAddress").is_some(), "{body}");
        // The client with no email carries an explicit null, not an absent key.
        assert_eq!(rows[1]["name"], "Globex");
        assert!(rows[1]["email"].is_null(), "{body}");
    }

    #[tokio::test]
    async fn a_client_detail_carries_its_invoices_and_open_balance() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/clients/1", &token).await;
        // Flattened: the client's own fields sit beside the history.
        assert_eq!(body["name"], "Acme Co");
        assert_eq!(body["email"], "ap@acme.test");

        let invoices = body["invoices"].as_array().expect("invoices");
        assert_eq!(invoices.len(), 2);
        // Newest number first.
        assert_eq!(invoices[0]["number"], 1251);
        assert_eq!(invoices[1]["number"], 1250);

        // 1251 open at 1850, 1250 open at 3200 - 2000.
        assert_eq!(body["outstanding"], 3050.0);
    }

    #[tokio::test]
    async fn an_unknown_client_id_is_404_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/clients/999999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["code"], "not_found");
        assert_eq!(body["error"]["details"]["reason"], "client_not_found");
    }

    #[tokio::test]
    async fn a_client_can_be_created_edited_and_deleted() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, created) = post_json(
            &app,
            "/api/clients",
            &token,
            &serde_json::json!({
                "name": "Initech",
                "email": "ap@initech.test",
                "billingAddress": "9 Cubicle Way",
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["id"].as_i64().expect("an id");
        assert_eq!(created["name"], "Initech");
        assert_eq!(created["billingAddress"], "9 Cubicle Way");
        // An omitted optional field is null, not absent.
        assert!(created["notes"].is_null(), "{created}");

        let (status, edited) = patch_json(
            &app,
            &format!("/api/clients/{id}"),
            &token,
            &serde_json::json!({ "name": "Initech LLC" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{edited}");
        assert_eq!(edited["name"], "Initech LLC");
        assert_eq!(
            edited["email"], "ap@initech.test",
            "untouched by the rename"
        );

        let (status, deleted) = delete_json(&app, &format!("/api/clients/{id}"), &token).await;
        assert_eq!(status, StatusCode::OK, "{deleted}");
        assert_eq!(deleted["deleted"], true);
        assert_eq!(deleted["id"], id);

        let listed = ok_json(&app, "/api/clients", &token).await;
        assert!(
            !listed.as_array().unwrap().iter().any(|c| c["id"] == id),
            "a deleted client is off the list: {listed}"
        );
    }

    #[tokio::test]
    async fn a_duplicate_client_name_is_a_409_with_the_name() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/clients",
            &token,
            &serde_json::json!({ "name": "Acme Co" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "duplicate_name");
        assert_eq!(body["error"]["details"]["name"], "Acme Co");

        // Renaming onto a taken name is the same refusal.
        let (status, body) = patch_json(
            &app,
            "/api/clients/2",
            &token,
            &serde_json::json!({ "name": "Acme Co" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "duplicate_name");
    }

    #[tokio::test]
    async fn a_patch_can_clear_an_email_but_omitting_it_leaves_it() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, kept) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({ "notes": "pays late" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{kept}");
        assert_eq!(kept["email"], "ap@acme.test", "absent leaves it alone");
        assert_eq!(kept["notes"], "pays late");

        let (status, cleared) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({ "email": null }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{cleared}");
        assert!(cleared["email"].is_null(), "null clears it: {cleared}");
        assert_eq!(cleared["notes"], "pays late", "and touches nothing else");
    }

    #[tokio::test]
    async fn an_all_absent_client_patch_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) =
            patch_json(&app, "/api/clients/1", &token, &serde_json::json!({})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
    }

    #[tokio::test]
    async fn deleting_a_client_with_invoices_is_blocked_with_a_count() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // Globex owns 1247 (void) and 1249 (overdue): a void invoice counts.
        let (status, body) = delete_json(&app, "/api/clients/2", &token).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "has_invoices");
        assert_eq!(body["error"]["details"]["count"], 2);
        assert_eq!(
            body["error"]["message"],
            "Cannot delete: client has 2 invoices"
        );

        // Refused means refused.
        let still_there = ok_json(&app, "/api/clients/2", &token).await;
        assert_eq!(still_there["name"], "Globex");
    }

    #[tokio::test]
    async fn an_empty_client_name_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for body in [
            serde_json::json!({ "name": "   " }),
            serde_json::json!({ "name": "" }),
        ] {
            let (status, json) = post_json(&app, "/api/clients", &token, &body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {json}");
        }

        let (status, json) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({ "name": " " }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    }

    #[tokio::test]
    async fn editing_or_deleting_an_unknown_client_is_a_404_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = patch_json(
            &app,
            "/api/clients/999999",
            &token,
            &serde_json::json!({ "name": "Ghost" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "client_not_found");

        let (status, body) = delete_json(&app, "/api/clients/999999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "client_not_found");
    }

    #[tokio::test]
    async fn a_non_numeric_client_id_answers_in_the_envelope() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/clients/acme", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
    }
}
