//! Invoicing clients: `GET /api/clients` and `GET /api/clients/{id}`.
//!
//! The detail route is `client_summary` with serde on it — the same round trip
//! `nigel client show` makes — so the browser and the terminal print one client
//! from one query.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::invoicing::clients::{self, ClientInvoiceRow};
use crate::models::Client;

use super::super::error::ApiResult;
use super::super::extract::ApiPath;
use super::super::state::AppState;
use super::{not_found_because, with_conn, with_conn_api};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/clients", get(list))
        .route("/clients/{id}", get(detail))
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
    async fn a_non_numeric_client_id_answers_in_the_envelope() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/clients/acme", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
    }
}
