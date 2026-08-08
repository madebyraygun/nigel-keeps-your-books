# Task 68.6 — Web UI: invoicing endpoints and screens at full parity

Parent: TASK-68 (Epic: Invoicing management surface). Supersedes TASK-62.
Depends on TASK-68.1 (edit/void data layer) and TASK-68.2 (preview render seam).

**Objective:** the browser reaches the CLI's invoicing surface — clients,
invoices, payments, preview, send, sync, A/R aging — behind the same locked and
session guards as every other route, with SPA screens that print the same
figures `nigel invoice list/show/aging` prints.

---

## 1. Where the codebase actually is

Surveyed before writing this; these are the load-bearing facts.

**Nothing invoicing is serializable.** `models::Client`, `Invoice`,
`InvoiceLineItem`, `InvoicePayment` all carry `#[allow(dead_code)]` +
`#[derive(Debug, Clone)]` and nothing else. Every non-invoicing model in the
same file has `#[derive(Debug, Clone, Serialize)]` +
`#[serde(rename_all = "camelCase")]` from task-31.2. `NewLineItem`,
`AgingBucket`, `InvoiceStatus`, `ImportSummary`, `PaymentLink`, `PaidSession`
have no derives at all.

**There are no invoicing routes.** `src/server/routes/` has eleven modules and
none of them mention invoicing.

**The data layer is missing the read functions an API needs.**
`cli/invoice.rs::list()` inlines its own `SELECT … INNER JOIN clients` — there
is no `list_invoices`. `InvoicePayment` is a dead struct: nothing constructs it,
there is no `list_payments`, and `paid_amount` returns a bare `f64` SUM.
`ar_aging` returns `Vec<AgingBucket>` with `label: &'static str` and no total,
no per-invoice detail, and an N+1 `paid_amount` call per open invoice.

**The guards live in the wrong layer and use the wrong error variant.**
`ensure_not_void` is a private function in `cli/invoice.rs`, so a handler
calling `invoicing::send::send_invoice` directly would send a void invoice.
It raises `NigelError::Other`, which `From<NigelError> for ApiError` maps to
**500**. So does the unknown-invoice error, whose message is
``No invoice #1248. Run `nigel invoice list` to see invoice numbers.`` —
a 500 whose text tells a browser user to open a terminal.

**68.1 fixes both**, and its spec (`2026-08-08-task-68-1-invoicing-edit-void-design.md`)
is the authority on how: `ensure_not_void` is retyped to
`Conflict { code: "void" }`, `find_invoice` to `NotFound`, with CLI output
byte-identical. This task consumes that; it does not redo it.

**`send_invoice` has no per-step error discrimination.** Every step returns the
same flat `Result<T, NigelError>`; a caller cannot tell "Stripe was down" from
"R2 rejected the upload" from "no `pdf` feature" without substring-matching.
The only structural signal is a side effect: `mark_published` is last, so any
earlier failure leaves the invoice a draft.

**`sync_all` writes partial failures to stderr.** Per-invoice failures are
`eprintln!`ed as `notice: invoice sync failed for #{number}: {e}` and the
function returns `Ok(total)`. Over HTTP those failures are simply lost.

**The three external clients have no timeouts.** `stripe.rs`, `r2.rs` and
`mailgun.rs` all call `reqwest::blocking::Client::new()`. In a CLI an
unreachable host is a hung terminal the user can Ctrl-C. In `nigel serve` it is
a blocking-pool thread that never returns.

---

## 2. Design decisions

### 2.1 Two route modules, not one

`routes/clients.rs` and `routes/invoices.rs`. The CLI has two command groups
(`nigel client`, `nigel invoice`) and `routes/` is one module per domain.
`invoices.rs` will be the largest route module after `imports.rs`; folding
clients into it would make it the largest by half again.

### 2.2 The invoice token never reaches the browser

`Invoice.token` gets `#[serde(skip_serializing)]`. It is the only access
control on a published invoice (CLAUDE.md: "the 16-character random token is
the only access control"), and a list endpoint that carries one token per row
puts every invoice's access control into devtools history, the Vite dev
server's network tab, and any future response cache.

What the SPA actually wants is the *address*, so the detail response carries a
computed `publicUrl: string | null` — `public_base_url` + `/` + token + `/`,
`null` when the invoice is unpublished or `public_base_url` is unset. One field,
one purpose, and a screen cannot accidentally build a link from a raw secret.

### 2.3 Send is a blocking request, not a job

`POST /api/invoices/{number}/send` runs the whole orchestration inside the
request and answers when it is done.

Reasons:

- `with_conn` is already `spawn_blocking`, and the three gateway traits are
  synchronous `reqwest::blocking`. The work has to run on that pool either way.
- **The invoice row is already the job record.** `published_at` and
  `stripe_payment_link_url` are the durable state a job store would duplicate.
  Two sources of truth for "did this invoice go out" is how they drift.
- A job registry costs a poll endpoint, an id vocabulary, an expiry policy and
  a reconnect story — for one button in a single-user localhost app.

The costs are real and are paid explicitly:

1. **Stage 4 adds timeouts to all three clients** (connect 10s, total 30s), so
   the worst case is bounded at roughly 90s plus render rather than unbounded.
   This is a bug fix that the CLI benefits from too.
2. The response carries a **step trace on success as well as failure**, so the
   screen can say what happened rather than just "done".
3. **The SPA never auto-retries a send.** A failure renders the failed step and
   requires a fresh confirmation — because step `email` succeeding and step
   `record` failing means the client already has the invoice.
4. Forward compatible: if this ever needs to be a job, a `202` carrying
   `{ "jobId": … }` is additive. The SPA's send handler branches on the status
   from day one so that change touches one function.

### 2.4 Confirmation is part of the contract

`POST …/send` requires `{"confirm": true}` in the body. Without it: `400`,
`details.reason = "confirmation_required"`.

AC #3 says send requires explicit confirmation in the UI. A UI-only dialog is a
convention the next screen can forget. The flag makes the dialog the only way
to reach the endpoint, and makes an accidental `curl` a no-op.

`sync` and `pay` do **not** require it. `sync` is idempotent by checkout
session id and already runs at CLI launch; `pay` writes one row a person typed.

### 2.5 Step-tagged send failures need a 502

The error-by-cause AC cannot be met with the current code table: an R2 outage
and a rusqlite panic both land on `internal`/500.

Add `ApiErrorCode::UpstreamFailed` → **502**, wire form `upstream_failed`, for
a failure inside Stripe, R2 or Mailgun. Everything else in the send path keeps
its natural code (404 for a missing invoice, 409 for a void one, 501 for a
build with no `pdf` feature, 500 for a database failure).

`details` carries the step regardless of code:

```json
{ "error": {
  "code": "upstream_failed",
  "message": "r2 403: SignatureDoesNotMatch",
  "details": {
    "reason": "send_failed",
    "step": "publish",
    "service": "r2",
    "completed": ["precheck", "payment_link", "render"],
    "emailSent": false,
    "invoiceStatus": "draft"
  } } }
```

`emailSent` is the field the screen's wording turns on. Every other failure is
safe to retry; a failure at `record` after `email` succeeded is not, and the
screen must say so rather than offering a Retry button.

### 2.6 Step vocabulary

`SendStep`, snake_case on the wire, in execution order:

| Step | What it does | Failure → |
|---|---|---|
| `config` | resolve `InvoicingConfig`, build the three clients | `409 send_not_configured`, `details.missing: []` |
| `load` | `get_invoice_by_number`, `get_client`, `line_items` | `404 invoice_not_found` / `client_not_found` |
| `precheck` | `ensure_not_void`; client has an email; total > 0 | `409 void` / `client_missing_email` / `invoice_not_payable` |
| `payment_link` | `gateway.create_payment_link` (skipped when one exists) | `502 upstream_failed`, `service: "stripe"` |
| `render` | HTML + PDF | `501 feature_disabled` (no `pdf`) else `500` |
| `publish` | `publisher.publish` to R2 | `502`, `service: "r2"` |
| `email` | `mailer.send_invoice` | `502`, `service: "mailgun"` |
| `record` | `mark_published` → `refresh_status` | `500`, `emailSent: true` |

Implementation keeps the blast radius at zero: `send.rs` gains
`send_invoice_traced(...) -> Result<SendOutcome, SendFailure>`, and the existing
`send_invoice` becomes a two-line wrapper
(`.map(|o| o.public_url).map_err(|f| f.source)`), so `cli/invoice.rs` and all
four existing send tests are untouched.

### 2.7 Invoice-state 409s follow the `details.reason` vocabulary

**Four reason codes come from 68.1 and are used as-is** — its spec publishes
them and this task must not rename them:

| Reason | Raised by |
|---|---|
| `void` | edit, send or pay of a void invoice |
| `not_draft` | editing a published invoice |
| `has_payments` | editing or voiding an invoice with recorded payments |
| `already_void` | voiding a void invoice |

68.1 raises them as `NigelError::Conflict { code, message }`, which the existing
`From<NigelError> for ApiError` already turns into a 409 with
`details.reason = code`. **No server-side work is needed to publish them** —
that is the whole point of the `Conflict` variant. This task adds tests that
assert each one arrives with the right code, and the SPA's rendering of them.

**Five reason codes are new here**, because their conditions only exist over
HTTP or only in the send path:

| Reason | Status | Also carries | Raised by |
|---|---|---|---|
| `client_missing_email` | 409 | `clientId`, `clientName` | send |
| `has_invoices` | 409 | `count` | `DELETE /api/clients/{id}` |
| `send_not_configured` | 409 | `missing` (array of key names) | send or sync with config gaps |
| `send_failed` | 502/501/500 | `step`, `service`, `completed`, `emailSent`, `invoiceStatus` | any send step failure |
| `confirmation_required` | 400 | — | send without `confirm: true` |

`no_balance` is 68.1-adjacent but not in its table: `payment_amount`'s
"no outstanding balance" refusal is currently `NigelError::Other`. Stage 3
retypes it to `Conflict { code: "no_balance" }` carrying `total` and `paid`,
since a 500 for "this invoice is already paid" is the same bug 68.1 is fixing
everywhere else.

**404s carry their reason from the route, not the error.** 68.1 answers an
unknown invoice or client with plain `NigelError::NotFound`, which becomes a
404 with no `details.reason`. That is right for the CLI and not enough for a
route that looks up two things — the exact case `ApiError::not_found_because`
exists for, and the exact bug it was added to fix on the review screen. So the
route layer wraps: a lookup that fails inside an invoice handler answers
`invoice_not_found`, one inside a client lookup answers `client_not_found`.
This touches no 68.1 file and adds no error variant.

`send_not_configured.missing` carries **key names only**, never values —
`["r2_bucket", "public_base_url"]`. The names are already public in
`docs/invoicing.md`.

### 2.8 Line items are replaced wholesale, never patched per row

`PATCH /api/invoices/{number}` takes an optional `items` array. Present means
"these are the line items now": the old rows are deleted, the new ones inserted
with derived `position`, and `subtotal`/`total` recomputed.

A per-row line-item API means a client reconciling positions across two
requests and a server holding a half-edited invoice between them. The CLI's
`--item` is already whole-list semantics. The SPA's repeatable-row editor sends
the array it is showing, which is the only thing it can honestly claim to know.

The API does **not** inherit the CLI's "descriptions cannot contain a colon"
rule — that is an artifact of parsing `desc:qty:unit` out of one argv string,
and JSON has no such ambiguity.

### 2.9 Aging takes an `asOf` parameter

`ar_aging` already takes `today: &str`. The route exposes it as an optional
`asOf=YYYY-MM-DD` (default: the server's local today, matching the CLI).

This is not a feature for its own sake: the parity fixtures cannot be committed
without it. A bucket boundary crossed overnight would fail the parity test
every morning, exactly as `fixture_capture.rs`'s fixed `YEAR = "2025"` exists
to prevent.

### 2.10 CLI text formatters are extracted so parity has an anchor

`reports-parity.test.ts` compares browser figures against
`/api/exports/{slug}?format=text`. There is no `nigel invoice … --mode export`,
so there is no such route to compare against.

Instead, Stage 1 extracts the printing in `cli/invoice.rs` and `cli/client.rs`
into pure functions — `format_invoice_list`, `format_invoice_show`,
`format_aging`, `format_client_list`, each `-> String` — mirroring
`cli/report/text.rs`. The CLI prints them; `fixture_capture.rs` calls them
directly to write the `.txt` side of each fixture pair. The TUI screens in
68.4/68.5 get the same functions for free.

### 2.11 The render seam comes from 68.2, unchanged

68.2's spec landed while this one was being written and already provides
exactly what a web preview needs — bytes, not files:

```rust
// src/invoicing/render.rs  (TASK-68.2)
pub struct RenderedInvoice { pub html: String, pub pdf: Option<Vec<u8>> }
pub enum PayButton<'a> { /* live link | placeholder */ }

pub fn render_invoice(conn: &Connection, invoice: &Invoice, client: &Client,
                      pay: PayButton<'_>, contact_email: &str) -> Result<RenderedInvoice>;
```

`pdf` is `None` on a build without the `pdf` feature, and 68.2 makes that the
non-fatal case each caller decides about. So the web preview routes are a thin
wrapper: `render_invoice` with `PayButton::placeholder` for a draft, and the
`preview.pdf` route turns `pdf: None` into the 501.

**68.6 adds no render code.** If Stage 2 finds itself writing a renderer, 68.2
has not landed and the correct move is to wait rather than to fork one.

### 2.12 68.1's signatures, as landed

68.1's spec also landed and is the authority. The relevant surface:

```rust
// clients.rs
pub fn ensure_client_exists(conn: &Connection, id: i64) -> Result<()>;
pub struct ClientUpdate { name, email, billing_address, notes }  // double-option nullables
impl ClientUpdate { pub fn is_empty(&self) -> bool; }
pub fn update_client(conn: &Connection, id: i64, update: &ClientUpdate) -> Result<()>;
pub struct ClientSummary { client: Client, invoices: Vec<ClientInvoiceRow>, outstanding: f64 }
pub fn client_summary(conn: &Connection, id: i64) -> Result<ClientSummary>;

// invoices.rs
pub struct InvoiceUpdate { issue_date, due_date, currency, notes, terms, items }
impl InvoiceUpdate { pub fn is_empty(&self) -> bool; }
pub fn update_invoice(conn: &Connection, invoice_id: i64, update: &InvoiceUpdate) -> Result<()>;
pub fn void_invoice(conn: &Connection, invoice_id: i64, voided_on: &str) -> Result<()>;
pub fn ensure_editable(conn: &Connection, invoice: &Invoice) -> Result<()>;
pub fn ensure_voidable(conn: &Connection, invoice: &Invoice) -> Result<()>;
pub fn validate_date(value: &str, what: &str) -> Result<()>;
pub fn validate_currency(code: &str) -> Result<String>;
```

Consequences this spec adopts rather than re-deriving:

- **`ClientDetail` is `ClientSummary` plus serde**, not a new query.
  `outstanding` and the invoice history already exist; the route adds the
  camelCase wrapper and nothing else.
- **`InvoiceUpdate.items = Some(v)` already replaces the whole line-item set**
  and recomputes totals inside one transaction. §2.8's whole-list semantics is
  68.1's semantics, not a web invention.
- **`ClientUpdate`/`InvoiceUpdate` already use `Option<Option<_>>`**, which is
  precisely what `routes::double_option` deserializes into. The PATCH bodies
  map field-for-field with no translation layer.
- **`canEdit` is `ensure_editable`, not a status check.** 68.1 blocks an edit
  on `void`, `not_draft` *and* `has_payments` — a draft that somehow carries a
  payment is not editable. A TypeScript re-derivation from `status` alone would
  get that wrong, which is why the flags are computed server-side (§4).
- **`void_invoice` takes the date it was voided on** and 68.1 adds a `voided_at`
  column in migration v5. The route passes the server's local today, the same
  value `send` and `pay` pass.
- **68.1 clears a stale Stripe payment link when an edit changes the total.**
  The invoice detail response must therefore be refetched after every edit, and
  the SPA must not cache a payment link across one (§5.6's refetch rule).
- `validate_date` and `validate_currency` are the validators the create and
  edit routes call. The HTTP layer adds no second date parser beyond the
  existing `routes::reports` one it uses for query parameters.

**One gap.** 68.1 has no `delete_client` — the CLI never grew a
`nigel client delete`. The web manager screen needs one (every other manager
has Delete, and a client added by mistake is otherwise permanent), so Stage 3
adds `clients::delete_client` and `clients::delete_blocker` in the data layer,
following `accounts::delete_blocker` exactly, raising
`NigelError::Blocked(DeleteBlock)` with a new `BlockReason::HasInvoices`. That
puts the reason code `has_invoices` in `DeleteBlock::reason_code()` where the
other two live, rather than inventing a parallel mechanism, and gives 68.4's
TUI client manager the same guard for free.

---

## 3. Endpoints

All under `/api`, all behind the session cookie and the locked guard. camelCase
in both directions. Errors in the standard envelope.

### Clients

| Method | Path | Body | Response | Errors |
|---|---|---|---|---|
| `GET` | `/api/clients` | — | `Client[]` (by name) | — |
| `GET` | `/api/clients/{id}` | — | `ClientDetail` | 404 `client_not_found` |
| `POST` | `/api/clients` | `NewClientRequest` | `201 Client` | 400 empty name; 409 `duplicate_name` |
| `PATCH` | `/api/clients/{id}` | `ClientPatch` | `Client` | 400 all-absent; 404; 409 `duplicate_name` |
| `DELETE` | `/api/clients/{id}` | — | `Deleted` | 404; 409 `has_invoices` + `count` |

`ClientDetail` is 68.1's `ClientSummary` on the wire — `client` fields flattened
plus `invoices: ClientInvoiceRow[]` and `outstanding` — the same data
`nigel client show` prints, and what makes a blocked delete explicable without
a second request.

`NewClientRequest`: `{ name, email?, billingAddress?, notes? }`.
`ClientPatch`: same fields, all optional, `email`/`billingAddress`/`notes` are
`double_option` (absent leaves, `null` clears). An all-absent patch is a 400,
matching categories.

### Invoices — read

| Method | Path | Query | Response | Errors |
|---|---|---|---|---|
| `GET` | `/api/invoices` | `status`, `clientId` | `InvoiceListRow[]` (number DESC) | 400 unknown `status`; 404 `client_not_found` |
| `GET` | `/api/invoices/{number}` | — | `InvoiceDetail` | 404 `invoice_not_found` |
| `GET` | `/api/invoices/aging` | `asOf` | `AgingReport` | 400 malformed `asOf` |
| `GET` | `/api/invoices/next-number` | — | `{ "number": 1249 }` | — |

`status` accepts the six status words plus `open` (`sent,partial,overdue`) —
the set `sync` and `ar_aging` already use. Anything else is a 400 naming the
legal set, the way `validate_match_type` does.

`{number}` is the **invoice number**, not the row id — it is what the CLI takes,
what the user reads off the invoice, and what `#/invoices?number=1248` carries.
Row ids stay internal. `ApiPath<i64>` already answers a non-numeric segment in
the envelope.

`next-number` exists so the new-invoice form can show the number it will get.
It reads `next_invoice_number` and reserves nothing.

### Invoices — write

| Method | Path | Body | Response | Errors |
|---|---|---|---|---|
| `POST` | `/api/invoices` | `NewInvoiceRequest` | `201 InvoiceDetail` | 400 no items / bad dates / total ≤ 0; 404 `client_not_found` |
| `PATCH` | `/api/invoices/{number}` | `InvoicePatch` | `InvoiceDetail` | 400 all-absent; 404; 409 `not_draft` / `void` / `has_payments` |
| `POST` | `/api/invoices/{number}/void` | `{}` | `InvoiceDetail` | 404; 409 `already_void` / `has_payments` |
| `POST` | `/api/invoices/{number}/pay` | `PayRequest` | `InvoiceDetail` | 400 bad amount/date/method; 404; 409 `void` / `no_balance` |
| `POST` | `/api/invoices/{number}/send` | `{ "confirm": true }` | `SendResult` | 400 `confirmation_required`; 404; 409; 501; 502 |
| `POST` | `/api/invoices/sync` | `{}` | `SyncResult` | 409 `send_not_configured`; 502 |

`NewInvoiceRequest`:
`{ clientId, issueDate, dueDate?, currency?="USD", items: [{description, quantity, unitAmount}], notes?, terms? }`.

`InvoicePatch`: `issueDate?`, `dueDate?` (double option), `currency?`,
`notes?` (double option), `terms?` (double option), `items?`.

`PayRequest`: `{ amount?, date, method?="direct_deposit" }`. `method` is
validated against the `invoice_payments` CHECK set
(`stripe`, `ach`, `direct_deposit`, `other`) **before** the insert, so an
unknown method is a 400 naming the set rather than a rusqlite constraint
violation surfacing as a 500. `amount` omitted means the whole outstanding
balance, exactly as `--amount` does, and inherits `payment_amount`'s NaN and
≤ 0 rejection.

Every write answers with the **whole `InvoiceDetail`**, not a patch echo. The
status is derived by `refresh_status` on almost every one of these, so a client
that patched a due date and got back only the due date would be showing a stale
status.

### Preview

| Method | Path | Response | Errors |
|---|---|---|---|
| `GET` | `/api/invoices/{number}/preview` | `text/html; charset=utf-8` | 404 |
| `GET` | `/api/invoices/{number}/preview.pdf` | `application/pdf` | 404; 501 `feature_disabled` |

The two non-JSON success responses in this task, and the same exception
`/api/exports/*` already carries — errors stay in the envelope.

The HTML response also sends `Content-Security-Policy: sandbox` and
`X-Content-Type-Options: nosniff`. The SPA renders it in
`<iframe sandbox>` (no `allow-same-origin`), so the CSP header is belt and
braces for a document served same-origin from a route the browser can also open
in a tab.

Both are addresses, so `ApiClient` owns them:
`invoicePreviewUrl(number, 'html' | 'pdf'): string`. The guard test that fails
the build on a quoted `/api/` literal outside `src/api` covers them.

Preview makes no network calls and works with no invoicing config set (68.2
AC #2). When `from_email` is unset the rendered contact line uses a visible
placeholder and the detail screen shows a `wc-notice-bar` listing the missing
keys from `/api/status`.

### Status

`GET /api/status` gains one object:

```json
"invoicing": {
  "sendConfigured": false,
  "syncConfigured": true,
  "missing": ["r2_bucket", "public_base_url"]
}
```

`missing` is the subset of send's nine required keys that are unset, in
`docs/invoicing.md`'s order. `syncConfigured` is `stripe_secret_key` alone.
Key names only — the values never leave the process. This is what greys out the
Send button and what the empty state on the invoices screen points at.

### No webhook endpoint

Stated here because it is a decision, not an omission. Stripe reconciliation
stays pull-based (CLAUDE.md: "Invoice payments are keyed by Stripe checkout
session ID, so `invoice sync` is idempotent; Stripe reconciliation is
pull-based (no webhook endpoint)"). `nigel serve` binds 127.0.0.1 and rejects
any request whose `Host` is not localhost — Stripe could not reach a webhook
route if one existed, and adding one would mean the first inbound-reachable
surface in the product.

---

## 4. Types

### Rust — serde derives (task-31.2 pattern)

`#[derive(Debug, Clone, Serialize)]` + `#[serde(rename_all = "camelCase")]` on:

- `models.rs`: `Client`, `Invoice`, `InvoiceLineItem`, `InvoicePayment`,
  `InvoiceStatus` (`#[serde(rename_all = "lowercase")]` — its wire form is the
  same six words `as_str()` returns).
- `invoicing/invoices.rs`: `NewLineItem` (+`Deserialize` — it is a request
  input), `AgingBucket`.
- `invoicing/import_invoiceshelf.rs`: `ImportSummary` (cheap, and 68.x may
  expose it).

Not derived: `PaymentLink`, `PaidSession` (internal gateway values),
`StripeClient` / `R2Publisher` / `MailgunClient` (they hold plaintext secrets
and must never be serializable), `InvoicingConfig` (same).

`Invoice.token` gets `#[serde(skip_serializing)]` (§2.2).

### Rust — new response structs

```rust
// invoicing/invoices.rs — one row of the list, joined and pre-totalled
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct InvoiceListRow {
    pub id: i64, pub number: i64, pub status: String,
    pub client_id: i64, pub client_name: Option<String>,
    pub issue_date: String, pub due_date: Option<String>,
    pub currency: String, pub total: f64, pub paid: f64, pub balance: f64,
}

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct AgingReport {
    pub as_of: String,
    pub buckets: Vec<AgingBucket>,   // always five, fixed order
    pub total: f64,
    pub invoices: Vec<AgingInvoice>, // number, clientName, dueDate, daysPastDue, balance, bucket
}

// server/routes/invoices.rs
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
struct InvoiceDetail {
    #[serde(flatten)] invoice: Invoice,   // token skipped
    client: Client,
    items: Vec<InvoiceLineItem>,
    payments: Vec<InvoicePayment>,
    paid: f64, balance: f64,
    public_url: Option<String>,
    can_edit: bool, can_send: bool, can_void: bool, can_pay: bool,
}
```

`canEdit` is `ensure_editable(conn, &invoice).is_ok()`, `canVoid` is
`ensure_voidable(...)`, and `canSend`/`canPay` are `ensure_not_void` plus the
balance and email checks — all 68.1's functions, called rather than
reimplemented. The SPA must not re-derive them from `status`: 68.1 blocks an
edit on `has_payments` as well as on status, and a second copy of that rule in
TypeScript is a second copy of the guardrails. The flags disable a control; the
409 is what enforces it.

`ClientDetail` is 68.1's `ClientSummary` with serde on it —
`client`, `invoices`, `outstanding` — flattened into the shape §3 describes.
No new query.

`client_name` on the list row is `Option` because the join becomes a **LEFT
JOIN**. The CLI's inner join silently drops an invoice whose client row is
missing; a list that hides invoices is worse than one that shows a dash.

`list_invoices` also fixes the aging N+1: paid amounts come from one
`GROUP BY invoice_id` aggregate, not one `SELECT SUM` per row.

### Rust — new error surface

```rust
// server/error.rs — the only new error variant this task adds
ApiErrorCode::UpstreamFailed,      // 502, "upstream_failed"

// invoicing/send.rs
pub enum SendStep { Config, Load, Precheck, PaymentLink, Render, Publish, Email, Record }
pub enum StepOutcome { Ok, Reused, Skipped }
pub struct SendOutcome { pub public_url: String, pub payment_link_url: Option<String>,
                         pub status: String, pub steps: Vec<(SendStep, StepOutcome)> }
pub struct SendFailure { pub step: SendStep, pub completed: Vec<SendStep>,
                         pub email_sent: bool, pub source: NigelError }

// invoicing/sync.rs
pub struct SyncReport { pub recorded: u32, pub invoices_checked: u32,
                        pub failures: Vec<(i64, String)> }   // (number, message)
```

### TypeScript — `web/apps/app/src/api/types.ts`

Hand-mirrored, camelCase, one interface per response struct, in the same commit
as the Rust side (the convention `docs/api.md` already states):

`Client`, `ClientDetail`, `NewClientRequest`, `ClientPatch`,
`InvoiceListRow`, `InvoiceLineItem`, `InvoicePayment`, `InvoiceDetail`,
`NewInvoiceRequest`, `InvoicePatch`, `PayRequest`,
`AgingBucket`, `AgingInvoice`, `AgingReport`,
`SendResult`, `SendStepResult`, `SyncResult`, `InvoicingStatus`,
plus `InvoiceStatus = 'draft' | 'sent' | 'partial' | 'paid' | 'overdue' | 'void'`
and `PaymentMethod = 'stripe' | 'ach' | 'direct_deposit' | 'other'`.

`ConflictDetails` gains the optional fields the new reasons carry: `step`,
`service`, `completed`, `emailSent`, `invoiceStatus`, `missing`, `status`,
`clientId`, `clientName`, `paid`, `total`, `number`.

The closed vocabularies follow the file's existing `as const` array + derived
union + type-guard shape: `CONFLICT_REASONS` gains the eleven reasons from
§2.7, `NOT_FOUND_REASONS` gains `invoice_not_found` and `client_not_found`, and
`API_ERROR_CODES` gains `upstream_failed`. Adding the code to that array is
what stops `errorFrom` normalizing a 502 to `'unknown'`.

`InvoiceListRow.status` is typed `string`, not `InvoiceStatus` — the same
deliberate widening `RuleRow.matchType` documents, because the `status` column
has no CHECK constraint and a row written by the InvoiceShelf importer or by
hand cannot be assumed to be one of the six.

### TypeScript — `ApiClient` additions

One typed method per endpoint, no generic `request()`:

```ts
getClients(): Promise<Client[]>;
getClient(id: number): Promise<ClientDetail>;
createClient(input: NewClientRequest): Promise<Client>;
updateClient(id: number, input: ClientPatch): Promise<Client>;
deleteClient(id: number): Promise<Deleted>;

getInvoices(params?: InvoiceListParams): Promise<InvoiceListRow[]>;
getInvoice(number: number): Promise<InvoiceDetail>;
getAging(params?: { asOf?: string }): Promise<AgingReport>;
getNextInvoiceNumber(): Promise<{ number: number }>;

createInvoice(input: NewInvoiceRequest): Promise<InvoiceDetail>;
updateInvoice(number: number, input: InvoicePatch): Promise<InvoiceDetail>;
voidInvoice(number: number): Promise<InvoiceDetail>;
payInvoice(number: number, input: PayRequest): Promise<InvoiceDetail>;
sendInvoice(number: number): Promise<SendResult>;   // sends { confirm: true }
syncInvoices(): Promise<SyncResult>;

invoicePreviewUrl(number: number, format: 'html' | 'pdf'): string;
```

`sendInvoice` takes no `confirm` argument — the client always sends `true`,
because reaching this method already means the dialog resolved. The flag is the
wire contract, not a parameter the caller gets to vary.

---

## 5. Screens

Two new `ScreenId`s: `'clients'` and `'invoices'`, both `inNav: true`, icons
`wc-icon-clients` and `wc-icon-invoice`.

Routing, following the "no path segments" rule:

| Route | View |
|---|---|
| `#/invoices` | list, all statuses |
| `#/invoices?status=open` | filtered list |
| `#/invoices?view=aging` | full A/R aging |
| `#/invoices?number=1248` | invoice detail |
| `#/invoices?number=1248&edit=1` | invoice editor (draft only) |
| `#/invoices?new=1` | new invoice |
| `#/clients` | client manager |
| `#/clients?id=3` | client detail drawer |

**The invoice editor is a full view, not a dialog** — a departure from the
manager pattern, and deliberate. `wc-manager-dialog` works for a rule form of
six fields; an invoice with eight line items inside `wa-dialog` is a scrolling
box inside a scrolling page. The *client* form stays a dialog, because it is
four fields and the manager pattern fits it exactly.

### 5.1 Invoices screen (`#/invoices`)

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Invoices                                              [ Sync now ] [+ New ] │
├────────────────────────────────────────────────────────────────────────────┤
│ ┌── A/R aging ─ as of 2026-08-07 ──────────────────── View aging report → ┐ │
│ │  Current      1-30        31-60       61-90       90+        Total      │ │
│ │  $4,200.00    $1,850.00   $0.00       $960.00     $0.00      $7,010.00  │ │
│ │  ████████     ███▌        ·           ██          ·                     │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                            │
│ Status:  (•) All  ( ) Draft  ( ) Open  ( ) Paid  ( ) Void   Client: [ all ▾]│
├──────┬──────────┬────────────────────┬────────────┬────────────┬───────────┤
│  #   │ Status   │ Client             │      Total │    Balance │ Due       │
├──────┼──────────┼────────────────────┼────────────┼────────────┼───────────┤
│ 1252 │ ◻ draft  │ Northwind Traders  │  $2,400.00 │  $2,400.00 │ —         │
│ 1251 │ ◆ sent   │ Acme Co            │  $1,850.00 │  $1,850.00 │ 2026-09-06│
│ 1250 │ ◑ partial│ Acme Co            │  $3,200.00 │  $1,200.00 │ 2026-08-20│
│ 1249 │ ▲ overdue│ Globex             │    $960.00 │    $960.00 │ 2026-06-30│
│ 1248 │ ● paid   │ Northwind Traders  │  $4,000.00 │      $0.00 │ 2026-07-01│
│ 1247 │ ⊘ void   │ Globex             │    $500.00 │          — │ —         │
└──────┴──────────┴────────────────────┴────────────┴────────────┴───────────┘
   Rows link to #/invoices?number=NNNN.  Empty: "No invoices yet — New invoice."
```

Notes: status is a glyph **and** a word (`wc-money`'s reasoning — a browser
cannot lean on colour alone, WCAG 1.4.1). A void invoice's balance is an em
dash, never `$0.00` — the `wc-import-history` null-balance precedent. The
aging strip is the parity surface for the TUI dashboard summary in 68.5.

`Sync now` is only enabled when `status.invoicing.syncConfigured`; disabled it
carries a tooltip naming `stripe_secret_key`.

### 5.2 Invoice detail (`#/invoices?number=1251`)

```
┌────────────────────────────────────────────────────────────────────────────┐
│ ← All invoices                                                             │
│                                                                            │
│ Invoice #1251   ◆ sent            Acme Co                                  │
│ $1,850.00 USD · $1,850.00 outstanding · issued 2026-08-07 · due 2026-09-06 │
│                                                                            │
│ [ Send… ]  [ Record payment… ]  [ Edit ]  [ Void… ]     Preview: HTML · PDF│
│            ^ enabled by canSend / canPay / canEdit / canVoid               │
├────────────────────────────────────────────────────────────────────────────┤
│ ┌── Line items ───────────────────────────────────────────────────────────┐│
│ │ Description                          Qty     Unit         Amount        ││
│ │ Consulting — August                 10.00  $150.00     $1,500.00        ││
│ │ Hosting                              1.00  $350.00       $350.00        ││
│ │                                             Subtotal    $1,500.00       ││
│ │                                             Total       $1,850.00       ││
│ └─────────────────────────────────────────────────────────────────────────┘│
│ ┌── Payments ────────────┐ ┌── Published ──────────────────────────────────┐│
│ │ (none yet)             │ │ https://billing.rygn.io/i/aBc123…/    [copy]  ││
│ │                        │ │ Pay link: https://buy.stripe.com/…    [copy]  ││
│ └────────────────────────┘ └───────────────────────────────────────────────┘│
│ ┌── Preview ──────────────────────────────────────────────────────────────┐ │
│ │ ⚠ from_email and r2_bucket are not set — Send is unavailable. Settings →│ │
│ │ ┌─────────────────────────────────────────────────────────────────────┐ │ │
│ │ │  <iframe sandbox src="/api/invoices/1251/preview">                  │ │ │
│ │ └─────────────────────────────────────────────────────────────────────┘ │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────┘
```

The send confirmation dialog names every consequence before it happens:

```
┌── Send invoice #1251? ────────────────────────────────────────┐
│ This will:                                                    │
│   • create a Stripe payment link for $1,850.00                │
│   • publish the invoice to billing.rygn.io                    │
│   • email it to ap@acme.test                                  │
│ Nothing is sent until you confirm. This cannot be undone.     │
│                                    [ Cancel ]  [ Send now ]   │
└───────────────────────────────────────────────────────────────┘

…in flight:                        …failed at publish:
┌──────────────────────────┐       ┌──────────────────────────────────────┐
│ ✓ Payment link (reused)  │       │ ✓ Payment link   ✓ Rendered          │
│ ✓ Rendered               │       │ ✗ Publishing to R2                   │
│ ⟳ Publishing to R2…      │       │   r2 403: SignatureDoesNotMatch       │
│ · Emailing               │       │ · Email not sent · still a draft      │
│ · Recording              │       │                    [ Close ] [ Retry ]│
└──────────────────────────┘       └──────────────────────────────────────┘
```

The `record`-step failure gets different wording and **no Retry button**:
"The invoice was emailed but Nigel could not record it. Run `nigel invoice show
1251` to check before sending again."

### 5.3 Client manager (`#/clients`)

Straight `wc-manager-layout` / `wc-manager-table` / `wc-manager-dialog`, the
fourth instance of the pattern.

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Clients                                                        [+ Add ]    │
├──────────────────────┬──────────────────────┬──────────┬─────────┬─────────┤
│ Name                 │ Email                │ Invoices │ Open    │         │
├──────────────────────┼──────────────────────┼──────────┼─────────┼─────────┤
│ Acme Co              │ ap@acme.test         │        7 │ $3,050  │ ✎  🗑    │
│ Globex               │ —                    │        3 │   $960  │ ✎  🗑    │
│ Northwind Traders    │ billing@nw.test      │        2 │     $0  │ ✎  🗑    │
└──────────────────────┴──────────────────────┴──────────┴─────────┴─────────┘
  Row → #/clients?id=3.  Empty: "No clients yet — Add one to start invoicing."

  ┌── Edit client ──────────────────────────┐   ┌── Delete Acme Co? ────────┐
  │ Name*        [ Acme Co               ]  │   │ 7 invoices bill this      │
  │ Email        [ ap@acme.test          ]  │   │ client. Void or delete    │
  │              Needed before an invoice   │   │ them first.               │
  │              can be sent.               │   │ [ Show those invoices ]   │
  │ Address      [ 1 Main St             ]  │   └───────────────────────────┘
  │ Notes        [                       ]  │
  │ ⚠ A client named "Acme Co" already exists│
  │                   [ Cancel ] [ Save ]   │
  └─────────────────────────────────────────┘
```

`Show those invoices` links to `#/invoices?clientId=3` — the `guardrailAction`
precedent, which today only exists for `has_active_rules` and is the one
guardrail that can point somewhere useful.

An email-less client shows an em dash plus inline helper text on the form,
because `client_missing_email` is the send failure most likely to be hit and
the cheapest to prevent.

### 5.4 Aging view (`#/invoices?view=aging`)

`wc-report-table` with the five buckets and a total row, then the per-invoice
breakdown (`AgingReport.invoices`) sorted by days past due descending. Composed
from existing components, not given one of its own — the `wc-k1-worksheet`
reasoning: a bespoke component would have to take `AgingReport` as a property
and drag API types into `@nigel/ui`.

### 5.5 New `@nigel/ui` components

Each with a co-located `.preview.ts` covering every visible state and a
`.test.ts` calling `describePreviewA11y` (mandatory workflow):

| Component | States the preview must cover |
|---|---|
| `wc-invoice-table` | list, empty, loading, each of the six statuses, void balance |
| `wc-invoice-status` | the six statuses (glyph + word, never colour alone) |
| `wc-line-items` | one row, many rows, empty, readonly, per-field errors, live subtotal |
| `wc-invoice-form` | new, editing, validation errors, saving |
| `wc-invoice-summary` | draft, sent, partial, overdue, paid, void, no due date |
| `wc-payment-list` | empty, one, many, stripe vs manual method |
| `wc-payment-form` | default (full balance), partial, each method, error |
| `wc-send-dialog` | confirm, in-flight, per-step failure, `record` failure, no-email refusal |
| `wc-aging-bars` | all zero, one bucket, all buckets, long labels |
| `wc-client-form` | add, edit, duplicate-name error, missing-email hint |
| `wc-invoice-preview` | loading, loaded, missing-config notice, pdf unavailable |

Two icons — `wc-icon-invoice` and `wc-icon-clients` — are added to
`web/packages/ui/src/icons/icons.ts` as `WcIconBase` subclasses alongside the
existing twenty-three, not as their own component files.

Every form component follows the established contract: fully controlled
(`value` in, `nc-<name>-form-change` out carrying the **whole** value), an
exported `EMPTY_*_FORM` constant, a pure `validate*Form(value) => *FormErrors`,
an `errors` property rendered as `<p class="error" role="alert">` beside the
field it belongs to, and a `disabled` property that disables every control.

`wc-line-items` uses up/down buttons for reorder, not drag and drop: a drag
handle has no keyboard equivalent that passes axe without building one anyway.

### 5.6 Error routing in the SPA

`screens/invoicing-errors.ts`, modelled exactly on `manager-errors.ts`: reason
codes are the contract, the count/name/step is formatted client-side, and the
server's sentence is shown only for a 400 or an unrecognized 409 reason. Two
extra rules for this domain:

- `send_failed` renders from `details.step` and `details.service`, with
  `details.message` (the upstream's own text — `r2 403: SignatureDoesNotMatch`)
  shown verbatim underneath. Our words for what failed, theirs for why.
- `send_not_configured` renders `details.missing` as key names with a link to
  `#/settings`, never as the server's sentence, which names settings.json and
  `NIGEL_` env vars — good CLI advice, useless beside a settings screen. This
  is the reconcile-screen 404 reasoning.

Where a failure lands follows the rule the managers already keep, extended for
one case this domain adds:

| Failure | Lands |
|---|---|
| Save (client or invoice form) | in the dialog / form, beside the field (`dialogError`) |
| Delete, void | the layout's alert region — `confirmDialog()` has already resolved |
| Send | **in the send dialog, which stays open** |
| Sync, row-level optimistic edits | a toast via `dispatchNcToast` |
| 423 / 401 | nowhere — the shell gates those before a screen exists |

Send is the departure. Every other confirm dialog resolves and removes itself
before the request is sent; the send dialog must survive its own request,
because it is the only place the step trace can be rendered and the only place
the result means anything. It resolves on Close, not on Send.

Every mutation ends with a refetch, never an optimistic splice: a send changes
a status, a payment changes a status and a balance, and a void changes what
`ar_aging` returns for every other row on the screen.

---

## 6. Figure parity

`web/apps/app/src/screens/invoicing-parity.test.ts`, modelled on
`reports-parity.test.ts`: every money figure the browser renders is compared,
per view, against every money figure in the CLI's own text, on absolute values.

The fixture pairs come from `fixture_capture.rs`, extended with:

- `seeded_invoicing_db()` in `testutil.rs` — three clients (one without an
  email), six invoices covering all six statuses with **literal dates**, and
  payments including one keyed by a Stripe session id.
- `AS_OF: &str = "2026-03-15"`, fixed for the same reason `YEAR = "2025"` is.
- Four pairs written into `web/apps/app/src/__fixtures__/invoicing/`:
  `invoices.json`/`.txt`, `invoice-1250.json`/`.txt`, `aging.json`/`.txt`,
  `clients.json`/`.txt`, plus a `manifest.json` in the reports manifest's shape.

The `.txt` side is produced by calling `cli::invoice::format_*` directly rather
than by fetching an export route, because there is no invoice export route
(§2.10). The capture test asserts the JSON side comes from the real router with
a real session, so the shape cannot drift from what a browser gets.

The test side reuses `reports-parity.test.ts` wholesale: `readFileSync` from
`../__fixtures__/invoicing`, `moneyTokens` (`/-?\$[\d,]+\.\d{2}/g`, sign
stripped, sorted) over a recursive shadow-DOM text walk, a `FakeApiClient`
primed only with the endpoints under test, `initializeAppStore` +
`store.refreshStatus()`, then mount → `updateComplete` → `setTimeout(0)` →
`updateComplete`. It keeps the "guards the guard" assertion too: the manifest's
entry count must equal the number of views under test, so a view added without
a fixture fails rather than silently passing on an empty set.

One thing the reports parity test does not have to handle and this one does:
the CLI prints money as `{:.2}` with **no `$` and no thousands separator**
(`cli/invoice.rs` does not use `src/fmt.rs`), while `wc-money` prints
`$1,850.00`. The extracted `format_*` functions therefore route through
`fmt.rs` so both sides speak one format — a small, deliberate change to CLI
output, and the only one in this task. It is called out here because it will
show up in a `nigel invoice list` diff and must not be mistaken for a
regression.

---

## 7. Stage decomposition

Five stages, each an independently-landable PR with its own green test suite.

### Stage 1 — Data layer: serde, typed errors, read functions, formatters

Rust only, no HTTP. Unblocks everything and is reviewable on its own.

Serialize derives (§4). `list_invoices`, `list_payments`, `AgingReport` (with
`ar_aging` re-expressed on top of it, so the CLI's five buckets cannot drift).
Payment-method validation, and `payment_amount`'s "no outstanding balance"
retyped from `Other` to `Conflict { code: "no_balance" }`.
`format_invoice_list` / `format_invoice_show` / `format_aging` /
`format_client_list` extracted.

Explicitly **not** here: the void guard, the typed not-found errors,
`update_client`, `update_invoice`, `void_invoice`, `ensure_editable`,
`ensure_voidable`, `client_summary`, `validate_date`, `validate_currency` — all
68.1's, and all consumed rather than rebuilt.

**Ships when:** `cargo test` and `cargo test --no-default-features` are green
and `nigel invoice list/show/aging` print the same rows in the same order as
before, with money now formatted by `fmt::money` (`$1,850.00`) instead of
`{:.2}` — the one intentional CLI output change in this task (§6), and the
formatting `wc-money` is already tested against.

### Stage 2 — Read API + status capability + parity fixtures

`routes/clients.rs` and `routes/invoices.rs` with the GET half. The two preview
routes over 68.2's `render_invoice`. `/api/status`'s `invoicing` object.
`DATA_ROUTES` extended. `seeded_invoicing_db` and the `fixture_capture.rs`
captures. `docs/api.md` grows an "Invoicing" reading section.

**Ships when:** the guard tests cover every new route, and the committed
fixtures regenerate byte-identically.

### Stage 3 — Write API: clients CRUD, invoice create/edit/void/pay

The write half, minus anything that touches the network. `WRITE_ROUTES`
extended. Every 409 reason in §2.7 that is not send-related gets a test that
asserts its `details`.

**Ships when:** a client and an invoice can be created, edited, voided and paid
over HTTP, and every guardrail answers with the right code and reason.

### Stage 4 — Send orchestration and sync

`SendStep` / `SendOutcome` / `SendFailure` and `send_invoice_traced`, with
`send_invoice` as a wrapper so the CLI is untouched. `SyncReport` and
`sync_all_report`. `ApiErrorCode::UpstreamFailed`. **Timeouts on all three
reqwest clients.** `POST …/send` and `POST /api/invoices/sync`.

**Ships when:** a fake gateway failing at each of the eight steps produces the
right status, code, `details.step` and `details.emailSent`, and the CLI's send
and sync output is unchanged.

### Stage 5 — SPA screens

`@nigel/ui` components with previews and axe tests; `api/types.ts` and
`api/client.ts` additions; `screens/clients.ts`, `screens/invoices.ts`,
`screens/invoice-data.ts`, `screens/invoicing-errors.ts`; two registry entries;
`invoicing-parity.test.ts`.

Large but coherent — task 31.16 shipped three manager screens as one. If a
reviewer wants it split, the seam is read-only screens plus components (5a) and
the write/send interactions (5b); nothing in 5a depends on 5b.

**Ships when:** the parity test passes, every preview state is axe-clean, and
`npm run build|test|lint|typecheck` are green.

---

## 8. Documentation

Not optional (CLAUDE.md's documentation policy):

- `docs/api.md` — invoicing reading section, writing section, the eleven new
  conflict reasons in the existing table, `upstream_failed` in the error-code
  table, the send step vocabulary, the `invoicing` object on `/api/status`.
- `docs/invoicing.md` — a "From the web UI" section, and the note that send
  over HTTP requires explicit confirmation.
- `CLAUDE.md` — Architecture entries for `routes/clients.rs`,
  `routes/invoices.rs` and the two SPA screens; Key Design Constraints entries
  for the blocking-send decision, the token exclusion, and the `asOf` fixture
  reasoning.
- `README.md` — the web UI feature list.

---

## Open questions for the orchestrator

1. **Is a blocking send acceptable?** §2.3 argues yes with bounded timeouts and
   no auto-retry. The counter-argument is that a 90-second `fetch` with no
   progress is a bad browser experience even if it is a correct one. If the
   answer is no, Stage 4 grows a job registry in `AppState` and a
   `GET /api/invoices/send-jobs/{id}` poll route, and the SPA branches on `202`.

2. **Nav shape.** This adds screens 13 and 14 to a flat sidebar. Group into
   sections ("Books" / "Billing"), or leave it flat? Grouping changes
   `wc-nav-sidebar` and `ScreenDef` (a `section` field) for every screen, not
   just the new two — visually contentious and worth deciding before Stage 5.

3. **Does aging deserve its own nav entry?** It is a report, and the reports
   screen already serves eight of them from a catalog. Putting A/R aging under
   `#/reports?report=aging` would reuse `wc-report-table`, `wc-export-links`
   and the export routes — but `/api/exports/aging` does not exist and
   `ReportKind` has no variant for it, so it would mean touching the report
   vocabulary that eight endpoints and two front ends share. Current plan keeps
   it on the invoices screen. Cheap to change now, expensive later.

4. **Invoice detail: full view or drawer?** §5.2 draws a full view at
   `#/invoices?number=1248`. A right-hand drawer over the list would keep the
   list visible while stepping through invoices, which is how a person actually
   chases receivables. The drawer costs a new `wc-*` primitive and a focus-trap
   decision; the full view costs a round trip back to the list.

5. **Should the preview iframe be on the detail screen by default?** It renders
   the real client-facing page, which is reassuring, but it is a second render
   per detail view and it makes the screen tall. Alternative: a "Preview" button
   that opens it. Drawn here as always-present-but-collapsible.

6. **Send subject line.** `send.rs` hardcodes `format!("Invoice #{} from
   Raygun", invoice.number)`. Every other product string is configurable and
   this one names a specific company in a tool being packaged for others. Out
   of scope for 68.6, but the send confirmation dialog will display the subject,
   which makes it visible in a way it has not been. Worth a follow-up task.

7. **Client delete: block or soft-delete — and does it belong here at all?**
   68.1 does not add a client delete; the CLI has never had one. This task adds
   `delete_client` blocking on `has_invoices`, matching accounts (categories
   soft-delete instead). Invoices are historical records that must not lose
   their client name, so blocking is the safer default — but it means a client
   can never be removed once billed. The larger question is whether inventing a
   destructive operation in the *web* task is the right place for it, or
   whether it should be pushed back into 68.1 so the CLI, the TUI (68.4) and
   the web get it together. Blocking on this one is cheap; it is one route and
   one dialog.

8. **Does `/api/status` reporting `invoicing.missing` while the database is
   locked bother anyone?** `/api/status` is ungated by design. The field names
   are already public in `docs/invoicing.md` and no values are exposed, but it
   does tell an unauthenticated-but-local caller which integrations this
   installation has configured. Alternative: move the object to a gated
   `GET /api/invoicing/config-status`, at the cost of a second request on the
   invoices screen.
