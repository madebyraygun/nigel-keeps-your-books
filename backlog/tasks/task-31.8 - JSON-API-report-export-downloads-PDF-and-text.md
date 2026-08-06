---
id: TASK-31.8
title: 'JSON API: report export downloads (PDF and text)'
status: In Progress
assignee:
  - '@agent-31.8'
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 20:45'
labels:
  - web
  - backend
dependencies:
  - TASK-31.3
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.8-export-api.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wire the existing export machinery (cli/export.rs, pdf.rs, report/text.rs) to download endpoints so every report can be exported from the browser with the same content as CLI exports. Respect the pdf feature gate: text export always available, PDF degrades gracefully when the feature is off.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every report downloads as PDF and text with content matching the CLI export output
- [ ] #2 Responses set correct content-type and filename headers
- [ ] #3 Builds without the pdf feature keep text export and return a clear error for PDF requests
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
IMPLEMENTS AFTER 31.5/31.6/31.7 LAND — last in the API sequence. Everything
below is written against 31.5's plan; where the landed code differs, I adapt to
what shipped rather than reshaping it.

## 1. Route table

New module src/server/routes/exports.rs, merged into data_router() in
routes/mod.rs so it inherits 31.4's locked guard (layer over the whole /api
router — nothing to opt into).

| Route (GET) | ReportKind | Data fn | Params (beyond `format`) |
|---|---|---|---|
| /api/exports/pnl | Pnl | reports::get_pnl | year, month, from+to |
| /api/exports/expenses | Expenses | reports::get_expense_breakdown | year, month |
| /api/exports/tax | Tax | reports::get_tax_summary | year |
| /api/exports/cashflow | Cashflow | reports::get_cashflow | year, month |
| /api/exports/balance | Balance | reports::get_balance | none |
| /api/exports/flagged | Flagged | reports::get_flagged | none |
| /api/exports/register | Register | reports::get_register | year, month, from+to, account |
| /api/exports/k1 | K1 | reports::get_k1_prep | year |

Eight explicit routes, not one `/api/exports/{report}` path param: the slugs
then match 31.5's report paths literally (grep-able, same spelling), an unknown
slug falls through to the existing JSON 404 fallback for free, and no
`ReportKind::from_slug` has to be invented — note `as_str()` returns `k1-prep`
for K1 while the route slug is `k1`, so a round-tripping slug parser would need
a second vocabulary. `report all` is deliberately not routed (see §7).

`format` is REQUIRED: `pdf` | `text`. Missing -> 400 bad_request
"Missing `format`: expected `pdf` or `text`."; unknown value -> 400
"Unknown `format` 'xml'. Expected 'pdf' or 'text'." (mirrors the CLI's --format
message). No default, because defaulting to pdf would make a bare URL 501 on a
no-pdf build and defaulting to text would silently hand back the wrong artifact.

## 2. Params — reuse 31.5, do not duplicate

Handlers take two Query extractors (Query does not touch the body, so it may
appear twice; serde ignores unknown fields, so each sees only what it declares):

    async fn pnl(State(state): State<AppState>,
                 Query(opts): Query<ExportOptions>,      // exports.rs
                 Query(raw): Query<RawReportQuery>)      // 31.5's struct
        -> ApiResult<Response>

    #[derive(Deserialize)]
    struct ExportOptions { format: Option<String>, month: Option<String> }

`month` is repeated in ExportOptions only to keep the RAW month string for the
PDF date-range label (31.5's ReportParams normalises it to Option<u32>). That
avoids touching RawReportQuery's field visibility.

Validation is 31.5's, unchanged: `ReportParams::parse(raw, ParamSpec::for_kind(kind))`
produces every 400 (bad year, bad month, unsupported param for the kind, lone
from/to, non-ISO dates), so export params behave identically to report params
by construction.

Visibility bumps I make in my commit against 31.5's routes/reports.rs (all
in-crate, no API surface change):
- `RawReportQuery`, `ReportParams` (+ fields), `ParamSpec` -> pub(crate)
- `ReportParams::parse` -> pub(crate)
- `ensure_account_exists` -> pub(crate) (register export gets the same 404)
- add `pub(crate) fn ParamSpec::for_kind(ReportKind) -> Self`
  (ranges = matches!(Pnl | Register), account = matches!(Register)) and switch
  31.5's eight construction sites to it, so the per-kind param matrix has one
  definition. If 31.5 already derives the spec from the kind, I use theirs and
  drop this.

## 3. Shared helpers extracted from the CLI

(a) date-range label — moves out of cli/export.rs (where it is cfg(pdf)) into
src/reports.rs, ungated, next to ReportKind:

    /// Human label for the period a report covers: the month string when one
    /// was given, otherwise `FY <year>`, otherwise the current fiscal year.
    pub fn date_range_label(month: Option<&str>, year: Option<i32>) -> String

Body is the existing logic verbatim (month.to_string() / format!("FY {y}") /
current-year fallback); the signature drops the `&Option<T>` params for
idiomatic borrows. cli/export.rs deletes its private copy and its now-unused
`chrono` import, and rewrites its nine call sites mechanically:
`date_range_label(&month, &year.or(my))` -> `reports::date_range_label(month.as_deref(), year.or(my))`
and `date_range_label(&None, &year)` -> `reports::date_range_label(None, year)`.
Same inputs, same logic, so PDF output is byte-identical — no CLI behaviour
change. New unit tests in reports.rs cover all three branches (month wins over
year; year -> "FY 2025"; neither -> current year).

(b) text header — cli/report/text.rs `with_header` goes from private to `pub`.
The server calls `with_header(&company, format_x(&data))`, which is exactly
what the CLI's text::pnl()/expenses()/... wrappers do after opening their own
connection. The server does NOT call those wrappers (they call
settings::get_data_dir() and open a second connection; the handler already has
one from with_conn).

(c) filename stem — the same `{name}-{YYYY-MM-DD}` stem is built in three
places today (cli/export.rs::default_path, cli/report/mod.rs::default_text_path,
and now the server). Extract `pub fn export_file_stem(name: &str) -> String`
into cli/report/mod.rs (ungated) and use it from all three. Optional-but-cheap;
it makes the header-parity test a single equality instead of a re-implemented
format string. If it turns into churn against 31.5/31.6 diffs, I drop it and
build the stem in exports.rs from the same two ingredients.

(d) pdf-off message — cli/report/mod.rs gains
`pub const PDF_DISABLED_MESSAGE: &str = "PDF export requires the 'pdf' feature — build with `cargo build --features pdf`";`
and its `dispatch_pdf_export` no-pdf arm uses it. The API's 501 body uses the
same const, and the test asserts equality against the const, so CLI and API can
never drift.

## 4. Handler shape

One 8-line handler per report, no cfg attributes in any of them:

    async fn pnl(State(state), Query(opts), Query(raw)) -> ApiResult<Response> {
        let kind = ReportKind::Pnl;
        let req = ExportRequest::parse(opts, raw, kind)?;   // format + ReportParams + raw month
        let fmt = req.format;
        let bytes = with_conn(&state, move |conn| {
            let company = db::get_metadata(conn, "company_name").unwrap_or_default();
            let range = reports::date_range_label(req.month_raw.as_deref(), req.params.year);
            let report = reports::get_pnl(conn, req.params.year, req.params.month,
                                          req.params.from.as_deref(), req.params.to.as_deref())?;
            render(ReportPayload::Pnl(&report), fmt, &company, &range)
        }).await?;
        Ok(download(kind, fmt, bytes))
    }

Company name and range are read inside the same blocking closure as the report
(one connection, one hop to the pool). Register additionally calls
`ensure_account_exists(conn, name)?` inside the closure, exactly as 31.5's
register handler does, so an unknown account is 404 on both routes.

with_conn's closure returns `crate::error::Result<T>`, but the pdf-disabled arm
needs an ApiError (501, not 500). So exports.rs uses a sibling
`with_conn_api` in routes/mod.rs whose closure returns `ApiResult<T>`; the
existing `with_conn` is reimplemented as a thin wrapper over it
(`work(conn).map_err(ApiError::from)`), so there is still one spawn_blocking +
get_connection site.

Rendering — the only two cfg sites in the task:

    enum ReportPayload<'a> {
        Pnl(&'a PnlReport), Expenses(&'a ExpenseBreakdown), Tax(&'a TaxSummary),
        Cashflow(&'a CashflowReport), Register(&'a RegisterReport),
        Flagged(&'a [FlaggedTransaction]), Balance(&'a BalanceReport),
        K1(&'a K1PrepReport),
    }

    fn render_text(p: ReportPayload) -> String              // match -> text::format_*
    #[cfg(feature = "pdf")]
    fn render_pdf(p: ReportPayload, company: &str, range: &str) -> ApiResult<Vec<u8>>
        // match -> pdf::render_*; flagged/balance take no range (their signatures omit it)
    #[cfg(not(feature = "pdf"))]
    fn render_pdf(_: ReportPayload, _: &str, _: &str) -> ApiResult<Vec<u8>>
        { Err(ApiError::feature_disabled(PDF_DISABLED_MESSAGE)) }

    fn render(p, fmt, company, range) -> ApiResult<Vec<u8>> {
        match fmt {
            Text => Ok(with_header(company, render_text(p)).into_bytes()),
            Pdf  => render_pdf(p, company, range),
        }
    }

The route module therefore compiles identically in both configurations, the
`format=pdf` 501 has exactly one source, and no handler contains an
`unreachable!`.

## 5. Response headers

    fn download(kind: ReportKind, fmt: ExportFormat, bytes: Vec<u8>) -> Response

- Content-Type: `application/pdf` | `text/plain; charset=utf-8`
- Content-Disposition: `attachment; filename="<stem>.<ext>"`
- Filename scheme (per format), matching the CLI's defaults exactly:
  pdf  -> `<ReportKind::as_str()>-<today %Y-%m-%d>.pdf`   e.g. `pnl-2026-08-06.pdf`
  text -> `<ReportKind::as_str()>-<today %Y-%m-%d>.txt`   e.g. `k1-prep-2026-08-06.txt`
  (K1's URL slug is `k1`, its filename base is `k1-prep` — the CLI's name.)
- No user-controlled bytes ever enter the filename (fixed slug + numeric date),
  so no quoting or RFC 5987 encoding is needed; the company name stays in the
  document body, not the header.

DEVIATION FROM THE SPEC TEXT, flagged for approval: the spec writes
`filename="nigel-<report>-<range>.<ext>"` and, in the same sentence, "matching
the CLI's default naming scheme (default_text_path / dispatch_pdf conventions)
as closely as practical". Those conflict — the CLI writes `<report>-<today>.<ext>`
with no `nigel-` prefix and no range. I follow the CLI so a browser download is
the same filename `nigel report … --mode export` would write. Cost: two
different periods exported on the same day produce the same name (the browser
disambiguates with "(1)"; the CLI silently overwrites). If the reviewer prefers
the literal spec pattern, it is a one-line change in `download()` to
`nigel-<slug>-<range-slug>.<ext>`.

## 6. ANSI regression

The startup line `colored::control::set_override(false)` moves into
`pub(crate) fn server::disable_ansi_output()`, called by `server::run()` — one
definition, so the test exercises the production statement rather than a copy.

Test (in exports.rs): call `disable_ansi_output()`, then GET every report with
`format=text` over the seeded DB and assert the body contains no `0x1b` byte.
The globals question the parent raised: `colored`'s override is process-global
and tests do not run `run()`, so the test sets it itself; it only ever lowers
the setting to the production value, and no other test in the crate raises it,
so there is no ordering hazard and no need for --test-threads=1. I deliberately
do NOT force `set_override(true)` first to prove the assertion has teeth — that
flip would race every concurrently-running test that renders text. Teeth come
instead from asserting the same bodies contain the strings that carry colour in
the TUI ("NET", "Total Income", the register's amount column), i.e. the report
did render the colour-bearing rows and they came back plain.

## 7. Out of scope (spec, restated so it is testable)

- bulk `report all` — no `/api/exports/all`; a test asserts it 404s through the
  API fallback. The SPA gets per-report buttons in 31.15.
- server-side file writing — `--output` / `--output-dir` semantics stay CLI-only;
  the API only streams bytes. No `exports/` directory is created by the server.
- print stylesheets — 31.15.

## 8. Docs (same commit)

- docs/api.md: new "Exports" section under Endpoints — the eight-route table,
  the shared param rules (cross-referenced to the Reports section, not
  duplicated), the required `format` param, the two content types, the exact
  filename scheme, and the error matrix (400 bad/missing format or bad params,
  404 unknown account or unknown report slug, 423 locked, 501 feature_disabled
  on a no-pdf build). One line stating that bulk export and file writing are
  CLI-only.
- CLAUDE.md: Architecture — add src/server/routes/exports.rs to the server
  layout note; record that `date_range_label` now lives in reports.rs (shared by
  the CLI PDF export and the API) and that `PDF_DISABLED_MESSAGE` is the single
  source for the pdf-off message.
- README.md: no change (serve is described there; endpoint detail lives in
  docs/api.md) — consistent with 31.5.

## 9. Tests

Reuses 31.5's src/server/testutil.rs (seeded_db / app / cookie helpers); I add
`get_bytes(app, uri, cookie) -> (StatusCode, HeaderMap, Vec<u8>)` for binary
bodies, and seed a `company_name` metadata row so the with_header branch is
exercised.

1. Text parity, all eight reports: body bytes ==
   `with_header(&company, text::format_x(&direct_data))` computed from a direct
   data-layer call on the same DB. That is the exact composition
   cli/report/text.rs performs, minus its `get_connection(get_data_dir()…)`
   line. LIMITATION, stated openly: a literal byte-compare against the CLI's
   own `text::pnl()` would require repointing settings::get_data_dir() at the
   temp DB, which reads ~/.config/nigel/settings.json — a test must not write
   the developer's real config, and there is no env override today. The
   composition compare is the honest equivalent; a `with_header`-drift bug is
   still caught, a "CLI wrapper stopped calling with_header" bug is not.
2. PDF, all eight (cfg(feature = "pdf")): 200, body starts with `%PDF`, length
   > 1000, Content-Type `application/pdf`, Content-Disposition equals the
   expected filename computed from the same stem helper.
3. no-pdf build (cfg(not(feature = "pdf"))): `format=pdf` -> 501 with
   `code == "feature_disabled"` and `message == PDF_DISABLED_MESSAGE`;
   `format=text` -> 200 on the same route. This is the case that only
   `cargo test --no-default-features --features serve` reaches.
4. Format param: missing -> 400, `format=xml` -> 400, both bad_request.
5. Shared param validation, table-driven over export routes and mirroring
   31.5's cases: from-without-to and to-without-from on pnl/register -> 400;
   month on tax -> 400; year on balance -> 400; account on pnl -> 400;
   year=abc -> 400; month=2025-13 -> 400. Same codes and messages as the
   corresponding /api/reports route (asserted by comparing both bodies).
6. Register: unknown account -> 404 not_found; known account -> 200 and the
   text body contains only that account's rows.
7. Content-Disposition/date: expected filename recomputed in the test from
   chrono::Local::now(), asserted for both formats on at least pnl and k1
   (k1 covers the slug/filename asymmetry).
8. Locked: with an encrypted, un-unlocked DB all eight export routes -> 423.
   Table-driven, added to 31.5's guard test list so a future unguarded route
   fails.
9. `GET /api/exports/all?format=text` -> 404 not_found (out-of-scope guard).
10. ANSI: §6.
11. reports::date_range_label unit tests (three branches) run in every feature
    configuration, including --no-default-features.

## 10. Verification matrix

1. cargo fmt --check
2. cargo build
3. cargo test
4. cargo test --no-default-features
5. cargo test --no-default-features --features serve   (the no-pdf serve combo:
   the only build where the 501 test in §9.3 runs)
6. cargo clippy --all-targets -- -D warnings
7. cargo clippy --no-default-features --all-targets -- -D warnings — expected
   to report exactly the two known task-34 needless_return lints
   (cli/dashboard.rs:852, cli/report/mod.rs:160) and nothing else
8. Manual smoke: `cargo run -- serve --no-open`, then for each report
   `curl -sD- -o out.pdf` and `-o out.txt` with the session cookie — confirm
   the two content types, the filename in Content-Disposition, `file out.pdf`
   says PDF, `out.txt` is ANSI-free, and that the .txt matches
   `nigel report <r> --mode export --format text` byte for byte.
<!-- SECTION:PLAN:END -->
