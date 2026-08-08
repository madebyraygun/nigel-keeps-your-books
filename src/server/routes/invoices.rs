//! Invoices: the list, one invoice in full, the A/R aging report, the number
//! the next draft will get, and the writes — create, edit, void, pay, send and
//! sync.
//!
//! Every write answers the whole refreshed detail, because the status is
//! derived rather than set and almost every write moves it.
//!
//! Send and sync are the two that leave the machine. Both take their
//! collaborators as the `PaymentGateway`/`AssetPublisher`/`Mailer` traits —
//! `send_with` and `sync_with` are the seams — so the whole orchestration is
//! exercised with fakes and no test in this file can reach the network. Only
//! the two handlers build the real clients, from `invoicing_config()`.
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
use crate::invoicing::gateway::{AssetPublisher, Mailer, PaymentGateway};
use crate::invoicing::invoices::{
    self as inv, AgingReport, InvoiceListRow, InvoiceUpdate, NewLineItem,
};
use crate::invoicing::render::{render_invoice, RenderedInvoice};
use crate::invoicing::render_html::{load_template, Branding};
use crate::invoicing::send::{send_invoice_traced, SendFailure, SendStep, StepOutcome};
use crate::invoicing::sync::{sync_all_report, SyncReport};
use crate::models::{Client, Invoice, InvoiceLineItem, InvoicePayment};

use super::super::error::{ApiError, ApiErrorCode, ApiResult};
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
        .route("/invoices/sync", post(sync))
        .route("/invoices/{number}", get(detail).patch(update))
        .route("/invoices/{number}/send", post(send))
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
#[derive(Debug, Serialize)]
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

/// A new draft. The client, the line items, the dates and the currency are all
/// `create_invoice`'s own checks; the number comes from the counter it advances
/// in the same transaction, so a refused create reserves nothing.
async fn create(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewInvoiceRequest>,
) -> ApiResult<(StatusCode, Json<InvoiceDetail>)> {
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
            &new.items,
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
    /// Field for field into the data layer's update struct, with only the date
    /// shape checked on the way through — `update_invoice` validates the line
    /// items itself, so the CLI and the API refuse the same ones.
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
            items: self.items,
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
// Send and sync
// ---------------------------------------------------------------------------

/// The whole body of a send request.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendRequest {
    /// Absent or `false` refuses the request and sends nothing.
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendStepResult {
    step: SendStep,
    outcome: StepOutcome,
}

/// What a completed send answers with: the refreshed invoice, because the
/// status has just moved, and the trace, because "done" is not what a screen
/// wants to say about an operation with seven steps and three third parties in
/// it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendResult {
    invoice: InvoiceDetail,
    public_url: String,
    payment_link_url: Option<String>,
    steps: Vec<SendStepResult>,
}

/// The refusal a request that never named the invoicing settings gets.
///
/// Key names only — the values never leave the process, and the names are
/// already public in `docs/invoicing.md`.
fn not_configured(what: &str, missing: &[&'static str]) -> ApiError {
    ApiError::conflict(
        format!(
            "{what} is not configured: missing {} (set each one in settings.json or the matching NIGEL_ env var)",
            missing.join(", ")
        ),
        serde_json::json!({
            "reason": "send_not_configured",
            "step": SendStep::Config.as_str(),
            "missing": missing,
        }),
    )
}

/// `POST /api/invoices/{number}/send` — the whole publish, inside the request.
///
/// The `confirm` flag is checked first and nothing else happens without it:
/// "send requires explicit confirmation" is a property of the endpoint rather
/// than a convention of whichever screen calls it, so an accidental `curl` is a
/// no-op instead of an invoice in a client's inbox.
///
/// It blocks rather than queueing because there is nothing a job registry would
/// hold that the invoice row does not: `published_at` and
/// `stripe_payment_link_url` are the durable state, and two sources of truth for
/// "did this go out" is how they drift. The gateway clients are synchronous
/// `reqwest::blocking` and `with_conn_api` is already `spawn_blocking`, so the
/// work lands on that pool either way; what makes the open request bounded is
/// `invoicing::REQUEST_TIMEOUT` on each of the three calls.
async fn send(
    State(state): State<AppState>,
    ApiPath(number): ApiPath<i64>,
    ApiJson(request): ApiJson<SendRequest>,
) -> ApiResult<Json<SendResult>> {
    if !request.confirm {
        return Err(ApiError::bad_request(
            "Sending an invoice requires an explicit confirmation: post {\"confirm\": true}.",
        )
        .with_details(serde_json::json!({ "reason": "confirmation_required" })));
    }

    let config = crate::settings::invoicing_config();
    let status = crate::settings::invoicing_status(&config);
    if !status.send_configured {
        return Err(not_configured("Sending invoices", &status.missing));
    }
    // Every one of the nine keys is present, so this cannot fail — it is the
    // same constructor `nigel invoice send` uses, kept rather than reimplemented
    // so the two front ends build the same clients.
    let (stripe, r2, mail) = crate::cli::invoice::build_clients(config)?;
    let contact_email = mail.from.clone();
    let today = crate::cli::today();

    let result = with_conn_api(&state, {
        let state = state.clone();
        move |conn| {
            send_with(
                conn,
                &state.data_dir(),
                number,
                &today,
                &contact_email,
                &stripe,
                &r2,
                &mail,
            )
        }
    })
    .await?;
    Ok(Json(result))
}

/// The send with its three collaborators passed in, which is what makes the
/// orchestration testable without a network: the handler resolves the settings
/// and builds the real clients, and everything below this line takes traits.
#[allow(clippy::too_many_arguments)]
fn send_with<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
    conn: &Connection,
    data_dir: &std::path::Path,
    number: i64,
    today: &str,
    contact_email: &str,
    gateway: &G,
    publisher: &P,
    mailer: &M,
) -> ApiResult<SendResult> {
    let invoice = find_invoice(conn, number)?;
    // Refused before the template is read and before any client is called, the
    // way `nigel invoice send` refuses it.
    inv::ensure_not_void(&invoice, "sent")?;
    // Loaded before the Stripe link, so a broken override costs no link, no
    // upload and no email.
    let template = load_template(data_dir)?;
    let company = crate::cli::invoice::company_name(conn);
    let branding = Branding {
        template: &template,
        company: &company,
        contact_email,
    };

    let outcome = send_invoice_traced(
        conn, invoice.id, today, &branding, gateway, publisher, mailer,
    )
    .map_err(|failure| send_error(conn, &invoice, failure))?;

    Ok(SendResult {
        invoice: detail_for(conn, find_invoice(conn, number)?)?,
        public_url: outcome.public_url,
        payment_link_url: outcome.payment_link_url,
        steps: outcome
            .steps
            .into_iter()
            .map(|(step, outcome)| SendStepResult { step, outcome })
            .collect(),
    })
}

/// The step-to-status mapping is `From<SendFailure>`'s; this adds the one piece
/// of context only the route holds — which client needs an email address —
/// because that refusal is a link to a form, not a sentence to read.
fn send_error(conn: &Connection, invoice: &Invoice, failure: SendFailure) -> ApiError {
    if matches!(
        failure.source,
        NigelError::Conflict {
            code: "client_missing_email",
            ..
        }
    ) {
        if let Ok(client) = get_client(conn, invoice.client_id) {
            return ApiError::conflict(
                failure.source.to_string(),
                serde_json::json!({
                    "reason": "client_missing_email",
                    "step": failure.step.as_str(),
                    "clientId": client.id,
                    "clientName": client.name,
                }),
            );
        }
    }
    ApiError::from(failure)
}

/// `POST /api/invoices/sync` — pull Stripe payments and record them.
///
/// No confirmation: the run is idempotent by checkout session id (it already
/// happens at every CLI launch), and it writes only payments Stripe says were
/// taken.
async fn sync(State(state): State<AppState>) -> ApiResult<Json<SyncReport>> {
    let Some(secret_key) = crate::settings::invoicing_config().stripe_secret_key else {
        return Err(not_configured("Payment sync", &["stripe_secret_key"]));
    };
    let gateway = crate::invoicing::stripe::StripeClient { secret_key };
    let today = crate::cli::today();

    let report = with_conn_api(&state, move |conn| sync_with(conn, &today, &gateway)).await?;
    Ok(Json(report))
}

/// Sync with the gateway passed in, for the same reason [`send_with`] takes
/// three: a test drives the whole route with fakes and no network.
///
/// Per-invoice failures ride back in the report — one deleted payment link must
/// not hide the payments the run did record — and only a run where every
/// invoice failed is an error.
fn sync_with<G: PaymentGateway>(
    conn: &Connection,
    today: &str,
    gateway: &G,
) -> ApiResult<SyncReport> {
    sync_all_report(conn, today, gateway).map_err(|err| match err {
        // A failure reading the books is ours, not Stripe's.
        NigelError::Db(_) => ApiError::from(err),
        other => ApiError::new(ApiErrorCode::UpstreamFailed, other.to_string())
            .with_details(serde_json::json!({ "service": "stripe" })),
    })
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
    /// with a 400 rather than answering the report. `/invoices/sync` is the
    /// same shape and has a test of its own beside the send cases.
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

        // Three ways a non-finite total arrives over HTTP. JSON cannot spell
        // NaN, but a literal can overflow, two finite factors can multiply past
        // f64::MAX, and two finite line totals can sum past it — any of which
        // would be stored as the total and serialized as `null` against a field
        // a client has typed as a number. Sent as raw text because `1e400` will
        // not compile as a Rust literal.
        let overflowing_literal = r#"{"clientId":1,"issueDate":"2026-04-01",
            "items":[{"description":"Work","quantity":1e400,"unitAmount":10.0}]}"#;
        let overflowing_product = r#"{"clientId":1,"issueDate":"2026-04-01",
            "items":[{"description":"Work","quantity":1e308,"unitAmount":1e308}]}"#;
        let overflowing_sum = r#"{"clientId":1,"issueDate":"2026-04-01","items":[
            {"description":"Big","quantity":1e154,"unitAmount":1e154},
            {"description":"Big","quantity":1e154,"unitAmount":1e154}]}"#;

        for body in [overflowing_literal, overflowing_product, overflowing_sum] {
            let (status, json) = send(
                &app,
                session_request("POST", "/api/invoices", &token, Some(body)),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {json}");
        }

        // Nothing above reserved a number.
        let next = ok_json(&app, "/api/invoices/next-number", &token).await;
        assert_eq!(next["number"], 1253);
    }

    /// The same rule on the edit half: `items` is a whole-list replacement, so
    /// it recomputes the total from whatever it is given.
    #[tokio::test]
    async fn patching_items_with_an_overflowing_product_is_a_400() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = r#"{"items":[{"description":"Work","quantity":1e308,"unitAmount":1e308}]}"#;
        let (status, json) = send(
            &app,
            session_request("PATCH", "/api/invoices/1252", &token, Some(body)),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");

        // The draft still totals what it did, and its total is still a number.
        let draft = ok_json(&app, "/api/invoices/1252", &token).await;
        assert_eq!(draft["total"], 2400.0);
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

    // -----------------------------------------------------------------------
    // Send and sync
    // -----------------------------------------------------------------------

    // The two seams the fakes go through, taken by name: everything else in
    // this module is exercised over HTTP.
    use super::{send_with, sync_with};
    use crate::error::{NigelError, Result as NigelResult};
    use crate::invoicing::gateway::{
        AssetPublisher, Mailer, PaidSession, PaymentGateway, PaymentLink,
    };
    use crate::models::{Client, Invoice};
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeGw {
        create_calls: RefCell<u32>,
    }
    impl PaymentGateway for FakeGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> NigelResult<PaymentLink> {
            *self.create_calls.borrow_mut() += 1;
            Ok(PaymentLink {
                id: "plink_fake".into(),
                url: "https://buy.stripe.com/fake".into(),
            })
        }
        fn paid_sessions(&self, _id: &str) -> NigelResult<Vec<PaidSession>> {
            Ok(vec![])
        }
    }

    struct FakePub;
    impl AssetPublisher for FakePub {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> NigelResult<String> {
            Ok(format!("https://billing.example.test/i/{token}/"))
        }
    }

    /// R2 refusing the way it does when the credentials are wrong.
    struct ForbiddenPub;
    impl AssetPublisher for ForbiddenPub {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> NigelResult<String> {
            Err(NigelError::Other(
                "r2 403: <Error><Code>SignatureDoesNotMatch</Code></Error>".into(),
            ))
        }
    }

    #[derive(Default)]
    struct FakeMail {
        sent: RefCell<u32>,
    }
    impl Mailer for FakeMail {
        fn send_invoice(&self, _t: &str, _s: &str, _h: &str, _p: &[u8]) -> NigelResult<()> {
            *self.sent.borrow_mut() += 1;
            Ok(())
        }
    }

    struct BrokenLinkGw;
    impl PaymentGateway for BrokenLinkGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> NigelResult<PaymentLink> {
            unreachable!("no link is created for these invoices")
        }
        fn paid_sessions(&self, id: &str) -> NigelResult<Vec<PaidSession>> {
            Err(NigelError::Other(format!(
                "stripe 404: no such payment link {id}"
            )))
        }
    }

    struct PayingGw;
    impl PaymentGateway for PayingGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> NigelResult<PaymentLink> {
            unreachable!("no link is created for these invoices")
        }
        fn paid_sessions(&self, id: &str) -> NigelResult<Vec<PaidSession>> {
            Ok(vec![PaidSession {
                session_id: format!("cs_{id}"),
                amount: 100.0,
            }])
        }
    }

    fn open_db(db_path: &std::path::Path) -> rusqlite::Connection {
        crate::db::open_connection(db_path, None).expect("open db")
    }

    /// Render an `ApiError` the way the router would, so a test asserts on the
    /// wire form rather than on the struct behind it.
    async fn error_json(err: super::ApiError) -> (StatusCode, serde_json::Value) {
        use axum::response::IntoResponse;
        let response = err.into_response();
        let status = response.status();
        (status, json_body(response).await)
    }

    #[tokio::test]
    async fn send_without_confirmation_is_a_400_and_sends_nothing() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for body in [
            serde_json::json!({}),
            serde_json::json!({ "confirm": false }),
        ] {
            let (status, json) = post_json(&app, "/api/invoices/1252/send", &token, &body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {json}");
            assert_eq!(json["error"]["details"]["reason"], "confirmation_required");
        }

        // Refused before the settings are even read, so nothing was published.
        let invoice = ok_json(&app, "/api/invoices/1252", &token).await;
        assert_eq!(invoice["status"], "draft");
        assert!(invoice["publishedAt"].is_null(), "{invoice}");
    }

    #[tokio::test]
    async fn send_with_no_invoicing_config_is_a_409_naming_the_missing_keys() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, json) = post_json(
            &app,
            "/api/invoices/1252/send",
            &token,
            &serde_json::json!({ "confirm": true }),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        assert_eq!(json["error"]["details"]["reason"], "send_not_configured");
        assert_eq!(json["error"]["details"]["step"], "config");
        let missing = json["error"]["details"]["missing"]
            .as_array()
            .expect("the missing key names");
        assert_eq!(missing.len(), 9, "{json}");
        assert!(missing.contains(&serde_json::json!("r2_bucket")), "{json}");
        assert!(
            missing.contains(&serde_json::json!("public_base_url")),
            "{json}"
        );
    }

    #[tokio::test]
    async fn sync_with_no_stripe_key_is_a_409_naming_it() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, json) =
            post_json(&app, "/api/invoices/sync", &token, &serde_json::json!({})).await;
        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        assert_eq!(json["error"]["details"]["reason"], "send_not_configured");
        assert_eq!(
            json["error"]["details"]["missing"],
            serde_json::json!(["stripe_secret_key"])
        );
    }

    /// `sync` is a literal beside `{number}`, and would otherwise be read as an
    /// invoice number — which `ApiPath<i64>` refuses with a 400.
    #[tokio::test]
    async fn the_sync_path_is_not_parsed_as_an_invoice_number() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, json) =
            post_json(&app, "/api/invoices/sync", &token, &serde_json::json!({})).await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "reached the sync handler: {json}"
        );
    }

    #[tokio::test]
    async fn sending_a_void_invoice_is_a_409_before_any_network_call() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);
        let gateway = FakeGw::default();
        let mailer = FakeMail::default();

        let err = send_with(
            &conn,
            db_path.parent().unwrap(),
            1247,
            AS_OF,
            "billing@example.test",
            &gateway,
            &FakePub,
            &mailer,
        )
        .expect_err("a void invoice cannot be sent");

        let (status, json) = error_json(err).await;
        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        assert_eq!(json["error"]["details"]["reason"], "void");
        assert_eq!(*gateway.create_calls.borrow(), 0);
        assert_eq!(*mailer.sent.borrow(), 0);
    }

    #[tokio::test]
    async fn sending_to_a_client_with_no_email_is_a_409_naming_the_client() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);
        let gateway = FakeGw::default();
        let mailer = FakeMail::default();

        // 1249 belongs to Globex, which has no email address.
        let err = send_with(
            &conn,
            db_path.parent().unwrap(),
            1249,
            AS_OF,
            "billing@example.test",
            &gateway,
            &FakePub,
            &mailer,
        )
        .expect_err("nowhere to send it");

        let (status, json) = error_json(err).await;
        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        assert_eq!(json["error"]["details"]["reason"], "client_missing_email");
        assert_eq!(json["error"]["details"]["step"], "precheck");
        assert_eq!(json["error"]["details"]["clientName"], "Globex");
        assert!(json["error"]["details"]["clientId"].is_i64(), "{json}");
        assert_eq!(*gateway.create_calls.borrow(), 0, "no link was created");
    }

    #[tokio::test]
    async fn sending_an_unknown_invoice_is_a_404_with_a_reason() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);

        let err = send_with(
            &conn,
            db_path.parent().unwrap(),
            9999,
            AS_OF,
            "billing@example.test",
            &FakeGw::default(),
            &FakePub,
            &FakeMail::default(),
        )
        .expect_err("no such invoice");

        let (status, json) = error_json(err).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
        assert_eq!(json["error"]["details"]["reason"], "invoice_not_found");
    }

    #[cfg(feature = "pdf")]
    #[tokio::test]
    async fn a_publish_failure_is_a_502_naming_the_step_and_the_service() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);
        let mailer = FakeMail::default();

        let err = send_with(
            &conn,
            db_path.parent().unwrap(),
            1252,
            AS_OF,
            "billing@example.test",
            &FakeGw::default(),
            &ForbiddenPub,
            &mailer,
        )
        .expect_err("R2 refused");

        let (status, json) = error_json(err).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{json}");
        assert_eq!(json["error"]["code"], "upstream_failed");
        assert_eq!(json["error"]["details"]["reason"], "send_failed");
        assert_eq!(json["error"]["details"]["step"], "publish");
        assert_eq!(json["error"]["details"]["service"], "r2");
        assert_eq!(json["error"]["details"]["emailSent"], false);
        assert_eq!(json["error"]["details"]["invoiceStatus"], "draft");
        assert_eq!(
            json["error"]["details"]["completed"],
            serde_json::json!(["load", "precheck", "payment_link", "render"])
        );
        // R2's own words, so the operator has something to search for.
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("SignatureDoesNotMatch"),
            "{json}"
        );
        assert_eq!(*mailer.sent.borrow(), 0, "no email went out");

        // And the invoice is still a draft, so a retry is the whole fix.
        assert_eq!(
            crate::invoicing::invoices::get_invoice_by_number(&conn, 1252)
                .unwrap()
                .status,
            "draft"
        );
    }

    #[cfg(not(feature = "pdf"))]
    #[tokio::test]
    async fn a_send_without_the_pdf_feature_is_a_501_at_the_render_step() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);

        let err = send_with(
            &conn,
            db_path.parent().unwrap(),
            1252,
            AS_OF,
            "billing@example.test",
            &FakeGw::default(),
            &FakePub,
            &FakeMail::default(),
        )
        .expect_err("nothing to attach");

        let (status, json) = error_json(err).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{json}");
        assert_eq!(json["error"]["code"], "feature_disabled");
        assert_eq!(json["error"]["details"]["step"], "render");
        assert_eq!(json["error"]["details"]["emailSent"], false);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn a_successful_send_answers_with_the_step_trace_and_the_refreshed_invoice() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);
        let gateway = FakeGw::default();
        let mailer = FakeMail::default();

        let result = send_with(
            &conn,
            db_path.parent().unwrap(),
            1252,
            AS_OF,
            "billing@example.test",
            &gateway,
            &FakePub,
            &mailer,
        )
        .expect("sends");

        let json = serde_json::to_value(&result).expect("serializes");
        assert_eq!(
            json["steps"],
            serde_json::json!([
                { "step": "load", "outcome": "ok" },
                { "step": "precheck", "outcome": "ok" },
                { "step": "payment_link", "outcome": "ok" },
                { "step": "render", "outcome": "ok" },
                { "step": "publish", "outcome": "ok" },
                { "step": "email", "outcome": "ok" },
                { "step": "record", "outcome": "ok" },
            ])
        );
        assert!(json["publicUrl"]
            .as_str()
            .unwrap()
            .starts_with("https://billing.example.test/i/"));
        assert_eq!(json["paymentLinkUrl"], "https://buy.stripe.com/fake");

        // The refreshed detail: the status has just moved, and a body echoing
        // only what was sent would be showing the draft.
        assert_eq!(json["invoice"]["status"], "sent");
        assert_eq!(json["invoice"]["canEdit"], false);
        assert!(
            json["invoice"].get("token").is_none(),
            "token leaked: {json}"
        );
        assert_eq!(*mailer.sent.borrow(), 1);

        // A resend reuses the link the client was already given.
        let again = send_with(
            &conn,
            db_path.parent().unwrap(),
            1252,
            AS_OF,
            "billing@example.test",
            &gateway,
            &FakePub,
            &mailer,
        )
        .expect("resends");
        let again = serde_json::to_value(&again).expect("serializes");
        assert_eq!(again["steps"][2]["outcome"], "reused");
        assert_eq!(*gateway.create_calls.borrow(), 1);
    }

    #[test]
    fn sync_reports_recorded_checked_and_per_invoice_failures() {
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);

        // Only 1251 carries a payment link and is still open.
        let report = sync_with(&conn, AS_OF, &PayingGw).expect("syncs");
        let json = serde_json::to_value(&report).expect("serializes");
        assert_eq!(json["recorded"], 1);
        assert_eq!(json["invoicesChecked"], 1);
        assert_eq!(json["failures"], serde_json::json!([]));
    }

    /// A run where every invoice failed is the only failure a sync answers with
    /// — and it is the gateway's, not ours.
    #[tokio::test]
    async fn a_sync_that_failed_everywhere_is_a_502() {
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);

        let err = sync_with(&conn, AS_OF, &BrokenLinkGw).expect_err("stripe refused");
        let (status, json) = error_json(err).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{json}");
        assert_eq!(json["error"]["code"], "upstream_failed");
        assert_eq!(json["error"]["details"]["service"], "stripe");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("no such payment link"),
            "{json}"
        );
    }

    /// One bad payment link does not hide the payments the run did record.
    #[test]
    fn sync_carries_a_partial_failure_back_as_data() {
        let (_dir, db_path) = seeded_db();
        let conn = open_db(&db_path);
        // A second open invoice with a link Stripe will refuse.
        crate::invoicing::invoices::set_payment_link(
            &conn,
            crate::invoicing::invoices::get_invoice_by_number(&conn, 1250)
                .unwrap()
                .id,
            "plink_seed_1250",
            "https://buy.stripe.com/test_seed_1250",
        )
        .unwrap();

        struct OnlyOneWorks;
        impl PaymentGateway for OnlyOneWorks {
            fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> NigelResult<PaymentLink> {
                unreachable!()
            }
            fn paid_sessions(&self, id: &str) -> NigelResult<Vec<PaidSession>> {
                if id == "plink_seed_1250" {
                    return Err(NigelError::Other(format!(
                        "stripe 404: no such payment link {id}"
                    )));
                }
                Ok(vec![PaidSession {
                    session_id: format!("cs_{id}"),
                    amount: 100.0,
                }])
            }
        }

        let report = sync_with(&conn, AS_OF, &OnlyOneWorks).expect("a partial run still answers");
        let json = serde_json::to_value(&report).expect("serializes");
        assert_eq!(json["recorded"], 1);
        assert_eq!(json["invoicesChecked"], 2);
        assert_eq!(json["failures"][0]["number"], 1250);
        assert!(
            json["failures"][0]["message"]
                .as_str()
                .unwrap()
                .contains("no such payment link"),
            "{json}"
        );
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
