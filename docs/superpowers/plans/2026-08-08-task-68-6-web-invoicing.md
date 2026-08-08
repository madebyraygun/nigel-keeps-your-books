# Task 68.6 — Web UI invoicing at full parity — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Spec: `docs/superpowers/specs/2026-08-08-task-68-6-web-invoicing-design.md`.
Read it first — every "why" below lives there.

**Goal:** the browser reaches the CLI's invoicing surface. Serialize derives on
the invoicing structs, JSON endpoints for clients, invoices, payments, preview,
send, sync and A/R aging behind the standard guards, and SPA screens that print
the same figures the CLI prints.

**Five stages, five PRs.** Each stage's suite is green on its own; no stage
leaves a half-wired endpoint or an unreachable screen behind.

| Stage | Scope | Depends on |
|---|---|---|
| 1 | Data layer: serde, read functions, text formatters | **68.1 merged** |
| 2 | Read API, `/api/status` capability, parity fixtures | Stage 1, **68.2 merged** |
| 3 | Write API: clients CRUD, invoice create/edit/void/pay | Stage 2 |
| 4 | Send orchestration and sync | Stage 3 |
| 5 | SPA screens | Stage 4 |

**68.1 and 68.2 are hard dependencies, not soft ones.** Their specs are in
`docs/superpowers/specs/2026-08-08-task-68-1-invoicing-edit-void-design.md` and
`…-68-2-invoice-preview-design.md`. This task consumes their data layer whole:
`ClientUpdate`, `InvoiceUpdate`, `update_client`, `update_invoice`,
`void_invoice`, `ensure_editable`, `ensure_voidable`, `client_summary`,
`validate_date`, `validate_currency`, the retyped `ensure_not_void` /
`find_invoice`, and `render_invoice`. Do not reimplement any of them.

## Global constraints

- **TDD, always.** Every task below is: write the failing test, watch it fail
  for the right reason, implement, watch it pass. A step that skips the failure
  is not done.
- `cargo test -- --test-threads=1` — the DB password is a process global.
  Also `cargo test --no-default-features -- --test-threads=1` before every
  Rust commit: the `pdf` feature gates render, and `serve` gates the routes.
- **Data-layer functions take `&Connection`** and are unit-tested with the
  `test_conn()` helper. Route handlers go through `routes::with_conn`.
- **Tests never touch the network.** The three gateway traits are faked, as
  `send.rs`'s existing tests already do.
- Money is `f64`. Cents exist only at the Stripe boundary.
- Conventional Commits (`feat:`, `test:`, `fix:`, `refactor:`, `docs:`).
- **Web work follows the Component-First UI Workflow (MANDATORY).** A component
  in `web/packages/ui/src/components/`, a co-located `.preview.ts` covering
  every visible state, a `.test.ts` ending in `describePreviewA11y(preview)`,
  and only then consumed by `web/apps/app`. No bespoke components in the app.
- **All server access goes through `web/apps/app/src/api/`.** A guard test
  fails the build on `fetch(`/XHR/WebSocket or a quoted `/api/` literal outside
  `src/api`. That includes preview URLs.
- **`docs/api.md` and `web/apps/app/src/api/types.ts` change in the same commit
  as the Rust struct.** That is the convention `docs/api.md` states about
  itself.

## Prerequisite check (do this first)

- [ ] **Step 0: confirm 68.1 and 68.2 have merged.**

```bash
rg -n "fn update_client|fn update_invoice|fn void_invoice|fn ensure_editable|fn ensure_voidable" src/invoicing/
rg -n "fn client_summary|fn validate_date|fn validate_currency|fn ensure_client_exists" src/invoicing/
rg -n "fn render_invoice|struct RenderedInvoice|enum PayButton" src/invoicing/render.rs
rg -n 'code: "void"|code: "not_draft"|code: "has_payments"|code: "already_void"' src/invoicing/
```

**If any of these is missing, stop and escalate.** Every one is 68.1's or
68.2's scope; building a second copy here guarantees a merge conflict and two
divergent guardrails. This plan starts where those two finish.

Two things 68.1 deliberately does **not** provide, which this task adds:

- `clients::delete_client` and `clients::delete_blocker` — the CLI never grew a
  `nigel client delete`, and the web manager screen needs one (Task 3.1).
- A `details.reason` on the 404s. 68.1 answers plain `NigelError::NotFound`;
  the route layer narrows it with `ApiError::not_found_because` (Task 2.1).

---

# Stage 1 — Data layer

Rust only, no HTTP. Reviewable and shippable on its own.

## Task 1.1: Serialize derives on the invoicing structs

**Files:** modify `src/models.rs`, `src/invoicing/invoices.rs`,
`src/invoicing/import_invoiceshelf.rs`.

**Interfaces:** produces camelCase JSON for `Client`, `Invoice`,
`InvoiceLineItem`, `InvoicePayment`, `InvoiceStatus`, `NewLineItem`,
`AgingBucket`, `ImportSummary`.

- [ ] **Step 1: Write the failing test** in `src/models.rs`:

```rust
#[test]
fn invoicing_structs_serialize_as_camel_case() {
    let invoice = Invoice { /* every field populated, token: "aBc123".into() */ };
    let value = serde_json::to_value(&invoice).unwrap();
    for key in ["issueDate", "dueDate", "stripePaymentLinkUrl", "publishedAt"] {
        assert!(value.get(key).is_some(), "missing {key} in {value}");
    }
    // The token is the only access control on a published invoice.
    assert!(value.get("token").is_none(), "token leaked: {value}");

    let client = Client { /* ... */ };
    assert!(serde_json::to_value(&client).unwrap().get("billingAddress").is_some());

    assert_eq!(serde_json::to_value(InvoiceStatus::Partial).unwrap(), "partial");
}
```

- [ ] **Step 2: Implement.** Add `Serialize` to the existing
  `#[derive(Debug, Clone)]` and `#[serde(rename_all = "camelCase")]` to each
  struct; `#[serde(rename_all = "lowercase")]` on `InvoiceStatus`;
  `#[serde(skip_serializing)]` on `Invoice.token`; `Deserialize` as well on
  `NewLineItem` (it is a request input). Drop `#[allow(dead_code)]` from any
  struct that is now genuinely used.

  Do **not** derive `Serialize` on `PaymentLink`, `PaidSession`, `StripeClient`,
  `R2Publisher`, `MailgunClient` or `InvoicingConfig` — three of those hold
  plaintext secrets and must stay unserializable by construction.

- [ ] **Step 3: Verify.** `cargo test -- --test-threads=1` and
  `cargo test --no-default-features -- --test-threads=1`.
- [ ] **Step 4: Commit** `feat: derive Serialize on the invoicing data structs`.

## Task 1.2: Retype `payment_amount`'s refusals

68.1 retypes the void guard and the not-found errors. It leaves one `Other`
behind, in `cli/invoice.rs::payment_amount`: "this invoice has no outstanding
balance" is a 500 today, which is the same bug 68.1 is fixing everywhere else.

**Files:** modify `src/invoicing/invoices.rs`, `src/cli/invoice.rs`.

**Interfaces:** produces `invoicing::invoices::payment_amount(&Invoice, f64,
Option<f64>) -> Result<f64>`, moved out of the CLI so both front ends share it.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn no_outstanding_balance_is_a_conflict_not_an_internal_error() {
    let err = payment_amount(&paid_invoice, 100.0, None).unwrap_err();
    assert!(matches!(err, NigelError::Conflict { code: "no_balance", .. }));
    // The CLI's sentence is unchanged, verbatim.
    assert!(err.to_string().contains("has no outstanding balance"));
}

#[test]
fn a_nan_or_negative_amount_is_invalid_not_a_junk_payment_row() {
    // NaN compares false against every bound and poisons every later SUM —
    // the reason the existing check is a negated positive, not `<= 0.0`.
    assert!(matches!(payment_amount(&inv, 0.0, Some(f64::NAN)), Err(NigelError::Invalid(_))));
    assert!(matches!(payment_amount(&inv, 0.0, Some(-5.0)), Err(NigelError::Invalid(_))));
}
```

- [ ] **Step 2: Implement.** Move the function verbatim into
  `invoicing::invoices`, swapping the two `NigelError::Other`s for
  `Conflict { code: "no_balance", message: <the same sentence> }` and
  `Invalid(<the same sentence>)`. `cli/invoice.rs::pay` calls the moved
  function. Add `total` and `paid` to the conflict by way of the message only —
  the `details` enrichment happens in the route (Stage 3), because
  `NigelError::Conflict` carries a code and a message and nothing else, and
  widening it for one call site is not worth it.
- [ ] **Step 3: Verify.** Both cargo test runs; `nigel invoice pay` on a settled
  invoice prints the sentence it printed before.
- [ ] **Step 4: Commit** `fix: retype the no-balance payment refusal as a conflict`.

## Task 1.3: Prove 68.1's guards hold for a non-CLI caller

Test-only, and the reason it is a task rather than an afterthought: every guard
68.1 adds is called from `cli/invoice.rs`. The HTTP handlers will call the data
layer directly. If any guard lives in the CLI wrapper rather than in
`send_invoice` / `record_payment` / `update_invoice` themselves, the web API
silently bypasses it — and a test written after the route exists would be
written against whatever the route happens to do.

**Files:** modify `src/invoicing/send.rs`, `src/invoicing/invoices.rs` tests.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn send_invoice_refuses_a_void_invoice_without_the_cli_wrapper() {
    // Call send_invoice directly with the fakes. It must refuse, and the fake
    // gateway's create_calls must still be 0 — refused before any network call.
}

#[test]
fn record_payment_refuses_a_void_invoice_without_the_cli_wrapper() { … }

#[test]
fn update_invoice_refuses_a_published_invoice_without_the_cli_wrapper() { … }
```

- [ ] **Step 2: Implement.** If a test fails, the fix is to move that guard call
  from `cli/invoice.rs` into the data-layer function, leaving the CLI's call in
  place (a second check costs one query and fails earlier with the same
  message). If all three pass, the tests stand as regression cover and no
  production code changes — which is a legitimate outcome for this task.
- [ ] **Step 3: Verify.** Both cargo test runs; every existing `send.rs` test
  passes unchanged.
- [ ] **Step 4: Commit** `test: assert the invoicing guards hold for direct callers`.

## Task 1.4: `list_invoices`, `list_payments`, and payment-method validation

**Files:** modify `src/invoicing/invoices.rs`, `src/cli/invoice.rs`.

**Interfaces:** produces `InvoiceListRow`, `list_invoices`, `list_payments`,
`validate_payment_method`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn list_invoices_orders_by_number_desc_and_carries_the_balance() { … }

#[test]
fn list_invoices_keeps_an_invoice_whose_client_row_is_missing() {
    // The CLI's INNER JOIN drops it silently. A LEFT JOIN keeps the row with
    // client_name: None, because a list that hides invoices is worse than one
    // that shows a dash.
}

#[test]
fn list_invoices_computes_paid_in_one_aggregate_not_one_query_per_row() {
    // Assert on results, not on query count: seed 3 invoices with 5 payments
    // spread across them and check every balance in one call.
}

#[test]
fn list_payments_returns_rows_in_paid_date_order() { … }

#[test]
fn an_unknown_payment_method_is_invalid_not_a_constraint_violation() {
    let err = validate_payment_method("bitcoin").unwrap_err();
    assert!(matches!(err, NigelError::Invalid(_)));
    assert!(err.to_string().contains("direct_deposit"));  // names the legal set
}
```

- [ ] **Step 2: Implement.**

```rust
pub fn list_invoices(conn: &Connection, status: Option<&str>, client_id: Option<i64>)
    -> Result<Vec<InvoiceListRow>>;
pub fn list_payments(conn: &Connection, invoice_id: i64) -> Result<Vec<InvoicePayment>>;
pub fn validate_payment_method(method: &str) -> Result<()>;
```

`status` accepts the six status words plus `open` (`sent,partial,overdue`).
`list_invoices` uses a `LEFT JOIN clients` and a
`LEFT JOIN (SELECT invoice_id, SUM(amount) … GROUP BY invoice_id)` so paid
amounts cost one aggregate. Call `validate_payment_method` from
`record_payment`.

- [ ] **Step 3: Verify.** Both cargo test runs; `nigel invoice list` output is
  unchanged at this point (it still uses its own SQL — 1.6 switches it).
- [ ] **Step 4: Commit** `feat: add invoice and payment list functions to the data layer`.

## Task 1.5: `AgingReport` with `ar_aging` re-expressed on top of it

**Files:** modify `src/invoicing/invoices.rs`.

**Interfaces:** produces `AgingReport`, `AgingInvoice`, `ar_aging_report`;
`ar_aging` becomes `ar_aging_report(...).map(|r| r.buckets)`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn ar_aging_report_buckets_match_ar_aging_exactly() {
    // The whole point of the refactor: the CLI's five figures cannot drift
    // from the web's, because there is only one computation.
    let report = ar_aging_report(&conn, "2026-03-15").unwrap();
    assert_eq!(report.buckets, ar_aging(&conn, "2026-03-15").unwrap());
}

#[test]
fn ar_aging_report_total_is_the_sum_of_its_buckets() { … }

#[test]
fn ar_aging_report_lists_each_open_invoice_with_its_bucket_and_days_past_due() { … }

#[test]
fn ar_aging_report_excludes_draft_paid_and_void() { … }

#[test]
fn a_malformed_as_of_date_is_invalid_not_other() {
    assert!(matches!(ar_aging_report(&conn, "March"), Err(NigelError::Invalid(_))));
}
```

- [ ] **Step 2: Implement.** `AgingBucket.label` stays `&'static str` (it
  serializes fine). `AgingInvoice { number, client_name, due_date,
  days_past_due, balance, bucket }`. One pass over `list_invoices(conn,
  Some("open"), None)` rather than the current per-row `paid_amount`.
- [ ] **Step 3: Verify.** Both cargo test runs; `nigel invoice aging` prints
  identical figures.
- [ ] **Step 4: Commit** `refactor: express ar_aging in terms of a fuller AgingReport`.

## Task 1.6: Extract the CLI text formatters

**Files:** modify `src/cli/invoice.rs`, `src/cli/client.rs`.

**Interfaces:** produces `format_invoice_list`, `format_invoice_show`,
`format_aging`, `format_client_list`, each `-> String`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn format_invoice_list_prints_the_same_columns_it_always_has() {
    let out = format_invoice_list(&rows);
    assert!(out.starts_with("Invoices\n"));
    for header in ["#", "Status", "Client", "Total", "Due"] {
        assert!(out.contains(header), "missing {header} in\n{out}");
    }
    assert!(out.contains("$1,850.00"));   // fmt::money, not {:.2} — spec §6
    assert!(out.contains("1252"));         // number DESC: newest first
}

#[test]
fn format_invoice_show_prints_the_line_items_paid_and_balance() { … }

#[test]
fn format_aging_prints_five_right_aligned_buckets_and_a_total() { … }

#[test]
fn format_client_list_prints_an_em_dash_for_a_client_with_no_email() { … }
```

- [ ] **Step 2: Implement.** Pure functions taking already-fetched data,
  mirroring `cli/report/text.rs`. Every money figure goes through
  `crate::fmt::money`. `list()`, `show()`, `aging()` and `client::list()` become
  fetch-then-`println!("{}", format_*(…))`. `list()` now calls
  `invoices::list_invoices`, deleting its inlined SQL.
- [ ] **Step 3: Verify.** Both cargo test runs. Run `nigel invoice list`,
  `show`, `aging` and `client list` against a demo database and eyeball the
  output: same rows, same order, money now `$1,850.00`.
- [ ] **Step 4: Commit** `refactor: extract the invoicing CLI output as pure formatters`.

## Task 1.7: Stage 1 documentation

- [ ] Update `CLAUDE.md`'s Invoicing architecture entry: the new data-layer
  functions, the typed errors, the formatters.
- [ ] Commit `docs: describe the invoicing data-layer additions`.
- [ ] **Open the Stage 1 PR.** `cargo test` and
  `cargo test --no-default-features` green, `cargo clippy` clean.

---

# Stage 2 — Read API

## Task 2.1: `routes/clients.rs` — the GET half

**Files:** create `src/server/routes/clients.rs`; modify
`src/server/routes/mod.rs`, `src/server/testutil.rs`.

**Interfaces:** produces `GET /api/clients`, `GET /api/clients/{id}`.

- [ ] **Step 1: Write the failing tests** in `routes/clients.rs`, following
  `accounts.rs`'s test module shape:

```rust
#[tokio::test]
async fn clients_list_matches_the_data_layer() {
    let (_dir, db_path) = seeded_invoicing_db();
    let (app, token) = app_for(&db_path);
    let body = ok_json(&app, "/api/clients", &token).await;
    let rows = body.as_array().expect("a bare array");
    assert_eq!(rows[0]["name"], "Acme Co");
    assert!(rows[0].get("billingAddress").is_some());
}

#[tokio::test]
async fn a_client_detail_carries_its_invoices_and_open_balance() { … }

#[tokio::test]
async fn an_unknown_client_id_is_404_with_a_reason() {
    // details.reason == "client_not_found"
}
```

- [ ] **Step 2: Implement.** `seeded_invoicing_db()` goes into `testutil.rs`
  first (Task 2.5 formalises its contents, but the shape is needed here): three
  clients — one with no email — six invoices covering all six statuses with
  literal dates, and payments including one carrying a Stripe session id.

  Then the module: `routes()` mounting `/clients` and `/clients/{id}`,
  `list` and `detail` handlers through `with_conn`, and `ClientDetail` as the
  camelCase wrapper over 68.1's `ClientSummary` — `client_summary` already
  returns the client, the invoice history and the outstanding balance in one
  round trip, so the route adds serde and nothing else. Merge into
  `data_router()`. Narrow the 404 to `client_not_found` (see Task 2.2).

- [ ] **Step 3: Verify.** Add `/api/clients` and `/api/clients/1` to
  `testutil::DATA_ROUTES` (bump the array length) and confirm the locked-guard
  and session tests in `src/server/mod.rs` cover them without edits — that is
  the point of the layered guard.
- [ ] **Step 4: Commit** `feat: serve clients over the JSON API`.

## Task 2.2: `routes/invoices.rs` — list, detail, aging, next-number

**Files:** create `src/server/routes/invoices.rs`; modify `routes/mod.rs`,
`testutil.rs`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[tokio::test]
async fn invoices_list_is_newest_first_and_carries_the_balance() { … }

#[tokio::test]
async fn the_list_can_be_filtered_by_status_and_client() {
    // ?status=open returns exactly the sent/partial/overdue rows
}

#[tokio::test]
async fn an_unknown_status_filter_is_a_400_naming_the_legal_set() { … }

#[tokio::test]
async fn an_unknown_client_id_filter_is_a_404_not_an_empty_list() {
    // The ensure_account_exists reasoning: filtering by something that does not
    // exist is a wrong question, not an empty answer.
}

#[tokio::test]
async fn an_invoice_detail_carries_items_payments_flags_and_no_token() {
    let body = ok_json(&app, "/api/invoices/1250", &token).await;
    assert!(body.get("token").is_none(), "token leaked: {body}");
    assert_eq!(body["canEdit"], false);       // 1250 is partial
    assert_eq!(body["canPay"], true);
    assert!(body["items"].as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn a_draft_invoice_can_be_edited_and_a_void_one_can_do_nothing() {
    // canEdit true only for draft; canSend/canPay/canVoid all false for void
}

#[tokio::test]
async fn aging_takes_an_as_of_date_and_defaults_to_today() { … }

#[tokio::test]
async fn a_malformed_as_of_is_a_400() {
    // The API is stricter with dates than the CLI (CLAUDE.md); "2026-3-1" is
    // a 400, not a silently widened query.
}
```

- [ ] **Step 2: Implement.** Routes:
  `/invoices` (get), `/invoices/aging` (get), `/invoices/next-number` (get),
  `/invoices/{number}` (get). **Mount the two literal paths before the
  `{number}` pattern** and add a test that `/api/invoices/aging` is not parsed
  as invoice number "aging" (axum prefers literals, but the test is what stops
  a future refactor from breaking it).

  `asOf` is validated with the same zero-padded `YYYY-MM-DD` rule
  `routes/reports.rs` enforces — reuse its parser rather than writing a second
  one.

  `InvoiceDetail` uses `#[serde(flatten)]` over `Invoice` and computes
  `publicUrl` from `settings::invoicing_config().public_base_url` + token,
  `None` when unpublished or unconfigured.

  The `can*` flags are 68.1's guards, **called, not reimplemented**:
  `canEdit = ensure_editable(conn, &inv).is_ok()`,
  `canVoid = ensure_voidable(...)`, `canSend`/`canPay` = `ensure_not_void` plus
  the balance and email checks. 68.1 blocks an edit on `has_payments` as well as
  on status, and a status-only re-derivation would get that wrong.

  **Narrow the 404s in the route.** 68.1 answers plain `NigelError::NotFound`,
  which becomes a 404 with no `details.reason`. A handler that looks up both an
  invoice and a client owes the caller the distinction — the exact case
  `ApiError::not_found_because` was added for, and the exact bug it fixed on the
  review screen. Wrap each lookup: `invoice_not_found` inside an invoice
  handler, `client_not_found` for a client lookup. No new error variant, and no
  68.1 file is touched.

- [ ] **Step 3: Verify.** Extend `DATA_ROUTES` with `/api/invoices`,
  `/api/invoices/1248`, `/api/invoices/aging`, `/api/invoices/next-number`.
  Both cargo test runs.
- [ ] **Step 4: Commit** `feat: serve invoices, detail and A/R aging over the JSON API`.

## Task 2.3: The `invoicing` object on `/api/status`

**Files:** modify `src/server/routes/status.rs`; `src/settings.rs`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[tokio::test]
async fn status_reports_which_invoicing_config_is_missing_by_name() {
    let status = status_json(&app, &token).await;
    assert_eq!(status["invoicing"]["sendConfigured"], false);
    let missing = status["invoicing"]["missing"].as_array().unwrap();
    assert!(missing.contains(&serde_json::json!("r2_bucket")));
}

#[test]
fn the_invoicing_status_never_carries_a_value() {
    // Serialize a fully-populated InvoicingConfig's status and assert the
    // rendered JSON contains none of the secret values.
}
```

- [ ] **Step 2: Implement.** `settings::invoicing_status(&InvoicingConfig)
  -> InvoicingStatus { send_configured, sync_configured, missing: Vec<&'static str> }`,
  `missing` in `docs/invoicing.md`'s key order. Add the field to
  `StatusResponse`. Key names only.
- [ ] **Step 3: Verify.** Both cargo test runs.
- [ ] **Step 4: Commit** `feat: report invoicing configuration state on /api/status`.

## Task 2.4: The two preview routes over 68.2's renderer

68.2 owns the rendering. This task adds two routes and nothing else — if you
find yourself writing render code, 68.2 has not merged (see Step 0).

**Files:** modify `src/server/routes/invoices.rs`.

**Interfaces:** consumes `invoicing::render::render_invoice(conn, &Invoice,
&Client, PayButton, contact_email) -> Result<RenderedInvoice>`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[tokio::test]
async fn the_preview_route_answers_html_with_a_sandbox_csp() {
    let response = get_response(&app, "/api/invoices/1248/preview", &token).await;
    assert_eq!(content_type(&response), "text/html; charset=utf-8");
    assert_eq!(header_str(&response, header::CONTENT_SECURITY_POLICY), "sandbox");
    assert!(body_string(response).await.contains("1248"));
}

#[tokio::test]
async fn a_draft_previews_with_a_placeholder_pay_button() {
    // 1252 is the seeded draft: PayButton::placeholder, no Stripe link, and no
    // network call — the route takes no gateway, which is the proof.
}

#[tokio::test]
async fn a_published_invoice_previews_with_its_real_pay_link() { … }

#[tokio::test]
async fn preview_works_with_no_invoicing_config_set() {
    // 68.2 AC #2. from_email unset must not 500 or render an empty contact.
}

#[tokio::test]
async fn preview_pdf_is_501_without_the_feature_and_bytes_with_it() {
    // Gated on cfg(feature = "pdf") in the test itself, both directions.
    // The 501 carries code "feature_disabled" in the envelope.
}

#[tokio::test]
async fn previewing_an_unknown_invoice_is_a_404_in_the_envelope() {
    // A non-JSON success route still answers errors as JSON — the exports.rs
    // property, restated here because it is easy to lose on a byte route.
}
```

- [ ] **Step 2: Implement.** Two routes, `/invoices/{number}/preview` and
  `/invoices/{number}/preview.pdf`, through `with_conn_api` — not `with_conn`,
  because the 501 is a distinction `NigelError` cannot carry, the same reason
  `exports.rs` uses it. Each loads the invoice and client, picks
  `PayButton::live(url)` or `PayButton::placeholder` from
  `stripe_payment_link_url`, and passes
  `invoicing_config().from_email.unwrap_or_default()` as the contact.

  The HTML response sets `Content-Type: text/html; charset=utf-8`,
  `Content-Security-Policy: sandbox` and `X-Content-Type-Options: nosniff`. The
  PDF response sets `Content-Type: application/pdf` and a `Content-Disposition`
  filename of `invoice-{number}.pdf`, mirroring `exports.rs`'s naming.

  `RenderedInvoice.pdf == None` becomes
  `ApiError::feature_disabled(cli::report::PDF_DISABLED_MESSAGE)` — the same
  sentence the CLI prints, the same handling `exports.rs` gives it.
- [ ] **Step 3: Verify.** Both cargo test runs — the no-`pdf` build must
  exercise the 501 path and the HTML route must still work in it.
- [ ] **Step 4: Commit** `feat: preview an invoice's rendered HTML and PDF over HTTP`.

## Task 2.5: Parity fixtures

**Files:** modify `src/server/testutil.rs`, `src/server/fixture_capture.rs`.

- [ ] **Step 1: Write the failing test.** Extend `fixture_capture.rs` with
  `capture_web_invoicing_fixtures`, `#[ignore]`d like its neighbour, and a
  non-ignored guard test asserting the four fixture pairs exist and parse.

- [ ] **Step 2: Implement.**
  - `seeded_invoicing_db()` in `testutil.rs`, dates as literals: clients Acme Co
    (`ap@acme.test`), Globex (no email), Northwind Traders; invoices 1247 (void),
    1248 (paid), 1249 (overdue), 1250 (partial), 1251 (sent), 1252 (draft);
    payments including one with a `stripe_checkout_session_id`.
  - `const AS_OF: &str = "2026-03-15";` — fixed for the same reason
    `YEAR = "2025"` is.
  - Four capture pairs into `web/apps/app/src/__fixtures__/invoicing/`:
    `invoices.json`/`.txt`, `invoice-1250.json`/`.txt`, `aging.json`/`.txt`,
    `clients.json`/`.txt`. The `.json` side is a real router response with a
    real session; the `.txt` side is `cli::invoice::format_*` called directly
    (there is no invoice export route — spec §2.10).
  - `manifest.json` in the reports manifest's shape.

- [ ] **Step 3: Verify.**
  `cargo test --features serve capture_web_invoicing_fixtures -- --ignored --nocapture`,
  then re-run and confirm `git diff` is empty — a capture that is not
  deterministic is not a fixture.
- [ ] **Step 4: Commit** `test: capture invoicing parity fixtures for the SPA`.

## Task 2.6: Stage 2 documentation

- [ ] `docs/api.md`: an "Invoicing" section under "Reading data" covering the
  seven GET routes, their parameters, `InvoiceDetail`'s shape, the `can*` flags,
  the two non-JSON preview routes, and the `invoicing` object on `/api/status`.
  Add `invoice_not_found` / `client_not_found` to the not-found reasons.
- [ ] `web/apps/app/src/api/types.ts`: the read-side interfaces and the
  vocabulary arrays. Same commit as the doc.
- [ ] `CLAUDE.md`: architecture entries for the two route modules.
- [ ] Commit `docs: document the invoicing read API`.
- [ ] **Open the Stage 2 PR.**

---

# Stage 3 — Write API

## Task 3.1: Client create, update, delete

**Files:** modify `src/server/routes/clients.rs`, `src/invoicing/clients.rs`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[tokio::test]
async fn a_client_can_be_created_edited_and_deleted() {
    // POST 201 → PATCH 200 → DELETE 200 {deleted: true}, then absent from the list
}

#[tokio::test]
async fn a_duplicate_client_name_is_a_409_with_the_name() { … }

#[tokio::test]
async fn a_patch_can_clear_an_email_but_omitting_it_leaves_it() {
    // double_option: {"email": null} clears, {} is a 400
}

#[tokio::test]
async fn an_all_absent_client_patch_is_a_400() { … }

#[tokio::test]
async fn deleting_a_client_with_invoices_is_blocked_with_a_count() {
    assert_eq!(body["error"]["details"]["reason"], "has_invoices");
    assert_eq!(body["error"]["details"]["count"], 3);
}

#[tokio::test]
async fn an_empty_client_name_is_a_400() { … }
```

- [ ] **Step 2: Implement.** `NewClientRequest` / `ClientPatch` with
  `double_option` on the three nullable fields — they deserialize straight into
  68.1's `ClientUpdate`, field for field, with no translation layer. An
  all-absent patch is caught by `ClientUpdate::is_empty()`, which 68.1 already
  provides; the route turns that into the 400.

  **The one new data-layer function:** `clients::delete_client` and
  `clients::delete_blocker`, modelled line for line on
  `accounts::delete_blocker`, raising `NigelError::Blocked(DeleteBlock)` with a
  new `BlockReason::HasInvoices` whose `reason_code()` is `has_invoices`. That
  puts it in the same mechanism as the other two block reasons rather than
  beside it, and gives 68.4's TUI client manager the same guard for free.
  `DeleteBlock::Display` gets its sentence in the existing shape:
  `Cannot delete: client has 3 invoices`.

  If `add_client` does not already reject a duplicate or empty name, add it in
  the data layer (the `accounts::add_account` precedent), not in the route.
- [ ] **Step 3: Verify.** Extend `WRITE_ROUTES` with the three entries and bump
  its length. Both cargo test runs.
- [ ] **Step 4: Commit** `feat: create, edit and delete clients over the JSON API`.

## Task 3.2: Invoice create and edit

**Files:** modify `src/server/routes/invoices.rs`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[tokio::test]
async fn an_invoice_can_be_created_with_line_items() {
    // 201, number == next_number, total == sum of qty*unit, status "draft"
}

#[tokio::test]
async fn creating_an_invoice_with_no_items_is_a_400() { … }

#[tokio::test]
async fn a_line_item_description_may_contain_a_colon() {
    // The CLI's "desc:qty:unit" restriction is an argv artifact, not a rule.
}

#[tokio::test]
async fn a_non_finite_quantity_is_a_400_not_a_nan_row() {
    // The payment_amount reasoning: a NaN poisons every later SUM.
}

#[tokio::test]
async fn an_invoice_totalling_zero_is_a_400() { … }

#[tokio::test]
async fn creating_an_invoice_for_an_unknown_client_is_a_404() { … }

#[tokio::test]
async fn patching_items_replaces_the_whole_list_and_recomputes_the_total() { … }

#[tokio::test]
async fn patching_a_published_invoice_is_a_409_naming_its_status() {
    assert_eq!(body["error"]["details"]["reason"], "not_draft");
    assert_eq!(body["error"]["details"]["status"], "sent");
}

#[tokio::test]
async fn patching_a_void_invoice_is_a_409_void() { … }

#[tokio::test]
async fn patching_a_draft_that_has_payments_is_a_409_has_payments() {
    // 68.1 blocks on payments as well as on status. canEdit must agree.
}

#[tokio::test]
async fn editing_the_total_clears_a_stale_stripe_payment_link() {
    // 68.1 clears it; assert the detail response comes back without one, so
    // the SPA cannot show a link that now bills the wrong amount.
}

#[tokio::test]
async fn a_patch_can_clear_a_due_date_and_omitting_it_leaves_it() { … }

#[tokio::test]
async fn every_invoice_write_answers_with_the_whole_detail() {
    // A due-date patch that flips the derived status must not answer with a
    // body that still says the old one.
}
```

- [ ] **Step 2: Implement.** `NewInvoiceRequest` and `InvoicePatch` as specced,
  deserializing into 68.1's `InvoiceUpdate` (its `items: Some(v)` already
  replaces the whole line-item set, recomputes the totals and clears a stale
  payment link, all in one transaction). Validate through 68.1's `validate_date`
  and `validate_currency` — no second date parser. `ensure_editable` is the
  draft-only guard, read inside the transaction from the *current* status, never
  from a value the client sent.
  Both run inside one `unchecked_transaction`. Validate dates with the same
  parser the report routes use. Draft-only enforcement reads the *current*
  status inside the transaction, not a value the client sent.
- [ ] **Step 3: Verify.** `WRITE_ROUTES` extended. Both cargo test runs.
- [ ] **Step 4: Commit** `feat: create and edit invoices over the JSON API`.

## Task 3.3: Void and pay

**Files:** modify `src/server/routes/invoices.rs`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[tokio::test]
async fn voiding_an_invoice_makes_it_refuse_send_and_pay() { … }

#[tokio::test]
async fn voiding_a_void_invoice_is_a_409_already_void() { … }

#[tokio::test]
async fn voiding_a_paid_invoice_is_a_409_carrying_paid_and_total() { … }

#[tokio::test]
async fn a_payment_defaults_to_the_whole_outstanding_balance() { … }

#[tokio::test]
async fn a_partial_payment_moves_the_status_to_partial_and_a_full_one_to_paid() { … }

#[tokio::test]
async fn paying_with_nothing_outstanding_is_a_409_no_balance() { … }

#[tokio::test]
async fn an_unknown_payment_method_is_a_400_naming_the_legal_set() {
    // Not a rusqlite CHECK violation surfacing as a 500.
}

#[tokio::test]
async fn a_negative_or_nan_payment_amount_is_a_400() { … }
```

- [ ] **Step 2: Implement.** `POST /invoices/{number}/void` calls 68.1's
  `ensure_voidable` then `void_invoice(conn, id, today)` — 68.1 writes
  `voided_at` and lets `refresh_status` derive the status, so the route passes
  the server's local today, the same value `send` and `pay` pass. `POST
  /invoices/{number}/pay` calls Task 1.2's moved `payment_amount` and
  `record_payment`.

  The route enriches the conflicts 68.1 raises with the numbers a screen wants:
  `has_payments` gains `paid`, `no_balance` gains `total` and `paid`. That
  enrichment happens here rather than in `NigelError::Conflict`, which carries a
  code and a message and nothing else — widening it for two call sites is not
  worth it, and the route already has the invoice in hand.

  Both answer `InvoiceDetail`.
- [ ] **Step 3: Verify.** `WRITE_ROUTES` extended. Both cargo test runs, plus
  `nigel invoice pay` still prints what it printed.
- [ ] **Step 4: Commit** `feat: void invoices and record payments over the JSON API`.

## Task 3.4: Stage 3 documentation

- [ ] `docs/api.md`: a "Invoicing" section under "Changing data"; the new
  conflict reasons added to the existing table.
- [ ] `web/apps/app/src/api/types.ts`: the write-side interfaces.
- [ ] Commit `docs: document the invoicing write API`.
- [ ] **Open the Stage 3 PR.**

---

# Stage 4 — Send and sync

## Task 4.1: Timeouts on the three external clients

Do this first. It is a standalone bug fix and everything after it inherits the
bound.

**Files:** modify `src/invoicing/stripe.rs`, `r2.rs`, `mailgun.rs`.

- [ ] **Step 1: Write the failing test.** A unit test asserting the shared
  builder produces a client with a total timeout set:

```rust
#[test]
fn the_http_client_has_a_bounded_timeout() {
    // http_client() is the one constructor; assert it is used by all three by
    // grepping is not a test — instead give each client a `fn client(&self)`
    // that returns http_client() and assert the builder does not panic and the
    // constants are what we intend.
    assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
    assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(30));
}
```

- [ ] **Step 2: Implement.** One `pub(crate) fn http_client() -> reqwest::blocking::Client`
  in `src/invoicing/mod.rs` with `.connect_timeout(10s).timeout(30s)`, replacing
  all four `reqwest::blocking::Client::new()` call sites. A CLI user gets a
  bounded failure instead of a hung terminal; a server gets its blocking thread
  back.
- [ ] **Step 3: Verify.** Both cargo test runs.
- [ ] **Step 4: Commit** `fix: bound the invoicing HTTP clients with timeouts`.

## Task 4.2: Step-tagged send

**Files:** modify `src/invoicing/send.rs`, `src/cli/invoice.rs`.

- [ ] **Step 1: Write the failing tests** in `send.rs`, reusing the existing
  fakes and adding one that fails at a chosen step:

```rust
#[test]
fn a_successful_send_reports_every_step_and_marks_the_link_reused() {
    let outcome = send_invoice_traced(…).unwrap();
    assert_eq!(outcome.steps.last().unwrap().0, SendStep::Record);
    // second send: payment_link is Reused, not Ok
}

#[test]
fn a_publish_failure_names_the_step_and_says_no_email_went_out() {
    let failure = send_invoice_traced(&conn, id, …, &FailPub, &mail).unwrap_err();
    assert_eq!(failure.step, SendStep::Publish);
    assert!(!failure.email_sent);
    assert_eq!(failure.completed, vec![SendStep::Precheck, SendStep::PaymentLink, SendStep::Render]);
    assert_eq!(get_invoice(&conn, id).unwrap().status, "draft");
}

#[test]
fn a_record_failure_reports_that_the_email_already_went_out() {
    // The one failure that is not safe to retry.
}

#[test]
fn a_client_with_no_email_fails_at_precheck_before_any_network_call() {
    // Assert the fake gateway's create_calls is still 0.
}

#[test]
fn send_invoice_still_returns_the_public_url_and_the_same_error_text() {
    // The wrapper's contract: cli/invoice.rs and its tests are untouched.
}
```

- [ ] **Step 2: Implement.** `SendStep`, `StepOutcome`, `SendOutcome`,
  `SendFailure` as specced. `send_invoice_traced` is the real orchestration;
  `send_invoice` becomes
  `send_invoice_traced(...).map(|o| o.public_url).map_err(|f| f.source)`.
  The precheck step calls 68.1's `ensure_not_void` and the email and total
  checks. **All four existing `send.rs` tests must pass unchanged** — if one
  needs editing, the wrapper is wrong.
- [ ] **Step 3: Verify.** Both cargo test runs.
- [ ] **Step 4: Commit** `feat: report which step of a send failed`.

## Task 4.3: `SyncReport`

**Files:** modify `src/invoicing/sync.rs`, `src/cli/invoice.rs`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn sync_all_report_names_the_invoices_that_failed() {
    // A gateway that fails for #1249 and succeeds for #1250 returns
    // recorded: 1, failures: [(1249, "…")] — instead of eprintln!ing it.
}

#[test]
fn sync_all_report_errors_only_when_every_invoice_failed() { … }

#[test]
fn sync_all_still_returns_the_same_count_the_cli_prints() { … }
```

- [ ] **Step 2: Implement.** `sync_all_report(conn, today, gateway)
  -> Result<SyncReport>` holding the real logic; `sync_all` becomes
  `sync_all_report(...).map(|r| r.recorded)` and the CLI keeps its `eprintln!`
  by iterating `report.failures` in `cli/invoice.rs`, where printing belongs.
- [ ] **Step 3: Verify.** Both cargo test runs; `nigel invoice sync` output
  unchanged, including the per-invoice notices.
- [ ] **Step 4: Commit** `refactor: return sync failures instead of printing them`.

## Task 4.4: The send and sync routes

**Files:** modify `src/server/error.rs`, `src/server/routes/invoices.rs`.

- [ ] **Step 1: Write the failing tests.** These need a seam for the fakes —
  add `AppState`-independent constructor functions in `routes/invoices.rs`
  (`fn send_with(conn, number, gateway, publisher, mailer)`) so the handler is a
  thin wrapper around a testable function, exactly as the export routes split
  fetch from render:

```rust
#[tokio::test]
async fn send_without_confirmation_is_a_400() {
    assert_eq!(body["error"]["details"]["reason"], "confirmation_required");
}

#[tokio::test]
async fn send_with_no_invoicing_config_is_a_409_naming_the_missing_keys() {
    assert_eq!(body["error"]["details"]["reason"], "send_not_configured");
    // details.missing contains "r2_bucket"; contains no secret values
}

#[tokio::test]
async fn a_publish_failure_is_a_502_naming_the_step_and_the_service() {
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"]["code"], "upstream_failed");
    assert_eq!(body["error"]["details"]["step"], "publish");
    assert_eq!(body["error"]["details"]["service"], "r2");
    assert_eq!(body["error"]["details"]["emailSent"], false);
    assert_eq!(body["error"]["details"]["invoiceStatus"], "draft");
}

#[tokio::test]
async fn a_render_failure_without_the_pdf_feature_is_a_501() { … }

#[tokio::test]
async fn sending_a_void_invoice_is_a_409_before_any_network_call() { … }

#[tokio::test]
async fn a_successful_send_answers_with_the_step_trace_and_the_public_url() { … }

#[tokio::test]
async fn sync_reports_recorded_checked_and_per_invoice_failures() { … }

#[test]
fn a_send_failure_body_never_carries_a_secret() {
    // Render every SendFailure variant through ApiError and assert the JSON
    // contains no key material — r2.rs and mailgun.rs put the status and body
    // of the upstream response into the message.
}
```

- [ ] **Step 2: Implement.** `ApiErrorCode::UpstreamFailed` → 502,
  `"upstream_failed"`, added to the status and `as_str` matches (they are
  exhaustive, so the compiler finds every site). `From<SendFailure> for ApiError`
  does the step→code mapping in **one** place — a `match` on `SendStep`, so a
  new step cannot be added without deciding its code.

  `POST /invoices/{number}/send` requires `{"confirm": true}`, builds the three
  clients from `invoicing_config()`, and runs `send_invoice_traced` inside
  `with_conn_api`. `POST /invoices/sync` needs only the gateway.

  **The upstream's message is passed through as the `message`, never
  reconstructed.** `r2 403: SignatureDoesNotMatch` is the only information
  anyone has about why R2 refused.

- [ ] **Step 3: Verify.** `WRITE_ROUTES` extended with both. Both cargo test
  runs. Confirm no test performs a real network call
  (`rg -n "reqwest" src/server/routes/invoices.rs` should find nothing).
- [ ] **Step 4: Commit** `feat: send and sync invoices over the JSON API`.

## Task 4.5: Stage 4 documentation

- [ ] `docs/api.md`: `upstream_failed` in the error-code table; the send step
  vocabulary and its example failure envelope; the `confirm` requirement; the
  sync response shape.
- [ ] `docs/invoicing.md`: a "From the web UI" section noting that send over
  HTTP requires explicit confirmation and reports the failing step.
- [ ] `CLAUDE.md`: Key Design Constraints entries for the blocking-send decision
  (with the timeout bound), the token exclusion, and the `asOf` fixture reason.
- [ ] `web/apps/app/src/api/types.ts`: `SendResult`, `SendStepResult`,
  `SyncResult`, `InvoicingStatus`, and `upstream_failed` in `API_ERROR_CODES`.
- [ ] Commit `docs: document the invoicing send and sync API`.
- [ ] **Open the Stage 4 PR.**

---

# Stage 5 — SPA screens

Every component task is: preview first, a11y test, then consume. The preview
harness is `npm run preview` on :9090.

## Task 5.1: `ApiClient` methods and the fake

**Files:** modify `web/apps/app/src/api/client.ts`,
`web/apps/app/src/__mocks__/fake-api-client.ts`.

- [ ] **Step 1: Write the failing tests** in the api client's test file: each
  new method hits the right path and method, `getInvoices` omits absent query
  parameters entirely (the server 400s on unsupported ones), `sendInvoice`
  always sends `{confirm: true}`, and `invoicePreviewUrl` builds
  `/api/invoices/1248/preview` and `…/preview.pdf`.
- [ ] **Step 2: Implement** the seventeen methods from spec §4 plus
  `invoicePreviewUrl`, and the matching `FakeApiClient` stubs.
- [ ] **Step 3: Verify.** `npm test`, `npm run typecheck`, `npm run lint` in
  `web/`. Confirm the api-seam guard test still passes.
- [ ] **Step 4: Commit** `feat(web): add the invoicing methods to the api client`.

## Task 5.2: Presentational components

**Files:** create in `web/packages/ui/src/components/`; modify `index.ts` and
`src/icons/icons.ts`.

Order matters — later components consume earlier ones.

- [ ] `wc-icon-invoice`, `wc-icon-clients` in `icons.ts`.
- [ ] `wc-invoice-status` — glyph **and** word, never colour alone (WCAG 1.4.1,
  the `wc-money` reasoning). Preview: all six statuses.
- [ ] `wc-money` reuse check — no new money formatter anywhere.
- [ ] `wc-aging-bars` — five labelled bars plus a total. Preview: all zero, one
  bucket, all buckets, long labels. Bars carry an accessible table, as
  `wc-bar-chart` does.
- [ ] `wc-invoice-table` — Preview: list, empty, loading, all six statuses, a
  void row whose balance is an **em dash, never `$0.00`** (the
  `wc-import-history` null-balance precedent).
- [ ] `wc-invoice-summary` — the detail header. Preview: draft, sent, partial,
  overdue, paid, void, no due date.
- [ ] `wc-payment-list` — Preview: empty, one, many, stripe vs manual.
- [ ] `wc-invoice-preview` — the iframe wrapper. Preview: loading, loaded,
  missing-config notice, pdf unavailable. `sandbox` with no `allow-same-origin`.

Each: `.preview.ts` declaring every state, `.test.ts` with behaviour assertions
against `data-*` hooks and `describePreviewA11y(preview)` as the last line, an
entry in `components/index.ts`.

- [ ] **Verify** `npm test && npm run lint && npm run typecheck`, then walk the
  harness at :9090 and look at every state.
- [ ] **Commit** `feat(ui): add the invoicing presentational components`.

## Task 5.3: Form and dialog components

- [ ] `wc-line-items` — repeatable rows (description, qty, unit, computed
  amount), Add row, Remove row, **up/down reorder buttons, not drag and drop**
  (no keyboard equivalent for a drag handle that passes axe without building
  one anyway), a live subtotal, per-field errors. Preview: one row, many rows,
  empty, readonly, per-field errors, saving.
- [ ] `wc-invoice-form` — client picker, issue/due dates, currency, notes,
  terms, and `wc-line-items`. Exports `EMPTY_INVOICE_FORM` and
  `validateInvoiceForm`, emits `nc-invoice-form-change` with the **whole**
  value. Preview: new, editing, validation errors, saving.
- [ ] `wc-payment-form` — amount (defaulting to the balance), date, method
  select. Reuses `wc-reconcile-form`'s currency-input treatment (rendered `$`
  prefix, `inputmode="decimal"`, commas stripped, tidy on blur) rather than
  inventing a second one.
- [ ] `wc-client-form` — name, email, address, notes; inline helper text under
  email saying an invoice cannot be sent without one. Preview: add, edit,
  duplicate-name error, missing-email hint.
- [ ] `wc-send-dialog` — the confirmation, the in-flight step list, and the
  outcome. Preview: confirm, in-flight, failure at `publish`, failure at
  `record` (**no Retry button**), refusal for a client with no email. It
  **stays open across its own request** — the one dialog that does.
- [ ] **Verify** `npm test && npm run lint && npm run typecheck`; walk the
  harness.
- [ ] **Commit** `feat(ui): add the invoicing form and dialog components`.

## Task 5.4: `screens/invoice-data.ts` and `screens/invoicing-errors.ts`

The pure half, tested without a DOM.

- [ ] **Step 1: Write the failing tests:**

```ts
describe('invoiceListParams', () => {
  it('omits status when the filter is "all"');
  it('carries clientId only when one is chosen');
});

describe('invoicePatch', () => {
  it('sends only changed fields — an all-absent PATCH is a 400');
  it('sends the whole items array when any row changed');
  it('sends dueDate: null to clear it, and omits it when unchanged');
});

describe('newInvoiceRequest', () => {
  it('drops empty trailing rows');
  it('refuses a total of zero before the request is made');
});

describe('sendFailureMessage', () => {
  it('names the step and the service in our words');
  it('shows the upstream message verbatim underneath');
  it('says the email already went out for a record-step failure');
  it('offers no retry for a record-step failure');
  it('renders send_not_configured as key names with a settings link');
  it('falls back to the server sentence for a 400 and an unknown 409 reason');
});
```

- [ ] **Step 2: Implement**, modelling `invoicing-errors.ts` on
  `manager-errors.ts` exactly: `conflictDetailsOf`, a reason→sentence table, the
  two deliberate fallbacks, and a `guardrailAction` for `has_invoices`
  pointing at `#/invoices?clientId=N`.
- [ ] **Step 3: Verify.** `npm test`.
- [ ] **Step 4: Commit** `feat(web): add the invoicing pure data and error mappers`.

## Task 5.5: The clients screen

**Files:** create `web/apps/app/src/screens/clients.ts`; modify `registry.ts`.

- [ ] **Step 1: Write the failing tests** driving the whole screen with
  `FakeApiClient`: list renders; Add opens the dialog and creates; Edit
  prefills and saves; a duplicate name renders **in the dialog**; Delete
  confirms through `confirmDialog()` and a blocked delete renders **in the
  layout's alert region** with the count and a "Show those invoices" link;
  every mutation refetches.
- [ ] **Step 2: Implement** on `wc-manager-layout` / `wc-manager-table` /
  `wc-manager-dialog` — the fourth instance of the pattern, so it should read
  like `accounts.ts` with a different form. Register the screen
  (`id: 'clients'`, `icon: 'wc-icon-clients'`, `inNav: true`).
- [ ] **Step 3: Verify.** `npm test && npm run lint && npm run typecheck`.
- [ ] **Step 4: Commit** `feat(web): add the clients management screen`.

## Task 5.6: The invoices screen

**Files:** create `web/apps/app/src/screens/invoices.ts`; modify `registry.ts`.

- [ ] **Step 1: Write the failing tests:**

```ts
it('lists invoices newest first with the aging strip above them');
it('filters by status from ?status=open and navigates rather than mutating state');
it('opens the detail view from ?number=1248');
it('shows the aging report from ?view=aging');
it('disables Send when canSend is false and names why');
it('disables Sync now when syncConfigured is false');
it('records a payment and refetches both the invoice and the list');
it('voids an invoice behind a confirm dialog');
it('edits a draft and refuses to open the editor for a published one');
it('sends only after the confirmation dialog resolves');
it('renders the failed step in the dialog and leaves it open');
it('offers no retry when the send failed at the record step');
it('refetches the list and the aging strip after a successful send');
```

- [ ] **Step 2: Implement.** One screen, four views keyed off `ctx.params`
  (`view`, `number`, `edit`, `new`) — the reports screen's precedent, since the
  router has no path segments. Filters navigate rather than set state, so they
  are links. The **editor is a full view, not a dialog** (spec §5); the client
  form stays a dialog.
- [ ] **Step 3: Verify.** `npm test && npm run lint && npm run typecheck`.
- [ ] **Step 4: Commit** `feat(web): add the invoices management screen`.

## Task 5.7: Figure parity

**Files:** create `web/apps/app/src/screens/invoicing-parity.test.ts`.

- [ ] **Step 1: Write the test.** Reuse `reports-parity.test.ts` wholesale:
  `readFileSync` from `../__fixtures__/invoicing`, `moneyTokens`
  (`/-?\$[\d,]+\.\d{2}/g`, sign stripped, sorted), a recursive shadow-DOM text
  walk, `FakeApiClient` primed only with the endpoint under test,
  `initializeAppStore` + `refreshStatus`, mount → `updateComplete` →
  `setTimeout(0)` → `updateComplete`.

  Four cases: the invoice list, invoice 1250's detail, the aging view, and the
  clients screen. Plus the "guards the guard" assertion — the manifest's entry
  count must equal the number of views under test, so a view added without a
  fixture fails instead of passing on an empty set.

- [ ] **Step 2: Make it pass.** If a figure differs, the question is *which
  side is right* — usually the CLI, occasionally neither. Do not paper over a
  mismatch by loosening the regex.
- [ ] **Step 3: Verify.** `npm test`; then `npm run build` and
  `cargo build --release`, and click through the real app.
- [ ] **Step 4: Commit** `test(web): assert invoicing figure parity with the CLI`.

## Task 5.8: Stage 5 documentation and the manual pass

- [ ] `CLAUDE.md`: "SPA invoicing" architecture entry in the voice of the
  existing screen entries — what the screens are, what departs from the TUI/CLI
  and why (full-view editor, dialog that survives its request, send
  confirmation, the `can*` flags coming from the server).
- [ ] `README.md`: invoicing in the web UI feature list.
- [ ] **Manual pass** against a demo database with `nigel serve`:
  - create a client, create an invoice, edit it, preview it, void it;
  - record a payment, watch the status and the aging strip move;
  - with no invoicing config: Send is disabled and says which keys are missing;
  - with test-mode Stripe keys and a scratch bucket: a real send, and a
    deliberate failure (wrong `r2_bucket`) showing the failing step.
  - resize to a narrow viewport: no horizontal page scroll, tables scroll in
    their own container.
- [ ] Commit `docs: describe the SPA invoicing screens`.
- [ ] **Open the Stage 5 PR.**

---

## Definition of done for TASK-68.6

- [ ] AC #1 — the invoicing structs derive `Serialize` following the task-31.2
      pattern (Task 1.1).
- [ ] AC #2 — the JSON API covers clients, invoices, payments, preview and
      aging behind the standard locked and session guards (Stages 2-4; the
      guard tests enumerate every route).
- [ ] AC #3 — send requires explicit confirmation and reports each step's
      failure by cause (Tasks 4.2, 4.4, 5.3).
- [ ] AC #4 — SPA screens cover client management, invoice management and aging
      with CLI figure parity (Tasks 5.5-5.7).
- [ ] No webhook endpoint exists; sync stays pull-based.
- [ ] `cargo test`, `cargo test --no-default-features`, `cargo clippy`,
      `npm test`, `npm run lint`, `npm run typecheck`, `npm run build` all green.
- [ ] `CLAUDE.md`, `README.md`, `docs/api.md` and `docs/invoicing.md` describe
      the current state of the system.
