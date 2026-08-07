---
id: TASK-31.5
title: 'JSON API: read-only endpoints (reports and lists)'
status: Done
assignee:
  - '@agent-31.5'
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 21:02'
labels:
  - web
  - backend
dependencies:
  - TASK-31.2
  - TASK-31.3
references:
  - src/reports.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.5-read-api.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wrap the existing pure report and list functions in GET endpoints: pnl, expenses, tax, cashflow, balance, flagged, register, and k1 with year/month/from/to params matching existing CLI semantics (including the from/to must-be-a-pair rule), plus list endpoints for accounts, categories, rules, imports history, and saved CSV profile names. Handlers stay thin: open a connection per request via get_connection, call the data layer, serialize the result.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 GET endpoints exist for all eight reports with date params matching CLI semantics, including from/to pair validation
- [x] #2 GET list endpoints exist for accounts, active categories, active rules, imports history, and CSV profile names
- [x] #3 Errors map to structured JSON with appropriate HTTP status codes (unknown account 404, bad params 400)
- [x] #4 The endpoint inventory is documented (docs/api.md or OpenAPI sketch)
<!-- AC:END -->



## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
IMPLEMENTS AFTER 31.4 LANDS (routes mount inside its locked guard).

## 1. Route table

Report routes live in src/server/routes/reports.rs; list routes go into the
domain files the epic layout names, so 31.6/31.7 add writes beside their reads
instead of re-splitting: routes/accounts.rs, routes/categories.rs,
routes/rules.rs, routes/imports.rs (both /api/imports and /api/csv-profiles --
saved profiles are an import concern and 31.7 owns import endpoints).

| Route (GET) | ReportKind | Data fn | Params accepted |
|---|---|---|---|
| /api/reports/pnl | Pnl | reports::get_pnl | year, month, from+to |
| /api/reports/expenses | Expenses | reports::get_expense_breakdown | year, month |
| /api/reports/tax | Tax | reports::get_tax_summary | year |
| /api/reports/cashflow | Cashflow | reports::get_cashflow | year, month |
| /api/reports/balance | Balance | reports::get_balance | none |
| /api/reports/flagged | Flagged | reports::get_flagged | none |
| /api/reports/register | Register | reports::get_register | year, month, from+to, account |
| /api/reports/k1 | K1 | reports::get_k1_prep | year |
| /api/accounts | - | cli::accounts::list_accounts | none |
| /api/categories | - | cli::categories::list_categories | none |
| /api/rules | - | cli::rules::list_rules (NEW) | none |
| /api/imports | - | cli::undo::list_imports (NEW) | none |
| /api/csv-profiles | - | importer::list_csv_profiles (NEW) | none |

Accepted params per route mirror the clap subcommand args exactly (Tax/K1 take
only year; Expenses/Cashflow take year+month and have no from/to support in the
data layer; Balance/Flagged take nothing). /api/categories/choices is SKIPPED --
CategoryRow is a strict superset of reviewer::CategoryChoice, so the extra
endpoint does not earn its keep; the SPA uses /api/categories.

ReportKind::All is not exposed (bulk export is 31.8).

## 2. Param parsing and validation (src/server/routes/reports.rs)

axum's Query rejection is a plain-text 400 that would bypass the error
envelope, so no typed numeric fields: one raw struct whose fields are all
Option<String>, which deserializes without failing on value shape.

    #[derive(Debug, Default, Deserialize)]
    struct RawReportQuery { year, month, from, to, account }  // all Option<String>

    struct ParamSpec { kind: ReportKind, ranges: bool, account: bool }
    struct ReportParams { year: Option<i32>, month: Option<u32>,
                          from: Option<String>, to: Option<String>,
                          account: Option<String> }
    impl ReportParams { fn parse(raw: RawReportQuery, spec: ParamSpec) -> ApiResult<Self> }

parse() is the single place every 400 is produced:

- Support check first, driven by spec: kind.granularity() == None rejects year
  and month; YearOnly rejects month; !spec.ranges rejects from/to; !spec.account
  rejects account. Message names the route and the unsupported param, e.g.
  "The tax report does not accept a `month` parameter."
- year: str::parse::<i32>() -> 400 "Invalid `year`: expected a 4-digit year,
  got \"abc\"". No range clamp (CLI does not clamp).
- month: must be exactly YYYY-MM -- split once on '-', two parts, year part
  parses to i32, month part is 2 chars and parses to u32 in 1..=12. Anything
  else -> 400. This is the deliberate divergence from cli::parse_month_opt,
  which silently yields (None, None) and would otherwise hand back an
  unfiltered whole-database report.
- Effective year replicates text.rs exactly: `year.or(month_year)` -- an
  explicit `year` wins over the year embedded in `month`. Documented in
  docs/api.md.
- from/to pair rule enforced HERE, not in reports::date_filter. date_filter
  returns NigelError::Other, which the existing From<NigelError> maps to
  internal/500; the param layer catches it first and returns 400. Also validates
  each as YYYY-MM-DD (chrono NaiveDate::parse_from_str) -- stricter than the
  CLI, matching the documented convention in the epic spec.
- account: presence only here; existence is checked against the DB (below).

## 3. Response wrapper

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ReportEnvelope<T> { granularity: DateGranularity, report: T }
    impl<T: Serialize> ReportEnvelope<T> {
        fn new(kind: ReportKind, report: T) -> Self {
            Self { granularity: kind.granularity(), report }
        }
    }

granularity is never hardcoded -- it always comes from ReportKind::granularity(),
so 31.1's mapping stays the single source of truth. Flagged wraps a
Vec<FlaggedTransaction> as `report: [...]`; nested arrays are fine.

## 4. Handler pattern

Shared helper (src/server/routes/mod.rs, pub(crate)), unless 31.4 already added
an equivalent -- in which case use theirs and drop this:

    pub(crate) async fn with_conn<T, F>(state: &AppState, f: F) -> ApiResult<T>
    where F: FnOnce(&Connection) -> crate::error::Result<T> + Send + 'static,
          T: Send + 'static
    {
        let db_path = state.db_path.clone();
        tokio::task::spawn_blocking(move || {
            let conn = crate::db::get_connection(&db_path)?;
            f(&conn)
        })
        .await
        .map_err(ApiError::internal)?   // JoinError
        .map_err(ApiError::from)
    }

Every handler is then three lines: parse params, with_conn(|conn| ...), wrap.
get_connection reads the process-global password itself, so unlock state from
31.4 is picked up with no plumbing. No pool, one connection per request, per the
epic spec.

Unknown account -> 404: reports::get_register does NOT validate the account
today (an unknown name just yields zero rows). Add a private
`ensure_account_exists(conn, name) -> Result<()>` in routes/reports.rs that does
`SELECT EXISTS(...)` and returns NigelError::UnknownAccount(name) -- the
existing From<NigelError> already maps that to 404. It runs inside the same
with_conn closure as get_register (one connection, no extra round trip to the
runtime).

## 5. New data-layer functions

(a) src/cli/rules.rs -- alongside the existing CLI printers, matching how
accounts.rs/categories.rs already colocate printer + &Connection data layer.
31.6 extracts the rest of the rules data layer into the same file.

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RuleRow {
        pub id: i64, pub pattern: String, pub match_type: String,
        pub vendor: Option<String>, pub category: String, pub category_id: i64,
        pub priority: i64, pub hit_count: i64,
    }
    pub fn list_rules(conn: &Connection) -> Result<Vec<RuleRow>>

    SELECT r.id, r.pattern, r.match_type, r.vendor, c.name, r.category_id,
           r.priority, r.hit_count
    FROM rules r JOIN categories c ON r.category_id = c.id
    WHERE r.is_active = 1
    ORDER BY r.priority DESC, r.id ASC

Three changes vs rules_manager::load_rules: errors propagate instead of
collapsing to an empty Vec; vendor keeps its Option instead of coalescing to
"" (SQL NULL and an empty vendor string are different things on the wire);
category_id is selected (needed by the SPA's edit form in 31.16). The added
`r.id ASC` tiebreak makes ordering deterministic and therefore testable.

Callers switched to it in the same commit:
- rules_manager.rs deletes its private RuleRow + load_rules and imports
  cli::rules::{list_rules, RuleRow}. new() keeps today's forgiving posture with
  unwrap_or_default(); reload() surfaces an Err through the existing
  set_status(). Render sites adapt: vendor -> as_deref().unwrap_or(""),
  field rename hits -> hit_count.
- cli::rules::list() (the comfy_table printer) drops its duplicate tuple query
  and formats list_rules output.

(b) src/cli/undo.rs

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ImportListItem {
        pub id: i64, pub filename: String, pub account_name: String,
        pub import_date: String, pub transaction_count: i64,
    }
    pub fn list_imports(conn: &Connection) -> Result<Vec<ImportListItem>>

    SELECT i.id, i.filename, COALESCE(a.name, '(unknown)'), i.import_date,
           COUNT(t.id)
    FROM imports i
    LEFT JOIN accounts a ON a.id = i.account_id
    LEFT JOIN transactions t ON t.import_id = i.id
    GROUP BY i.id
    ORDER BY i.id DESC

Single pass (COUNT(t.id) ignores the NULL side, so an import with zero
transactions reports 0) rather than the per-row subquery. get_last_import
becomes a thin wrapper -- list_imports(conn)?.into_iter().next() mapped into
LastImport -- so the "most recent import" SQL exists once. LastImport keeps its
field names (import_id, ...), so undo_manager.rs and cli/undo.rs::run() are
untouched. Tradeoff: undo now reads every import row instead of LIMIT 1; import
counts are in the dozens and the query is one table pass, so this is noise.

(c) src/importer.rs -- next to save_csv_profile/load_csv_profile:

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CsvProfile { pub name: String, pub config: GenericCsvConfig }
    pub fn list_csv_profiles(conn: &Connection) -> Result<Vec<CsvProfile>>

    SELECT name, date_col, desc_col, amount_col, date_format
    FROM csv_profiles ORDER BY name

Nested config (not #[serde(flatten)]) -- GenericCsvConfig already carries
Serialize + camelCase from 31.2, and a nested object is a cleaner mirror for
31.9's types.ts.

## 6. Router wiring

Each new route module exposes `pub fn router() -> Router<AppState>`; 31.5 merges
them into whatever 31.4 named its guarded/data router so every route inherits
the 423 guard. routes/mod.rs keeps ping + the JSON 404 fallback outside the
guard. If 31.4's marker turns out to be an extractor rather than a layer, the
handlers take it as their first argument instead -- either way no route of mine
mounts outside the guard, and a test asserts that for all 13.

## 7. Error mapping summary

| Case | Code | Status |
|---|---|---|
| bad year / bad month / unsupported param / lone from or to / non-ISO date | bad_request | 400 |
| missing session cookie (existing layer) | unauthorized | 401 |
| unknown account on register | not_found | 404 |
| unknown /api path (existing fallback) | not_found | 404 |
| encrypted DB not unlocked (31.4 guard) | locked | 423 |
| SQL / IO failure, spawn_blocking JoinError | internal | 500 |

## 8. Docs

- docs/api.md: new "Reports" and "Lists" subsections under Endpoints. One block
  per route: method+path, param table (name, type, required, notes), response
  struct name(s), an abbreviated JSON example, and the error cases that route
  can produce. A leading table gives the 13-route inventory at a glance. Also
  documents the `{granularity, report}` wrapper, the three granularity strings,
  and the year-vs-month precedence rule.
- CLAUDE.md: Architecture gets the routes/ file-per-domain note and the three
  new data-layer functions (cli::rules::list_rules + public RuleRow,
  cli::undo::list_imports, importer::list_csv_profiles); Key Design Constraints
  gains one line -- the API rejects malformed month/date params with 400 where
  the CLI silently ignores them.
- README.md: no change (serve is already described; endpoint detail lives in
  docs/api.md).

## 9. Tests

New #[cfg(test)] helper module src/server/testutil.rs:
- seeded_db(dir) -> PathBuf: init_db, then fixed-date fixtures -- 2 accounts,
  ~8 transactions across 2024 and 2025 (income + expense, one flagged, one
  uncategorized, two vendors), 2 rules at different priorities, 2 import rows
  (one with zero transactions), 2 csv profiles. Fixed dates, not demo
  generation, so year/month filter assertions are stable year over year.
- app(db_path) -> (Router, cookie): builds AppState + crate::server::build_router
  (private to `server`, visible to descendant modules) and performs the
  /auth?token= exchange, mirroring the existing session_cookie_from_auth_unlocks_ping
  test.
- get_json(app, uri, cookie) -> (StatusCode, serde_json::Value).

Tests (src/server/routes/reports.rs, .../rules.rs etc., plus the data-layer
modules):
1. Each of the 13 routes returns 200 with a session over the seeded DB.
2. Each report body has `granularity` equal to the serialized
   ReportKind::granularity() and a `report` object whose figures equal a direct
   data-layer call on the same DB (serde_json::to_value(direct) == body["report"]).
3. camelCase keys asserted explicitly on at least one field per report
   (totalIncome, topVendors, runningBalance, accountName, categoryId, hitCount,
   transactionCount, dateFormat) -- guards against a future field losing its
   rename_all.
4. Param validation: from-without-to and to-without-from -> 400 bad_request on
   pnl and register; month=2025-13, month=2025, month=garbage -> 400;
   year=abc -> 400; unsupported params -> 400 (from on expenses, month on tax,
   year on balance, account on pnl).
5. year/month precedence: ?year=2024&month=2025-03 filters to 2024-03 (matches
   text.rs's year.or(my)).
6. Unknown account -> 404 not_found; known account -> 200 and rows are filtered
   to it.
7. Locked: with an encrypted, un-unlocked DB every one of the 13 routes returns
   423 -- table-driven over the route list, so a future route added outside the
   guard fails the test. Reuses 31.4's encrypted-DB test helper; runs serial
   with the password global (31.4 already flags --test-threads=1 for that file).
8. Data-layer unit tests: list_rules ordering (priority DESC then id ASC),
   vendor NULL stays None, inactive rules excluded, category_id populated;
   list_imports ordering (newest id first), transaction_count correct including
   the zero case, get_last_import still returns the same row as before;
   list_csv_profiles ordering by name and round-trip against save_csv_profile.
9. cargo test --no-default-features must still cover items 8 (data layer is
   feature-independent); server tests are cfg(feature = "serve").

## 10. Verification matrix

1. cargo fmt --check
2. cargo build
3. cargo test
4. cargo test --no-default-features
5. cargo test --no-default-features --features serve   (serve compiles without pdf/gusto)
6. cargo clippy --all-targets -- -D warnings
7. cargo clippy --no-default-features --all-targets -- -D warnings
   -- expected to report exactly the 2 known task-34 needless_return lints
   (cli/dashboard.rs:852, cli/report/mod.rs:160) and nothing else
8. Manual smoke: cargo run -- serve --no-open, then curl each of the 13 routes
   with the session cookie plus the 400/404 cases, confirming envelope shape.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented on top of 31.4 (HEAD 6f7a2f2), adapted to its landed guard shape.

## Adaptation to 31.4

31.4's locked guard became a layer over the whole /api router with a by-name
exemption list, not a route_layer on data_router(). data_router() survived as
the documented merge point, so all 13 routes mount there and inherit the 423
guard by default. No with_conn-equivalent had landed (31.4 used spawn_blocking
inline for company_name), so the planned shared helper was added to
routes/mod.rs. Per the coordinator's note, no test expects 404-while-locked:
the guard answers 423 before the fallback runs.

## Verification (8 commands)

1. cargo fmt --check                                          clean
2. cargo build                                                clean
3. cargo clippy --all-targets -- -D warnings                  clean
4. cargo clippy --no-default-features --all-targets -- -D warnings
   exactly the 2 known task-34 needless_return lints
   (cli/dashboard.rs:852, cli/report/mod.rs:160), nothing else
5. cargo test -- --test-threads=1                             382 + 25 pass
6. cargo test --no-default-features -- --test-threads=1       309 + 26 pass
7. cargo test --no-default-features --features serve -- --test-threads=1
                                                              375 + 25 pass
8. manual curl smoke, below

Note on invocation: the suite needs --test-threads=1 (the process-global DB
password mutex), which is what CI runs. Plain `cargo test` fails two of 31.4's
pre-existing unlock tests on parallelism; serial they pass.

## Manual smoke

Against `nigel serve --no-open --port 5731` over an isolated HOME with demo
data. All 13 routes returned 200 with well-formed bodies and the right
granularity: pnl/expenses/cashflow/register monthAndYear, tax/k1 yearOnly,
balance/flagged none. Spot checks: register?year=2025 narrowed to 150 rows,
register?account=BofA%20Checking returned 270 rows all matching that account,
/api/rules preserved null vendors, /api/imports and /api/csv-profiles returned
bare empty arrays (demo inserts transactions directly, so it records neither).

Error cases, all with the standard envelope:
- lone from / lone to                              400 bad_request
- month=2025-13, month=2025-3                      400 bad_request
- year=abc                                         400 bad_request
- from/to on expenses, month on tax, year on balance  400 bad_request
- from=2025-1-5 (unpadded)                         400 bad_request
- register?account=Nope                            404 not_found, names the account
- no session cookie                                401 unauthorized
- Host: evil.com                                   403 forbidden
- /api/nope                                        404 not_found

The encrypted/423 path could not be smoked by hand: `nigel password set` reads
through rpassword, which needs a real TTY and rejects a piped stdin. It is
covered instead by two table-driven integration tests that drive a genuinely
encrypted database through the real router across all 13 routes
(a_locked_database_refuses_every_data_route, unlocking_opens_every_data_route),
which is stronger evidence than a single curl would have been.

## Notes on the data layer

- get_last_import now delegates to list_imports; its five pre-existing tests
  still pass unchanged, which is the regression check for that refactor.
- rules_manager.rs lost its private RuleRow and load_rules. new() keeps the
  forgiving unwrap_or_default(); reload() now reports an error through the
  existing status line instead of silently emptying the list.
- The tuple fixture table in testutil.rs became a small Fixture struct after
  clippy::type_complexity fired on the 7-field tuple.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds the read half of the JSON API: eight report endpoints and five list
endpoints, all mounted inside 31.4's locked guard, all thin wrappers over the
existing data layer.

## Endpoints

GET /api/reports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1} and
GET /api/{accounts,categories,rules,imports,csv-profiles}.

Each report answers `{ granularity, report }`, where granularity comes from
`ReportKind::granularity()` rather than a second hardcoded table, so the SPA
(31.15) can build its date controls from the response. Lists answer with bare
JSON arrays.

## Parameter handling

Which date parameters a route accepts mirrors its `nigel report` subcommand
exactly, derived from that same granularity plus per-route ranges/account
flags. The API is deliberately stricter than the CLI in three places, because a
parameter the CLI shrugs off is a wrong answer over HTTP where nobody is
watching the screen:

- `month` must be YYYY-MM; `cli::parse_month_opt` answers a typo with
  `(None, None)`, which would silently widen the query to the whole database.
- `from`/`to` must be zero-padded YYYY-MM-DD and must come as a pair. The pair
  rule is enforced in the route layer so it surfaces as 400 rather than as
  `date_filter`'s error mapping to 500.
- A parameter a route cannot honour (`from` on expenses, `month` on tax,
  `account` on anything but the register) is 400, not ignored.

`?year=2024&month=2025-03` means March 2024, matching the CLI's
`year.or(month_year)`.

Unknown `account` on the register is 404: `get_register` reports an unknown
account as an empty register, which over HTTP is indistinguishable from an
account with no transactions, so the route checks existence first.

Query parameters are deserialized as strings and validated by hand — a typed
`year: Option<i32>` would turn `?year=abc` into an axum `Query` rejection,
which answers in plain text and would be the one response on the API that skips
the error envelope.

## Data layer

Three additions, per the epic's ownership contract:

- `cli::rules::list_rules` with a public `RuleRow`. Replaces the private,
  error-swallowing `rules_manager::load_rules`: errors propagate, `vendor`
  keeps its Option instead of coalescing NULL to the empty string,
  `category_id` is selected for the SPA's edit form, and an id tiebreak makes
  the order deterministic. `rules_manager.rs` and the `nigel rules list`
  printer both switch to it, so the query exists once.
- `cli::undo::list_imports` returning `ImportListItem`, one pass with a grouped
  count. `get_last_import` becomes its first row, so the "most recent import"
  SQL also exists once.
- `importer::list_csv_profiles` returning `CsvProfile { name, config }`.

## Tests

37 new tests. Unit tests cover parameter validation and the three new
data-layer functions (ordering, NULL vendors, zero-transaction imports,
camelCase round-trips). Integration tests run a seeded fixed-date database
through the real router: every report's figures are compared against a direct
data-layer call on the same database, camelCase keys are asserted on nested
rows where a lost rename_all would actually bite, and the error cases are
checked end to end. Two table-driven tests take a genuinely encrypted database
across all 13 routes — 423 while locked, 200 after unlock — so a future route
mounted outside the guard fails the suite.

The server test helpers 31.4 had built inline moved to `server/testutil.rs` and
are now shared rather than duplicated.

## Verification

cargo fmt/build/test pass on default, --no-default-features, and
--no-default-features --features serve. Clippy is clean on default features and
reports only the two known task-34 lints on --no-default-features. All 13
routes plus nine error cases were smoke-tested by curl against a running
server.

Docs: `docs/api.md` gains the full inventory (route table, parameter semantics,
envelope shape, list responses); CLAUDE.md gains the routes/ layout, the new
data-layer functions, and two design constraints covering the parameter
strictness and the register's 404.
<!-- SECTION:FINAL_SUMMARY:END -->
