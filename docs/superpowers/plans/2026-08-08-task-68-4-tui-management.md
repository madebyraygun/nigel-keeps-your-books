# Task 68.4 — TUI client and invoice management screens: implementation plan

Design: `docs/superpowers/specs/2026-08-08-task-68-4-tui-management-design.md`

Screen ids (S1–S10) below refer to that spec's wireframes.

## How to work this plan

Each step is RED → GREEN → verify. The verify command for every step is:

```bash
cargo test -- --test-threads=1
```

The DB password is a process global, so tests are serial. Run
`cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` before the
final step. Send-path work additionally needs `--features pdf` (the default) —
step 13 explains why.

## Testing conventions to follow

Manager screens in this repo are **mostly untested**. `account_manager.rs`,
`category_manager.rs`, `rules_manager.rs`, `reconcile_manager.rs`,
`import_manager.rs`, `undo_manager.rs` and `load_manager.rs` have no `mod tests`.
The two precedents:

- `src/cli/settings_manager.rs::tests` — temp-dir DB, construct the manager,
  drive `handle_key`, assert on manager state and DB rows.
- `src/browser.rs::tests` — pure state (scroll, selection, buffers), no terminal.

Follow both. Do **not** test `draw()` — no manager does, and a `TestBackend`
suite would be the only one in the crate (step 16 offers it as optional).

Every test DB in this plan needs migrations, because the invoicing tables arrive
in migration v4:

```rust
fn test_conn() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::get_connection(&dir.path().join("t.db")).unwrap();
    db::init_db(&conn).unwrap();
    crate::migrations::run_migrations(&conn).unwrap();
    (dir, conn)
}
```

(`src/invoicing/invoices.rs::tests` has exactly this helper — copy it.)

## Dependency on task 68.1

68.1's spec has landed
(`docs/superpowers/specs/2026-08-08-task-68-1-invoicing-edit-void-design.md`).
This plan is written against its real signatures:

```rust
invoicing::clients::{ClientUpdate, update_client(conn, id, &ClientUpdate)}
invoicing::invoices::void_invoice(conn, invoice_id, voided_on: &str)
invoicing::invoices::ensure_voidable(conn, &Invoice)
invoicing::invoices::validate_date(value, what)
models::Invoice::voided_at            // migration v5
```

**Blocked on 68.1's code landing: steps 5, 10 (the date check only) and 11.**
Steps 1–4, 6–9, 12–14 touch nothing 68.1 owns and can proceed in parallel.

Two things to re-check when 68.1's code is merged, because a spec is not a diff:

- Whether `cli::invoice::ensure_not_void` survived or was relocated into
  `invoicing::invoices` as part of routing void through
  `NigelError::Conflict { code: "void" }`. Step 2 and step 12 call whichever
  exists; they must not re-derive the check.
- Whether `Invoice.voided_at` and `INVOICE_COLS` really carry the new column, so
  the detail view's `Voided <date>` line has something to read.

---

## Step 1 — `list_invoices` and `payments` in the data layer

`src/invoicing/invoices.rs`

**RED.** Add to the existing `mod tests`:

- `list_invoices_is_newest_first_and_carries_client_and_paid` — seed one client
  and three invoices; record a partial payment against the middle one; assert
  the returned numbers descend, `client_name` is the joined name, and `paid`
  is the recorded amount on the middle row and `0.0` on the others.
- `list_invoices_on_an_empty_book_is_empty`.
- `payments_come_back_oldest_first` — record three payments with out-of-order
  `paid_date`s, assert the returned order is by `paid_date` then `id`.
- `payments_for_an_invoice_with_none_is_empty`.

**GREEN.** Add:

```rust
pub struct InvoiceListRow {
    pub id: i64,
    pub number: i64,
    pub status: String,
    pub client_name: String,
    pub total: f64,
    pub paid: f64,
    pub due_date: Option<String>,
}

pub fn list_invoices(conn: &Connection) -> Result<Vec<InvoiceListRow>>;
pub fn payments(conn: &Connection, invoice_id: i64) -> Result<Vec<InvoicePayment>>;
```

`list_invoices` is one statement — `invoices JOIN clients` with a
`LEFT JOIN invoice_payments` + `GROUP BY`, or a correlated
`COALESCE((SELECT SUM(amount) ...), 0)`. Not `paid_amount` per row: that is N+1
on a screen that redraws.

`payments` returns `models::InvoicePayment`, which exists and is currently
unused.

Both propagate row-deserialization errors (`collect::<Result<Vec<_>, _>>()`) —
CLAUDE.md: "Database row deserialization errors are propagated, never silently
discarded".

---

## Step 2 — Open the CLI guard helpers to the TUI

`src/cli/invoice.rs`

**RED.** None — this is a visibility change with no behaviour change, and the
existing tests (`void_invoices_are_refused_before_any_network_call_or_payment`,
`default_payment_is_the_outstanding_balance`, `missing_*_names_the_setting`,
…) are the regression net. They must still pass unchanged.

**GREEN.** Make the surviving private helpers `pub(crate)`:

- `build_clients(InvoicingConfig) -> Result<(StripeClient, R2Publisher, MailgunClient)>`
- `payment_amount(&Invoice, paid: f64, requested: Option<f64>) -> Result<f64>`
- `ensure_not_void(&Invoice, action: &str) -> Result<()>` — **only if 68.1 left
  it in place.** 68.1 routes the void guard through
  `NigelError::Conflict { code: "void" }`, which may have relocated it into
  `invoicing::invoices`. Call whichever exists; do not add a second one.

Nothing else moves. The point is that the TUI enforces the same guards in the
same words rather than re-deriving them.

---

## Step 3 — `ClientManager` skeleton and list navigation

New file `src/cli/client_manager.rs`; register it in `src/cli/mod.rs`.

**RED.** New `mod tests`:

- `new_loads_clients_sorted_by_name` — seed three out of order, assert the
  loaded order.
- `new_on_an_empty_book_has_no_selection_and_does_not_panic`.
- `down_and_up_move_the_selection_and_clamp` — Down past the end stays on the
  last row; Up from 0 stays at 0.
- `esc_closes` / `q_closes` — assert `ClientAction::Close`.
- `keys_on_an_empty_list_do_not_panic` — `Down`, `e`, `Enter` on zero clients.

**GREEN.** Copy `CategoryManager`'s shape:

```rust
pub enum ClientAction { Continue, Close }
enum Screen { List, Add(ClientForm), Edit(ClientForm) }

pub struct ClientManager {
    clients: Vec<Client>,
    selection: usize,
    scroll_offset: usize,
    last_visible_rows: usize,
    screen: Screen,
    status_message: Option<String>,
    status_ttl: u8,
    greeting: String,
}
```

`new(conn, greeting)` is infallible (`list_clients(conn).unwrap_or_default()`),
matching `CategoryManager::new`. `reload`, `set_status` (TTL 3), `ensure_visible`
are line-for-line the category manager's. Only `Screen::List` handling in this
step; `a` and `e` are no-ops so far.

---

## Step 4 — Add-client form (S2)

**RED.**

- `a_opens_the_add_form_with_empty_fields`.
- `enter_with_a_blank_name_reports_it_and_stays_on_the_form` — status message is
  exactly `Name is required`, screen is still `Add`, no row written.
- `enter_saves_a_client_and_returns_to_the_list` — drive the four fields by
  `KeyCode::Char`, assert the DB row, the reloaded list, and the status
  `Added client: Acme Co`.
- `blank_optional_fields_are_stored_as_null` — email/address/notes left empty
  come back as `None`, not `Some("")`.
- `fields_are_trimmed`.
- `esc_cancels_without_writing`.
- `tab_and_backtab_cycle_the_four_fields`.

**GREEN.** Four `FieldKind::Text` fields (Name, Email, Address, Notes) using
`category_manager`'s `FormField`. Only Name is required. Email is **not**
shape-checked — `nigel client add` does not check it either, and inventing a
TUI-only rule would let the front ends disagree about what a client is.

Note the trap `category_manager` already has: `KeyCode::Char('a')` reaches the
form's text handler, not the list's `a`, because the screen match runs first.
Keep the same ordering.

---

## Step 5 — Edit-client form (S3) · **blocked on 68.1**

**RED.**

- `e_opens_the_edit_form_prefilled_from_the_selected_row` — including `None`
  fields rendering as empty strings, not `"None"`.
- `enter_updates_the_client_and_returns_to_the_list` — assert the DB row and the
  status `Updated client: Acme Co`.
- `clearing_an_optional_field_writes_null` — the field must travel as
  `Some(None)`, not `None`. `None` would leave the old value in place, so this
  test is the one that pins the `ClientUpdate` convention.
- `an_unchanged_field_still_round_trips_its_current_value`.
- `a_blank_name_is_refused` — the message comes from `update_client` itself
  (`Name is required`), so assert it is rendered verbatim rather than
  pre-empted by a screen-local check.
- `a_data_layer_error_is_shown_verbatim_and_keeps_the_form_open` — force a
  failure (e.g. a `BEFORE UPDATE` trigger that RAISEs, the technique
  `failed_create_rolls_back_and_leaves_numbering_usable` uses) and assert the
  status message is `e.to_string()`, unre-worded.
- `e_on_an_empty_list_does_nothing`.

**GREEN.** `Screen::Edit(ClientForm)` sharing the add form's key handler via a
`FormMode` enum, exactly as `category_manager::handle_form_key` does. Saves
through `clients::update_client(conn, id, &ClientUpdate)`.

The form holds every current value, so it sends every field: `name: Some(v)`,
and each optional field as `Some(None)` when left blank or `Some(Some(v))`
otherwise. `ClientUpdate::is_empty()` therefore never fires and 68.1's
`Nothing to update` error is unreachable from this screen — that is fine, but do
not add a screen-local "nothing changed" check to compensate. Editing a row and
saving it unchanged is a harmless no-op write, exactly as
`category_manager`'s `update_category` already is.

---

## Step 6 — Client screen rendering

**RED.** None (no manager tests `draw`). Two pure helpers do get tests, in this
file's `mod tests`:

- `truncate_leaves_short_strings_alone_and_ellipsises_long_ones`.
- `optional_display_renders_none_as_an_em_dash` — the `—` rule for absent
  email/address.

**GREEN.** `draw` per S1/S2/S3: header + `━` separator + content + footer, the
four-`Constraint::Length(1)/Fill(1)` layout every manager uses. Columns:
marker 3, Name 28 (truncate 26), Email 28 (truncate 26), Billing address fills
the rest. Empty state `No clients yet. Press 'a' to add one.` Status message
takes over the footer in yellow when set, as `category_manager` does.

**Manual check:** `cargo run` → `k` → add a client, edit it, resize the terminal
below 80 columns and confirm nothing panics (ratatui truncates, but the
format-width strings must not index past the end).

---

## Step 7 — Wire Clients into the dashboard

`src/cli/dashboard.rs`

**RED.** New `mod tests` in `dashboard.rs` (it has none today) with three table
invariants — cheap, and they are what catches the renumbering hazard:

- `menu_shortcuts_are_unique` — every `MENU_ITEMS.1` distinct, and none of them
  `'q'` (quit) .
- `menu_labels_advertise_their_own_shortcut` — each label starts with
  `format!("[{}] ", key)`.
- `the_right_column_has_room_for_every_item` —
  `MENU_ITEMS.len() - MENU_LEFT_COUNT <= MENU_LEFT_COUNT`. This is the
  regression test for the clipping bug below.

**GREEN.**

1. `MENU_ITEMS` grows to 15 entries; insert at indices 8 and 9:
   `("[n] Invoices", 'n')`, `("[k] Clients", 'k')`.
2. **`MENU_LEFT_COUNT` 7 → 8.** The menu block is `MENU_LEFT_COUNT + 1` rows,
   one of which is the "What would you like to do?" title, so the right column
   can only show `MENU_LEFT_COUNT` items. At 15 items with `MENU_LEFT_COUNT = 7`
   the right column needs 8 rows and gets 7 — Snake silently disappears.
3. `activate_menu_item`: add arms 8 (Invoices, step 14) and 9 (Clients), and
   **renumber every arm from the old 8 upward by two** (View report 8→10,
   Export 9→11, Load 10→12, Settings 11→13, Snake 12→14).
4. `menu_item_line`'s `if i == 2` flagged-count case still points at Review —
   verify, do not move it.
5. `DashboardScreen::Clients(ClientManager)`, a `draw` arm, and a `handle_key`
   arm identical to the Categories arm.

**Manual check:** `cargo run`, confirm all 15 items render in two columns with
Snake still visible, and `k` opens Clients.

---

## Step 8 — `InvoiceManager` list state (S4)

New file `src/cli/invoice_manager.rs`; register it in `src/cli/mod.rs`.

**RED.**

- `new_loads_invoices_newest_first`.
- `new_on_an_empty_book_does_not_panic`.
- `navigation_clamps_at_both_ends` — Up/Down/Home/End/PageUp/PageDown.
- `esc_closes` / `q_closes`.
- `status_style_maps_every_invoice_status` — a table test over
  `draft/sent/partial/paid/overdue/void` asserting the `Style` from the spec's
  table, plus an unknown string falling back to the default style rather than
  panicking (the column is a `TEXT`, not an enum).
- `balance_is_total_minus_paid`.

**GREEN.**

```rust
pub enum InvoiceAction { Continue, Close, Perform }
enum Screen { List, Detail, ConfirmSend, Sending, ActionResult { .. },
              PayForm(PayForm), ConfirmVoid }
```

Struct mirrors `RulesManager` (rows, selection, scroll_offset,
last_visible_rows, screen, status_message, status_ttl, greeting) plus
`detail: Option<Detail>` and `detail_scroll: usize`.

`fn status_style(status: &str) -> Style` is a free function so it is directly
testable.

---

## Step 9 — Invoice detail view (S5)

**RED.**

- `enter_loads_the_detail_for_the_selected_invoice` — assert invoice number,
  client name, line-item count, payment count and `paid`.
- `esc_from_detail_returns_to_the_list_without_closing_the_screen`.
- `enter_on_an_empty_list_does_nothing`.
- `a_load_failure_reports_and_stays_on_the_list` — delete the client row out
  from under the invoice, assert the status message and `Screen::List`.
- `detail_scroll_clamps_at_zero`.

**GREEN.**

```rust
struct Detail {
    invoice: Invoice,
    client: Client,
    items: Vec<InvoiceLineItem>,
    payments: Vec<InvoicePayment>,
    paid: f64,
}
```

`fn load_detail(&mut self, conn) -> Result<()>` built from `get_invoice`,
`get_client`, `line_items`, `payments`, `paid_amount`; called on `Enter` and
after every mutation. Draw per S5: `Payments` section omitted entirely when
empty; `Pay link` only when `stripe_payment_link_url.is_some()`; the action keys
drop out of the footer for a void invoice.

---

## Step 10 — Record-payment form (S9)

**RED.**

- `p_on_a_void_invoice_is_refused_before_the_form_opens` — status is exactly
  `Invoice #1252 is void and cannot be paid.` (from
  `cli::invoice::ensure_not_void`), screen stays `Detail`.
- `p_prefills_the_amount_with_the_outstanding_balance_and_today`.
- `p_on_a_settled_invoice_prefills_an_empty_amount`.
- `method_options_are_exactly_the_four_the_schema_allows` — assert the option
  list equals `["direct_deposit", "ach", "stripe", "other"]` and defaults to
  `direct_deposit`.
- `every_method_option_is_actually_insertable` — loop the four options through
  `record_payment` against a real DB. This is the test that would have caught
  `check`/`wire`: `invoice_payments.method` carries
  `CHECK (method IN ('stripe','ach','direct_deposit','other'))`.
- `left_and_right_cycle_the_method`.
- Validation table, one test per row of the spec's table: empty amount,
  unparseable amount, `0`, `-25`, `nan`/`inf` text, empty date, `2026-8-7`,
  `not-a-date`. Assert the exact message and that no payment row was written.
  The two date rows assert 68.1's `validate_date` wording
  (`Invalid payment date: 2026-8-7 (expected YYYY-MM-DD)`), not a screen-local
  sentence — **blocked on 68.1**; until it lands, leave the date unvalidated
  and the two tests `#[ignore]`d rather than inventing a message that will have
  to be unpicked.
- `a_valid_payment_is_recorded_and_returns_to_detail` — assert the
  `invoice_payments` row, the refreshed status, the reloaded detail balance, and
  the status line `Recorded $750.00 against invoice #1254 (paid).`
- `an_overpayment_is_allowed` — 250 against a 100 invoice records and marks
  paid. `payment_amount` accepts it deliberately.
- `commas_are_stripped_from_the_amount` — `1,250.00` parses, as
  `reconcile_manager` does.
- `esc_cancels_without_writing`.

**GREEN.** Three fields: Amount (Text, accepts digits `.` `,`), Date (Text,
accepts digits and `-`), Method (Selector). Validation in the spec's order.

The date check is `invoices::validate_date(&date, "payment date")` — 68.1's
function, not a rule invented here, so the CLI and the TUI cannot disagree about
what a date is. It matters because the date is typed free-hand into a prefilled
field, and a malformed one poisons `refresh_status` (ISO string comparison
against `due_date`) and `ar_aging` (`parse_from_str`, which falls back to *today*
on failure).

One deliberate departure from `cli/invoice.rs`, worth a line in the commit
message: the zero/negative message drops the CLI's `--amount ` prefix, because a
flag name is useless beside a form field.

---

## Step 11 — Void with confirmation (S10) · **blocked on 68.1**

**RED.**

- `v_opens_the_confirmation_naming_the_invoice_client_and_total`.
- `n_and_esc_cancel_without_writing`.
- `y_voids_and_reloads_the_detail` — status column is `void`, `voided_at` is
  today, status line is `Voided invoice #1256.`, footer no longer offers actions.
- `void_writes_todays_date_as_voided_at` — the date argument is not incidental;
  `refresh_status` derives `void` from `voided_at`, so passing the wrong thing
  produces an invoice that will not stay void.
- `v_on_an_already_void_invoice_is_refused_before_the_dialog` — `Screen` stays
  `Detail`, message is `Invoice #1252 is already void.` verbatim.
- `v_on_an_invoice_with_payments_is_refused_before_the_dialog` — message is
  `Invoice #1254 has 1250.00 in recorded payments and cannot be voided.`
- `a_void_invoice_cannot_then_be_sent_or_paid` — an end-to-end assertion through
  the screen that the terminal status really is terminal.

**GREEN.** `Screen::ConfirmVoid` rendered inline at the bottom of the detail
view with the footer swapped to `y=void  n=cancel` — `category_manager`'s
delete-confirmation pattern. Pre-flight `invoices::ensure_voidable(conn, &inv)`,
then `invoices::void_invoice(conn, id, &chrono::Local::now().format("%Y-%m-%d").to_string())`,
then `load_detail`. The detail view gains a `Voided  <date>` line whenever
`invoice.voided_at.is_some()`.

---

## Step 12 — Send confirmation and its three guards (S6)

No network in this step: everything here happens *before* `send_invoice`.

**RED.**

- `s_on_a_void_invoice_is_refused_before_the_dialog` — exactly
  `Invoice #1252 is void and cannot be sent.`
- `s_on_a_client_with_no_email_is_refused_before_the_dialog` — exactly
  `client 'Acme Co' has no email`, the wording `send.rs` uses.
- `s_with_missing_invoicing_config_names_the_first_absent_key` — drive
  `build_clients` with an empty `InvoicingConfig`, assert the message contains
  `stripe_secret_key` and that the screen stayed on `Detail`. (Structure the
  guard so the config is injectable in tests rather than read from the user's
  real `settings.json`.)
- `the_confirmation_names_the_recipient_and_total`.
- `a_published_invoice_gets_the_resend_wording` — `published_at.is_some()`
  switches "Send" to "Re-send" and mentions link reuse.
- `n_and_esc_cancel_and_send_nothing`.
- `y_moves_to_sending_and_returns_perform` — assert `Screen::Sending` **and**
  `InvoiceAction::Perform`. This is the seam the dashboard depends on; assert
  both halves.

**GREEN.** Guards in the spec's order, then `Screen::ConfirmSend`. `y` sets
`Screen::Sending`, stashes the built clients (or the invoice id, rebuilding in
`perform_pending`), and returns `Perform`.

`build_clients` touches no network — it only reads settings and names the first
absent key — so running it at confirm time turns a multi-second failure into an
instant one.

---

## Step 13 — Execute the send (S7, S8)

Gate this module's send tests `#[cfg(all(test, feature = "pdf"))]`, the same gate
`src/invoicing/send.rs` uses: sending needs a real PDF to publish and attach, so
without the feature `render_pdf` always errors and the orchestration cannot be
exercised.

**RED.** Local `FakeGw` / `FakePub` / `FailPub` / `FakeMail` fakes (copy the ~30
lines from `src/invoicing/send.rs::tests` — they are private to that module).

- `a_successful_send_publishes_emails_and_shows_the_url` — assert
  `Screen::ActionResult { is_error: false }`, the URL in the lines, the reloaded
  invoice status `sent`, and one email sent.
- `a_failed_send_reports_the_error_verbatim_and_the_reloaded_status` — with
  `FailPub`: the result carries `upload down` and
  `Invoice #1254 is still draft.`, the invoice is still `draft`, no email sent.
- `a_failed_resend_says_the_invoice_is_still_sent_not_still_draft` — publish
  first, then fail the second attempt. This is the whole reason the sentence is
  derived from the reloaded row rather than hardcoded.
- `a_resend_reuses_the_existing_payment_link` — `create_calls == 1` after two
  sends.
- `any_key_on_the_result_returns_to_the_reloaded_detail`.

**GREEN.**

```rust
pub fn perform_pending(&mut self, conn: &Connection);          // real clients
pub(crate) fn perform_send<G, P, M>(&mut self, conn, today,
    contact_email, gateway, publisher, mailer);                // testable half
```

`perform_send` calls `send_invoice`, reloads the detail, and sets
`Screen::ActionResult`. On failure the second line is built from the *reloaded*
invoice's status.

`perform_pending` finishes by **draining buffered input**:

```rust
while crossterm::event::poll(std::time::Duration::ZERO).unwrap_or(false) {
    let _ = crossterm::event::read();
}
```

Without it, a user who mashed Enter while the terminal was unresponsive
dismisses the result screen before reading it. This cannot be unit-tested (there
is no terminal) — verify it by hand in step 15.

---

## Step 14 — Wire Invoices into the dashboard, including the `Perform` seam

**RED.** Extend `dashboard.rs::tests`:

- `every_menu_index_has_an_activate_arm` — drive `activate_menu_item` for
  `0..MENU_ITEMS.len()` against a seeded temp DB and assert the resulting
  `DashboardScreen` discriminant matches an expected table. This is what catches
  a missed renumber in step 7 as well as a missed arm here. (Skip index 12
  Snake's tick loop; constructing it is enough.)

**GREEN.**

1. `DashboardScreen::Invoices(InvoiceManager)`, a `draw` arm, and a
   `handle_key` arm.
2. The `Perform` seam, beside the existing `pending_reload` local:

```rust
let mut pending_invoice_work = false;

DashboardScreen::Invoices(ref mut mgr) => {
    match mgr.handle_key(key.code, &conn) {
        InvoiceAction::Close => return_home = true,
        InvoiceAction::Continue => {}
        InvoiceAction::Perform => pending_invoice_work = true,
    }
    false
}

// after the borrow ends
if pending_invoice_work {
    let _ = terminal.draw(|frame| dashboard.draw(frame)); // paint S7
    if let DashboardScreen::Invoices(ref mut mgr) = dashboard.screen {
        mgr.perform_pending(&conn);
    }
}
```

The extra `terminal.draw` is the whole trick: the loop is
draw → blocking `event::read()` → `handle_key`, so work done inside `handle_key`
happens after the last paint and would freeze the screen on the *confirmation
dialog*. Painting once more first means the frozen frame is S7, which says it is
frozen.

---

## Step 15 — Manual verification

`cargo run` against a demo database seeded with clients and invoices in every
status (`nigel demo` has none — seed with `nigel client add` and
`nigel invoice new`, and hand-edit statuses in SQLite for `void`/`overdue`).

- [ ] All 15 menu items render in two columns; Snake is still visible.
- [ ] `k` → add, edit, blank-name refusal, Esc.
- [ ] `n` → scroll a list longer than the window; status colours read correctly
      on a dark and a light terminal.
- [ ] `Enter` → detail with line items, payments, balance, pay link.
- [ ] `p` → record a partial payment; balance and status update; record the rest;
      status becomes `paid`.
- [ ] `v` on a draft → confirm → `void`; footer drops the actions; `s` and `p`
      are now refused with the CLI's sentences.
- [ ] `s` with no `stripe_secret_key` configured → instant, named refusal.
- [ ] `s` against Stripe/Mailgun **test** credentials → S7 renders, the terminal
      is unresponsive for the duration, mashing Enter during the wait does
      **not** skip S8.
- [ ] Resize below 80 columns on every screen; nothing panics.
- [ ] Esc from every screen and sub-screen lands where the footer says it will.

---

## Step 16 — Optional: one layout smoke test

Only if the wireframes are treated as a contract. There is no `TestBackend`
precedent in this crate, so this is a new pattern and should be one test, not a
suite:

```rust
let backend = ratatui::backend::TestBackend::new(80, 24);
```

Render the invoice list and assert the column header row and the footer hint
string appear. Skip if the reviewer would rather not introduce the pattern.

---

## Step 17 — Documentation

CLAUDE.md's Documentation Policy makes this part of the work, not a follow-up.

- **CLAUDE.md**
  - Architecture: two new bullets, styled like the Category Manager and Rules
    Manager bullets — Client Manager (list/add/edit, no delete, data layer in
    `invoicing/clients.rs`) and Invoice Manager (list, detail, send/pay/void).
  - Architecture, Dashboard bullet: add `n`=Invoices and `k`=Clients to the
    shortcut list.
  - Architecture, Invoicing bullet: mention `list_invoices`/`payments`.
  - Key Design Constraints: one bullet on draw-then-block — why the TUI paints
    S7 before running `send_invoice` on the main thread, and why a thread plus a
    second `Connection` was rejected.
  - Project Structure: `client_manager.rs`, `invoice_manager.rs`.
- **README.md** — the two screens wherever dashboard commands are listed.
- **docs/invoicing.md** — a short "From the dashboard" section pointing at `n`
  and `k` and naming what the TUI cannot do (create an invoice).

---

## Step 18 — Final verification

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
```

The `--no-default-features` run is not optional: it is what proves the
`#[cfg(all(test, feature = "pdf"))]` gate on the send tests is correct and that
the invoice screen still compiles in a build where `send_invoice` can never
succeed. Confirm the output of each before claiming the task is done.

---

## Notes and hazards

- **`MENU_LEFT_COUNT` is the silent one.** Adding menu items without bumping it
  clips the right column with no error. Step 7's third test is the guard.
- **`activate_menu_item` matches on bare integers.** Inserting mid-array
  renumbers everything after it. Step 14's table test is the guard.
- **`invoice_payments.method` has a CHECK constraint** — only `stripe`, `ach`,
  `direct_deposit`, `other`. A fifth selector option is a runtime failure, not a
  compile error.
- **`Screen` matching order in form handlers.** `KeyCode::Char('a')` must reach
  the form's text input, not the list's "add" binding. `category_manager` gets
  this right by matching on `self.screen` first; copy the structure.
- **`void_invoice` takes a date, and it is load-bearing.** 68.1 makes `void` a
  status derived from `voided_at` the way `sent` is derived from `published_at`.
  Passing a wrong or empty date yields an invoice that `refresh_status` will
  quietly un-void.
- **`ClientUpdate` distinguishes absent from cleared.** `None` leaves a column
  alone; `Some(None)` clears it. The edit form always means the latter for a
  blank optional field.
- **Never re-word a `NigelError`.** Every guardrail message in these screens is
  either `e.to_string()` or a sentence lifted verbatim from `cli/invoice.rs` /
  `send.rs`. The two sanctioned departures are the payment amount/date messages
  in step 10, and they are sanctioned because they would otherwise name a CLI
  flag that does not exist on the screen.
- **`refresh_status` is not called on screen open.** The list prints the stored
  status, exactly as `nigel invoice list` does, so a newly-overdue invoice still
  reads `sent`. Open question 4 in the spec; do not "fix" it silently.
