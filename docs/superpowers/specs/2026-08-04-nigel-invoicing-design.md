# Nigel Invoicing + Static Web Publishing — Design (v1)

Status: approved-pending-review
Date: 2026-08-04
Related backlog: task-1 (add-on), task-2 (clients), task-3 (PDF), task-4 (email), task-6 (Stripe)

## Goal

Give Nigel first-class invoicing so it can replace InvoiceShelf as our billing tool.
Nigel authors an invoice, renders a PDF and a static HTML page, publishes both to a
Cloudflare R2 bucket served under `billing.rygn.io/i/{token}/`, and emails the client via
Mailgun. Every published invoice offers two ways to pay: a Stripe Payment Link (card/ACH)
and direct-deposit instructions. Nigel stays the system of record and a **local, single-user,
offline-first Rust TUI/CLI app** — it reaches out over HTTPS (R2, Stripe, Mailgun) but nothing
ever reaches in. No server, no webhook, no inbound network surface.

## Non-goals (v1)

Explicitly deferred to keep v1 shippable. Each is a known fast-follow:

- **Double-entry journal layer** (task-1's "debit AR / credit revenue"). v1 tracks AR from
  open-invoice balances; the schema is shaped so `journal_entries` bolts on later without a
  painful migration.
- **Automatic bank→invoice matching.** Direct deposits are recorded with a manual mark-paid
  command in v1; auto-matching reuses the existing reconciler as a fast-follow.
- Recurring/scheduled invoices (task-8), duplicate-invoice (task-7), automated reminders
  (task-5), PDF template customization (part of task-3).

## Key conventions honored

- **Money is `REAL` (f64)** everywhere in Nigel (`transactions.amount`). Invoice tables follow
  suit. Integer cents appear only at the Stripe boundary (`unit_amount`), converted with an
  explicit round.
- **Migrations** are appended to the versioned `MIGRATIONS` array in `src/migrations.rs`
  (`execute_batch` per `up`). One new migration adds all invoicing tables.
- **Config/secrets** live in the settings file (JSON, restricted perms via
  `restrict_file_permissions`) with environment-variable overrides.
- New command groups follow the existing `src/cli/*_manager.rs` module pattern; PDF work
  extends `src/pdf.rs`.

## Architecture

```
   Nigel (local, Rust TUI/CLI)                      outbound HTTPS only
   ┌──────────────────────────────┐
   │ invoice/client commands      │
   │ pdf.rs   (PDF render)        │──► R2 bucket ──► billing.rygn.io/i/{token}/
   │ html.rs  (static page)       │        (Cloudflare routes /i/* → bucket)
   │ publish.rs (S3 upload)       │
   │ stripe.rs (Payment Link+sync)│──► Stripe API (create link; poll sessions)
   │ mailgun.rs (send)            │──► Mailgun API (email link + PDF)
   │ encrypted SQLite (sqlcipher) │
   └──────────────────────────────┘
```

Confirmation is **pull-based**: Nigel polls Stripe for paid sessions on demand and on launch.
Because there is no webhook, the R2 host is 100% static and the ops box is untouched.

## Data model

One migration adds four tables. Money columns are `REAL`; all amounts are in the invoice's
`currency`.

- **clients** — `id, name, email, billing_address, notes, created_at`. (task-2)
- **invoices** —
  `id, number INTEGER UNIQUE, client_id, issue_date, due_date, status TEXT,
   currency TEXT, subtotal REAL, tax REAL, total REAL, notes TEXT, terms TEXT,
   token TEXT UNIQUE, stripe_payment_link_id TEXT, stripe_payment_link_url TEXT,
   published_at TEXT, created_at TEXT`.
  - `number` follows the existing scheme; next value = `max(number)+1`, surfaced as a
    `next_invoice_number` setting (seeded to 1248 if no history is imported).
  - `token` = 16-char base62 from `rand` (already a dependency) — the unguessable R2 path
    segment. Published invoices are unlisted and not enumerable; there is no login.
- **invoice_line_items** — `id, invoice_id, description, quantity REAL, unit_amount REAL,
  line_total REAL, position INTEGER`.
- **invoice_payments** — `id, invoice_id, amount REAL, paid_date TEXT,
  method TEXT CHECK(method IN ('stripe','ach','direct_deposit','other')),
  stripe_checkout_session_id TEXT, recorded_at TEXT`.
  - `stripe_checkout_session_id` is the idempotency key for Stripe sync (a session is recorded
    at most once).

### Status is derived, not free-typed

`status` is computed from publish state and the payment sum, never hand-edited into an
inconsistent value:

- `draft` — created, not yet published/sent.
- `sent` — published to R2 + emailed; `paid_sum == 0`.
- `partial` — `0 < paid_sum < total`.
- `paid` — `paid_sum >= total`.
- `overdue` — `sent`/`partial`, `due_date` in the past, still owing.
- `void` — explicit manual state.

## Modules

Following `src/cli/*_manager.rs` conventions:

- **`invoice` / `client` command groups** — see Commands below.
- **`src/pdf.rs`** — gains an invoice renderer alongside the existing report renderers.
- **`src/html.rs`** (new) — a dependency-free `include_str!` HTML template rendered per invoice
  (line items, totals, a **Pay online** button → Payment Link URL, and a **direct-deposit
  instructions** block). No JS, no external assets.
- **`src/publish.rs`** (new) — uploads `{token}/index.html` + `{token}/invoice.pdf` to R2 over
  the S3 API. Adds one small signing crate, **`rusty-s3`**, which produces signed request
  data for the blocking `reqwest` already in deps (no async runtime pulled in).
- **`src/stripe.rs`** (new) — Stripe REST over blocking `reqwest`:
  - *Create link:* per invoice, create an inline `Price` (Payment Links require a Price, not
    inline `price_data`) then a persistent `payment_link` carrying `metadata.invoice_id`.
    Store `stripe_payment_link_id/url` on the invoice.
  - *Sync:* for each invoice with a link and still owing, `GET /v1/checkout/sessions?
    payment_link={id}&status=complete`; for any session with `payment_status=paid` not already
    in `invoice_payments`, insert a payment (`method='stripe'`, `stripe_checkout_session_id`,
    amount from `amount_total`/100.0) and recompute status. Idempotent by session id.
- **`src/mailgun.rs`** (new) — emails the invoice link + PDF from `billing@rygn.io` via the US
  Mailgun HTTP API (`api.mailgun.net`), mirroring the ops box's working Mailgun setup.
- **`src/importer` addition** — `--from-invoiceshelf <database.sqlite>` reads InvoiceShelf's
  SQLite (customers, invoices, invoice_items, payments), converts **integer cents → REAL
  dollars**, maps statuses, and creates the corresponding `clients / invoices /
  invoice_line_items / invoice_payments` rows, then sets `next_invoice_number = max+1`. One-time
  tool; exact InvoiceShelf column names are verified against the live schema during
  implementation.

## Commands

- `nigel client add|list|show` — manage clients (task-2).
- `nigel invoice new` — create a draft (client, line items, dates, terms).
- `nigel invoice list|show INV-####` — browse; `show` reflects derived status + paid amount.
- `nigel invoice send INV-####` — the money-out path: render PDF+HTML → create/reuse Stripe
  Payment Link → upload to R2 → Mailgun email → set `published_at`, status `sent`. Prints the
  public URL and local PDF path.
- `nigel invoice sync` — poll Stripe for paid sessions across owing invoices; record payments,
  flip status. Also runs on Nigel launch. Pull-based; nothing listens.
- `nigel invoice pay INV-#### --date <d> --method <ach|direct_deposit|other> [--amount <a>]` —
  manual mark-paid for direct deposits (defaults to full remaining balance).
- `nigel invoice aging` — AR aging report (current / 30 / 60 / 90+), computed from open-invoice
  balances. Renders via the existing `comfy-table` report style.
- `nigel invoice import --from-invoiceshelf <db>` — one-time history import.

## Error handling

- **Publish is transactional in effect:** `send` performs Stripe → R2 → Mailgun in order; a
  failure at any step aborts with a clear error and leaves the invoice `draft` (nothing
  half-sent). Re-running `send` is safe — the Payment Link is created once and reused if already
  present.
- **Sync is idempotent** by `stripe_checkout_session_id`; a double run records nothing twice.
- **Secrets missing/invalid** → actionable error naming the missing setting/env var, never a
  silent no-op.
- Money conversions to Stripe cents use an explicit round; a non-representable amount is an
  error, not a truncation.

## Testing (TDD, per Nigel's CI)

- Unit: status derivation, AR aging math, base62 token generation, and the Stripe / Mailgun /
  R2 **request builders** verified against fakes (no live network) — the gateway-fake pattern
  proven on the InvoiceShelf StripePay module.
- Sync idempotency: replaying the same completed session records exactly one payment.
- Importer: a fixture InvoiceShelf SQLite maps to the expected Nigel rows with correct
  cents→REAL conversion and `next_invoice_number`.
- Dispatch: new subcommands wired through the existing `tests/cli_dispatch.rs`.

## Deployment / routing notes

- Cloudflare routes `billing.rygn.io/i/*` to the R2 bucket (custom-domain binding or a route),
  leaving the root on InvoiceShelf during transition. Once Nigel fully replaces InvoiceShelf,
  R2 can take the whole domain with no change to Nigel.
- New settings: Stripe secret key, Mailgun API key, and R2 credentials (account id, access
  key/secret, bucket, public base URL). Restricted-perms settings file + env-var overrides.

## Open items for the plan

- Confirm exact InvoiceShelf table/column names for the importer against the live DB.
- Decide the PDF invoice layout details (reuse company header from settings).
- Confirm the R2 custom-domain vs route mechanism in Cloudflare.
