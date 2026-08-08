# TASK-68.1 — CLI: client show/edit, invoice edit/void — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans` to work this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Design:** `docs/superpowers/specs/2026-08-08-task-68-1-invoicing-edit-void-design.md`. Read
it first — this plan implements it and does not restate its rationale.

**Goal:** make invoicing editable and cancellable. `client show`/`client edit`, draft-only
`invoice edit`, `invoice void` with confirmation, `--notes`/`--terms` on `new` and `edit`
rendered in HTML as well as PDF, and `NotFound` instead of a raw FOREIGN KEY error for unknown
ids.

**Foundation for the epic:** every operation is a `&Connection`-in / plain-struct-out function
in `src/invoicing/` with a thin printing wrapper in `src/cli/`. TASK-68.4 (TUI) and TASK-68.6
(web) call the data layer directly. Do not put logic in the CLI wrappers.

---

## Global constraints

- **Data layer takes `&Connection`.** Tests use the existing `test_conn()` helper (tempdir →
  `get_connection` → `init_db` → `run_migrations`). CLI wrappers open with
  `get_connection(&get_data_dir().join("nigel.db"))` and only print.
- **Guard failures are `NigelError::Conflict { code, message }`,** never `Other`. The code is
  what TASK-68.6 turns into a 409 `details.reason`.
- **Assert variants, not just strings.** Guard tests match on
  `NigelError::Conflict { code: "…", .. }`, following
  `a_deactivated_rule_refuses_further_edits` in `src/cli/rules.rs`.
- **Exact wording is specified in the design's "Error variants and exact wording" table.** Do
  not paraphrase it.
- **Tests never touch the network.** `tests/cli_dispatch.rs`'s `TestEnv` already clears every
  `NIGEL_*` invoicing var; use it.
- **Conventional Commits** (`feat:`, `fix:`, `test:`, `docs:`), one commit per task.
- **Migrations append to `MIGRATIONS` in `src/migrations.rs`.** Never edit `db::SCHEMA` —
  the invoicing tables are not in it. `LATEST_VERSION` derives from the array.

## Verification

Each task names its own `cargo test` filter. "Verify" in every task means that filter, then
`cargo test -- --test-threads=1`, `cargo clippy -- -D warnings`, `cargo fmt --check`. Task 12
adds the two no-default-features runs. `--test-threads=1` is not optional — the DB password is
a process global.

## File map

Every file is a modification; nothing new is created.

- `src/migrations.rs` — v5 (`voided_at`); `src/models.rs` — `Invoice.voided_at`
- `src/invoicing/clients.rs` — `NotFound`, `ensure_client_exists`, `ClientUpdate`,
  `update_client`, `ClientSummary`, `client_summary`
- `src/invoicing/invoices.rs` — `voided_at` plumbing, guards, `InvoiceUpdate`,
  `update_invoice`, `void_invoice`, validation helpers
- `src/invoicing/render_html.rs` + `templates/invoice.html` — `{{NOTES}}`/`{{TERMS}}`
- `src/cli/client.rs` — `show`, `edit`; `src/cli/invoice.rs` — `new` notes/terms, `edit`,
  `void`, re-typed `find_invoice`/`ensure_not_void`
- `src/cli/mod.rs` — clap variants; `src/main.rs` — dispatch arms
- `tests/cli_dispatch.rs` — end-to-end command tests
- `CLAUDE.md`, `docs/invoicing.md`, `README.md` — docs

---

## Task 1: Migration v5 — `voided_at`, and void becomes a derived status

**Blocked on Open question 1 in the design.** Confirm with the orchestrator that `voided_at` is
wanted before starting; if it is rejected, skip this task and have `void_invoice` (Task 7)
write `status = 'void'` directly.

**Files:**
- Modify: `src/migrations.rs`, `src/models.rs`, `src/invoicing/invoices.rs`
- Test: inline in `src/migrations.rs` and `src/invoicing/invoices.rs`

**Interfaces produced:** `invoices.voided_at` column; `Invoice.voided_at`; `refresh_status`
derives `void` from it.

- [ ] **Step 1: Write the failing tests**

In `src/migrations.rs`, extend `mod invoicing_migration_tests` with a test asserting
`voided_at` appears in `PRAGMA table_info(invoices)`, plus a test that a pre-v5 database
carrying `status='void'` gets `voided_at` backfilled (set `schema_version` to `4`, insert a
void row, run migrations, assert `voided_at` is non-null). Follow the shape of
`k1_backfill_tests::backfills_stock_categories_only_and_never_overwrites`.

In `src/invoicing/invoices.rs`, add `void_is_derived_from_voided_at`: create an invoice, set
`voided_at` by raw SQL, assert `refresh_status` returns `"void"` and that a full payment does
not move it off `void`.

- [ ] **Step 2: Run and watch them fail**

`cargo test voided_at -- --test-threads=1` → FAIL (no such column).

- [ ] **Step 3: Append the migration**

Add to `MIGRATIONS` in `src/migrations.rs`, version `5`, description
`"add voided_at to invoices so void is a derived status like sent"`, with the `ALTER TABLE` +
backfill `execute_batch` from the design's Migration v5 section.

- [ ] **Step 4: Plumb the column**

- `src/models.rs`: add `pub voided_at: Option<String>` to `Invoice`, after `published_at`.
- `src/invoicing/invoices.rs`: append `voided_at` to `INVOICE_COLS`; read it at index 16 in
  `row_to_invoice`; change `refresh_status`'s first guard from
  `if inv.status == InvoiceStatus::Void.as_str()` to `if inv.voided_at.is_some()`, returning
  `InvoiceStatus::Void.as_str().to_string()`.

- [ ] **Step 5: Fix the callers the new field breaks**

Every `Invoice { … }` literal needs the field. They are in test modules only:
`src/invoicing/render_html.rs`, `src/pdf.rs`, `src/invoicing/send.rs`, `src/invoicing/stripe.rs`.
Add `voided_at: None`. Update the existing `void_is_never_downgraded` test in
`src/invoicing/invoices.rs` to set `voided_at` instead of `status='void'` — same assertions.

- [ ] **Step 6: Verify**

```bash
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1   # catches pdf-gated literals
cargo clippy -- -D warnings && cargo fmt --check
```

- [ ] **Step 7: Commit** — `feat: derive invoice void status from a voided_at column`

---

## Task 2: Client lookups answer NotFound (TASK-65)

**Files:**
- Modify: `src/invoicing/clients.rs`, `src/invoicing/invoices.rs`
- Test: inline in both

**Interfaces produced:** `get_client` returning `NotFound`; `ensure_client_exists`;
`create_invoice` validating the client id.

- [ ] **Step 1: Write the failing tests**

In `src/invoicing/clients.rs`:
- `unknown_client_id_is_not_found` — `get_client(&conn, 99)` errors with
  `NigelError::NotFound` whose message is exactly `Client not found: id 99`.
- `ensure_client_exists_passes_for_a_real_client_and_fails_otherwise`.

In `src/invoicing/invoices.rs`:
- `creating_an_invoice_for_an_unknown_client_is_not_found_and_writes_nothing` — assert the
  error is `NotFound` containing `id 99`, that `SELECT COUNT(*) FROM invoices` is `0`, and
  that `next_number` is still `1248` (the number must not be consumed).

- [ ] **Step 2: Run and watch them fail**

`cargo test client -- --test-threads=1` → FAIL (`Database error: Query returned no rows` /
`FOREIGN KEY constraint failed`).

- [ ] **Step 3: Implement**

- `get_client`: map the `rusqlite::Error` the way `accounts::get_account` does —
  `QueryReturnedNoRows` → `NigelError::NotFound(format!("Client not found: id {id}"))`,
  everything else → `NigelError::Db`.
- `ensure_client_exists`: `SELECT EXISTS(SELECT 1 FROM clients WHERE id = ?1)`, same body
  shape as `cli::categories::ensure_category_exists`.
- `create_invoice`: call `ensure_client_exists(conn, client_id)?` as its **first** statement,
  before `conn.unchecked_transaction()`.

- [ ] **Step 4: Verify**

`cargo test -- --test-threads=1`, clippy, fmt.

- [ ] **Step 5: Commit** — `fix: report an unknown invoice client id as not found`

---

## Task 3: `ClientUpdate` and `update_client`

**Files:**
- Modify: `src/invoicing/clients.rs`
- Test: inline

**Interfaces produced:** `ClientUpdate`, `ClientUpdate::is_empty`, `update_client`.

- [ ] **Step 1: Write the failing tests**

- `updating_one_field_leaves_the_others_alone` — set only `email`, assert name/address/notes
  unchanged.
- `some_none_clears_a_nullable_field` — `email: Some(None)` writes SQL NULL.
- `an_empty_update_is_rejected` — `NigelError::Invalid` with message
  `Nothing to update — provide at least one flag`.
- `a_blank_name_is_rejected` — `NigelError::Invalid` with `Name is required`.
- `updating_a_missing_client_is_not_found` — `Client not found: id 99`.

- [ ] **Step 2: Run and watch them fail** — `cargo test update_client -- --test-threads=1`.

- [ ] **Step 3: Implement**

Add `ClientUpdate` (`#[derive(Debug, Default, Clone)]`) with the four fields from the design,
`is_empty()`, and `update_client`. Build the `SET` clause dynamically from the populated
fields exactly as `rules::update_rule` does (`Vec<Box<dyn ToSql>>` + numbered placeholders);
reject the empty update and the blank name before touching the database; treat
`conn.execute(...)? == 0` as `NotFound`.

- [ ] **Step 4: Verify** — `cargo test -- --test-threads=1`, clippy, fmt.

- [ ] **Step 5: Commit** — `feat: add a client update data-layer function`

---

## Task 4: `ClientSummary` for `client show`

**Files:**
- Modify: `src/invoicing/clients.rs`
- Test: inline

**Interfaces produced:** `ClientInvoiceRow`, `ClientSummary`, `client_summary`.

- [ ] **Step 1: Write the failing tests**

- `summary_lists_a_clients_invoices_newest_first` — three invoices, assert the `number`
  ordering is descending.
- `summary_outstanding_counts_only_open_invoices` — one `sent` with a partial payment, one
  `paid`, one `void`; assert `outstanding` equals only the open remainder.
- `summary_for_a_client_with_no_invoices_is_empty_not_an_error`.
- `summary_for_a_missing_client_is_not_found`.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement**

`client_summary` calls `get_client` first (so an unknown id is `NotFound` before any join),
then one query joining `invoices` to a `SUM(amount)` subselect on `invoice_payments`,
`ORDER BY number DESC`. `outstanding` sums `total - paid` over rows whose status is in
`('sent','partial','overdue')` — the same filter `ar_aging` uses — clamped at zero per row so
an overpayment cannot subtract from another invoice's balance.

- [ ] **Step 4: Verify** — `cargo test client_summary -- --test-threads=1`, then the full suite.

- [ ] **Step 5: Commit** — `feat: add a client summary with invoice history and balance`

---

## Task 5: Edit and void guards

**Files:**
- Modify: `src/invoicing/invoices.rs`, `src/cli/invoice.rs`
- Test: inline in both

**Interfaces produced:** `ensure_editable`, `ensure_voidable`; `ensure_not_void` and
`find_invoice` re-typed.

- [ ] **Step 1: Write the failing tests**

In `src/invoicing/invoices.rs`, one test per row of the design's guard matrix:
- `a_clean_draft_is_editable_and_voidable`.
- `a_published_invoice_refuses_edits` — `Conflict { code: "not_draft", .. }`, message contains
  `has already been sent and cannot be edited`.
- `a_void_invoice_refuses_edits` — `Conflict { code: "void", .. }`.
- `an_invoice_with_payments_refuses_edit_and_void` — record `50.0` against a `100.0` draft;
  both guards answer `Conflict { code: "has_payments", .. }` and the message carries `50.00`.
- `voiding_a_void_invoice_is_already_void` — `Conflict { code: "already_void", .. }`.

In `src/cli/invoice.rs`, extend the existing
`void_invoices_are_refused_before_any_network_call_or_payment` to also assert the variant is
`Conflict { code: "void", .. }`, and extend `unknown_invoice_number_gets_a_readable_error` to
assert `NigelError::NotFound`.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement the guards**

In `src/invoicing/invoices.rs`:

- `ensure_editable(conn, invoice)` — in order: `voided_at.is_some()` → `void`;
  `status != "draft"` → `not_draft`; `paid_amount(conn, invoice.id)? > 0.0` → `has_payments`.
- `ensure_voidable(conn, invoice)` — `voided_at.is_some()` → `already_void`;
  `paid_amount(conn, invoice.id)? > 0.0` → `has_payments`.

Both return `NigelError::Conflict { code, message }` with the design's exact wording.

- [ ] **Step 4: Re-type the two existing sites**

`src/cli/invoice.rs`: `ensure_not_void` returns `Conflict { code: "void", … }` with its
message string unchanged; `find_invoice` returns `NotFound(…)` with its message unchanged.
Confirm no other assertion in the tree matches on those variants
(`rg 'NigelError::Other' src/ | rg -i invoice`).

- [ ] **Step 5: Verify** — `cargo test -- --test-threads=1`, clippy, fmt.

- [ ] **Step 6: Commit** — `feat: add invoice edit and void guards with machine-readable codes`

---

## Task 6: `InvoiceUpdate` and `update_invoice`

**Files:**
- Modify: `src/invoicing/invoices.rs`
- Test: inline

**Interfaces produced:** `InvoiceUpdate`, `update_invoice`, `replace_line_items`,
`validate_date`, `validate_currency`; `create_invoice` validating its inputs.

- [ ] **Step 1: Write the failing tests**

Field updates:
- `editing_the_due_date_leaves_everything_else_alone`.
- `clearing_the_due_date_writes_null` — `due_date: Some(None)`.
- `editing_notes_and_terms_persists_both`.
- `an_empty_update_is_rejected` — `Invalid`, `Nothing to update — provide at least one flag`.

Line items:
- `replacing_line_items_renumbers_positions_densely` — start with three lines, replace with
  two, assert positions are `[0, 1]` and the old rows are gone.
- `replacing_line_items_recomputes_subtotal_and_total`.
- `omitting_items_leaves_the_existing_lines_alone`.
- `a_failed_line_item_insert_leaves_the_invoice_untouched` — reuse the `CREATE TRIGGER
  fail_line_items` technique from `failed_create_rolls_back_and_leaves_numbering_usable`;
  assert the old lines and old total survive.

Validation:
- `a_malformed_date_is_rejected_on_create_and_on_edit` — `Invalid`,
  `Invalid issue date: 2026-13-45 (expected YYYY-MM-DD)`.
- `currency_is_normalized_to_uppercase_and_must_be_three_letters` — `eur` → `EUR`; `dollars`
  → `Invalid`.

Stale link:
- `changing_the_total_clears_a_stale_stripe_link` — set both link columns by raw SQL, edit the
  items, assert both are NULL.
- `an_edit_that_does_not_move_the_money_keeps_the_link` — edit only `notes`, assert the link
  survives.

Guard integration:
- `update_invoice_refuses_a_published_invoice` — the `not_draft` conflict surfaces from
  `update_invoice`, not just from `ensure_editable`.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement the validators**

`validate_date(value, what)` parses with `chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")`
(chrono is already imported in this module) and errors
`Invalid {what} date: {value} (expected YYYY-MM-DD)`. `validate_currency(code)` requires
exactly three ASCII alphabetic characters and returns the uppercased `String`, else
`Invalid currency: {code} (expected a 3-letter code like USD)`.

Wire both into `create_invoice` as well as `update_invoice` (design Open question 7).

- [ ] **Step 4: Implement `replace_line_items`**

Private. `DELETE FROM invoice_line_items WHERE invoice_id = ?1`, then insert each item at
`position = idx`, computing `line_total = quantity * unit_amount`. Return
`(subtotal, subtotal + tax)` where `tax` is read from the invoice row and left untouched.

- [ ] **Step 5: Implement `update_invoice`**

Order of operations:
1. `get_invoice` → `ensure_editable`.
2. Reject an empty update.
3. Validate the date and currency fields that are present.
4. Open `conn.unchecked_transaction()`.
5. Build and run the dynamic `UPDATE invoices SET …` for the scalar fields (same
   `Vec<Box<dyn ToSql>>` technique as `update_client`).
6. If `items.is_some()`, call `replace_line_items` and `UPDATE invoices SET subtotal = ?,
   total = ?`.
7. If the total or the currency moved and `stripe_payment_link_id IS NOT NULL`, null both link
   columns.
8. `tx.commit()`, then `refresh_status(conn, invoice_id, &invoice.issue_date)` so the derived
   status reflects the new totals.

- [ ] **Step 6: Verify** — `cargo test update_invoice -- --test-threads=1`, then the full suite,
  clippy, fmt.

- [ ] **Step 7: Commit** — `feat: add a draft-only invoice update data-layer function`

---

## Task 7: `void_invoice`

**Files:**
- Modify: `src/invoicing/invoices.rs`
- Test: inline

**Interfaces produced:** `void_invoice`.

- [ ] **Step 1: Write the failing tests**

- `voiding_a_draft_sets_voided_at_and_the_void_status`.
- `voiding_a_sent_invoice_is_allowed`.
- `voiding_an_invoice_with_payments_is_refused` — `Conflict { code: "has_payments", .. }`.
- `voiding_a_void_invoice_is_refused` — `Conflict { code: "already_void", .. }`.
- `a_voided_invoice_leaves_the_aging_buckets` — void a `sent` invoice, assert every
  `ar_aging` bucket is `0.0`.
- `voiding_a_missing_invoice_is_not_found`.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement**

```
get_invoice → ensure_voidable → UPDATE invoices SET voided_at = ?1 WHERE id = ?2
            → refresh_status(conn, invoice_id, voided_on)
```

Do **not** write `status` directly — `refresh_status` derives `void` from `voided_at`, which is
the whole point of Task 1.

- [ ] **Step 4: Verify** — `cargo test void_invoice -- --test-threads=1`, then the full suite.

- [ ] **Step 5: Commit** — `feat: add invoice void to the data layer`

---

## Task 8: Notes and terms in the HTML invoice

**Files:**
- Modify: `src/invoicing/templates/invoice.html`, `src/invoicing/render_html.rs`
- Test: inline in `render_html.rs`

**Interfaces produced:** `{{NOTES}}` and `{{TERMS}}` placeholders.

- [ ] **Step 1: Write the failing tests**

- `renders_notes_and_terms_when_present` — both headings and both bodies appear.
- `omits_the_headings_when_absent` — neither `Notes` nor `Terms` appears for a `None`/`None`
  invoice.
- `notes_are_escaped_and_newlines_become_breaks` — a note containing `<script>` and a `\n`
  renders `&lt;script&gt;` and `<br>`, and no raw `<script>`.
- `a_note_containing_a_placeholder_stays_literal_text` — mirrors the existing
  `client_name_containing_a_placeholder_stays_literal_text`; the single-pass `expand` already
  guarantees this, so the test is a regression pin.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement**

Add `{{NOTES}}` and `{{TERMS}}` to `templates/invoice.html` after the direct-deposit block,
matching the PDF's ordering. In `render_invoice_html`, build each as
`<h3>Notes</h3><p>…</p>` from `esc(text).replace('\n', "<br>")`, or `String::new()` when the
field is `None` — the same shape the existing `pay` and `due` locals use.

- [ ] **Step 4: Verify** — `cargo test render -- --test-threads=1`, clippy, fmt.

- [ ] **Step 5: Commit** — `feat: render invoice notes and terms in the published HTML`

---

## Task 9: CLI — `client show` and `client edit`

**Files:**
- Modify: `src/cli/client.rs`, `src/cli/mod.rs`, `src/main.rs`
- Test: `tests/cli_dispatch.rs`

**Interfaces produced:** `nigel client show <id>`, `nigel client edit <id> [flags]`.

- [ ] **Step 1: Write the failing tests**

In `tests/cli_dispatch.rs`, using `TestEnv`:
- `client_show_prints_details_and_invoice_history` — init, add a client, create an invoice,
  `client show 1`, assert stdout contains the name, the email, the invoice number, and
  `Outstanding`.
- `client_show_for_an_unknown_id_fails_with_not_found` — `.failure()` and stderr contains
  `Client not found: id 99`.
- `client_edit_changes_the_email` — `client edit 1 --email new@acme.test`, then `client show 1`
  shows the new address.
- `client_edit_with_no_flags_fails` — stderr contains `Nothing to update`.

- [ ] **Step 2: Run and watch them fail** — `cargo test client_ --test cli_dispatch -- --test-threads=1`.

- [ ] **Step 3: Add the clap variants**

`src/cli/mod.rs`, `ClientCommands`: `Show { id: i64 }` and
`Edit { id: i64, name: Option<String>, email: Option<String>, address: Option<String>,
notes: Option<String> }`, the four optionals as `#[arg(long)]`. Every variant and every field
needs a doc comment — that is the `--help` text, and every sibling has one.

- [ ] **Step 4: Implement the wrappers**

`src/cli/client.rs`:
- `show(id)` — `client_summary`, then print the header/fields block and the comfy-table
  invoice table from the design's `client show` sample. Print `No invoices.` for an empty
  history. Use `-` for an absent optional field, matching `invoice show`'s
  `unwrap_or("-")` on the due date.
- `edit(id, name, email, address, notes)` — build a `ClientUpdate` with
  `email: email.map(Some)` (and likewise for address and notes), call `update_client`, then
  print `Updated client {id}: {name}` reading the name back from `get_client` so the line is
  right whether or not the name was the field that changed.

- [ ] **Step 5: Dispatch**

`src/main.rs`: add `ClientCommands::Show { id } => cli::client::show(id)` and the `Edit` arm.

- [ ] **Step 6: Verify** — the dispatch tests, then the full suite, clippy, fmt.

- [ ] **Step 7: Commit** — `feat: add nigel client show and client edit`

---

## Task 10: CLI — `invoice new --notes/--terms`, `invoice edit`, `invoice void`

**Files:**
- Modify: `src/cli/invoice.rs`, `src/cli/mod.rs`, `src/main.rs`
- Test: inline in `src/cli/invoice.rs`

**Interfaces produced:** `nigel invoice new --notes/--terms`, `nigel invoice edit`,
`nigel invoice void`.

- [ ] **Step 1: Write the failing tests**

Inline in `src/cli/invoice.rs` (pure helpers only — the command wrappers open the real
database and are covered end to end in Task 11):
- `parse_items_is_optional_for_an_edit` — a helper that maps an empty `--item` vec to `None`
  and a populated one through `parse_items`.
- `confirm_prompt_names_the_invoice` — the confirmation summary line contains the number, the
  client name, the total, and the status.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Add the clap variants**

`src/cli/mod.rs`. `InvoiceCommands::New` gains `notes: Option<String>` and
`terms: Option<String>`, both `#[arg(long)]`.

Two new variants, each field carrying a doc comment for `--help`:

- `Edit { number: i64, issue_date: Option<String> (`#[arg(long = "issue")]`),
  due_date: Option<String> (`long = "due"`), currency: Option<String>, notes: Option<String>,
  terms: Option<String>, items: Vec<String> (`long = "item"`) }` — the `--item` help text must
  say it replaces every existing line, and the variant's own doc comment must say published and
  void invoices refuse edits.
- `Void { number: i64, yes: bool }` — `--yes` documented as "required when stdin is not a TTY",
  matching `recategorize`'s wording.

Reuse the flag names and `long =` renames from `New` so `edit` and `new` read the same.

- [ ] **Step 4: Implement the wrappers**

`src/cli/invoice.rs`:

- `new(...)`: thread `notes` and `terms` through to `create_invoice` in place of the hardcoded
  `None, None`.
- `edit(number, issue, due, currency, notes, terms, items)`: `find_invoice`, build an
  `InvoiceUpdate` (`due: due.map(Some)`, likewise notes and terms; `items` becomes `None` when
  the vec is empty, else `Some(parse_items(&items)?)`), call `update_invoice`, re-read the
  invoice, print `Updated draft invoice #{number} — {total:.2} {currency}`. If the invoice had
  a Stripe link before the edit and does not after, print the extra
  `Cleared the stale Stripe payment link; …` line.
- `void(number, yes, today)`: `find_invoice`, `get_client` for the name, print the summary
  line, then the confirmation. A local `fn confirm_void(invoice, yes) -> Result<bool>` copying
  `recategorize`'s TTY logic (`std::io::IsTerminal`, `[y/N]`, `Aborted.` on no, and the
  non-TTY error `Refusing to void invoice #{number} without confirmation. Pass --yes.`).
  On yes: `void_invoice(&conn, invoice.id, today)`, print `Voided invoice #{number}.`, and
  when `invoice.published_at.is_some()` print the published-invoice warning from the design.

- [ ] **Step 5: Dispatch**

`src/main.rs`: destructure the new `New` fields, add the `Edit` arm, and add
`InvoiceCommands::Void { number, yes } => cli::invoice::void(number, yes, &today())`.

- [ ] **Step 6: Verify** — `cargo test -- --test-threads=1`, clippy, fmt. Also
  `cargo run -- invoice --help` and confirm every new flag has help text.

- [ ] **Step 7: Commit** — `feat: add nigel invoice edit and invoice void`

---

## Task 11: End-to-end command tests

**Files:**
- Modify: `tests/cli_dispatch.rs`

**Interfaces produced:** none — coverage.

- [ ] **Step 1: Write the tests** (they should pass immediately if Tasks 9 and 10 are right;
  if any fails, that is a real defect, not a test to relax)

- `invoice_new_persists_notes_and_terms` — create with both flags, assert via `env.db()` that
  the columns hold the text.
- `invoice_edit_updates_a_draft` — create, `invoice edit 1248 --due 2026-10-01 --item
  "Rework:2:250"`, then `invoice show 1248` shows `500.00` and the new due date.
- `invoice_edit_refuses_a_void_invoice` — void it first, then edit `.failure()` with
  `is void and cannot be edited` on stderr.
- `invoice_void_requires_confirmation_without_a_tty` — `invoice void 1248` with no `--yes`
  `.failure()`, stderr contains `Pass --yes`, and the DB still shows `status = 'draft'`.
- `invoice_void_with_yes_voids_and_blocks_send_and_pay` — `--yes` succeeds; a following
  `invoice pay 1248 --date …` fails with `void and cannot be paid`. (`invoice send` is not
  exercised here — it would need credentials; the guard is unit-tested in Task 5.)
- `invoice_new_with_an_unknown_client_reports_not_found` — `.failure()`, stderr contains
  `Client not found: id 99`, and `SELECT COUNT(*) FROM invoices` is `0`.

Note for the implementer: `assert_cmd`'s `.failure()` reads the process exit code; the error
text arrives on **stderr** because `main` prints `Error: {e}` there. Match the existing failing
tests in the file for the exact predicate style.

- [ ] **Step 2: Run** — `cargo test --test cli_dispatch -- --test-threads=1`.

- [ ] **Step 3: Verify** — full suite, clippy, fmt.

- [ ] **Step 4: Commit** — `test: cover invoice edit, void, and notes end to end`

---

## Task 12: Documentation

**Files:**
- Modify: `CLAUDE.md`, `docs/invoicing.md`, `README.md`

Per CLAUDE.md's Documentation Policy, the work is not complete until these land.

- [ ] **Step 1: CLAUDE.md**

- **Commands block** — add after the existing `nigel client add` line:
  ```
  nigel client show 1                               # One client: details plus invoice history
  nigel client edit 1 --email ap@acme.test          # Update a client's name/email/address/notes
  nigel invoice new … --notes "…" --terms "Net 30"  # Notes and terms render on the invoice
  nigel invoice edit 1248 --due 2026-09-30          # Edit a draft (published invoices refuse)
  nigel invoice void 1248                           # Cancel an invoice (confirms; --yes to skip)
  ```
- **Architecture, invoicing bullet** — mention `update_client`/`client_summary`,
  `update_invoice`/`void_invoice`, and that `voided_at` derives the `void` status.
- **Migrations bullet** — append `v5 adds voided_at to invoices so void is derived rather than
  hand-set`.
- **Project Structure** — update the `client.rs` and `invoice.rs` comment lines to list the
  new subcommands.
- **Key Design Constraints** — add: edit is draft-only and additionally refused once any
  payment is recorded (which is what makes a currency change after payment unreachable); void
  is terminal, refused for invoices with payments, and does not unpublish the R2 page or
  deactivate the Stripe link; an edit that moves the total or currency clears a stale payment
  link; editing a client affects the next send, not already-published pages.
- **Guardrail reason-code list** — extend with `void`, `not_draft`, `has_payments`,
  `already_void`.

- [ ] **Step 2: docs/invoicing.md**

Add sections, in the file's existing voice: "Inspecting and editing a client"
(`client show`/`client edit`, with the note that changes take effect on the next send),
"Editing a draft invoice" (the flag table, the all-or-nothing `--item` rule, and the guard
matrix in prose), "Voiding an invoice" (confirmation, what void does and does not tear down,
that it is terminal), and the `--notes`/`--terms` flags in the "Creating an invoice" section.

- [ ] **Step 3: README.md**

`rg 'nigel invoice|nigel client' README.md` — if it enumerates commands, add the five new
lines; if it does not, leave it.

- [ ] **Step 4: Final verification**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
```

Then a manual smoke run against a scratch data dir:

```bash
cargo run -- init --data-dir /tmp/nigel-68-1
cargo run -- client add "Acme Co" --email ap@acme.test
cargo run -- client show 1
cargo run -- client edit 1 --address "500 Market St"
cargo run -- invoice new --client 1 --issue 2026-08-08 --item "Consulting:10:150" \
    --notes "Thanks" --terms "Net 30"
cargo run -- invoice edit 1248 --due 2026-09-07 --item "Consulting:12:150"
cargo run -- invoice show 1248        # expect 1800.00 and the new due date
cargo run -- invoice void 1248        # expect the [y/N] prompt
cargo run -- invoice pay 1248 --date 2026-08-20   # expect "void and cannot be paid"
cargo run -- invoice new --client 99 --issue 2026-08-08 --item "X:1:1"  # expect "Client not found: id 99"
```

Confirm each output before claiming the task is done — do not infer it.

- [ ] **Step 5: Commit** — `docs: document client show/edit and invoice edit/void`

- [ ] **Step 6: Backlog**

Per the repo's memory note, file Backlog.md task updates as commits on `main`, never on a PR
branch, and only via the CLI:

```bash
backlog task edit 68.1 --check-ac 1 --check-ac 2 --check-ac 3 --check-ac 4
backlog task edit 68.1 -s Done
backlog task edit 38 -s Done      # absorbed: notes/terms flags + client show
backlog task edit 65 -s Done      # absorbed: unknown client id now answers NotFound
```

Do not edit the task markdown files directly.

---

## Self-review

**Acceptance-criteria coverage:**
- AC #1 `client show`/`client edit`, changes take effect on the next send → Tasks 3, 4, 9;
  the "next send" property is a consequence of `send_invoice` re-reading the client, noted in
  the design's Edge cases and documented in Task 12.
- AC #2 `invoice edit` is draft-only and refuses published/void by status → Tasks 5, 6, 10,
  11.
- AC #3 `invoice void` with confirmation; voided invoices refuse send and pay → Tasks 7, 10,
  11 (pay end to end; send's guard unit-tested in Task 5 because a real send needs
  credentials).
- AC #4 notes and terms settable at `new` and `edit`, rendered on the invoice → Tasks 6, 8,
  10, 11. PDF rendering already existed; HTML did not.
- TASK-65 → Task 2.
- TASK-38 → Tasks 8, 9, 10 (flags + `client show` + docs in Task 12).

**Ordering:** the data layer (1–8) lands before any CLI surface (9–11), so the TUI and web
tasks of the epic have something to call even if the CLI work slips. Task 1 is the only one
gated on an orchestrator decision, and it is first because `voided_at` changes the `Invoice`
struct every later task touches.

**Type consistency:** `ClientUpdate` and `InvoiceUpdate` share one convention —
`Option<T>` leaves alone, `Option<Option<T>>` clears — matching `rules::RuleUpdate` and the
`double_option` PATCH bodies TASK-68.6 will need. `NewLineItem` is reused unchanged from
`create_invoice`. Every guard returns `Conflict { code, message }` with a code from the design's
table.

**No placeholders:** every step names its file, its test, and its verification command. The
one deferred decision (Open question 1) is flagged at the top of Task 1 with the fallback
spelled out, not left as "decide later".

---

## Open questions for the orchestrator

The design document argues each of these in full; this is the short form, because they gate the
plan.

1. **Migration v5 / `voided_at` — gates Task 1, decide first.** The brief said v5 was needed for
   notes and terms; those columns already exist from v4. I propose v5 for `voided_at` instead,
   so `void` is derived from a timestamp the way `sent` is derived from `published_at` rather
   than being the one status read back out of the column it is supposed to compute. Alternative:
   no migration, and `void_invoice` writes `status = 'void'` directly.
2. **No `--clear-*` flags in v1**, following the set-only precedent in `rules update`. The
   data-layer structs support clearing, so each is one clap field plus one `map(Some)` later.
   `--clear-due` is the one with real semantics behind it (no due date means never overdue).
3. **Void of a partially paid invoice is refused.** Allowing it needs the payments deleted
   first, which needs an `invoice payment delete` command that does not exist.
4. **Stripe link deactivation on void** is out of scope; v1 prints a warning. Doing it needs a
   `deactivate_payment_link` on the `PaymentGateway` trait and makes `invoice void` a network
   operation that can fail — probably its own subtask under TASK-68.
5. **Re-publishing on client edit** is not part of `client edit`. Refreshing already-published
   pages would be a `nigel invoice republish` command.
6. **Re-typing `ensure_not_void` and `find_invoice`** to `Conflict`/`NotFound` moves their HTTP
   status from 500 to 409/404. CLI output is byte-identical and nothing breaks, but it changes a
   shipped surface — confirm it belongs here.
7. **Date and currency validation on `create_invoice`** (Task 6) fixes a pre-existing hole for
   about six lines. Say so if you want this task's diff kept strictly additive.
