# Invoicing

Nigel bills clients end to end: draft an invoice, publish it as a static page and
PDF on Cloudflare R2, email it through Mailgun with a Stripe payment link, and
reconcile payments back into the books.

Invoicing is accounts-receivable only. Invoices and payments live in their own
tables (`clients`, `invoices`, `invoice_line_items`, `invoice_payments`) and never
touch the transaction register — record the bank deposit as a transaction the way
you would any other income.

Sending requires a build with the `pdf` feature (the default). Without it,
`nigel invoice send` stops at the render step — nothing is published or emailed,
because there is no PDF to upload or attach.

## Configuration

Secrets and endpoints resolve from the environment first, then from
`~/.config/nigel/settings.json`.

| settings.json key | Environment variable | Required for | Default |
|---|---|---|---|
| `stripe_secret_key` | `NIGEL_STRIPE_SECRET_KEY` | `send`, `sync` | — |
| `mailgun_api_key` | `NIGEL_MAILGUN_API_KEY` | `send` | — |
| `mailgun_domain` | `NIGEL_MAILGUN_DOMAIN` | `send` | `rygn.io` |
| `from_email` | `NIGEL_FROM_EMAIL` | `send` | `billing@rygn.io` |
| `r2_account_id` | `NIGEL_R2_ACCOUNT_ID` | `send` | — |
| `r2_access_key` | `NIGEL_R2_ACCESS_KEY` | `send` | — |
| `r2_secret_key` | `NIGEL_R2_SECRET_KEY` | `send` | — |
| `r2_bucket` | `NIGEL_R2_BUCKET` | `send` | — |
| `public_base_url` | `NIGEL_PUBLIC_BASE_URL` | `send` | — |

A missing value is reported by name, e.g.
`missing invoicing config: r2_bucket (set it in settings.json or the matching NIGEL_ env var)`.

Environment variables keep credentials out of the settings file. If you do store
them in `settings.json`, note that Nigel writes that file with owner-only
permissions on Unix. Use Stripe test keys (`sk_test_…`) while trying things out.

```json
{
  "data_dir": "/home/you/Documents/nigel",
  "stripe_secret_key": "sk_test_...",
  "mailgun_api_key": "...",
  "mailgun_domain": "rygn.io",
  "from_email": "billing@rygn.io",
  "r2_account_id": "...",
  "r2_access_key": "...",
  "r2_secret_key": "...",
  "r2_bucket": "billing",
  "public_base_url": "https://billing.rygn.io/i"
}
```

## Clients

```bash
nigel client add "Acme Co" --email ap@acme.test --address "1 Main St, Portland OR"
nigel client list
```

`--email` is optional at creation, but an invoice cannot be sent to a client
without one. `nigel client list` prints the client IDs that `invoice new` takes.

## Creating an invoice

```bash
nigel invoice new --client 1 --issue 2026-08-04 --due 2026-09-03 \
  --item "Consulting:10:150" \
  --item "Hosting:1:45"
```

- `--item "desc:qty:unit"` is repeatable; at least one is required. The line total
  is `qty × unit`, and the invoice total is the sum of the lines. Descriptions
  cannot contain a colon.
- `--due` is optional. An invoice with no due date never goes overdue and ages
  from its issue date.
- `--currency` defaults to `USD`.
- Numbers are assigned sequentially, starting at 1248, and are not reused.

New invoices are drafts. Nothing has been rendered, uploaded, or emailed yet.

```bash
nigel invoice list            # number, status, client, total, due date
nigel invoice show 1248       # line items, amount paid, balance, payment link
```

## Sending

```bash
nigel invoice send 1248
```

One command does the whole publish:

1. Creates a Stripe Payment Link for the invoice total, if the invoice does not
   already have one. Resending reuses the existing link, so a client who bookmarked
   it can still pay.
2. Renders the invoice to HTML and PDF.
3. Uploads both to R2 as `i/{token}/index.html` and `i/{token}/invoice.pdf`, where
   `token` is the invoice's random 16-character identifier.
4. Emails the client through Mailgun — HTML body, PDF attached, subject
   `Invoice #1248 from Raygun`.
5. Marks the invoice published, which moves it from `draft` to `sent` (or straight
   to `overdue` if its due date has already passed).

If any step fails the invoice stays a draft and no email goes out, so a failed
send is safe to retry. The command prints the public URL on success:
`Sent invoice #1248: https://billing.rygn.io/i/aBc123.../`.

The published page shows the line items, the total, a Pay button linking to
Stripe, and bank-transfer instructions.

## Recording payments

Stripe payments are pulled in, not pushed by webhook:

```bash
nigel invoice sync
```

`sync` walks every open invoice (`sent`, `partial`, or `overdue`) that has a
payment link, asks Stripe for that link's completed checkout sessions, and records
any it has not seen. Payments are keyed by checkout session ID, so re-running it
records nothing twice. It prints `Recorded N new payment(s)`.

Payments made outside Stripe are entered by hand:

```bash
nigel invoice pay 1248 --date 2026-08-20                        # the whole balance
nigel invoice pay 1248 --date 2026-08-20 --amount 500           # a partial payment
nigel invoice pay 1248 --date 2026-08-20 --method ach
```

`--amount` defaults to the outstanding balance and must be positive; overpayments
are allowed, since banks make them. `--method` is one of `stripe`, `ach`,
`direct_deposit` (the default), or `other`.

### Sync on launch

Every subcommand that reads or writes the books runs a sync first, as long as a
Stripe secret key is configured. It is best-effort: it prints
`notice: recorded 2 new invoice payment(s)` when it finds something and
`notice: invoice sync skipped: <reason>` when Stripe or the network is unavailable,
and either way the command you typed runs normally. `init`, `demo`, `load`,
`update`, `password`, `restore`, `completions`, and `invoice sync` itself skip the
hook.

## Trying it end to end in test mode

With Stripe test-mode keys and a scratch R2 bucket exported, the whole round trip
can be rehearsed against a throwaway data directory:

```bash
nigel client add "Test Client" --email you@example.com
nigel invoice new --client 1 --issue 2026-08-04 --item "Consulting:1:100"
nigel invoice send 1248        # publishes to R2, emails, creates the Stripe link
# pay via the emailed link with Stripe's test card 4242 4242 4242 4242, then:
nigel invoice sync             # records the payment and flips the invoice to paid
nigel invoice aging            # the settled invoice is out of the buckets
```

## Status

Status is derived from what has happened to the invoice, and is recalculated
whenever it is published or paid:

| Status | Meaning |
|---|---|
| `draft` | Created but never published |
| `sent` | Published, nothing paid |
| `partial` | Published, paid in part |
| `overdue` | Published, past its due date, with a balance |
| `paid` | Paid in full (settled to within half a cent) |
| `void` | Cancelled; cannot be sent or paid |

## A/R aging

```bash
nigel invoice aging
```

Buckets the outstanding balance of every open invoice by how long it has been due
— `current`, `1-30`, `31-60`, `61-90`, `90+` days past due. Invoices with no due
date age from their issue date.

## Importing from InvoiceShelf

```bash
nigel invoice import --from-invoiceshelf ~/invoiceshelf/database.sqlite
```

Copies customers, invoices, line items, and payments out of an InvoiceShelf SQLite
database, converting its integer cents to Nigel's dollar amounts. Imported
invoices keep their original numbers and arrive already published — `paid` or
`sent`, following InvoiceShelf's paid status — with their payments recorded under
method `other`. The next number Nigel assigns continues above the highest number
imported. The command reports what it moved:
`Imported 12 clients, 87 invoices, 91 payments. Next invoice number: 1361`.

Run it once, against a fresh Nigel database — it does not reconcile against
invoices that already exist.

## Hosting: billing.rygn.io → R2

Published invoices are static files in an R2 bucket. Nigel uploads them with the
S3 API to `https://{r2_account_id}.r2.cloudflarestorage.com`, using the R2 access
key pair, and builds client-facing links from `public_base_url`.

The two sides meet at Cloudflare: expose the bucket at a hostname you control —
a custom domain on the bucket, or an equivalent route into it — so an object
stored at key `i/{token}/index.html` is served at, for example,
`https://billing.rygn.io/i/{token}/`, and set `public_base_url` to
`https://billing.rygn.io/i`. Keep the `i/` prefix in `public_base_url` aligned
with that mapping — Nigel writes keys under `i/`, and the base URL only tells it
what public address that prefix answers on.

Tokens are random and unguessable, and nothing enumerates the bucket, so an
invoice is readable only by someone holding its link.

To bill from a different domain, point that hostname at your bucket and set
`public_base_url` (or `NIGEL_PUBLIC_BASE_URL`) to its `…/i` prefix.
