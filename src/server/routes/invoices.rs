//! Invoices: the list, one invoice in full, the A/R aging report, and the
//! number the next draft will get.
//!
//! Every guard the detail response reports as a `can*` flag is 68.1's own
//! function, called rather than re-derived — `ensure_editable` blocks on
//! recorded payments as well as on status, and a status-only copy of that rule
//! in a client would disagree with the 409 it is meant to predict.

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::invoicing::clients::get_client;
use crate::invoicing::invoices::{self as inv, AgingReport, InvoiceListRow};
use crate::invoicing::render::{render_invoice, RenderedInvoice};
use crate::invoicing::render_html::{load_template, Branding};
use crate::models::{Client, Invoice, InvoiceLineItem, InvoicePayment};

use super::super::error::{ApiError, ApiResult};
use super::super::extract::ApiPath;
use super::super::state::AppState;
use super::{not_found_because, with_conn, with_conn_api};

pub fn routes() -> Router<AppState> {
    // The two literal paths are mounted before the `{number}` pattern. axum
    // prefers a literal segment either way; the order is here so that reading
    // the file cannot suggest otherwise, and a test pins the behaviour.
    Router::new()
        .route("/invoices", get(list))
        .route("/invoices/aging", get(aging))
        .route("/invoices/next-number", get(next_number))
        .route("/invoices/{number}", get(detail))
        .route("/invoices/{number}/preview", get(preview_html))
        .route("/invoices/{number}/preview.pdf", get(preview_pdf))
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// One invoice with everything a detail screen prints.
///
/// The invoice's own fields are flattened, so `token` stays skipped and the
/// computed `publicUrl` is the only address that crosses the wire.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InvoiceDetail {
    #[serde(flatten)]
    invoice: Invoice,
    client: Client,
    items: Vec<InvoiceLineItem>,
    payments: Vec<InvoicePayment>,
    paid: f64,
    balance: f64,
    /// Where the published page lives, or `null` when the invoice was never
    /// published or `public_base_url` is unset. Never an error: an unconfigured
    /// installation still has invoices worth looking at.
    public_url: Option<String>,
    can_edit: bool,
    can_send: bool,
    can_void: bool,
    can_pay: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NextNumber {
    number: i64,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// The invoice behind a number, or a 404 that says it was the invoice missing.
fn find_invoice(conn: &Connection, number: i64) -> ApiResult<Invoice> {
    // `get_invoice_by_number` reports an absent row as a rusqlite error, which
    // the global mapping would call a 500. The CLI narrows it the same way.
    match inv::get_invoice_by_number(conn, number) {
        Ok(invoice) => Ok(invoice),
        Err(crate::error::NigelError::Db(rusqlite::Error::QueryReturnedNoRows)) => Err(
            ApiError::not_found_because(format!("No invoice #{number}."), "invoice_not_found"),
        ),
        Err(other) => Err(ApiError::from(other)),
    }
}

fn detail_for(conn: &Connection, invoice: Invoice) -> ApiResult<InvoiceDetail> {
    let client = get_client(conn, invoice.client_id)
        .map_err(|e| not_found_because(e, "client_not_found"))?;
    let items = inv::line_items(conn, invoice.id)?;
    let payments = inv::payments(conn, invoice.id)?;
    let paid = inv::paid_amount(conn, invoice.id)?;

    let can_edit = inv::ensure_editable(conn, &invoice).is_ok();
    let can_void = inv::ensure_voidable(conn, &invoice).is_ok();
    let not_void = inv::ensure_not_void(&invoice, "sent").is_ok();
    let can_send = not_void && client.email.is_some() && invoice.total > 0.0;
    let can_pay = not_void && inv::payment_amount(&invoice, paid, None).is_ok();

    Ok(InvoiceDetail {
        public_url: public_url(&invoice),
        balance: invoice.total - paid,
        invoice,
        client,
        items,
        payments,
        paid,
        can_edit,
        can_send,
        can_void,
        can_pay,
    })
}

/// The published page's address, built from the token the response never
/// carries. `None` rather than an error when nothing has been configured — a
/// missing setting is a fact about the installation, not a failed request.
fn public_url(invoice: &Invoice) -> Option<String> {
    invoice.published_at.as_ref()?;
    let base = crate::settings::invoicing_config().public_base_url?;
    Some(crate::invoicing::r2::public_url(&base, &invoice.token))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// The list filters, taken as strings so a malformed one lands in the error
/// envelope instead of axum's plain-text `Query` rejection.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    status: Option<String>,
    client_id: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<Vec<InvoiceListRow>>> {
    let client_id = query
        .client_id
        .as_deref()
        .map(|value| {
            value.parse::<i64>().map_err(|_| {
                ApiError::bad_request(format!(
                    "Invalid `clientId`: expected a client id, got \"{value}\"."
                ))
            })
        })
        .transpose()?;

    let rows = with_conn_api(&state, move |conn| {
        // Filtering by a client that does not exist is a wrong question, not an
        // empty answer — the same reasoning `ensure_account_exists` applies to
        // the register.
        if let Some(id) = client_id {
            crate::invoicing::clients::ensure_client_exists(conn, id)
                .map_err(|e| not_found_because(e, "client_not_found"))?;
        }
        // `statuses_for` already refuses an unknown word by naming the legal
        // set, and that refusal is an `Invalid`, so it arrives as a 400.
        Ok(inv::list_invoices(
            conn,
            query.status.as_deref(),
            client_id,
        )?)
    })
    .await?;
    Ok(Json(rows))
}

async fn detail(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
) -> ApiResult<Json<InvoiceDetail>> {
    let detail = with_conn_api(&state, move |conn| {
        let invoice = find_invoice(conn, number)?;
        detail_for(conn, invoice)
    })
    .await?;
    Ok(Json(detail))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgingQuery {
    as_of: Option<String>,
}

async fn aging(
    State(state): State<AppState>,
    Query(query): Query<AgingQuery>,
) -> ApiResult<Json<AgingReport>> {
    // The server's own today, which is what `nigel invoice aging` uses.
    let as_of = match query.as_of.as_deref() {
        Some(value) => super::reports::parse_date("asOf", value)?,
        None => chrono::Local::now().format("%Y-%m-%d").to_string(),
    };
    let report = with_conn(&state, move |conn| inv::ar_aging_detail(conn, &as_of)).await?;
    Ok(Json(report))
}

/// The number the next draft will take. Reads the counter and reserves nothing,
/// so a form can show it before anyone commits to creating an invoice.
async fn next_number(State(state): State<AppState>) -> ApiResult<Json<NextNumber>> {
    let number = with_conn(&state, inv::next_number).await?;
    Ok(Json(NextNumber { number }))
}

// ---------------------------------------------------------------------------
// Preview
// ---------------------------------------------------------------------------

/// Render an invoice through the seam `send` publishes through.
///
/// No gateway is passed in, which is the proof it makes no network call, and no
/// invoicing config is required: an unset `from_email` renders the same visible
/// placeholder `nigel invoice preview` prints a notice about.
fn render(
    conn: &Connection,
    data_dir: &std::path::Path,
    number: i64,
) -> ApiResult<RenderedInvoice> {
    let invoice = find_invoice(conn, number)?;
    let client = get_client(conn, invoice.client_id)
        .map_err(|e| not_found_because(e, "client_not_found"))?;

    // Loaded before anything is rendered, so a broken override is a 400 naming
    // the path rather than a page nobody approved.
    let template = load_template(data_dir)?;
    let company = crate::cli::invoice::company_name(conn);
    let (contact_email, _placeholder) =
        crate::cli::invoice::contact_email_for_preview(&crate::settings::invoicing_config());
    let branding = Branding {
        template: &template,
        company: &company,
        contact_email: &contact_email,
    };

    Ok(render_invoice(
        conn,
        &invoice,
        &client,
        crate::cli::invoice::pay_button_for(&invoice),
        &branding,
    )?)
}

async fn preview_html(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
) -> ApiResult<Response> {
    let data_dir = state.data_dir();
    let rendered = with_conn_api(&state, move |conn| render(conn, &data_dir, number)).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            // The SPA frames this in a sandboxed iframe, but the route is also
            // openable in a tab, where the document would otherwise be a
            // same-origin page rendering database text.
            (header::CONTENT_SECURITY_POLICY, "sandbox"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        rendered.html,
    )
        .into_response())
}

async fn preview_pdf(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
) -> ApiResult<Response> {
    let data_dir = state.data_dir();
    let bytes = with_conn_api(&state, move |conn| {
        // The same sentence the CLI prints, answered the way `exports.rs`
        // answers it: HTML still renders in such a build, only the PDF cannot.
        render(conn, &data_dir, number)?
            .pdf
            .ok_or_else(|| ApiError::feature_disabled(crate::cli::report::PDF_DISABLED_MESSAGE))
    })
    .await?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                // Nothing from the database reaches the header: the stem is a
                // fixed string and the number is digits.
                format!("attachment; filename=\"invoice-{number}.pdf\""),
            ),
        ],
        Body::from(bytes),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn invoices_list_is_newest_first_and_carries_the_balance() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/invoices", &token).await;
        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0]["number"], 1252);
        assert_eq!(rows[5]["number"], 1247);

        let partial = &rows[2];
        assert_eq!(partial["number"], 1250);
        assert_eq!(partial["status"], "partial");
        assert_eq!(partial["total"], 3200.0);
        assert_eq!(partial["paid"], 2000.0);
        assert_eq!(partial["balance"], 1200.0);
        assert_eq!(partial["clientName"], "Acme Co");
    }

    #[tokio::test]
    async fn the_list_can_be_filtered_by_status_and_client() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let open = ok_json(&app, "/api/invoices?status=open", &token).await;
        let numbers: Vec<i64> = open
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["number"].as_i64().unwrap())
            .collect();
        assert_eq!(numbers, vec![1251, 1250, 1249], "{open}");

        let draft = ok_json(&app, "/api/invoices?status=draft", &token).await;
        assert_eq!(draft.as_array().unwrap().len(), 1);
        assert_eq!(draft[0]["number"], 1252);

        // Acme Co is client 1: invoices 1251 and 1250.
        let acme = ok_json(&app, "/api/invoices?clientId=1", &token).await;
        let numbers: Vec<i64> = acme
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["number"].as_i64().unwrap())
            .collect();
        assert_eq!(numbers, vec![1251, 1250], "{acme}");

        let both = ok_json(&app, "/api/invoices?clientId=1&status=sent", &token).await;
        assert_eq!(both.as_array().unwrap().len(), 1);
        assert_eq!(both[0]["number"], 1251);
    }

    #[tokio::test]
    async fn an_unknown_status_filter_is_a_400_naming_the_legal_set() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/invoices?status=pending", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
        let message = body["error"]["message"].as_str().unwrap();
        for word in [
            "draft", "sent", "partial", "paid", "overdue", "void", "open",
        ] {
            assert!(message.contains(word), "{word} missing from {message}");
        }
    }

    #[tokio::test]
    async fn an_unknown_client_id_filter_is_a_404_not_an_empty_list() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/invoices?clientId=999999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "client_not_found");

        let (status, body) = get_json(&app, "/api/invoices?clientId=acme", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    #[tokio::test]
    async fn an_invoice_detail_carries_items_payments_flags_and_no_token() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/invoices/1250", &token).await;
        assert!(body.get("token").is_none(), "token leaked: {body}");

        // Flattened, so the invoice's own fields sit at the top level.
        assert_eq!(body["number"], 1250);
        assert_eq!(body["status"], "partial");
        assert_eq!(body["client"]["name"], "Acme Co");
        assert_eq!(body["items"].as_array().unwrap().len(), 2);
        assert_eq!(body["payments"].as_array().unwrap().len(), 1);
        assert_eq!(
            body["payments"][0]["stripeCheckoutSessionId"],
            "cs_test_seed_1250"
        );
        assert_eq!(body["paid"], 2000.0);
        assert_eq!(body["balance"], 1200.0);

        // Published, but no `public_base_url` is configured under TempConfig.
        assert!(body["publicUrl"].is_null(), "{body}");

        // Published and part-paid: editing is refused, voiding is refused,
        // and there is still a balance to settle.
        assert_eq!(body["canEdit"], false);
        assert_eq!(body["canVoid"], false);
        assert_eq!(body["canPay"], true);
        assert_eq!(body["canSend"], true);
    }

    #[tokio::test]
    async fn a_draft_can_be_edited_and_a_void_one_can_do_nothing() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let draft = ok_json(&app, "/api/invoices/1252", &token).await;
        assert_eq!(draft["status"], "draft");
        assert_eq!(draft["canEdit"], true);
        assert_eq!(draft["canVoid"], true);
        assert_eq!(draft["canPay"], true);
        // Never published, so no address to hand out.
        assert!(draft["publicUrl"].is_null(), "{draft}");

        let voided = ok_json(&app, "/api/invoices/1247", &token).await;
        assert_eq!(voided["status"], "void");
        for flag in ["canEdit", "canSend", "canVoid", "canPay"] {
            assert_eq!(voided[flag], false, "{flag} on a void invoice: {voided}");
        }

        // Globex has no email, so a send is refused before any network call.
        let overdue = ok_json(&app, "/api/invoices/1249", &token).await;
        assert!(overdue["client"]["email"].is_null());
        assert_eq!(overdue["canSend"], false);

        // Settled in full: nothing left to pay.
        let paid = ok_json(&app, "/api/invoices/1248", &token).await;
        assert_eq!(paid["status"], "paid");
        assert_eq!(paid["canPay"], false);
    }

    #[tokio::test]
    async fn a_public_url_is_built_from_the_configured_base() {
        let _config = TempConfig::new();
        let mut settings = crate::settings::load_settings();
        settings.public_base_url = Some("https://billing.example.test/i".to_string());
        crate::settings::save_settings(&settings).expect("settings");

        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");
        let seeded = crate::invoicing::invoices::get_invoice_by_number(&conn, 1251).unwrap();

        let body = ok_json(&app, "/api/invoices/1251", &token).await;
        assert_eq!(
            body["publicUrl"],
            format!("https://billing.example.test/i/{}/", seeded.token)
        );
        // The address, never the secret it is built from.
        assert!(body.get("token").is_none(), "{body}");
    }

    #[tokio::test]
    async fn an_unknown_invoice_number_is_a_404_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/invoices/9999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["code"], "not_found");
        assert_eq!(body["error"]["details"]["reason"], "invoice_not_found");
    }

    #[tokio::test]
    async fn aging_takes_an_as_of_date_and_defaults_to_today() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, &format!("/api/invoices/aging?asOf={AS_OF}"), &token).await;
        assert_eq!(body["asOf"], AS_OF);
        // 1251 and 1250 are not yet due; 1249 is 44 days past.
        assert_eq!(body["outstanding"], 4010.0);
        let buckets = body["buckets"].as_array().expect("five buckets");
        assert_eq!(buckets.len(), 5);
        assert_eq!(buckets[0]["label"], "current");
        assert_eq!(buckets[0]["total"], 3050.0);
        assert_eq!(buckets[2]["label"], "31-60");
        assert_eq!(buckets[2]["total"], 960.0);

        let invoices = body["invoices"].as_array().expect("open invoices");
        assert_eq!(invoices.len(), 3);
        assert_eq!(invoices[0]["number"], 1249, "sorted by days past due");
        assert_eq!(invoices[0]["daysPastDue"], 44);

        // No `asOf` is the server's today, which the seeded books are behind.
        let today = ok_json(&app, "/api/invoices/aging", &token).await;
        assert!(today["asOf"].as_str().unwrap().len() == 10, "{today}");
    }

    #[tokio::test]
    async fn a_malformed_as_of_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for value in ["2026-3-1", "March", "2026-13-01", ""] {
            let (status, body) =
                get_json(&app, &format!("/api/invoices/aging?asOf={value}"), &token).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "asOf={value}: {body}");
            assert_eq!(body["error"]["code"], "bad_request", "asOf={value}");
        }
    }

    /// axum prefers a literal segment over a pattern, and both of these would
    /// otherwise be read as invoice numbers — which `ApiPath<i64>` would refuse
    /// with a 400 rather than answering the report.
    #[tokio::test]
    async fn the_literal_paths_are_not_parsed_as_invoice_numbers() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let aging = ok_json(&app, "/api/invoices/aging", &token).await;
        assert!(aging.get("buckets").is_some(), "{aging}");

        let next = ok_json(&app, "/api/invoices/next-number", &token).await;
        assert_eq!(next["number"], 1253);
    }

    #[tokio::test]
    async fn next_number_reserves_nothing() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let first = ok_json(&app, "/api/invoices/next-number", &token).await;
        let second = ok_json(&app, "/api/invoices/next-number", &token).await;
        assert_eq!(first, second);
    }

    // -----------------------------------------------------------------------
    // Preview
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn the_preview_route_answers_html_with_a_sandbox_csp() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let response = get_response(&app, "/api/invoices/1248/preview", &token).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(content_type(&response), "text/html; charset=utf-8");
        assert_eq!(
            header_str(&response, axum::http::header::CONTENT_SECURITY_POLICY),
            "sandbox"
        );
        assert_eq!(
            header_str(&response, axum::http::header::X_CONTENT_TYPE_OPTIONS),
            "nosniff"
        );

        let body = body_string(response).await;
        assert!(body.contains("1248"), "{body}");
        assert!(body.contains("Northwind Traders"), "{body}");
    }

    #[tokio::test]
    async fn a_draft_previews_with_a_placeholder_pay_button() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // 1252 is the draft: no Stripe link, and the route takes no gateway,
        // which is what makes "no network call" structural rather than asserted.
        let body =
            body_string(get_response(&app, "/api/invoices/1252/preview", &token).await).await;
        assert!(!body.contains("buy.stripe.com"), "{body}");
        assert!(body.contains("Brand refresh"), "{body}");
    }

    #[tokio::test]
    async fn a_published_invoice_previews_with_its_real_pay_link() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body =
            body_string(get_response(&app, "/api/invoices/1251/preview", &token).await).await;
        assert!(
            body.contains("https://buy.stripe.com/test_seed_1251"),
            "{body}"
        );
    }

    /// 68.2's second acceptance criterion: preview needs no invoicing config.
    #[tokio::test]
    async fn preview_works_with_no_invoicing_config_set() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let response = get_response(&app, "/api/invoices/1250/preview", &token).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        // The contact line renders the same visible placeholder the CLI notices
        // about, rather than an empty address or a 500.
        assert!(body.contains("(from_email not configured)"), "{body}");
    }

    /// A byte route still answers its failures as JSON — the `exports.rs`
    /// property, restated here because it is easy to lose on a document route.
    #[tokio::test]
    async fn previewing_an_unknown_invoice_is_a_404_in_the_envelope() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for uri in PREVIEW_ROUTES.map(|route| route.replace("1248", "9999")) {
            let (status, body) = get_json(&app, &uri, &token).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
            assert_eq!(body["error"]["details"]["reason"], "invoice_not_found");
        }
    }

    /// The override is validated when it is loaded, so a typo is a 400 naming
    /// the file rather than a stock page nobody approved.
    #[tokio::test]
    async fn a_broken_template_is_a_400_naming_the_path() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let data_dir = db_path.parent().unwrap();
        let templates = data_dir.join("templates");
        std::fs::create_dir_all(&templates).expect("templates dir");
        std::fs::write(templates.join("invoice.html"), "<p>{{NOPE}}</p>").expect("template");

        let (app, token) = app_for(&db_path);
        let (status, body) = get_json(&app, "/api/invoices/1248/preview", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("invoice.html"),
            "{body}"
        );
    }

    #[cfg(feature = "pdf")]
    #[tokio::test]
    async fn preview_pdf_answers_bytes_with_the_feature() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let response = get_response(&app, "/api/invoices/1248/preview.pdf", &token).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(content_type(&response), "application/pdf");
        assert_eq!(
            header_str(&response, axum::http::header::CONTENT_DISPOSITION),
            "attachment; filename=\"invoice-1248.pdf\""
        );
        let bytes = body_bytes(response).await;
        assert!(bytes.starts_with(b"%PDF"), "not a PDF");
    }

    #[cfg(not(feature = "pdf"))]
    #[tokio::test]
    async fn preview_pdf_is_501_without_the_feature_and_html_still_works() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/invoices/1248/preview.pdf", &token).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{body}");
        assert_eq!(body["error"]["code"], "feature_disabled");
        assert_eq!(
            body["error"]["message"],
            crate::cli::report::PDF_DISABLED_MESSAGE
        );

        let html = get_response(&app, "/api/invoices/1248/preview", &token).await;
        assert_eq!(html.status(), StatusCode::OK, "HTML preview still works");
    }

    #[tokio::test]
    async fn a_locked_database_refuses_both_preview_routes() {
        let (_dir, db_path) = seeded_db();
        encrypt(&db_path);
        let (app, token) = app_for(&db_path);

        for uri in PREVIEW_ROUTES {
            let (status, body) = get_json(&app, uri, &token).await;
            assert_eq!(status, StatusCode::LOCKED, "{uri} while locked: {body}");
            assert_eq!(body["error"]["code"], "locked", "for {uri}");
        }
    }
}
