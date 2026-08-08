//! Invoices: the list, one invoice in full, the A/R aging report, the number
//! the next draft will get, and the writes — create, edit, void and pay.
//!
//! Every write answers the whole refreshed detail, because the status is
//! derived rather than set and almost every write moves it.
//!
//! Every guard the detail response reports as a `can*` flag is 68.1's own
//! function, called rather than re-derived — `ensure_editable` blocks on
//! recorded payments as well as on status, and a status-only copy of that rule
//! in a client would disagree with the 409 it is meant to predict.

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::NigelError;
use crate::invoicing::clients::get_client;
use crate::invoicing::invoices::{
    self as inv, AgingReport, InvoiceListRow, InvoiceUpdate, NewLineItem,
};
use crate::invoicing::render::{render_invoice, RenderedInvoice};
use crate::invoicing::render_html::{load_template, Branding};
use crate::models::{Client, Invoice, InvoiceLineItem, InvoicePayment};

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::{double_option, not_found_because, with_conn, with_conn_api};

pub fn routes() -> Router<AppState> {
    // The two literal paths are mounted before the `{number}` pattern. axum
    // prefers a literal segment either way; the order is here so that reading
    // the file cannot suggest otherwise, and a test pins the behaviour.
    Router::new()
        .route("/invoices", get(list).post(create))
        .route("/invoices/aging", get(aging))
        .route("/invoices/next-number", get(next_number))
        .route("/invoices/{number}", get(detail).patch(update))
        .route("/invoices/{number}/void", post(void))
        .route("/invoices/{number}/pay", post(pay))
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
    inv::get_invoice_by_number(conn, number).map_err(|e| not_found_because(e, "invoice_not_found"))
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
// Writes
// ---------------------------------------------------------------------------

/// The data layer's conflict with the figures a screen would otherwise have to
/// read out of the sentence.
///
/// The code and the message stay exactly as the data layer wrote them — only
/// `details` grows, and only for the codes that carry a number worth rendering.
/// The enrichment lives here rather than in `NigelError::Conflict` because that
/// variant carries a code and a message and nothing else, and widening it for
/// three call sites that already hold the invoice is not worth it.
fn enrich_conflict(err: NigelError, invoice: &Invoice, paid: f64) -> ApiError {
    let details = match &err {
        NigelError::Conflict { code, .. } => match *code {
            "not_draft" => Some(serde_json::json!({
                "reason": code, "status": invoice.status,
            })),
            "has_payments" | "no_balance" => Some(serde_json::json!({
                "reason": code, "total": invoice.total, "paid": paid,
            })),
            _ => None,
        },
        _ => None,
    };
    match details {
        Some(details) => ApiError::conflict(err.to_string(), details),
        None => ApiError::from(err),
    }
}

/// The line items a create or an edit is asking for, refused before they reach
/// a row.
///
/// `create_invoice` and `update_invoice` sum these into `subtotal`/`total`, so a
/// non-finite figure here poisons every later aggregate over the column — the
/// same reasoning `payment_amount` rejects a NaN amount with. JSON cannot spell
/// `NaN`, but an overflowing literal deserializes to infinity.
fn checked_items(items: Vec<NewLineItem>) -> ApiResult<Vec<NewLineItem>> {
    if items.is_empty() {
        return Err(ApiError::bad_request(
            "An invoice needs at least one line item.",
        ));
    }
    for item in &items {
        if !item.quantity.is_finite() || !item.unit_amount.is_finite() {
            return Err(ApiError::bad_request(format!(
                "Line item \"{}\" needs a finite quantity and unit amount.",
                item.description
            )));
        }
    }
    // Every figure is finite by now, so a plain comparison says what it means.
    let total: f64 = items.iter().map(|i| i.quantity * i.unit_amount).sum();
    if total <= 0.0 {
        return Err(ApiError::bad_request(format!(
            "An invoice must total more than zero, got {total:.2}."
        )));
    }
    Ok(items)
}

fn default_currency() -> String {
    "USD".to_string()
}

/// Every date on this API is zero-padded `YYYY-MM-DD`.
///
/// This is the same parser `/api/reports` and the aging route use, not a second
/// one: the data layer's `validate_date` goes through chrono, which accepts
/// `2026-4-1`, and the HTTP API is deliberately stricter with dates than the
/// CLI is. `create_invoice` and `record_payment` validate again on the way in.
fn checked_date(param: &str, value: &str) -> ApiResult<()> {
    super::reports::parse_date(param, value).map(|_| ())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewInvoiceRequest {
    client_id: i64,
    issue_date: String,
    due_date: Option<String>,
    #[serde(default = "default_currency")]
    currency: String,
    items: Vec<NewLineItem>,
    notes: Option<String>,
    terms: Option<String>,
}

/// A new draft. The client, the dates and the currency are `create_invoice`'s
/// own checks; the number comes from the counter it advances in the same
/// transaction, so a refused create reserves nothing.
async fn create(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewInvoiceRequest>,
) -> ApiResult<(StatusCode, Json<InvoiceDetail>)> {
    let items = checked_items(new.items)?;
    checked_date("issueDate", &new.issue_date)?;
    if let Some(ref due) = new.due_date {
        checked_date("dueDate", due)?;
    }
    let detail = with_conn_api(&state, move |conn| {
        let id = inv::create_invoice(
            conn,
            new.client_id,
            &new.issue_date,
            new.due_date.as_deref(),
            &new.currency,
            &items,
            new.notes.as_deref(),
            new.terms.as_deref(),
        )
        // The only thing `create_invoice` looks up is the client.
        .map_err(|e| not_found_because(e, "client_not_found"))?;
        detail_for(conn, inv::get_invoice(conn, id)?)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

/// `items` is a whole-list replacement, matching the CLI's repeatable `--item`:
/// a per-row API would mean a client reconciling positions across two requests
/// and a server holding a half-edited invoice between them.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvoicePatch {
    issue_date: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    due_date: Option<Option<String>>,
    currency: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    terms: Option<Option<String>>,
    items: Option<Vec<NewLineItem>>,
}

impl InvoicePatch {
    /// Field for field into the data layer's update struct, with the dates and
    /// the line items checked on the way through.
    fn into_update(self) -> ApiResult<InvoiceUpdate> {
        if let Some(ref issue) = self.issue_date {
            checked_date("issueDate", issue)?;
        }
        if let Some(Some(ref due)) = self.due_date {
            checked_date("dueDate", due)?;
        }
        Ok(InvoiceUpdate {
            issue_date: self.issue_date,
            due_date: self.due_date,
            currency: self.currency,
            notes: self.notes,
            terms: self.terms,
            items: self.items.map(checked_items).transpose()?,
        })
    }
}

async fn update(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
    ApiJson(patch): ApiJson<InvoicePatch>,
) -> ApiResult<Json<InvoiceDetail>> {
    let update = patch.into_update()?;
    if update.is_empty() {
        return Err(ApiError::bad_request(
            "Nothing to update — provide at least one of `issueDate`, `dueDate`, `currency`, `notes`, `terms`, or `items`.",
        ));
    }

    let detail = with_conn_api(&state, move |conn| {
        let invoice = find_invoice(conn, number)?;
        let paid = inv::paid_amount(conn, invoice.id)?;
        // `update_invoice` re-reads the row and runs `ensure_editable` inside
        // its own transaction, so draft-only is enforced against the current
        // status rather than anything the client sent — and this route must not
        // open a transaction of its own around it.
        inv::update_invoice(conn, invoice.id, &update)
            .map_err(|e| enrich_conflict(e, &invoice, paid))?;
        detail_for(conn, find_invoice(conn, number)?)
    })
    .await?;
    Ok(Json(detail))
}

/// Cancel an invoice. `void_invoice` writes `voided_at` and lets
/// `refresh_status` derive the status from it, so the route passes the server's
/// own today — the value `pay` passes too.
async fn void(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
) -> ApiResult<Json<InvoiceDetail>> {
    let today = crate::cli::today();
    let detail = with_conn_api(&state, move |conn| {
        let invoice = find_invoice(conn, number)?;
        let paid = inv::paid_amount(conn, invoice.id)?;
        inv::void_invoice(conn, invoice.id, &today)
            .map_err(|e| enrich_conflict(e, &invoice, paid))?;
        detail_for(conn, find_invoice(conn, number)?)
    })
    .await?;
    Ok(Json(detail))
}

/// `amount` omitted means the whole outstanding balance, exactly as `--amount`
/// omitted does. `method` defaults to the one a bank transfer arrives as.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PayRequest {
    amount: Option<f64>,
    date: String,
    #[serde(default = "default_method")]
    method: String,
}

fn default_method() -> String {
    "direct_deposit".to_string()
}

async fn pay(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
    ApiJson(request): ApiJson<PayRequest>,
) -> ApiResult<Json<InvoiceDetail>> {
    // The method refusal is the data layer's, so the CLI and the API name the
    // same legal set. `record_payment` checks it again; asking here is what
    // keeps a request that cannot succeed from opening a connection at all.
    inv::validate_payment_method(&request.method)?;
    checked_date("date", &request.date)?;

    let detail = with_conn_api(&state, move |conn| {
        let invoice = find_invoice(conn, number)?;
        let paid = inv::paid_amount(conn, invoice.id)?;
        let amount = inv::payment_amount(&invoice, paid, request.amount)
            .map_err(|e| enrich_conflict(e, &invoice, paid))?;
        inv::record_payment(
            conn,
            invoice.id,
            amount,
            &request.date,
            &request.method,
            None,
        )
        .map_err(|e| enrich_conflict(e, &invoice, paid))?;
        detail_for(conn, find_invoice(conn, number)?)
    })
    .await?;
    Ok(Json(detail))
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
    let rendered = with_conn_api(&state, {
        let state = state.clone();
        // The data directory is read inside the closure, under the same guard
        // the connection is opened under: a data-directory switch landing
        // between the two would render the new database's invoice through the
        // old directory's template.
        move |conn| render(conn, &state.data_dir(), number)
    })
    .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            // The SPA frames this in a sandboxed iframe, but the route is also
            // openable in a tab, where the document would otherwise be a
            // same-origin page rendering database text.
            (header::CONTENT_SECURITY_POLICY, "sandbox"),
            // Overriding the blanket `DENY`, which blocks same-origin framing
            // too and would leave the SPA's preview iframe blank. `nosniff`
            // comes from that same middleware and is not restated here.
            (header::X_FRAME_OPTIONS, "SAMEORIGIN"),
        ],
        rendered.html,
    )
        .into_response())
}

async fn preview_pdf(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
) -> ApiResult<Response> {
    let bytes = with_conn_api(&state, {
        let state = state.clone();
        move |conn| {
            // The same sentence the CLI prints, answered the way `exports.rs`
            // answers it: HTML still renders in such a build, only the PDF
            // cannot.
            render(conn, &state.data_dir(), number)?
                .pdf
                .ok_or_else(|| ApiError::feature_disabled(crate::cli::report::PDF_DISABLED_MESSAGE))
        }
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
    // Create and edit
    // -----------------------------------------------------------------------

    /// A body good enough to create an invoice, so a test that varies one field
    /// says what it is varying.
    fn new_invoice() -> serde_json::Value {
        serde_json::json!({
            "clientId": 1,
            "issueDate": "2026-04-01",
            "dueDate": "2026-05-01",
            "items": [
                { "description": "Consulting: April", "quantity": 10.0, "unitAmount": 150.0 },
                { "description": "Hosting", "quantity": 1.0, "unitAmount": 50.0 },
            ],
            "notes": "Thanks",
            "terms": "Net 30",
        })
    }

    #[tokio::test]
    async fn an_invoice_can_be_created_with_line_items() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let next = ok_json(&app, "/api/invoices/next-number", &token).await;
        let (status, created) = post_json(&app, "/api/invoices", &token, &new_invoice()).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");

        assert_eq!(created["number"], next["number"]);
        assert_eq!(created["status"], "draft");
        assert_eq!(created["total"], 1550.0);
        assert_eq!(created["subtotal"], 1550.0);
        assert_eq!(created["currency"], "USD", "the default");
        assert_eq!(created["client"]["name"], "Acme Co");
        assert_eq!(created["items"].as_array().unwrap().len(), 2);
        assert_eq!(created["items"][0]["lineTotal"], 1500.0);
        assert_eq!(created["notes"], "Thanks");
        assert_eq!(created["terms"], "Net 30");
        assert_eq!(created["paid"], 0.0);
        assert_eq!(created["balance"], 1550.0);
        // A draft has been published nowhere, so there is no address for it.
        assert!(created["publicUrl"].is_null(), "{created}");
        assert!(created.get("token").is_none(), "token leaked: {created}");
        assert_eq!(created["canEdit"], true);

        // And the counter moved, so the next draft gets the next number.
        let after = ok_json(&app, "/api/invoices/next-number", &token).await;
        assert_eq!(
            after["number"].as_i64().unwrap(),
            next["number"].as_i64().unwrap() + 1
        );
    }

    /// The CLI's `desc:qty:unit` splitting is an argv artifact; JSON has no such
    /// ambiguity, so a description reads as a description.
    #[tokio::test]
    async fn a_line_item_description_may_contain_a_colon() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (_, created) = post_json(&app, "/api/invoices", &token, &new_invoice()).await;
        assert_eq!(created["items"][0]["description"], "Consulting: April");
    }

    #[tokio::test]
    async fn a_malformed_invoice_is_a_400_before_anything_is_written() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let cases: [(&str, serde_json::Value); 5] = [
            ("no items", serde_json::json!({ "items": [] })),
            (
                "a zero total",
                serde_json::json!({ "items": [
                    { "description": "Work", "quantity": 0.0, "unitAmount": 150.0 }
                ] }),
            ),
            (
                "a malformed issue date",
                serde_json::json!({ "issueDate": "2026-4-1" }),
            ),
            (
                "a malformed due date",
                serde_json::json!({ "dueDate": "April" }),
            ),
            (
                "a bad currency",
                serde_json::json!({ "currency": "dollars" }),
            ),
        ];

        for (what, overrides) in cases {
            let mut body = new_invoice();
            for (key, value) in overrides.as_object().unwrap() {
                body[key] = value.clone();
            }
            let (status, json) = post_json(&app, "/api/invoices", &token, &body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {json}");
        }

        // A quantity that overflows an f64 — JSON cannot spell NaN, and this is
        // how a non-finite figure actually arrives — never reaches a row, one
        // way or the other: either serde refuses the literal or `checked_items`
        // does. Sent as raw text because the literal will not compile in Rust.
        let body = r#"{"clientId":1,"issueDate":"2026-04-01",
            "items":[{"description":"Work","quantity":1e400,"unitAmount":10.0}]}"#;
        let (status, json) = send(
            &app,
            session_request("POST", "/api/invoices", &token, Some(body)),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");

        // Nothing above reserved a number.
        let next = ok_json(&app, "/api/invoices/next-number", &token).await;
        assert_eq!(next["number"], 1253);
    }

    /// The unit half of the same rule, where a non-finite figure can actually
    /// be constructed.
    #[test]
    fn checked_items_refuses_an_empty_list_a_non_finite_figure_and_a_zero_total() {
        use super::checked_items;

        let line = |quantity: f64, unit_amount: f64| {
            vec![crate::invoicing::invoices::NewLineItem {
                description: "Work".into(),
                quantity,
                unit_amount,
            }]
        };

        assert!(checked_items(vec![]).is_err());
        assert!(checked_items(line(f64::NAN, 10.0)).is_err());
        assert!(checked_items(line(1.0, f64::INFINITY)).is_err());
        assert!(checked_items(line(0.0, 150.0)).is_err(), "a zero total");
        assert!(
            checked_items(line(-1.0, 150.0)).is_err(),
            "a negative total"
        );
        assert!(checked_items(line(2.0, 150.0)).is_ok());
    }

    #[tokio::test]
    async fn creating_an_invoice_for_an_unknown_client_is_a_404() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let mut body = new_invoice();
        body["clientId"] = serde_json::json!(999999);
        let (status, json) = post_json(&app, "/api/invoices", &token, &body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
        assert_eq!(json["error"]["details"]["reason"], "client_not_found");
    }

    #[tokio::test]
    async fn patching_items_replaces_the_whole_list_and_recomputes_the_total() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // 1252 is the seeded draft: one line at 2,400.
        let (status, patched) = patch_json(
            &app,
            "/api/invoices/1252",
            &token,
            &serde_json::json!({ "items": [
                { "description": "Brand refresh - deposit", "quantity": 1.0, "unitAmount": 3000.0 },
                { "description": "Rush fee", "quantity": 1.0, "unitAmount": 500.0 },
            ] }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{patched}");

        let items = patched["items"].as_array().expect("items");
        assert_eq!(items.len(), 2, "replaced, not appended: {patched}");
        assert_eq!(items[0]["position"], 0);
        assert_eq!(items[1]["position"], 1);
        assert_eq!(patched["subtotal"], 3500.0);
        assert_eq!(patched["total"], 3500.0);
        assert_eq!(patched["balance"], 3500.0);
    }

    /// A link is priced in the amount it was created with, so an edit that
    /// moves the total leaves it billing the wrong figure.
    #[tokio::test]
    async fn editing_the_total_clears_a_stale_stripe_payment_link() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = crate::db::open_connection(&db_path, None).expect("open db");
        let draft = crate::invoicing::invoices::get_invoice_by_number(&conn, 1252).unwrap();
        crate::invoicing::invoices::set_payment_link(
            &conn,
            draft.id,
            "plink_seed_1252",
            "https://buy.stripe.com/test_seed_1252",
        )
        .unwrap();
        drop(conn);

        let (app, token) = app_for(&db_path);
        let before = ok_json(&app, "/api/invoices/1252", &token).await;
        assert_eq!(
            before["stripePaymentLinkUrl"],
            "https://buy.stripe.com/test_seed_1252"
        );

        let (status, patched) = patch_json(
            &app,
            "/api/invoices/1252",
            &token,
            &serde_json::json!({ "items": [
                { "description": "Brand refresh - deposit", "quantity": 1.0, "unitAmount": 999.0 }
            ] }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{patched}");
        assert!(
            patched["stripePaymentLinkUrl"].is_null(),
            "a link billing 2,400 must not survive an edit to 999: {patched}"
        );
        assert!(patched["stripePaymentLinkId"].is_null(), "{patched}");
    }

    #[tokio::test]
    async fn a_patch_can_clear_a_due_date_and_omitting_it_leaves_it() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, dated) = patch_json(
            &app,
            "/api/invoices/1252",
            &token,
            &serde_json::json!({ "dueDate": "2026-04-12" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{dated}");
        assert_eq!(dated["dueDate"], "2026-04-12");

        // Absent leaves it.
        let (_, renoted) = patch_json(
            &app,
            "/api/invoices/1252",
            &token,
            &serde_json::json!({ "notes": "Deposit only" }),
        )
        .await;
        assert_eq!(renoted["dueDate"], "2026-04-12");
        assert_eq!(renoted["notes"], "Deposit only");

        // Null clears it, so the invoice can never go overdue.
        let (_, cleared) = patch_json(
            &app,
            "/api/invoices/1252",
            &token,
            &serde_json::json!({ "dueDate": null }),
        )
        .await;
        assert!(cleared["dueDate"].is_null(), "{cleared}");
        assert_eq!(cleared["notes"], "Deposit only");
    }

    #[tokio::test]
    async fn an_all_absent_invoice_patch_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) =
            patch_json(&app, "/api/invoices/1252", &token, &serde_json::json!({})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    #[tokio::test]
    async fn patching_an_unknown_invoice_is_a_404_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = patch_json(
            &app,
            "/api/invoices/9999",
            &token,
            &serde_json::json!({ "notes": "hello" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "invoice_not_found");
    }

    #[tokio::test]
    async fn patching_a_published_invoice_is_a_409_naming_its_status() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = patch_json(
            &app,
            "/api/invoices/1251",
            &token,
            &serde_json::json!({ "notes": "too late" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "not_draft");
        assert_eq!(body["error"]["details"]["status"], "sent");
    }

    #[tokio::test]
    async fn patching_a_void_invoice_is_a_409_void() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = patch_json(
            &app,
            "/api/invoices/1247",
            &token,
            &serde_json::json!({ "notes": "cancelled" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "void");
    }

    /// An edit is blocked by recorded payments as well as by status, which is
    /// why `canEdit` is the guard called and not a status comparison.
    #[tokio::test]
    async fn patching_a_draft_that_has_payments_is_a_409_has_payments() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // A payment against an unpublished draft leaves it a draft.
        let (status, paid) = post_json(
            &app,
            "/api/invoices/1252/pay",
            &token,
            &serde_json::json!({ "amount": 400.0, "date": "2026-03-14" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{paid}");
        assert_eq!(paid["status"], "draft");
        assert_eq!(paid["canEdit"], false);

        let (status, body) = patch_json(
            &app,
            "/api/invoices/1252",
            &token,
            &serde_json::json!({ "notes": "too late" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "has_payments");
        assert_eq!(body["error"]["details"]["paid"], 400.0);
        assert_eq!(body["error"]["details"]["total"], 2400.0);
    }

    // -----------------------------------------------------------------------
    // Void and pay
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn voiding_an_invoice_makes_it_refuse_send_and_pay() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, voided) = post_json(
            &app,
            "/api/invoices/1252/void",
            &token,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{voided}");
        assert_eq!(voided["status"], "void");
        assert!(voided["voidedAt"].as_str().is_some(), "{voided}");
        for flag in ["canEdit", "canSend", "canVoid", "canPay"] {
            assert_eq!(voided[flag], false, "{flag} after a void: {voided}");
        }

        let (status, body) = post_json(
            &app,
            "/api/invoices/1252/pay",
            &token,
            &serde_json::json!({ "amount": 100.0, "date": "2026-03-20" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "void");
    }

    #[tokio::test]
    async fn voiding_a_void_invoice_is_a_409_already_void() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/invoices/1247/void",
            &token,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "already_void");
    }

    #[tokio::test]
    async fn voiding_a_paid_invoice_is_a_409_carrying_paid_and_total() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/invoices/1248/void",
            &token,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "has_payments");
        assert_eq!(body["error"]["details"]["paid"], 4000.0);
        assert_eq!(body["error"]["details"]["total"], 4000.0);
    }

    #[tokio::test]
    async fn voiding_an_unknown_invoice_is_a_404_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for uri in ["/api/invoices/9999/void", "/api/invoices/9999/pay"] {
            let (status, body) = post_json(
                &app,
                uri,
                &token,
                &serde_json::json!({ "date": "2026-03-20" }),
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
            assert_eq!(body["error"]["details"]["reason"], "invoice_not_found");
        }
    }

    #[tokio::test]
    async fn a_payment_defaults_to_the_whole_outstanding_balance() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // 1250: 3,200 total, 2,000 already paid.
        let (status, paid) = post_json(
            &app,
            "/api/invoices/1250/pay",
            &token,
            &serde_json::json!({ "date": "2026-03-14" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{paid}");
        assert_eq!(paid["paid"], 3200.0);
        assert_eq!(paid["balance"], 0.0);
        assert_eq!(paid["status"], "paid");
        assert_eq!(paid["canPay"], false);

        let payments = paid["payments"].as_array().expect("payments");
        assert_eq!(payments.len(), 2);
        assert_eq!(payments[1]["amount"], 1200.0);
        assert_eq!(payments[1]["method"], "direct_deposit", "the default");
        assert_eq!(payments[1]["paidDate"], "2026-03-14");
    }

    #[tokio::test]
    async fn a_partial_payment_moves_the_status_to_partial_and_a_full_one_to_paid() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // 1251: sent, 1,850 outstanding.
        let (status, partial) = post_json(
            &app,
            "/api/invoices/1251/pay",
            &token,
            &serde_json::json!({ "amount": 500.0, "date": "2026-03-14", "method": "ach" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{partial}");
        assert_eq!(partial["status"], "partial");
        assert_eq!(partial["balance"], 1350.0);
        assert_eq!(partial["payments"][0]["method"], "ach");

        let (_, settled) = post_json(
            &app,
            "/api/invoices/1251/pay",
            &token,
            &serde_json::json!({ "amount": 1350.0, "date": "2026-03-15" }),
        )
        .await;
        assert_eq!(settled["status"], "paid");
        assert_eq!(settled["balance"], 0.0);
    }

    #[tokio::test]
    async fn paying_with_nothing_outstanding_is_a_409_no_balance() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/invoices/1248/pay",
            &token,
            &serde_json::json!({ "date": "2026-03-14" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "no_balance");
        assert_eq!(body["error"]["details"]["total"], 4000.0);
        assert_eq!(body["error"]["details"]["paid"], 4000.0);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("no outstanding balance"),
            "{body}"
        );
    }

    /// Not a rusqlite CHECK violation surfacing as a 500.
    #[tokio::test]
    async fn an_unknown_payment_method_is_a_400_naming_the_legal_set() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/invoices/1251/pay",
            &token,
            &serde_json::json!({ "date": "2026-03-14", "method": "bitcoin" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let message = body["error"]["message"].as_str().unwrap();
        for method in crate::invoicing::invoices::PAYMENT_METHODS {
            assert!(message.contains(method), "{method} missing from {message}");
        }
    }

    #[tokio::test]
    async fn a_bad_payment_amount_or_date_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let cases = [
            serde_json::json!({ "amount": -5.0, "date": "2026-03-14" }),
            serde_json::json!({ "amount": 0.0, "date": "2026-03-14" }),
            serde_json::json!({ "date": "2026-3-14" }),
            serde_json::json!({ "date": "March" }),
        ];

        for body in cases {
            let (status, json) = post_json(&app, "/api/invoices/1251/pay", &token, &body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {json}");
        }

        // An amount that overflows an f64: `payment_amount`'s NaN rejection is
        // a negated positive test for exactly this, since a non-finite row
        // poisons every later SUM.
        let (status, json) = send(
            &app,
            session_request(
                "POST",
                "/api/invoices/1251/pay",
                &token,
                Some(r#"{"amount":1e400,"date":"2026-03-14"}"#),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");

        // None of them recorded anything.
        let invoice = ok_json(&app, "/api/invoices/1251", &token).await;
        assert_eq!(invoice["paid"], 0.0);
    }

    /// A due-date patch can flip a derived status, so a body that echoed only
    /// the field that was sent would be showing the old one.
    #[tokio::test]
    async fn every_invoice_write_answers_with_the_whole_detail() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let writes: [(&str, serde_json::Value); 3] = [
            (
                "/api/invoices/1252",
                serde_json::json!({ "notes": "Deposit only" }),
            ),
            (
                "/api/invoices/1251/pay",
                serde_json::json!({ "date": AS_OF }),
            ),
            ("/api/invoices/1252/void", serde_json::json!({})),
        ];

        for (uri, body) in writes {
            let (status, json) = if uri.ends_with("1252") {
                patch_json(&app, uri, &token, &body).await
            } else {
                post_json(&app, uri, &token, &body).await
            };
            assert_eq!(status, StatusCode::OK, "{uri}: {json}");
            for key in [
                "number",
                "status",
                "client",
                "items",
                "payments",
                "paid",
                "balance",
                "publicUrl",
                "canEdit",
                "canSend",
                "canVoid",
                "canPay",
            ] {
                assert!(
                    json.get(key).is_some(),
                    "{uri} answered without {key}: {json}"
                );
            }
            assert!(
                json.get("token").is_none(),
                "{uri} leaked the token: {json}"
            );
        }
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
