//! Report downloads under `/api/exports`.
//!
//! The renderers already had the right shape: `pdf::render_*` turn a report into
//! bytes and `cli::report::text::format_*` turn one into a string, neither
//! knowing where the result goes. So these handlers are the report handlers of
//! `routes::reports` with a different last step — same data functions, same
//! parameters, same validation — and the only new question is which of the two
//! renderers to call and what to name the file.

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rusqlite::Connection;
use serde::Deserialize;

use crate::cli::report::export_file_stem;
use crate::cli::report::text::{self, with_header};
use crate::db::get_metadata;
use crate::reports::{self, ReportKind};

use super::super::error::{ApiError, ApiResult};
use super::super::state::AppState;
use super::reports::{ParamSpec, RawQuery, ReportParams};
use super::{ensure_account_exists, with_conn_api};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/exports/pnl", get(pnl))
        .route("/exports/expenses", get(expenses))
        .route("/exports/tax", get(tax))
        .route("/exports/cashflow", get(cashflow))
        .route("/exports/balance", get(balance))
        .route("/exports/flagged", get(flagged))
        .route("/exports/register", get(register))
        .route("/exports/k1", get(k1))
}

// ---------------------------------------------------------------------------
// The request
// ---------------------------------------------------------------------------

/// The parameters an export adds to its report's own.
///
/// `month` is deliberately read a second time here — `ReportParams` keeps the
/// parsed month number, and the PDF's period label wants the string the caller
/// actually wrote.
#[derive(Debug, Deserialize)]
struct ExportOptions {
    format: Option<String>,
    month: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ExportFormat {
    Pdf,
    Text,
}

impl ExportFormat {
    /// No default: guessing would mean either handing back a text file to
    /// something that asked for a PDF, or answering `501` on a build without the
    /// feature to a caller who never mentioned PDFs.
    fn parse(value: Option<&str>) -> ApiResult<Self> {
        match value {
            Some("pdf") => Ok(Self::Pdf),
            Some("text") => Ok(Self::Text),
            Some(other) => Err(ApiError::bad_request(format!(
                "Unknown `format` \"{other}\". Expected 'pdf' or 'text'."
            ))),
            None => Err(ApiError::bad_request(
                "Missing `format`: expected 'pdf' or 'text'.",
            )),
        }
    }

    fn content_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Text => "text/plain; charset=utf-8",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Text => "txt",
        }
    }
}

struct ExportRequest {
    format: ExportFormat,
    params: ReportParams,
    month: Option<String>,
}

impl ExportRequest {
    fn parse(opts: ExportOptions, raw: RawQuery, kind: ReportKind) -> ApiResult<Self> {
        let format = ExportFormat::parse(opts.format.as_deref())?;
        let params = ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
        Ok(Self {
            format,
            params,
            month: opts.month,
        })
    }

    fn range(&self) -> String {
        reports::date_range_label(self.month.as_deref(), self.params.year)
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// A fetched report on its way to a renderer. The enum exists so the `pdf`
/// feature is answered in exactly one place instead of once per handler.
enum ReportPayload<'a> {
    Pnl(&'a reports::PnlReport),
    Expenses(&'a reports::ExpenseBreakdown),
    Tax(&'a reports::TaxSummary),
    Cashflow(&'a reports::CashflowReport),
    Register(&'a reports::RegisterReport),
    Flagged(&'a [reports::FlaggedTransaction]),
    Balance(&'a reports::BalanceReport),
    K1(&'a reports::K1PrepReport),
}

/// Render one report into the bytes the download carries. The company name is
/// read here rather than in each handler, on the connection the report was
/// fetched with.
fn render(
    conn: &Connection,
    payload: ReportPayload,
    request: &ExportRequest,
) -> ApiResult<Vec<u8>> {
    let company = get_metadata(conn, "company_name").unwrap_or_default();
    match request.format {
        ExportFormat::Text => Ok(with_header(&company, render_text(payload)).into_bytes()),
        ExportFormat::Pdf => render_pdf(payload, &company, &request.range()),
    }
}

fn render_text(payload: ReportPayload) -> String {
    match payload {
        ReportPayload::Pnl(report) => text::format_pnl(report),
        ReportPayload::Expenses(report) => text::format_expenses(report),
        ReportPayload::Tax(report) => text::format_tax(report),
        ReportPayload::Cashflow(report) => text::format_cashflow(report),
        ReportPayload::Register(report) => text::format_register(report),
        ReportPayload::Flagged(rows) => text::format_flagged(rows),
        ReportPayload::Balance(report) => text::format_balance(report),
        ReportPayload::K1(report) => text::format_k1(report),
    }
}

#[cfg(feature = "pdf")]
fn render_pdf(payload: ReportPayload, company: &str, range: &str) -> ApiResult<Vec<u8>> {
    let bytes = match payload {
        ReportPayload::Pnl(report) => crate::pdf::render_pnl(report, company, range)?,
        ReportPayload::Expenses(report) => crate::pdf::render_expenses(report, company, range)?,
        ReportPayload::Tax(report) => crate::pdf::render_tax(report, company, range)?,
        ReportPayload::Cashflow(report) => crate::pdf::render_cashflow(report, company, range)?,
        ReportPayload::Register(report) => crate::pdf::render_register(report, company, range)?,
        // These two describe their own period — "12 items", "As of today" — so
        // the label has nothing to tell them.
        ReportPayload::Flagged(rows) => crate::pdf::render_flagged(rows, company)?,
        ReportPayload::Balance(report) => crate::pdf::render_balance(report, company)?,
        ReportPayload::K1(report) => crate::pdf::render_k1(report, company, range)?,
    };
    Ok(bytes)
}

/// Without the feature there is no renderer to call, so the request is refused
/// with the same sentence `nigel report --mode export` prints.
#[cfg(not(feature = "pdf"))]
fn render_pdf(_payload: ReportPayload, _company: &str, _range: &str) -> ApiResult<Vec<u8>> {
    Err(ApiError::feature_disabled(
        crate::cli::report::PDF_DISABLED_MESSAGE,
    ))
}

/// Wrap rendered bytes as a download.
///
/// The filename matches what the CLI would have written for the same report
/// today, so a file saved from the browser and one written by
/// `nigel report … --mode export` are named alike. Nothing from the database
/// reaches the header — the slug is a fixed string and the date is digits — so
/// there is no quoting or RFC 5987 encoding to get wrong.
fn download(kind: ReportKind, format: ExportFormat, bytes: Vec<u8>) -> Response {
    let filename = format!("{}.{}", export_file_stem(kind.as_str()), format.extension());
    (
        [
            (header::CONTENT_TYPE, format.content_type().to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        Body::from(bytes),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn pnl(
    State(state): State<AppState>,
    Query(opts): Query<ExportOptions>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Response> {
    let kind = ReportKind::Pnl;
    let request = ExportRequest::parse(opts, raw, kind)?;
    let format = request.format;
    let bytes = with_conn_api(&state, move |conn| {
        let report = reports::get_pnl(
            conn,
            request.params.year,
            request.params.month,
            request.params.from.as_deref(),
            request.params.to.as_deref(),
        )?;
        render(conn, ReportPayload::Pnl(&report), &request)
    })
    .await?;
    Ok(download(kind, format, bytes))
}

async fn expenses(
    State(state): State<AppState>,
    Query(opts): Query<ExportOptions>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Response> {
    let kind = ReportKind::Expenses;
    let request = ExportRequest::parse(opts, raw, kind)?;
    let format = request.format;
    let bytes = with_conn_api(&state, move |conn| {
        let report =
            reports::get_expense_breakdown(conn, request.params.year, request.params.month)?;
        render(conn, ReportPayload::Expenses(&report), &request)
    })
    .await?;
    Ok(download(kind, format, bytes))
}

async fn tax(
    State(state): State<AppState>,
    Query(opts): Query<ExportOptions>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Response> {
    let kind = ReportKind::Tax;
    let request = ExportRequest::parse(opts, raw, kind)?;
    let format = request.format;
    let bytes = with_conn_api(&state, move |conn| {
        let report = reports::get_tax_summary(conn, request.params.year)?;
        render(conn, ReportPayload::Tax(&report), &request)
    })
    .await?;
    Ok(download(kind, format, bytes))
}

async fn cashflow(
    State(state): State<AppState>,
    Query(opts): Query<ExportOptions>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Response> {
    let kind = ReportKind::Cashflow;
    let request = ExportRequest::parse(opts, raw, kind)?;
    let format = request.format;
    let bytes = with_conn_api(&state, move |conn| {
        let report = reports::get_cashflow(conn, request.params.year, request.params.month)?;
        render(conn, ReportPayload::Cashflow(&report), &request)
    })
    .await?;
    Ok(download(kind, format, bytes))
}

async fn balance(
    State(state): State<AppState>,
    Query(opts): Query<ExportOptions>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Response> {
    let kind = ReportKind::Balance;
    let request = ExportRequest::parse(opts, raw, kind)?;
    let format = request.format;
    let bytes = with_conn_api(&state, move |conn| {
        let report = reports::get_balance(conn)?;
        render(conn, ReportPayload::Balance(&report), &request)
    })
    .await?;
    Ok(download(kind, format, bytes))
}

async fn flagged(
    State(state): State<AppState>,
    Query(opts): Query<ExportOptions>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Response> {
    let kind = ReportKind::Flagged;
    let request = ExportRequest::parse(opts, raw, kind)?;
    let format = request.format;
    let bytes = with_conn_api(&state, move |conn| {
        let rows = reports::get_flagged(conn)?;
        render(conn, ReportPayload::Flagged(&rows), &request)
    })
    .await?;
    Ok(download(kind, format, bytes))
}

async fn register(
    State(state): State<AppState>,
    Query(opts): Query<ExportOptions>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Response> {
    let kind = ReportKind::Register;
    let request = ExportRequest::parse(opts, raw, kind)?;
    let format = request.format;
    let bytes = with_conn_api(&state, move |conn| {
        if let Some(account) = request.params.account.as_deref() {
            ensure_account_exists(conn, account)?;
        }
        let report = reports::get_register(
            conn,
            request.params.year,
            request.params.month,
            request.params.from.as_deref(),
            request.params.to.as_deref(),
            request.params.account.as_deref(),
        )?;
        render(conn, ReportPayload::Register(&report), &request)
    })
    .await?;
    Ok(download(kind, format, bytes))
}

async fn k1(
    State(state): State<AppState>,
    Query(opts): Query<ExportOptions>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Response> {
    let kind = ReportKind::K1;
    let request = ExportRequest::parse(opts, raw, kind)?;
    let format = request.format;
    let bytes = with_conn_api(&state, move |conn| {
        let report = reports::get_k1_prep(conn, request.params.year)?;
        render(conn, ReportPayload::K1(&report), &request)
    })
    .await?;
    Ok(download(kind, format, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::testutil::*;
    use axum::http::StatusCode;
    use std::path::PathBuf;

    const COMPANY: &str = "Raygun LLC";

    /// The seeded database plus a company name — every text export opens with
    /// one, so leaving it unset would skip half of `with_header`.
    fn export_db() -> (tempfile::TempDir, PathBuf) {
        let (dir, db_path) = seeded_db();
        let conn = crate::db::open_connection(&db_path, None).expect("open db");
        crate::db::set_metadata(&conn, "company_name", COMPANY).expect("company name");
        (dir, db_path)
    }

    async fn text_body(app: &axum::Router, uri: &str, token: &str) -> String {
        let response = get_response(app, uri, token).await;
        assert_eq!(response.status(), StatusCode::OK, "GET {uri}");
        assert_eq!(
            content_type(&response),
            "text/plain; charset=utf-8",
            "{uri}"
        );
        body_string(response).await
    }

    /// The CLI's text export is `with_header(company, format_x(report))` written
    /// to a file; the only thing this endpoint does differently is skip the file.
    ///
    /// Calling `cli::report::text::pnl()` itself would prove the last step too,
    /// but those wrappers open their own connection through
    /// `settings::get_data_dir()` — the developer's real data directory, which a
    /// test has no business repointing.
    #[tokio::test]
    async fn text_exports_are_what_the_cli_writes_to_a_file() {
        crate::server::disable_ansi_output();
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let cases: [(&str, String); 8] = [
            (
                "/api/exports/pnl?format=text",
                text::format_pnl(&reports::get_pnl(&conn, None, None, None, None).unwrap()),
            ),
            (
                "/api/exports/expenses?format=text",
                text::format_expenses(&reports::get_expense_breakdown(&conn, None, None).unwrap()),
            ),
            (
                "/api/exports/tax?format=text",
                text::format_tax(&reports::get_tax_summary(&conn, None).unwrap()),
            ),
            (
                "/api/exports/cashflow?format=text",
                text::format_cashflow(&reports::get_cashflow(&conn, None, None).unwrap()),
            ),
            (
                "/api/exports/balance?format=text",
                text::format_balance(&reports::get_balance(&conn).unwrap()),
            ),
            (
                "/api/exports/flagged?format=text",
                text::format_flagged(&reports::get_flagged(&conn).unwrap()),
            ),
            (
                "/api/exports/register?format=text",
                text::format_register(
                    &reports::get_register(&conn, None, None, None, None, None).unwrap(),
                ),
            ),
            (
                "/api/exports/k1?format=text",
                text::format_k1(&reports::get_k1_prep(&conn, None).unwrap()),
            ),
        ];

        for (uri, formatted) in cases {
            let body = text_body(&app, uri, &token).await;
            assert_eq!(body, with_header(COMPANY, formatted), "body of {uri}");
            assert!(body.starts_with(COMPANY), "{uri} lost its header");
        }
    }

    #[tokio::test]
    async fn text_exports_carry_no_ansi_escapes() {
        crate::server::disable_ansi_output();
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        for route in EXPORT_ROUTES {
            let uri = format!("{route}?format=text");
            let response = get_response(&app, &uri, &token).await;
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let bytes = body_bytes(response).await;
            assert!(
                !bytes.contains(&0x1b),
                "{uri} answered with an escape sequence"
            );
        }

        // The rows that carry colour in the terminal are the ones that came back
        // plain: without this the assertion above would also pass on an empty
        // report that never reached a formatter.
        let pnl = text_body(&app, "/api/exports/pnl?format=text", &token).await;
        assert!(pnl.contains("NET") && pnl.contains("Total Income"), "{pnl}");
        let register = text_body(&app, "/api/exports/register?format=text", &token).await;
        assert!(register.contains("$5,000.00"), "{register}");
    }

    #[tokio::test]
    async fn date_parameters_reach_the_report() {
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        let everything = text_body(&app, "/api/exports/register?format=text", &token).await;
        assert!(everything.contains("2024-11-04"), "{everything}");

        let this_year =
            text_body(&app, "/api/exports/register?format=text&year=2025", &token).await;
        assert!(!this_year.contains("2024-11-04"), "{this_year}");
        assert!(this_year.contains("2025-01-15"), "{this_year}");
    }

    #[tokio::test]
    async fn downloads_are_named_the_way_the_cli_names_them() {
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        // k1 is the one whose URL slug ("k1") and filename ("k1-prep") differ.
        let mut cases = vec![
            ("/api/exports/pnl?format=text", "pnl", "txt"),
            ("/api/exports/k1?format=text", "k1-prep", "txt"),
        ];
        if cfg!(feature = "pdf") {
            cases.push(("/api/exports/pnl?format=pdf", "pnl", "pdf"));
            cases.push(("/api/exports/k1?format=pdf", "k1-prep", "pdf"));
        }

        for (uri, name, extension) in cases {
            let response = get_response(&app, uri, &token).await;
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let expected = format!(
                "attachment; filename=\"{}.{extension}\"",
                export_file_stem(name)
            );
            assert_eq!(
                header_str(&response, header::CONTENT_DISPOSITION),
                expected,
                "disposition for {uri}"
            );
        }
    }

    #[tokio::test]
    async fn the_format_parameter_is_required_and_spelt_exactly() {
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        for uri in [
            "/api/exports/pnl",
            "/api/exports/pnl?format=",
            "/api/exports/pnl?format=xml",
            "/api/exports/pnl?format=PDF",
            "/api/exports/pnl?format=csv&year=2025",
        ] {
            let (status, body) = get_json(&app, uri, &token).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "GET {uri} gave {body}");
            assert_eq!(body["error"]["code"], "bad_request", "for {uri}");
        }
    }

    /// The export routes do not validate parameters themselves — they hand them
    /// to the same parser `/api/reports` uses. Comparing the two answers is what
    /// keeps that true.
    #[tokio::test]
    async fn parameters_fail_exactly_as_they_do_on_the_report_routes() {
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        let cases = [
            ("pnl", "from=2025-01-01"),
            ("pnl", "to=2025-12-31"),
            ("pnl", "month=2025-13"),
            ("pnl", "month=nope"),
            ("pnl", "year=abc"),
            ("pnl", "account=BofA%20Checking"),
            ("register", "from=2025-01-01"),
            ("expenses", "from=2025-01-01&to=2025-12-31"),
            ("tax", "month=2025-03"),
            ("balance", "year=2025"),
            ("flagged", "month=2025-03"),
            ("k1", "month=2025-03"),
        ];

        for (report, query) in cases {
            let (export_status, export_body) = get_json(
                &app,
                &format!("/api/exports/{report}?format=text&{query}"),
                &token,
            )
            .await;
            let (report_status, report_body) =
                get_json(&app, &format!("/api/reports/{report}?{query}"), &token).await;

            assert_eq!(export_status, StatusCode::BAD_REQUEST, "{report}?{query}");
            assert_eq!(export_status, report_status, "{report}?{query}");
            assert_eq!(export_body, report_body, "{report}?{query}");
        }
    }

    #[tokio::test]
    async fn the_register_export_tells_an_unknown_account_from_an_empty_one() {
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(
            &app,
            "/api/exports/register?format=text&account=Nope%20Bank",
            &token,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");

        let known = text_body(
            &app,
            "/api/exports/register?format=text&account=BofA%20Credit%20Card",
            &token,
        )
        .await;
        assert!(known.contains("BofA Credit Card"), "{known}");
        assert!(!known.contains("BofA Checking"), "{known}");
    }

    #[tokio::test]
    async fn a_locked_database_refuses_every_export() {
        let (_dir, db_path) = seeded_db();
        encrypt(&db_path);
        let (app, token) = app_for(&db_path);

        for route in EXPORT_ROUTES {
            let uri = format!("{route}?format=text");
            let (status, body) = get_json(&app, &uri, &token).await;
            assert_eq!(status, StatusCode::LOCKED, "{uri} while locked: {body}");
            assert_eq!(body["error"]["code"], "locked", "for {uri}");
        }
    }

    /// Bulk export stays a CLI affair: a browser downloads one file at a time,
    /// and `report all` writes eight of them into a directory.
    #[tokio::test]
    async fn bulk_export_is_not_an_endpoint() {
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/exports/all?format=text", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }

    #[cfg(feature = "pdf")]
    #[tokio::test]
    async fn pdf_exports_are_pdfs() {
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        for route in EXPORT_ROUTES {
            let uri = format!("{route}?format=pdf");
            let response = get_response(&app, &uri, &token).await;
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            assert_eq!(content_type(&response), "application/pdf", "{uri}");
            let bytes = body_bytes(response).await;
            assert!(bytes.starts_with(b"%PDF"), "{uri} is not a PDF");
            assert!(
                bytes.len() > 1000,
                "{uri} rendered only {} bytes",
                bytes.len()
            );
        }
    }

    #[cfg(not(feature = "pdf"))]
    #[tokio::test]
    async fn without_the_feature_pdf_is_refused_and_text_still_works() {
        let (_dir, db_path) = export_db();
        let (app, token) = app_for(&db_path);

        for route in EXPORT_ROUTES {
            let (status, body) = get_json(&app, &format!("{route}?format=pdf"), &token).await;
            assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{route}: {body}");
            assert_eq!(body["error"]["code"], "feature_disabled", "for {route}");
            assert_eq!(
                body["error"]["message"],
                crate::cli::report::PDF_DISABLED_MESSAGE,
                "for {route}"
            );

            let response = get_response(&app, &format!("{route}?format=text"), &token).await;
            assert_eq!(response.status(), StatusCode::OK, "text export of {route}");
        }
    }
}
