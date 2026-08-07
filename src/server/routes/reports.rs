//! The eight report endpoints under `/api/reports`.
//!
//! Each handler is the same three steps: validate the query string, run the
//! matching `reports::get_*` on the blocking pool, wrap the result. The reports
//! themselves are untouched — this module only translates HTTP into their
//! arguments, and it is deliberately stricter than the CLI about doing so: a
//! parameter the CLI would quietly ignore is a wrong answer over HTTP, where
//! nobody is watching the screen.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::reports::{self, DateGranularity, ReportKind};

use super::super::error::{ApiError, ApiResult};
use super::super::state::AppState;
use super::{ensure_account_exists, with_conn};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/reports/pnl", get(pnl))
        .route("/reports/expenses", get(expenses))
        .route("/reports/tax", get(tax))
        .route("/reports/cashflow", get(cashflow))
        .route("/reports/balance", get(balance))
        .route("/reports/flagged", get(flagged))
        .route("/reports/register", get(register))
        .route("/reports/k1", get(k1))
}

// ---------------------------------------------------------------------------
// Response envelope
// ---------------------------------------------------------------------------

/// Every report answers with its own date granularity alongside the data, so
/// the SPA can build the right date controls without a table of its own.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportEnvelope<T> {
    granularity: DateGranularity,
    report: T,
}

impl<T> ReportEnvelope<T> {
    fn new(kind: ReportKind, report: T) -> Json<Self> {
        Json(Self {
            granularity: kind.granularity(),
            report,
        })
    }
}

// ---------------------------------------------------------------------------
// Query parameters
// ---------------------------------------------------------------------------

/// The query string before validation.
///
/// Every field is a `String` on purpose: a typed `year: Option<i32>` would make
/// `?year=abc` an axum `Query` rejection, which answers in plain text and would
/// be the one response on the whole API that skips the error envelope.
#[derive(Debug, Default, Deserialize)]
pub(super) struct RawQuery {
    year: Option<String>,
    month: Option<String>,
    from: Option<String>,
    to: Option<String>,
    account: Option<String>,
}

/// Which parameters a route accepts, mirroring the clap arguments of the
/// matching `nigel report` subcommand. Year and month support is read off
/// `ReportKind::granularity()` so the two cannot drift; `ranges` and `account`
/// are the two axes granularity does not describe.
pub(super) struct ParamSpec {
    kind: ReportKind,
    ranges: bool,
    account: bool,
}

impl ParamSpec {
    fn new(kind: ReportKind) -> Self {
        Self {
            kind,
            ranges: false,
            account: false,
        }
    }

    fn ranges(mut self) -> Self {
        self.ranges = true;
        self
    }

    fn account(mut self) -> Self {
        self.account = true;
        self
    }

    /// The parameter matrix for a report. Reading it off the kind is what keeps
    /// `/api/reports/pnl` and `/api/exports/pnl` from disagreeing about which
    /// parameters a report will answer to.
    pub(super) fn for_kind(kind: ReportKind) -> Self {
        let spec = Self::new(kind);
        match kind {
            ReportKind::Pnl => spec.ranges(),
            ReportKind::Register => spec.ranges().account(),
            _ => spec,
        }
    }
}

/// A validated query string, in the shape the `reports::get_*` functions take.
#[derive(Debug, Default, PartialEq)]
pub(super) struct ReportParams {
    pub(super) year: Option<i32>,
    pub(super) month: Option<u32>,
    pub(super) from: Option<String>,
    pub(super) to: Option<String>,
    pub(super) account: Option<String>,
}

impl ReportParams {
    pub(super) fn parse(raw: RawQuery, spec: ParamSpec) -> ApiResult<Self> {
        let granularity = spec.kind.granularity();
        let report = spec.kind.as_str();

        if raw.year.is_some() && granularity == DateGranularity::None {
            return Err(unsupported(report, "year"));
        }
        if raw.month.is_some() && granularity != DateGranularity::MonthAndYear {
            return Err(unsupported(report, "month"));
        }
        if !spec.ranges {
            if raw.from.is_some() {
                return Err(unsupported(report, "from"));
            }
            if raw.to.is_some() {
                return Err(unsupported(report, "to"));
            }
        }
        if raw.account.is_some() && !spec.account {
            return Err(unsupported(report, "account"));
        }

        let explicit_year = raw.year.as_deref().map(parse_year).transpose()?;
        let (month_year, month) = match raw.month.as_deref() {
            Some(value) => {
                let (year, month) = parse_month(value)?;
                (Some(year), Some(month))
            }
            None => (None, None),
        };

        let (from, to) = match (raw.from.as_deref(), raw.to.as_deref()) {
            (Some(from), Some(to)) => {
                (Some(parse_date("from", from)?), Some(parse_date("to", to)?))
            }
            (Some(_), None) => {
                return Err(ApiError::bad_request(
                    "`from` requires `to` (both date boundaries must be specified).",
                ))
            }
            (None, Some(_)) => {
                return Err(ApiError::bad_request(
                    "`to` requires `from` (both date boundaries must be specified).",
                ))
            }
            (None, None) => (None, None),
        };

        Ok(Self {
            // An explicit `year` outranks the year inside `month`, which is what
            // `cli::report::text` does with `year.or(month_year)`.
            year: explicit_year.or(month_year),
            month,
            from,
            to,
            account: raw.account,
        })
    }
}

fn unsupported(report: &str, param: &str) -> ApiError {
    ApiError::bad_request(format!(
        "The {report} report does not accept a `{param}` parameter."
    ))
}

fn parse_year(value: &str) -> ApiResult<i32> {
    value.parse().map_err(|_| {
        ApiError::bad_request(format!(
            "Invalid `year`: expected a year like 2025, got \"{value}\"."
        ))
    })
}

/// `YYYY-MM`, strictly. `cli::parse_month_opt` answers a malformed month with
/// `(None, None)`, which over HTTP would silently widen the request to the whole
/// database instead of reporting the typo.
pub(super) fn parse_month(value: &str) -> ApiResult<(i32, u32)> {
    let invalid = || {
        ApiError::bad_request(format!(
            "Invalid `month`: expected YYYY-MM, got \"{value}\"."
        ))
    };

    let (year, month) = value.split_once('-').ok_or_else(invalid)?;
    if year.len() != 4 || month.len() != 2 {
        return Err(invalid());
    }
    let year: i32 = year.parse().map_err(|_| invalid())?;
    let month: u32 = month.parse().map_err(|_| invalid())?;
    if !(1..=12).contains(&month) {
        return Err(invalid());
    }
    Ok((year, month))
}

/// `YYYY-MM-DD`, strictly. The length check rejects `2025-1-5`, which chrono
/// would otherwise accept and which does not compare correctly against the
/// zero-padded dates stored in the database.
fn parse_date(param: &str, value: &str) -> ApiResult<String> {
    if value.len() == 10 && NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() {
        return Ok(value.to_string());
    }
    Err(ApiError::bad_request(format!(
        "Invalid `{param}`: expected YYYY-MM-DD, got \"{value}\"."
    )))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn pnl(
    State(state): State<AppState>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Json<ReportEnvelope<reports::PnlReport>>> {
    let kind = ReportKind::Pnl;
    let params = ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
    let report = with_conn(&state, move |conn| {
        reports::get_pnl(
            conn,
            params.year,
            params.month,
            params.from.as_deref(),
            params.to.as_deref(),
        )
    })
    .await?;
    Ok(ReportEnvelope::new(kind, report))
}

async fn expenses(
    State(state): State<AppState>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Json<ReportEnvelope<reports::ExpenseBreakdown>>> {
    let kind = ReportKind::Expenses;
    let params = ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
    let report = with_conn(&state, move |conn| {
        reports::get_expense_breakdown(conn, params.year, params.month)
    })
    .await?;
    Ok(ReportEnvelope::new(kind, report))
}

async fn tax(
    State(state): State<AppState>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Json<ReportEnvelope<reports::TaxSummary>>> {
    let kind = ReportKind::Tax;
    let params = ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
    let report = with_conn(&state, move |conn| {
        reports::get_tax_summary(conn, params.year)
    })
    .await?;
    Ok(ReportEnvelope::new(kind, report))
}

async fn cashflow(
    State(state): State<AppState>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Json<ReportEnvelope<reports::CashflowReport>>> {
    let kind = ReportKind::Cashflow;
    let params = ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
    let report = with_conn(&state, move |conn| {
        reports::get_cashflow(conn, params.year, params.month)
    })
    .await?;
    Ok(ReportEnvelope::new(kind, report))
}

async fn balance(
    State(state): State<AppState>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Json<ReportEnvelope<reports::BalanceReport>>> {
    let kind = ReportKind::Balance;
    ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
    let report = with_conn(&state, reports::get_balance).await?;
    Ok(ReportEnvelope::new(kind, report))
}

async fn flagged(
    State(state): State<AppState>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Json<ReportEnvelope<Vec<reports::FlaggedTransaction>>>> {
    let kind = ReportKind::Flagged;
    ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
    let report = with_conn(&state, reports::get_flagged).await?;
    Ok(ReportEnvelope::new(kind, report))
}

async fn register(
    State(state): State<AppState>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Json<ReportEnvelope<reports::RegisterReport>>> {
    let kind = ReportKind::Register;
    let params = ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
    let report = with_conn(&state, move |conn| {
        if let Some(account) = params.account.as_deref() {
            ensure_account_exists(conn, account)?;
        }
        reports::get_register(
            conn,
            params.year,
            params.month,
            params.from.as_deref(),
            params.to.as_deref(),
            params.account.as_deref(),
        )
    })
    .await?;
    Ok(ReportEnvelope::new(kind, report))
}

async fn k1(
    State(state): State<AppState>,
    Query(raw): Query<RawQuery>,
) -> ApiResult<Json<ReportEnvelope<reports::K1PrepReport>>> {
    let kind = ReportKind::K1;
    let params = ReportParams::parse(raw, ParamSpec::for_kind(kind))?;
    let report = with_conn(&state, move |conn| reports::get_k1_prep(conn, params.year)).await?;
    Ok(ReportEnvelope::new(kind, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(pairs: &[(&str, &str)]) -> RawQuery {
        let mut raw = RawQuery::default();
        for (key, value) in pairs {
            let value = Some((*value).to_string());
            match *key {
                "year" => raw.year = value,
                "month" => raw.month = value,
                "from" => raw.from = value,
                "to" => raw.to = value,
                "account" => raw.account = value,
                other => panic!("unknown param {other}"),
            }
        }
        raw
    }

    fn pnl_spec() -> ParamSpec {
        ParamSpec::new(ReportKind::Pnl).ranges()
    }

    #[test]
    fn an_explicit_year_outranks_the_year_inside_month() {
        let params =
            ReportParams::parse(raw(&[("year", "2024"), ("month", "2025-03")]), pnl_spec())
                .expect("valid");
        assert_eq!(params.year, Some(2024));
        assert_eq!(params.month, Some(3));
    }

    #[test]
    fn month_alone_supplies_both_halves() {
        let params = ReportParams::parse(raw(&[("month", "2025-03")]), pnl_spec()).expect("valid");
        assert_eq!(params.year, Some(2025));
        assert_eq!(params.month, Some(3));
    }

    #[test]
    fn malformed_months_are_rejected_rather_than_ignored() {
        for value in ["2025-13", "2025-00", "2025", "2025-3", "25-03", "nope", ""] {
            let err = ReportParams::parse(raw(&[("month", value)]), pnl_spec())
                .expect_err(&format!("{value} should be rejected"));
            assert_eq!(err.code().as_str(), "bad_request", "for {value}");
        }
    }

    #[test]
    fn a_non_numeric_year_is_a_bad_request() {
        let err = ReportParams::parse(raw(&[("year", "abc")]), pnl_spec()).expect_err("rejected");
        assert_eq!(err.code().as_str(), "bad_request");
    }

    #[test]
    fn a_lone_date_boundary_is_a_bad_request() {
        for pairs in [vec![("from", "2025-01-01")], vec![("to", "2025-12-31")]] {
            let err = ReportParams::parse(raw(&pairs), pnl_spec()).expect_err("rejected");
            assert_eq!(err.code().as_str(), "bad_request");
        }

        let params = ReportParams::parse(
            raw(&[("from", "2025-01-01"), ("to", "2025-12-31")]),
            pnl_spec(),
        )
        .expect("a complete pair is fine");
        assert_eq!(params.from.as_deref(), Some("2025-01-01"));
        assert_eq!(params.to.as_deref(), Some("2025-12-31"));
    }

    #[test]
    fn boundary_dates_must_be_zero_padded_iso() {
        for value in ["2025-1-5", "01/01/2025", "2025-01-32", "yesterday"] {
            let err =
                ReportParams::parse(raw(&[("from", value), ("to", "2025-12-31")]), pnl_spec())
                    .expect_err(&format!("{value} should be rejected"));
            assert_eq!(err.code().as_str(), "bad_request", "for {value}");
        }
    }

    #[test]
    fn each_report_rejects_the_parameters_it_cannot_honour() {
        let cases: [(ParamSpec, &str, &str); 6] = [
            (ParamSpec::new(ReportKind::Tax), "month", "tax"),
            (ParamSpec::new(ReportKind::K1), "month", "k1-prep"),
            (ParamSpec::new(ReportKind::Balance), "year", "balance"),
            (ParamSpec::new(ReportKind::Flagged), "month", "flagged"),
            (ParamSpec::new(ReportKind::Expenses), "from", "expenses"),
            (ParamSpec::new(ReportKind::Cashflow), "to", "cashflow"),
        ];

        for (spec, param, name) in cases {
            let value = match param {
                "month" => "2025-03",
                "year" => "2025",
                _ => "2025-01-01",
            };
            let err = ReportParams::parse(raw(&[(param, value)]), spec)
                .expect_err(&format!("{name} should reject {param}"));
            assert_eq!(err.code().as_str(), "bad_request", "{name}/{param}");
        }
    }

    #[test]
    fn only_the_register_takes_an_account() {
        let err = ReportParams::parse(raw(&[("account", "Checking")]), pnl_spec())
            .expect_err("pnl has no account filter");
        assert_eq!(err.code().as_str(), "bad_request");

        let params = ReportParams::parse(
            raw(&[("account", "Checking")]),
            ParamSpec::new(ReportKind::Register).ranges().account(),
        )
        .expect("valid");
        assert_eq!(params.account.as_deref(), Some("Checking"));
    }

    #[test]
    fn an_empty_query_string_filters_nothing() {
        let params = ReportParams::parse(RawQuery::default(), pnl_spec()).expect("valid");
        assert_eq!(params, ReportParams::default());
    }

    // -----------------------------------------------------------------------
    // Over HTTP
    // -----------------------------------------------------------------------

    use crate::server::testutil::*;
    use axum::http::StatusCode;
    use serde::Serialize;

    fn value<T: Serialize>(report: T) -> serde_json::Value {
        serde_json::to_value(report).expect("serializes")
    }

    #[tokio::test]
    async fn every_report_answers_with_its_granularity_and_the_data_layer_figures() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let cases = [
            (
                "/api/reports/pnl",
                ReportKind::Pnl,
                value(reports::get_pnl(&conn, None, None, None, None).unwrap()),
            ),
            (
                "/api/reports/expenses",
                ReportKind::Expenses,
                value(reports::get_expense_breakdown(&conn, None, None).unwrap()),
            ),
            (
                "/api/reports/tax",
                ReportKind::Tax,
                value(reports::get_tax_summary(&conn, None).unwrap()),
            ),
            (
                "/api/reports/cashflow",
                ReportKind::Cashflow,
                value(reports::get_cashflow(&conn, None, None).unwrap()),
            ),
            (
                "/api/reports/balance",
                ReportKind::Balance,
                value(reports::get_balance(&conn).unwrap()),
            ),
            (
                "/api/reports/flagged",
                ReportKind::Flagged,
                value(reports::get_flagged(&conn).unwrap()),
            ),
            (
                "/api/reports/register",
                ReportKind::Register,
                value(reports::get_register(&conn, None, None, None, None, None).unwrap()),
            ),
            (
                "/api/reports/k1",
                ReportKind::K1,
                value(reports::get_k1_prep(&conn, None).unwrap()),
            ),
        ];

        for (uri, kind, expected) in cases {
            let body = ok_json(&app, uri, &token).await;
            assert_eq!(
                body["granularity"],
                value(kind.granularity()),
                "granularity for {uri}"
            );
            assert_eq!(body["report"], expected, "figures for {uri}");
        }
    }

    #[tokio::test]
    async fn report_fields_are_camel_case() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let expectations: [(&str, &[&str]); 8] = [
            ("/api/reports/pnl", &["totalIncome", "totalExpenses"]),
            ("/api/reports/expenses", &["topVendors"]),
            ("/api/reports/tax", &["lineItems"]),
            ("/api/reports/cashflow", &["months"]),
            ("/api/reports/balance", &["ytdNetIncome"]),
            ("/api/reports/flagged", &[]),
            ("/api/reports/register", &["rows"]),
            (
                "/api/reports/k1",
                &["grossReceipts", "totalDeductions", "otherDeductionsTotal"],
            ),
        ];

        for (uri, keys) in expectations {
            let body = ok_json(&app, uri, &token).await;
            for key in keys {
                assert!(
                    body["report"].get(key).is_some(),
                    "{uri} is missing {key}: {body}"
                );
            }
        }

        // Nested rows carry the renames too, which is where a forgotten
        // rename_all would actually bite the SPA.
        let register = ok_json(&app, "/api/reports/register", &token).await;
        let row = &register["report"]["rows"][0];
        for key in ["accountName", "categoryId", "isFlagged"] {
            assert!(row.get(key).is_some(), "register row missing {key}: {row}");
        }

        let balance = ok_json(&app, "/api/reports/balance", &token).await;
        assert!(balance["report"]["accounts"][0]
            .get("accountType")
            .is_some());

        let tax = ok_json(&app, "/api/reports/tax", &token).await;
        let line = &tax["report"]["lineItems"][0];
        for key in ["taxLine", "categoryType"] {
            assert!(line.get(key).is_some(), "tax line missing {key}: {line}");
        }

        let cashflow = ok_json(&app, "/api/reports/cashflow", &token).await;
        assert!(cashflow["report"]["months"][0]
            .get("runningBalance")
            .is_some());
    }

    #[tokio::test]
    async fn date_filters_narrow_the_result() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let all = ok_json(&app, "/api/reports/register", &token).await;
        assert_eq!(all["report"]["rows"].as_array().unwrap().len(), 8);

        let y2024 = ok_json(&app, "/api/reports/register?year=2024", &token).await;
        assert_eq!(y2024["report"]["rows"].as_array().unwrap().len(), 2);

        let march = ok_json(&app, "/api/reports/register?month=2025-03", &token).await;
        assert_eq!(march["report"]["rows"].as_array().unwrap().len(), 3);

        let range = ok_json(
            &app,
            "/api/reports/register?from=2025-01-01&to=2025-02-28",
            &token,
        )
        .await;
        assert_eq!(range["report"]["rows"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn an_explicit_year_outranks_month_over_http_too() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // 2024-12 has one row; the month's own year (2025) would find none.
        let body = ok_json(
            &app,
            "/api/reports/register?year=2024&month=2025-12",
            &token,
        )
        .await;
        let rows = body["report"]["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["date"], "2024-12-30");
    }

    #[tokio::test]
    async fn bad_parameters_are_a_json_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let cases = [
            "/api/reports/pnl?from=2025-01-01",
            "/api/reports/pnl?to=2025-12-31",
            "/api/reports/register?from=2025-01-01",
            "/api/reports/pnl?month=2025-13",
            "/api/reports/pnl?month=nope",
            "/api/reports/pnl?year=abc",
            "/api/reports/pnl?account=BofA%20Checking",
            "/api/reports/expenses?from=2025-01-01&to=2025-12-31",
            "/api/reports/tax?month=2025-03",
            "/api/reports/balance?year=2025",
            "/api/reports/flagged?month=2025-03",
            "/api/reports/k1?month=2025-03",
        ];

        for uri in cases {
            let (status, body) = get_json(&app, uri, &token).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "GET {uri} gave {body}");
            assert_eq!(body["error"]["code"], "bad_request", "for {uri}");
        }
    }

    #[tokio::test]
    async fn the_register_tells_an_unknown_account_from_an_empty_one() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) =
            get_json(&app, "/api/reports/register?account=Nope%20Bank", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Nope Bank"),
            "message should name the account: {body}"
        );

        let known = ok_json(
            &app,
            "/api/reports/register?account=BofA%20Credit%20Card",
            &token,
        )
        .await;
        let rows = known["report"]["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows
            .iter()
            .all(|row| row["accountName"] == "BofA Credit Card"));
    }
}
