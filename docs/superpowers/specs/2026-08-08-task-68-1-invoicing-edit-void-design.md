# TASK-68.1 — CLI: client show/edit, invoice edit/void — Design

**Task:** TASK-68.1 (parent epic TASK-68). Absorbs TASK-38 (notes/terms flags, `client show`)
and fixes TASK-65 (unknown client id surfaces a raw FOREIGN KEY error).

**Position in the epic:** this is the foundation. TASK-68.4 (TUI screens) and TASK-68.6 (web
endpoints) call the functions designed here. Every operation is therefore a
`&Connection`-in / plain-struct-out function in `src/invoicing/`, with a thin printing wrapper
in `src/cli/` — the same split `src/cli/rules.rs` uses for `list_rules`/`add_rule`/
`update_rule`/`deactivate_rule`.

---

## Goal

Make the invoicing surface editable and cancellable:

1. `client show <id>` and `client edit <id>` — inspect and amend a client.
2. `invoice edit <number>` — amend a **draft** invoice's issue date, due date, currency,
   notes, terms, and line items.
3. `invoice void <number>` — cancel an invoice, with confirmation, making the already-guarded
   `void` status reachable for the first time.
4. `invoice new --notes/--terms` — the flags TASK-38 asks for, plus notes/terms rendering in
   the HTML invoice.
5. Unknown ids answer `NigelError::NotFound`, never `Database error: FOREIGN KEY constraint
   failed`.

## Non-goals

- No `unvoid`. Void is terminal, as `refresh_status` already assumes.
- No refund or credit-note model. An invoice with money against it cannot be voided or edited.
- No per-line-item CLI surgery (`--set-item 2:…`). `--item` replaces the whole set.
- No Stripe payment-link deactivation on void, and no re-publish of an already-published R2
  page on edit. Both are called out under Edge cases and Open questions.
- No TUI or web surface. Those are TASK-68.4 and TASK-68.6, consuming what is built here.

---

## Corrections to the task's stated premises

Two things stated in the task brief do not match the code, and the design accounts for the
code as it is:

- **`invoices` already has `notes` and `terms` columns.** Migration v4
  (`src/migrations.rs:58-114`) creates them, `Invoice` carries them
  (`src/models.rs:141-142`), `create_invoice` persists them
  (`src/invoicing/invoices.rs:53-60`), and `src/pdf.rs:864-873` renders both. No migration is
  needed for notes/terms. What is missing is (a) the CLI flags — `cli::invoice::new` hardcodes
  `None, None` at `src/cli/invoice.rs:133-135` — and (b) HTML rendering:
  `src/invoicing/templates/invoice.html` has no `{{NOTES}}`/`{{TERMS}}` placeholder, so
  TASK-38's claim that "the HTML/PDF templates render them" is only half true.
- **Migration v5 is still needed, for a different reason:** `voided_at`. See below.

---

## Migration v5

```rust
Migration {
    version: 5,
    description: "add voided_at to invoices so void is a derived status like sent",
    up: |conn| {
        conn.execute_batch(
            "ALTER TABLE invoices ADD COLUMN voided_at TEXT;
             UPDATE invoices SET voided_at = COALESCE(published_at, issue_date)
                 WHERE status = 'void' AND voided_at IS NULL;",
        )?;
        Ok(())
    },
},
```

**Why this column earns its place.** CLAUDE.md states the invariant plainly: *"Invoice status
is derived, never set by hand: `refresh_status` recomputes draft/sent/partial/paid/overdue
from `published_at`, the payment total, and the due date."* `void` is today the single
exception — `refresh_status` reads the `status` column it is supposed to be computing
(`src/invoicing/invoices.rs:166-168`), which is a self-referential source of truth. Adding a
`void_invoice` that writes `status = 'void'` directly would cement that exception in a second
place.

`voided_at` removes it. Void becomes derivable exactly the way `sent` is derivable from
`published_at`, and `void_invoice` writes a timestamp and then calls `refresh_status` like
every other mutation. It also answers "when was this voided", which TASK-68.4 and TASK-68.6
will want on a detail screen and which cannot be reconstructed after the fact.

The backfill is defensive — `import_invoiceshelf` only ever writes `paid` or `sent` — but it
costs one statement and protects any database whose status was set by hand.

**Consequential changes:**

- `src/models.rs` — `Invoice` gains `pub voided_at: Option<String>`.
- `src/invoicing/invoices.rs` — `INVOICE_COLS` gains `voided_at`; `row_to_invoice` reads index
  16; `refresh_status`'s first check becomes `if inv.voided_at.is_some() { … }`.
- The existing test `void_is_never_downgraded` sets `status='void'` by raw SQL; it must set
  `voided_at` instead. Test-only change, same assertion.

---

## Data-layer function signatures

All of these live in `src/invoicing/`, take `&Connection` first, and return plain structs.

### `src/invoicing/clients.rs`

```rust
/// A client by id. Now reports a missing row as NotFound rather than letting
/// `QueryReturnedNoRows` escape as `Database error: …`.
pub fn get_client(conn: &Connection, id: i64) -> Result<Client>;

/// Cheap existence probe, mirroring `cli::categories::ensure_category_exists`.
/// This is the TASK-65 fix: `create_invoice` calls it before inserting.
pub fn ensure_client_exists(conn: &Connection, id: i64) -> Result<()>;

/// Fields to change on a client. `None` leaves a field alone; `Some(None)`
/// clears it — the shape `cli::rules::RuleUpdate` uses for `vendor`, and the
/// shape the web layer's `double_option` PATCH bodies need (TASK-68.6).
#[derive(Debug, Default, Clone)]
pub struct ClientUpdate {
    pub name: Option<String>,                    // NOT NULL — cannot be cleared
    pub email: Option<Option<String>>,
    pub billing_address: Option<Option<String>>,
    pub notes: Option<Option<String>>,
}

impl ClientUpdate {
    pub fn is_empty(&self) -> bool;
}

pub fn update_client(conn: &Connection, id: i64, update: &ClientUpdate) -> Result<()>;

/// One row of a client's invoice history, for `client show`.
#[derive(Debug, Clone)]
pub struct ClientInvoiceRow {
    pub number: i64,
    pub status: String,
    pub issue_date: String,
    pub due_date: Option<String>,
    pub total: f64,
    pub paid: f64,
}

/// A client plus everything `client show` prints, in one round trip.
#[derive(Debug, Clone)]
pub struct ClientSummary {
    pub client: Client,
    pub invoices: Vec<ClientInvoiceRow>,   // newest number first
    pub outstanding: f64,                   // open invoices only; void/paid excluded
}

pub fn client_summary(conn: &Connection, id: i64) -> Result<ClientSummary>;
```

`update_client` builds its `SET` clause dynamically from the populated fields, the same
technique `rules::update_rule` uses (`src/cli/rules.rs:220-252`). An empty update is an error,
not a silent no-op. A blank `name` is rejected the way `accounts::rename_account` rejects one.
`UPDATE … WHERE id = ?` affecting zero rows is `NotFound`.

`outstanding` sums `total - paid` over invoices whose status is `sent`, `partial`, or
`overdue` — the same filter `ar_aging` uses — so a voided or fully paid invoice contributes
nothing.

### `src/invoicing/invoices.rs`

```rust
/// Fields to change on a draft invoice. Same `Option`/`Option<Option<_>>`
/// convention as `ClientUpdate`. `items: Some(v)` replaces the entire line-item
/// set; `None` leaves the existing lines alone.
#[derive(Debug, Default)]
pub struct InvoiceUpdate {
    pub issue_date: Option<String>,
    pub due_date: Option<Option<String>>,
    pub currency: Option<String>,
    pub notes: Option<Option<String>>,
    pub terms: Option<Option<String>>,
    pub items: Option<Vec<NewLineItem>>,
}

impl InvoiceUpdate {
    pub fn is_empty(&self) -> bool;
}

/// Apply a partial update to a draft invoice. Guarded by `ensure_editable`.
/// Runs in one transaction: field updates, line-item replacement, total
/// recomputation, stale-payment-link clearing, and `refresh_status`.
pub fn update_invoice(conn: &Connection, invoice_id: i64, update: &InvoiceUpdate) -> Result<()>;

/// Cancel an invoice. Guarded by `ensure_voidable`. Writes `voided_at` and lets
/// `refresh_status` derive the `void` status from it.
pub fn void_invoice(conn: &Connection, invoice_id: i64, voided_on: &str) -> Result<()>;

/// May this invoice be edited? Draft, not void, no recorded payments.
pub fn ensure_editable(conn: &Connection, invoice: &Invoice) -> Result<()>;

/// May this invoice be voided? Not already void, no recorded payments.
pub fn ensure_voidable(conn: &Connection, invoice: &Invoice) -> Result<()>;

/// Delete and reinsert the line items at dense positions 0..n-1, returning the
/// recomputed (subtotal, total). Private; called inside `update_invoice`'s
/// transaction.
fn replace_line_items(conn: &Connection, invoice_id: i64, items: &[NewLineItem])
    -> Result<(f64, f64)>;

/// `YYYY-MM-DD` or an `Invalid` error naming the field.
pub fn validate_date(value: &str, what: &str) -> Result<()>;

/// Normalizes a 3-letter code to uppercase, or an `Invalid` error.
pub fn validate_currency(code: &str) -> Result<String>;
```

`create_invoice` gains three lines at the top: `ensure_client_exists`, `validate_date` on
issue and due, `validate_currency` on the code. That closes TASK-65 and stops a malformed date
from reaching `ar_aging`, which today errors at report time on a date it accepted at creation
time.

### `src/invoicing/render_html.rs` and `templates/invoice.html`

Two new placeholders, emitted as empty strings when the field is `None` — exactly how `{{DUE}}`
and `{{PAY}}` already behave:

```
{{NOTES}}   →  <h3>Notes</h3><p>…</p>     or  ""
{{TERMS}}   →  <h3>Terms</h3><p>…</p>     or  ""
```

Placed after the direct-deposit block, matching the PDF's ordering (`src/pdf.rs:864-873`).
Content runs through the existing `esc()` and then newline → `<br>`, so a multi-line terms
block reads as written and cannot inject markup.

---

## CLI grammar

### `nigel client show <id>`

```
nigel client show 3
```

Mirrors the shape of `invoice show`: a header line, labelled fields, a table, a total.

```
Client #3  Acme Co
Email:    ap@acme.test
Address:  123 Main St, Portland OR
Notes:    -
+------+---------+------------+--------+--------+
| #    | Status  | Issued     | Total  | Paid   |
+------+---------+------------+--------+--------+
| 1249 | sent    | 2026-08-04 | 250.00 |   0.00 |
| 1248 | paid    | 2026-07-01 | 400.00 | 400.00 |
+------+---------+------------+--------+--------+
Outstanding: 250.00
```

A client with no invoices prints the header and fields, then `No invoices.`

### `nigel client edit <id> [flags]`

```
nigel client edit 3 --email billing@acme.test
nigel client edit 3 --name "Acme Corporation" --address "500 Market St"
```

| Flag | Type | Effect |
|---|---|---|
| `--name <s>` | optional | Rename. Blank is rejected. |
| `--email <s>` | optional | Set billing email. |
| `--address <s>` | optional | Set billing address. |
| `--notes <s>` | optional | Set internal notes. |

No flags → `Nothing to update — provide at least one flag`, the exact wording
`rules::update_rule` uses.

**No `--clear-*` flags in v1.** This follows the precedent set in `cli::rules::update`, which
notes that "the CLI can set a vendor but has no flag for clearing one" and passes
`vendor.map(Some)`. The `ClientUpdate` struct supports clearing so TASK-68.4/68.6 can offer it;
only the CLI declines to. Listed under Open questions.

Output: `Updated client 3: Acme Corporation`

### `nigel invoice new` — two new flags

```
nigel invoice new --client 1 --issue 2026-08-04 --item "Consulting:10:150" \
    --notes "Thanks for the work this quarter." \
    --terms "Net 30. Late payments accrue 1.5% monthly."
```

| Flag | Type | Effect |
|---|---|---|
| `--notes <s>` | optional | Free text, rendered under a "Notes" heading in HTML and PDF. |
| `--terms <s>` | optional | Free text, rendered under a "Terms" heading in HTML and PDF. |

Both plumb straight into `create_invoice`'s existing `notes`/`terms` parameters, replacing the
hardcoded `None, None`.

### `nigel invoice edit <number> [flags]`

```
nigel invoice edit 1248 --due 2026-09-30
nigel invoice edit 1248 --item "Discovery:1:2000" --item "Build:40:150"
nigel invoice edit 1248 --currency EUR --terms "Net 15"
```

| Flag | Type | Effect |
|---|---|---|
| `--issue <YYYY-MM-DD>` | optional | New issue date. |
| `--due <YYYY-MM-DD>` | optional | New due date. |
| `--currency <CODE>` | optional | New 3-letter currency code, normalized to uppercase. |
| `--notes <s>` | optional | Replace notes. |
| `--terms <s>` | optional | Replace terms. |
| `--item "desc:qty:unit"` | repeatable | **Replaces every line item.** Parsed by the existing `parse_item`. |

No flags → `Nothing to update — provide at least one flag`.

Output, echoing the recomputed total the way `invoice new` does:

```
Updated draft invoice #1248 — 8000.00 EUR
```

When the edit changed the total or currency on an invoice that already carried a Stripe link:

```
Updated draft invoice #1248 — 8000.00 EUR
Cleared the stale Stripe payment link; `nigel invoice send 1248` will create a new one.
```

### `nigel invoice void <number> [--yes]`

```
nigel invoice void 1248
nigel invoice void 1248 --yes
```

Confirmation follows the `recategorize` pattern verbatim
(`src/cli/recategorize.rs:280-297`): a TTY gets a `[y/N]` prompt, a non-TTY without `--yes` is
a hard error rather than a silent apply.

```
$ nigel invoice void 1248
Invoice #1248 — Acme Co, 250.00 USD, sent 2026-08-04.
Void it? [y/N] y
Voided invoice #1248.
```

Answering anything but `y` prints `Aborted.` and exits `0`. Non-TTY without `--yes`:

```
Error: Refusing to void invoice #1248 without confirmation. Pass --yes.
```

Voiding an already-published invoice appends a warning, because nothing is torn down:

```
Voided invoice #1248.
Warning: this invoice was already published. Its page and Stripe payment link stay live —
deactivate the link in Stripe if you do not want it paid.
```

### Clap wiring

`src/cli/mod.rs` — `ClientCommands` gains `Show { id: i64 }` and `Edit { id, name, email,
address, notes }`; `InvoiceCommands` gains `Edit { number, issue_date, due_date, currency,
notes, terms, items }` and `Void { number, yes }`, and `New` gains `notes`/`terms`.
`src/main.rs` gains the matching dispatch arms; `Void` receives `&today()` for `voided_at`, the
same way `Send`/`Pay` already receive it.

---

## Guard matrix

Rows are operations; columns are the invoice's current status. ✅ allowed, ❌ refused.

| Operation | draft | sent | partial | overdue | paid | void |
|---|---|---|---|---|---|---|
| `invoice show` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `invoice edit` | ✅ ¹ | ❌ `not_draft` | ❌ `not_draft` | ❌ `not_draft` | ❌ `not_draft` | ❌ `void` |
| `invoice void` | ✅ | ✅ | ❌ `has_payments` | ✅ | ❌ `has_payments` | ❌ `already_void` |
| `invoice send` | ✅ | ✅ resend | ✅ resend | ✅ resend | ✅ resend | ❌ `void` (existing) |
| `invoice pay` | ✅ | ✅ | ✅ | ✅ | ✅ ² | ❌ `void` (existing) |
| `client edit` | always allowed — see Edge cases | | | | | |

¹ A draft may still hold a recorded payment (`invoice pay` works on drafts today), in which
case edit is refused with `has_payments`.
² Requires an explicit `--amount`; the existing "no outstanding balance" error otherwise.

**Guard evaluation order** matters, because the more specific message should win:

- `ensure_editable`: void → `not draft` → `has payments`. (A draft can never be void, so the
  first two are disjoint in practice; ordering is for readability.)
- `ensure_voidable`: already void → `has payments`.

**Why payments block both.** One rule closes three holes at once: voiding a partially paid
invoice would strand its `invoice_payments` rows against a record that has left every aging
bucket; editing line items under a recorded payment would silently restate what the client
already paid against; and changing `currency` after a payment would misdenominate a settled
amount. Refusing on `paid_amount(conn, id) > 0` covers all three without inventing a
credit-note model Nigel does not have.

### Error variants and exact wording

Guard failures use `NigelError::Conflict { code, message }`, which CLAUDE.md already
designates as the 409-with-machine-readable-reason variant — TASK-68.6 needs the code, and the
`Display` text is the message alone, so the CLI prints exactly what it prints today.

| Condition | Variant | Message |
|---|---|---|
| Void invoice, any of edit/send/pay | `Conflict { code: "void" }` | `Invoice #1248 is void and cannot be edited.` (`sent` / `paid` for the others) |
| Published invoice, edit | `Conflict { code: "not_draft" }` | `Invoice #1248 has already been sent and cannot be edited. Void it and issue a new one.` |
| Payments recorded, edit | `Conflict { code: "has_payments" }` | `Invoice #1248 has 500.00 in recorded payments and cannot be edited.` |
| Payments recorded, void | `Conflict { code: "has_payments" }` | `Invoice #1248 has 500.00 in recorded payments and cannot be voided.` |
| Already void, void | `Conflict { code: "already_void" }` | `Invoice #1248 is already void.` |
| Empty update | `Invalid` | `Nothing to update — provide at least one flag` |
| Blank client name | `Invalid` | `Name is required` |
| Bad date | `Invalid` | `Invalid issue date: 2026-13-45 (expected YYYY-MM-DD)` |
| Bad currency | `Invalid` | `Invalid currency: dollars (expected a 3-letter code like USD)` |
| Unknown client id | `NotFound` | `Client not found: id 99` |
| Unknown invoice number | `NotFound` | `No invoice #9999. Run \`nigel invoice list\` to see invoice numbers.` |

Wording conventions taken from the existing code, and deliberately preserved:

- Guard sentences are full sentences with a trailing period and lead with `Invoice #<number>`
  — the shape `ensure_not_void` established (`src/cli/invoice.rs:58-66`).
- `NotFound` messages have **no** trailing period and read `<Thing> not found: id <n>`, matching
  `accounts::get_account` and `categories::ensure_category_exists`.
- `Invalid` messages have no trailing period, matching `accounts::add_account` and
  `rules::update_rule`.
- Money in a message is `{:.2}` with no currency symbol, matching `payment_amount`.

Two existing sites are re-typed without changing a character of their output:
`ensure_not_void` moves from `Other` to `Conflict { code: "void" }`, and `find_invoice` moves
from `Other` to `NotFound`. Both are today mapped to HTTP 500 by the server's error mapping;
after the change they are 409 and 404, which is what TASK-68.6 needs and what the CLI tests
(which assert on substrings of `Display`) will not notice.

New reason codes to add to CLAUDE.md's published list alongside `has_transactions`,
`has_active_rules`, `duplicate_name`, `already_inactive`, `no_transactions`: **`void`**,
**`not_draft`**, **`has_payments`**, **`already_void`**.

---

## Edge cases

**Editing line items renumbers.** `replace_line_items` deletes every row for the invoice and
reinserts at dense positions `0..n-1`, so row ids are reissued and positions never sparse.
Nothing in the schema references `invoice_line_items.id`, so this is safe — but any caller
holding a line-item id across an edit must re-read. `--item` is all-or-nothing: omitted leaves
the lines alone, supplied replaces them entirely. There is no way to leave an invoice with zero
lines; `parse_items`' existing "an invoice needs at least one --item" rule applies to the
replacement set.

**Totals are recomputed, tax is not touched.** `subtotal` becomes the sum of the new lines,
`total` becomes `subtotal + tax`, and `tax` stays whatever it was (always `0.0` today — Nigel
has no tax feature). The recomputation happens inside the same transaction as the line-item
rewrite, so a failure mid-edit leaves the invoice exactly as it was.

**Void of a paid invoice is refused,** as is void of a partially paid one — see the guard
matrix rationale above. The only way to cancel an invoice with money against it is to record
the offsetting movement in the transaction register, which is where cash actually lives.

**Currency change after a payment is impossible by construction.** Edit requires draft *and*
zero payments, so there is no state in which a recorded payment's denomination can be changed
underneath it.

**Void of a published invoice does not unpublish it.** The R2 objects at `i/{token}/index.html`
and `i/{token}/invoice.pdf` stay served, and the Stripe Payment Link stays chargeable. Worse,
`sync_all` filters on `status IN ('sent','partial','overdue')`, so a payment made against a
voided invoice's link would never be recorded. v1 mitigates with the warning line quoted in the
CLI grammar above; actually deactivating the link needs a new `PaymentGateway` method and is
listed under Open questions.

**Editing a draft with a stale Stripe link.** `send_invoice` sets the payment link *before*
`mark_published` (`src/invoicing/send.rs:26-31`), so a send that fails at R2 or Mailgun leaves
a draft carrying a live link for the old amount. If an edit changes `total` or `currency`,
`update_invoice` clears `stripe_payment_link_id` and `stripe_payment_link_url`, so the next
`send` creates a fresh link at the right amount instead of reusing the wrong one. The abandoned
Stripe link is left alone, same as in the void case.

**Editing a client does not rewrite published invoices.** Published pages are static R2
snapshots; `send_invoice` reads the client fresh on every call. So a corrected email or address
takes effect on the next `send` — including a re-send of the same invoice, which overwrites the
same token path — which is exactly what AC #1 asks for. Already-delivered emails and any
already-fetched PDF keep the old details.

**Void is terminal and there is no unvoid.** A voided invoice disappears from `ar_aging` and
from `sync_all` (both filter on open statuses) but stays in `invoice list` with status `void`,
and its number is never reused — `next_invoice_number` only ever moves forward.

**`refresh_status` now derives void from `voided_at`.** A row with `status = 'void'` but
`voided_at IS NULL` — reachable only by hand-editing the database — would be re-derived back
to a live status on the next `refresh_status`. The migration's backfill covers the only
realistic source of such rows.

**Unknown ids, end to end.** `invoice new --client 99` calls `ensure_client_exists` before it
opens a transaction, so it reports `Client not found: id 99` and inserts nothing — no invoice
row, no number consumed (TASK-65 AC #1). `client show 99` and `client edit 99` report the same
string. `invoice edit 9999` and `invoice void 9999` reuse `find_invoice`'s existing
`No invoice #9999. Run \`nigel invoice list\` to see invoice numbers.`

**Non-TTY void without `--yes` exits non-zero.** Scripts get a refusal, never a silent
cancellation of a real invoice.

---

## Testing

Data-layer tests are inline `#[cfg(test)] mod tests` in each `src/invoicing/*.rs`, using the
existing `test_conn()` helper (tempdir → `get_connection` → `init_db` → `run_migrations`).
CLI-shape tests are inline in `src/cli/*.rs` for pure helpers, and end-to-end command tests go
in `tests/cli_dispatch.rs` using the existing `TestEnv` (temp `HOME`, invoicing env vars
cleared, so no test can reach Stripe, R2, or Mailgun).

Every guard gets a test that asserts the `NigelError` **variant and code**, not just the
message, following `a_deactivated_rule_refuses_further_edits` in `src/cli/rules.rs`.

CI commands, from `.github/workflows/ci.yml`:

```
cargo fmt --check
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
```

---

## Documentation to update

- **CLAUDE.md** — Commands block (five new lines); the Invoicing architecture bullet
  (edit/void, `voided_at`); the Migrations bullet (v5); Project Structure comments for
  `client.rs` and `invoice.rs`; Key Design Constraints (the edit/void guard rules, the
  "published pages are not retro-updated" note); and the guardrail reason-code list (`void`,
  `not_draft`, `has_payments`, `already_void`).
- **docs/invoicing.md** — new sections for `client show`/`client edit`, `invoice edit`,
  `invoice void`, and the `--notes`/`--terms` flags on `new` and `edit`.
- **README.md** — check its command list and add the new commands if it enumerates them.

---

## Open questions for the orchestrator

1. **`voided_at` / migration v5.** The task brief said v5 was needed for notes/terms; it is
   not, those columns exist. I have proposed v5 for `voided_at` instead, on the argument that
   it makes `void` derived like every other status rather than the one hand-set exception.
   The alternative is **no migration at all** and a `void_invoice` that writes
   `status = 'void'` directly. Confirm which you want before Task 1 is implemented — everything
   downstream keys off it.
2. **Clear flags.** `--clear-due`, `--clear-notes`, `--clear-terms`, `--clear-email`,
   `--clear-address` are omitted, following the `rules update` precedent of set-only flags.
   The data-layer structs support clearing, so adding them later is one clap field and one
   `map(Some)` change each. `--clear-due` is the one with real semantics behind it — an invoice
   with no due date never goes overdue — so it is the most defensible to add now if you want
   exactly one.
3. **Void of a partially paid invoice** is refused. If the business wants "void anyway, the
   payments were entered in error", the alternative is to allow it and require the payments to
   be deleted first — which needs an `invoice payment delete` command that does not exist and
   is not in this task's scope.
4. **Stripe link deactivation on void** is out of scope; v1 prints a warning instead. Doing it
   properly needs a `deactivate_payment_link` method on the `PaymentGateway` trait plus fakes,
   and makes `invoice void` a network operation that can fail — which would be a different
   command shape. Worth its own subtask under TASK-68 if you want it.
5. **Re-publishing on client edit.** AC #1 says changes "take effect on the next send", which
   this design satisfies. If you also want already-published pages refreshed, that is a
   `nigel invoice republish` command, not part of `client edit`.
6. **Retyping `ensure_not_void` and `find_invoice`** from `NigelError::Other` to `Conflict`
   and `NotFound` changes their HTTP status from 500 to 409/404. CLI output is byte-identical
   and no current test breaks, but it is a behavior change to an existing shipped surface.
   Confirm you want it folded in here rather than filed separately.
7. **Date and currency validation on `create_invoice`.** Adding `validate_date` /
   `validate_currency` to `update_invoice` is required; applying the same helpers to
   `create_invoice` fixes a pre-existing hole (a malformed `--issue` is accepted at creation
   and blows up later in `ar_aging`) for about six lines. Included in the plan; say so if you
   would rather keep this task's diff strictly additive.
