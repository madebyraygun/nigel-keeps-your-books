# Task 68.5 — A/R aging report view and dashboard summary: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans` to work this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** aging becomes a first-class report — browsable in the TUI, exportable
as text and PDF, reachable from the dashboard picker — and the dashboard home
screen shows outstanding A/R. Per
`docs/superpowers/specs/2026-08-08-task-68-5-aging-report-view-design.md`.

**Architecture:** a detail-bearing data function in `src/invoicing/invoices.rs`;
a new `ReportKind::Aging` / `ReportCommands::Aging` pair driving the existing
report machinery (view / text / pdf / picker); a one-line summary on the
dashboard home screen. No new module.

**Tech stack:** Rust, ratatui, comfy-table, printpdf (feature-gated), rusqlite,
assert_cmd for integration tests.

## Global constraints

- `cargo test -- --test-threads=1`, `cargo clippy --all-targets -- -D warnings`
  and `cargo fmt --check` pass after **every** task.
- Also run `cargo test --no-default-features -- --test-threads=1` after task 5
  (the PDF work) and at the end — aging must build without the `pdf` feature.
- Bucket boundaries, status filter and half-cent slack are unchanged. If an
  existing `ar_aging` test needs editing to pass, the refactor is wrong.
- Aging has **no** date parameters. Never call `.with_date(...)` on its view.
- No comment justifies an edit or records history; that belongs in the commit.

---

### Task 1: Detail-bearing aging data (`src/invoicing/invoices.rs`)

**Files:** modify `src/invoicing/invoices.rs` (types + `ar_aging_detail` above
`ar_aging`; tests in the existing `mod tests`).

**Interfaces produced** (consumed by tasks 4, 6, 9):

```rust
pub struct AgingBucket { pub label: &'static str, pub count: usize, pub total: f64 }
pub struct AgingInvoice {
    pub number: i64, pub client: String, pub due_date: String,
    pub days_past_due: i64, pub bucket: &'static str,
    pub total: f64, pub paid: f64, pub balance: f64,
}
pub struct AgingReport {
    pub as_of: String, pub buckets: Vec<AgingBucket>,
    pub invoices: Vec<AgingInvoice>, pub outstanding: f64,
}
pub fn ar_aging_detail(conn: &Connection, today: &str) -> Result<AgingReport>;
```

All three derive `Serialize` + `#[serde(rename_all = "camelCase")]`.

- [ ] **Step 1 — failing tests.** In `mod tests`, using the existing `test_conn()`
  and `add_client`/`create_invoice` helpers, write:
  - `aging_detail_buckets_by_days_past_due` — invoices due at exactly 0, 1, 30,
    31, 60, 61, 90 and 91 days before `today` land in current, 1-30, 1-30, 31-60,
    31-60, 61-90, 61-90, 90+ respectively.
  - `aging_detail_falls_back_to_issue_date` — a NULL `due_date` ages from
    `issue_date`, and `AgingInvoice::due_date` reports the date actually used.
  - `aging_detail_subtracts_payments` — a partial payment reduces `balance` and
    the bucket total; a paid-in-full invoice appears in neither.
  - `aging_detail_excludes_draft_and_void` — only `sent`/`partial`/`overdue`.
  - `aging_detail_counts_and_total` — each bucket's `count` equals the invoices
    assigned to it, and `outstanding` equals the sum of bucket totals.
  - `aging_detail_orders_oldest_first` — `invoices` sorted by `days_past_due` DESC.
  - `aging_detail_carries_client_name` — the client name joins through.
- [ ] **Step 2 — implement.** One statement joining `clients`, filtering the three
  statuses, `COALESCE(due_date, issue_date)`; per row compute `paid` via the
  existing `paid_amount`, skip `balance <= 0.0`, classify with the *existing*
  index ladder, accumulate `count`/`total`, then sort.
- [ ] **Step 3 — re-express `ar_aging`** as
  `Ok(ar_aging_detail(conn, today)?.buckets)`. Its existing test must pass
  unedited.
- [ ] **Step 4 — verify:** `cargo test -- --test-threads=1`, clippy, fmt.

---

### Task 2: `ReportKind::Aging` (`src/reports.rs`)

**Files:** modify `src/reports.rs`.

- [ ] **Step 1 — failing test.** Add `(Aging, "aging", None)` to the `expected`
  table in `report_kind_slugs_and_granularity`. It fails to compile — that is the
  red state.
- [ ] **Step 2 — implement.** Add `Aging` to `ReportKind`; `as_str() => "aging"`;
  `granularity()` groups it with `Flagged | Balance => DateGranularity::None`.
- [ ] **Step 3 — verify.** The build now fails in `src/cli/mod.rs` only after
  task 3 adds the clap variant; `ParamSpec::for_kind`'s `_ => spec` arm absorbs
  the new kind with no server change (intentional — see spec, out of scope).

---

### Task 3: `nigel report aging` (`src/cli/mod.rs`)

**Files:** modify `src/cli/mod.rs`. Also promote `today()`.

- [ ] **Step 1 — add the clap variant** after `K1`:

```rust
/// A/R aging — outstanding invoices by age. Always as of today.
Aging {
    #[command(flatten)]
    output: ReportOutputArgs,
},
```

- [ ] **Step 2 — follow the compiler:** `output_args()` and `kind()` gain arms.
  `report::dispatch_text`, `view::build_view` and `export::dispatch_pdf` will
  also fail to compile; leave them failing until tasks 4-6, or stub them with
  `todo!()` and remove the stubs as those tasks land. Do not leave a `todo!()`
  behind a passing test.
- [ ] **Step 3 — move `today()`** from `src/main.rs` into `src/cli/mod.rs` as
  `pub fn today() -> String`, and have `main.rs` call `cli::today()`. The report
  text layer needs the same string and a second copy of the format would drift.
- [ ] **Step 4 — verify** once tasks 4-6 land; this task alone does not build.

---

### Task 4: Text formatter (`src/cli/report/text.rs`)

**Files:** modify `src/cli/report/text.rs`, `src/cli/report/mod.rs`.

- [ ] **Step 1 — failing tests** in `text.rs`'s test module (or a new one, matching
  the file's convention):
  - `format_aging_lists_every_bucket` — all five labels appear even when a bucket
    is zero, plus a `Total Outstanding` row.
  - `format_aging_lists_open_invoices` — invoice number, client, due date and
    balance appear, oldest first.
  - `format_aging_empty_state` — no open invoices produces a recognisable
    "No open invoices." line and still prints the zeroed buckets.
- [ ] **Step 2 — implement** `pub fn format_aging(data: &AgingReport) -> String`
  (pure, two comfy-tables: buckets then invoices, the shape `format_k1` uses for
  multiple sections) and the impure wrapper
  `pub fn aging(today: &str) -> Result<String>` that opens the connection, reads
  `company_name`, calls `ar_aging_detail`, and returns `with_header(&company, …)`.
- [ ] **Step 3 — wire dispatch.** `report::dispatch_text` gains
  `ReportCommands::Aging { .. } => text::aging(&crate::cli::today())`.
- [ ] **Step 4 — verify:** `cargo test`, and by hand
  `cargo run -- report aging | cat` (non-TTY path) prints the table.

---

### Task 5: PDF export (`src/pdf.rs`, `src/cli/export.rs`)

**Files:** modify `src/pdf.rs`, `src/cli/export.rs`, `src/cli/report/mod.rs`.

- [ ] **Step 1 — failing test** in `src/pdf.rs`'s tests, mirroring the
  `render_balance` test: `render_aging` returns non-empty bytes starting with
  `%PDF`.
- [ ] **Step 2 — implement** `pub fn render_aging(report: &AgingReport, company: &str)
  -> Result<Vec<u8>>`, modelled line-for-line on `render_balance` (no date-range
  argument — the as-of date is part of the report and prints under the title).
- [ ] **Step 3 — `export::aging`**, `#[cfg(feature = "pdf")]`, mirroring
  `export::balance` with `default_path("aging")`; add the
  `ReportCommands::Aging { .. } => aging(output)` arm to `dispatch_pdf`.
- [ ] **Step 4 — bulk export.** Add `("aging", text::aging(&today))` to
  `report::mod::export_all_text`'s list and `export::aging` to `export::all`.
- [ ] **Step 5 — verify both feature sets:**
  `cargo test -- --test-threads=1` and
  `cargo test --no-default-features -- --test-threads=1`. Without `pdf`,
  `nigel report aging --mode export` must print `PDF_DISABLED_MESSAGE`, not panic.

---

### Task 6: The interactive view (`src/cli/report/view.rs`)

**Files:** modify `src/cli/report/view.rs`.

- [ ] **Step 1 — failing tests.** `view.rs` has no test module today; add one with
  a `TableReportView` built by `build_aging`-equivalent construction (or make the
  row-building pure and test that):
  - `aging_view_has_no_date_navigation` — `date_params()` is `(None, None)`.
  - `aging_view_ignores_period_keys` — `Left`, `Right` and `m` each return
    `ReportViewAction::Continue`, never `Reload`.
  - `aging_view_scrolls` — `Down` then `Up` restores offset 0.
- [ ] **Step 2 — implement** `pub(crate) fn build_aging() -> Result<Box<dyn ReportView>>`:
  open the connection, `ar_aging_detail(&conn, &crate::cli::today())`, build rows
  per the spec's wireframe —
  - widths `[Fill(1), Length(22), Length(12), Length(6), Length(14)]`,
    header `["Invoice", "Client", "Due", "Days", "Balance"]`;
  - `section_row("SUMMARY", 5)`, one row per bucket with `label (count)` in
    column 0 and `money_cell(total)` in column 4, then a bold
    `Total Outstanding (n)` row;
  - an aged bucket with `total > 0.005` renders its label cell yellow;
  - `blank_row(5)`, `section_row("OPEN INVOICES", 5)`, one row per invoice
    (`#{number}`, client, due date, `days_past_due` or `—`, `money_cell(balance)`);
  - empty state: a single `No open invoices.` row.
  Finish with `TableReportView::new(format!("A/R Aging — as of {}", data.as_of),
  header, rows, widths)` — **no `.with_date(...)`**.
- [ ] **Step 3 — wire dispatch.** `build_view` gains
  `ReportCommands::Aging { .. } => build_aging()`. `view::dispatch` needs no
  change (aging is neither `Register` nor `All`).
- [ ] **Step 4 — verify:** `cargo test`; by hand `cargo run -- report aging`
  scrolls, `q` closes, and arrows/`m` do nothing.

---

### Task 7: `nigel invoice aging` delegates (`src/cli/invoice.rs`)

**Files:** modify `src/cli/invoice.rs`; drop the now-unused `ar_aging` import if
nothing else there uses it.

- [ ] **Step 1 — failing test** in `tests/cli_dispatch.rs`:
  `invoice_aging_prints_bucket_labels` — on a demo/init database,
  `nigel invoice aging` exits 0 and stdout contains `current`, `1-30` and `90+`.
- [ ] **Step 2 — implement:** the body becomes
  `println!("{}", crate::cli::report::text::aging(today)?); Ok(())`.
  Keep the `today: &str` parameter and `main.rs`'s call site as they are — this
  command stays non-interactive and never routes through `report::dispatch`.
- [ ] **Step 3 — verify:** `cargo test`; by hand `nigel invoice aging | cat`.

---

### Task 8: Dashboard report pickers (`src/cli/dashboard.rs`)

**Files:** modify `src/cli/dashboard.rs`.

- [ ] **Step 1 — failing guard test.** `report_picker_indices_match_report_slugs`:
  assert `REPORT_TYPES.len() == 9`, `EXPORT_TYPES.len() == 10`, that index 8 in
  both is `"A/R Aging"`, and that `EXPORT_TYPES[9] == "All Reports"`. This test
  is the point of the task — six parallel index-keyed lists are the fragile part.
- [ ] **Step 2 — implement:**
  - `REPORT_TYPES` gains `"A/R Aging"` at index 8.
  - `EXPORT_TYPES` gains `"A/R Aging"` at index 8, pushing `"All Reports"` to 9.
  - `enter_report_view_with_date`: `8 => super::report::view::build_aging()`.
    Aging takes no year/month; ignore both, exactly as arms 5 and 6 do.
  - `do_export`: `8 => super::export::aging(None)?` and the bulk branch moves
    from `8 =>` to `9 =>`.
  - `do_text_export`: add `"aging"` to the `names` array at index 8, add
    `8 => super::report::text::aging(&crate::cli::today())?`, change the
    `if idx == 8` bulk branch to `if idx == 9`, and add aging to the bulk list.
- [ ] **Step 3 — verify:** `cargo test`; by hand launch the dashboard, `v` → A/R
  Aging opens the view, `e` → A/R Aging → Text writes `aging-<date>.txt`, and
  `e` → All Reports still exports everything.

---

### Task 9: Dashboard home A/R line (`src/cli/dashboard.rs`)

**Files:** modify `src/cli/dashboard.rs`.

- [ ] **Step 1 — failing tests** for the pure derivation. Extract
  `fn ar_summary(report: &AgingReport) -> Option<ArSummary>` so it is testable
  without a terminal:
  - `ar_summary_is_none_when_nothing_outstanding` — `outstanding` of `0.0` and of
    `0.004` both yield `None` (half-cent slack).
  - `ar_summary_picks_oldest_non_empty_bucket` — with balances in `current` and
    `61-90`, `oldest_bucket == "61-90"`; with only `current`, it is `"current"`.
  - `ar_summary_reports_total` — `outstanding` carries through.
- [ ] **Step 2 — implement:**
  - `struct ArSummary { outstanding: f64, oldest_bucket: &'static str, oldest_total: f64 }`
    and `HomeData { …, ar: Option<ArSummary> }`.
  - In `load_data`, after the existing report loads:
    `let ar = invoicing::invoices::ar_aging_detail(conn, &crate::cli::today()).ok().and_then(|r| ar_summary(&r));`
    — **`.ok()`, never `?`**: an invoicing failure must not blank the dashboard,
    and the same expression is the "only when open invoices exist" gate.
  - In `draw_home`, change the stats constraint to
    `Constraint::Length(if has_ar { 6 } else { 5 })` (computed from
    `self.home_data.as_ref().is_some_and(|d| d.ar.is_some())`, before the layout).
  - Append the line to `stats_lines` when `data.ar` is `Some`:
    `" A/R Outstanding"` (15-char label, aligns with the block above),
    `money_span(outstanding)`, then `format!("  oldest {oldest_bucket}")` styled
    `FOOTER_STYLE` when the oldest bucket is `current`, yellow otherwise.
- [ ] **Step 3 — verify:** `cargo test`; by hand, a database with no invoices
  renders exactly as before, and one with an open invoice shows the line.
  Confirm on an 80×24 terminal that the extra row does not clip the bar chart.

---

### Task 10: Documentation

**Files:** modify `CLAUDE.md`, `README.md`, `docs/invoicing.md`.

- [ ] **Step 1 — `CLAUDE.md`:**
  - Reports bullet: `ReportKind` gains `Aging` (slug `aging`,
    `DateGranularity::None`).
  - Invoicing bullet: `ar_aging_detail` alongside `ar_aging`.
  - Commands block: `nigel report aging   # A/R aging buckets and open invoices`.
  - Key Design Constraints: one line recording that aging is always as-of-today
    (no date parameters, no period navigation), that `nigel invoice aging` prints
    the report module's own text so the two cannot drift, and that the web API
    does not serve aging until 68.6.
- [ ] **Step 2 — `README.md`:** add aging to the report list.
- [ ] **Step 3 — `docs/invoicing.md`:** the A/R aging section gains
  `nigel report aging` and its sample output is replaced with the delegated
  table.
- [ ] **Step 4 — check for stale prose.** Any place that says "eight reports"
  now says nine. Grep `eight` and `k1-prep` across `CLAUDE.md`, `README.md` and
  `docs/`; leave the SPA's own eight alone (the web still serves eight — that is
  68.6's to change).

---

### Task 11: Final verification

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] Manual pass with a seeded database carrying at least one current, one
  30-day and one 90+ invoice:
  - `nigel report aging` — interactive, scrolls, arrows/`m` inert, `q` closes
  - `nigel report aging | cat` — plain text
  - `nigel report aging --mode export --format text` — file written
  - `nigel report aging --mode export` — PDF written
  - `nigel report all --format text` — nine files
  - `nigel invoice aging` — same numbers as the view
  - `nigel` — dashboard shows the A/R line; picker offers A/R Aging for both view
    and export; a database with no invoices shows no A/R line
- [ ] Re-read the spec's acceptance criteria and confirm both are met.
- [ ] Report to the orchestrator; do **not** run `backlog task edit` or commit
  unless instructed.
