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
because there is no PDF to upload or attach. `nigel invoice preview` is the
exception: without the feature it writes the HTML and says why there is no PDF,
rather than stopping.

## From the dashboard

Everything below is a terminal command, but the day-to-day half of it is on the
dashboard as well. Run `nigel` and press:

- `k` — **Clients.** The list, `a` to add one, `e` to edit the selected one.
  There is no delete: a client with invoices must not disappear from under them.
- `n` — **Invoices.** The list (number, status, client, total, balance, due) with
  `Enter` to open one. The actions live on the open invoice, not the list: `s`
  sends it, `p` records a payment against it, `v` voids it — each with a
  confirmation, and each refused in the same words the CLI would use.

The dashboard **cannot draft an invoice**: `nigel invoice new` is still the only
way to create one, and the empty invoice list says so. The list also shows the
stored status, exactly as `nigel invoice list` does, so an invoice that crossed
its due date since it was last written still reads `sent` rather than `overdue`
until something touches it.

Sending from the dashboard blocks the terminal for the few seconds the three
network hops take. The screen says so while it waits, and keys pressed during
the wait are discarded rather than dismissing the result.

## Configuration

Secrets and endpoints resolve from the environment first, then from
`~/.config/nigel/settings.json`.

| settings.json key | Environment variable | Required for | Default |
|---|---|---|---|
| `stripe_secret_key` | `NIGEL_STRIPE_SECRET_KEY` | `send`, `sync` | — |
| `mailgun_api_key` | `NIGEL_MAILGUN_API_KEY` | `send` | — |
| `mailgun_domain` | `NIGEL_MAILGUN_DOMAIN` | `send` | — |
| `from_email` | `NIGEL_FROM_EMAIL` | `send` | — |
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
  "mailgun_domain": "mg.example.com",
  "from_email": "billing@example.com",
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

### Inspecting and editing a client

```bash
nigel client show 1
nigel client edit 1 --email billing@acme.test
nigel client edit 1 --name "Acme Corporation" --address "500 Market St"
```

`client show` prints the client's details, every invoice it has ever had (newest
number first) and the balance still open against it — void and fully paid
invoices contribute nothing.

`client edit` takes `--name`, `--email`, `--address` and `--notes`; the flags you
leave off are left alone, and passing none at all is an error rather than a silent
no-op. A blank `--name` is refused, since the column is required. `--notes` is
internal and never appears on an invoice.

Edits take effect on the **next** send. Published pages are static snapshots on
R2, so a corrected address reaches the client when the invoice is next sent —
including a re-send of the same invoice, which overwrites the same URL. Emails
already delivered keep the old details.

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
- `--currency` defaults to `USD` and must be a 3-letter code; it is stored
  uppercase.
- `--notes` and `--terms` are free text, e.g.
  `--notes "Thanks for the work this quarter."` and
  `--terms "Net 30. Late payments accrue 1.5% monthly."`. Both render under
  their own headings on the invoice page and on the PDF, and are omitted
  entirely when unset.
- Numbers are assigned sequentially, starting at 1248, and are not reused.

New invoices are drafts. Nothing has been rendered, uploaded, or emailed yet.

```bash
nigel invoice list            # number, status, client, total, due date
nigel invoice show 1248       # line items, amount paid, balance, payment link
```

## Previewing

```bash
nigel invoice preview 1248
nigel invoice preview 1248 --output-dir ~/Desktop
```

Preview renders exactly what `send` would publish — the same HTML page, the same
PDF — to local files, and prints where they landed:

```
Wrote /home/you/Documents/nigel/previews/invoice-1248.html
Wrote /home/you/Documents/nigel/previews/invoice-1248.pdf
```

| | Path |
|---|---|
| Default | `<data_dir>/previews/invoice-<number>.html` and `.pdf` |
| `--output-dir DIR` | `DIR/invoice-<number>.html` and `.pdf` |

The filenames carry no date. An exported report is a period snapshot you keep
beside its neighbours; a preview is a scratch view of one invoice, so
re-previewing after an edit overwrites in place and a browser reload shows the
new render. The default directory is created 0700 and every file 0600, the same
handling `nigel report` gives an export; a directory you name yourself is not
re-permissioned.

The Pay button is the only thing that can differ from what a client receives:

| Invoice state | Preview renders |
|---|---|
| Has a Stripe payment link, not void | The real link, exactly as sent |
| No link yet, not void | An inert placeholder where the button will go |
| Void | Nothing — even if the invoice still carries a live link |

A void invoice previews rather than refusing, with
`notice: invoice #1248 is void — this preview is for reference only.` on stderr.
Looking at what you cancelled is legitimate; offering a working payment link for
it is not, which is why the button is dropped even when the Stripe URL is still
in the row.

Preview is the one invoicing command that works on a fresh install: it needs no
Stripe, R2, or Mailgun configuration and makes no network call. With `from_email`
unset the direct-deposit contact line renders `(from_email not configured)` and
the command says so on stderr — the page is still complete enough to check the
figures and the layout.

In a build without the `pdf` feature the HTML is written, no PDF is, and the exit
status is still 0:

```
Wrote /home/you/Documents/nigel/previews/invoice-1248.html
notice: PDF export requires the 'pdf' feature — build with `cargo build --features pdf`
```

A PDF left over from an earlier `pdf`-enabled run is left alone rather than
deleted — it may have been kept deliberately, and the notice already explains why
it was not refreshed.

## Editing a draft invoice

```bash
nigel invoice edit 1248 --due 2026-09-30
nigel invoice edit 1248 --item "Discovery:1:2000" --item "Build:40:150"
nigel invoice edit 1248 --currency EUR --terms "Net 15"
nigel invoice edit 1248 --clear-due
```

| Flag | Effect |
|---|---|
| `--issue <YYYY-MM-DD>` | New issue date |
| `--due <YYYY-MM-DD>` | New due date |
| `--clear-due` | Remove the due date, so the invoice never goes overdue |
| `--currency <CODE>` | New 3-letter currency code, stored uppercase |
| `--notes <s>` | Replace the notes |
| `--terms <s>` | Replace the terms |
| `--item "desc:qty:unit"` | **Replaces every line item**, repeatable |

`--item` is all or nothing: leave it off and the existing lines stand, supply it
and the whole set is rewritten and the subtotal and total recomputed. There is no
way to leave an invoice with no lines. Passing no flags at all is an error.

Editing is **draft only**. A published invoice answers
`Invoice #1248 has already been sent and cannot be edited. Void it and issue a new
one.`, and a void one answers `Invoice #1248 is void and cannot be edited.` An
invoice with any payment recorded against it is also refused, whatever its status
— the client has settled against those figures, and restating them under a
recorded payment would misdescribe what was paid. That is also why a currency
change after a payment is unreachable.

If the edit moves the total or the currency on an invoice that already carried a
Stripe payment link — which happens when a send failed after the link was created
— the link is cleared, and the next `send` makes a fresh one at the right amount.

## Voiding an invoice

```bash
nigel invoice void 1248
nigel invoice void 1248 --yes
```

Void cancels an invoice. On a terminal it names the invoice and asks
`Void it? [y/N]`; anything but `y` prints `Aborted.` and changes nothing. Without
a terminal, `--yes` is required — a script gets a refusal rather than a silently
cancelled invoice.

A voided invoice leaves the aging buckets and stops being polled for Stripe
payments, but stays in `invoice list` with status `void`, and its number is never
reused. Void is terminal: there is no unvoid, and a void invoice refuses send,
pay, and edit. An invoice with payments recorded against it cannot be voided —
cancel the money side by recording the offsetting movement in the transaction
register, which is where cash actually lives.

Voiding does **not** tear down a published invoice: the R2 page and PDF stay
served and the Stripe payment link stays chargeable, so the command warns you to
deactivate the link in Stripe yourself.

## Sending

```bash
nigel invoice send 1248
```

One command does the whole publish:

1. Creates a Stripe Payment Link for the invoice total, if the invoice does not
   already have one. Resending reuses the existing link, so a client who bookmarked
   it can still pay.
2. Renders the invoice to HTML and PDF — the same `render_invoice` seam
   `nigel invoice preview` writes locally, so a preview cannot disagree with
   what is published.
3. Uploads both to R2 as `i/{token}/index.html` and `i/{token}/invoice.pdf`, where
   `token` is the invoice's random 16-character identifier.
4. Emails the client through Mailgun — HTML body, PDF attached, subject
   `Invoice #1248 from Acme LLC`, or plain `Invoice #1248` when no business name
   is set. The name comes from the same setting the dashboard's settings screen
   edits.
5. Marks the invoice published, which moves it from `draft` to `sent` (or straight
   to `overdue` if its due date has already passed).

If any step fails the invoice stays a draft and no email goes out, so a failed
send is safe to retry. The command prints the public URL on success:
`Sent invoice #1248: https://billing.rygn.io/i/aBc123.../`.

The published page shows the line items, the total, any notes and terms, a Pay
button linking to Stripe, and bank-transfer instructions. The direct-deposit line
tells the client to get in touch at `from_email`, the same address the invoice is
sent from.

## Customizing the invoice page

The page a client opens is yours to change without rebuilding Nigel. Put a file
here and it renders instead of the built-in one:

```
<data_dir>/templates/invoice.html
```

No file there means the built-in page renders, exactly as it always has.

```bash
nigel invoice template export                      # write the built-in page out to edit
nigel invoice template export --output ~/mine.html # somewhere else
nigel invoice template export --force              # overwrite an existing template
nigel invoice template path                        # where Nigel looks, and what it found
```

`export` refuses to overwrite an existing file without `--force`, because the
file it would clobber is your own work. Neither command opens the database, so
both run on a machine that has never seen `nigel init`.

### The iteration loop

```bash
nigel invoice template export
$EDITOR ~/Documents/nigel/templates/invoice.html
nigel invoice preview 1248 && open ~/Documents/nigel/previews/invoice-1248.html
```

`preview` renders through the same code `send` publishes through, makes no
network call, and needs no Stripe, R2, or Mailgun configuration — so a template
can go through twenty revisions before anything is ever sent.

### Placeholders

A template is plain HTML with `{{KEY}}` placeholders. There are no conditionals,
loops, or includes: the fragment placeholders are already empty when they have
nothing to say.

| Placeholder | Kind | Value |
|---|---|---|
| `{{NUMBER}}` | text | Invoice number (**required**) |
| `{{CLIENT}}` | text | Client name (**required**) |
| `{{CLIENT_EMAIL}}` | text | Client billing email, empty when unset |
| `{{CLIENT_ADDRESS}}` | text | Client billing address, empty when unset |
| `{{COMPANY}}` | text | Your business name, empty when unset |
| `{{ISSUE}}` | text | Issue date |
| `{{DUE_DATE}}` | text | Due date, empty when there is none |
| `{{DUE}}` | fragment | `<br>Due: …`, empty when there is none |
| `{{ROWS}}` | fragment | The line-item `<tr>` rows (**required**) |
| `{{CURRENCY}}` | text | Currency code |
| `{{SUBTOTAL}}` | text | Subtotal, two decimals |
| `{{TAX}}` | text | Tax, two decimals |
| `{{TOTAL}}` | text | Total, two decimals (**required**) |
| `{{NOTES}}` | fragment | Notes block, empty when unset |
| `{{TERMS}}` | fragment | Terms block, empty when unset |
| `{{PAY_URL}}` | text | Stripe payment link, empty when there is none |
| `{{PAY}}` | fragment | The Pay button, empty when there is no link |
| `{{CONTACT}}` | text | Direct-deposit contact address (`from_email`) |

**Text** placeholders are HTML-escaped values you can put in element content or
inside a quoted attribute value. **Fragment**
placeholders are pre-built markup and are content-only — putting one in an
attribute produces broken HTML. `{{DUE_DATE}}` and `{{PAY_URL}}` are the escaped
text alternatives for authors who want to place those two values themselves.

Where a placeholder may go:

> Placeholders are safe in element content and inside quoted attribute values.
> Do not put one inside `<script>`, `<style>`, an unquoted attribute, or a
> position where the value becomes a URL scheme.

Every value is escaped on the way in, so a client named `Acme <script> Co` is
text on the page and never markup. The template itself is **not** sanitized —
your `<script>` and your web fonts are the feature, and anyone who can write the
file can already run programs as you.

### When a template is refused

A template that is there but broken is an error naming the path. Nigel does not
quietly mail the stock page instead: an invoice you did not approve reaching a
client is worse than a send you have to retry. On `send` the template is loaded
before the Stripe link is created, so a broken one costs nothing — no link, no
upload, no email.

| Condition | What you get |
|---|---|
| Cannot be read (permissions, a directory, invalid UTF-8) | `Cannot read invoice template <path>: …` |
| Empty or whitespace only | `Invoice template <path> is empty.` |
| Larger than 1 MiB | `Invoice template <path> is <n> bytes; the limit is 1 MiB.` |
| Missing `{{NUMBER}}`, `{{CLIENT}}`, `{{ROWS}}` or `{{TOTAL}}` | `… is missing required placeholder(s): {{TOTAL}}. …` |
| Uses a `{{KEY}}` that is not in the table above | `… uses unknown placeholder(s): {{TOTL}}. …` |

The four required placeholders are what an invoice is — which invoice, who owes,
for what, how much. An unknown one is always a typo, and refusing is how you find
out before `{{TOTL}}` appears on a page someone is reading.

Only `{{` + `SCREAMING_SNAKE` + `}}` counts as a placeholder, so a CSS rule, a JS
template literal, or a `{{ not a key }}` aside passes through as literal text.
Checking happens when the template is loaded, which is why `nigel invoice
template path` and `nigel invoice preview` both report the problem.

The PDF is not customizable — it is built by code rather than from a template.

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
`update`, `password`, `restore`, `completions`, `invoice sync` itself, and
`invoice preview` skip the hook — preview is defined to make no network call, and
the launch sync would make that false on a configured machine.

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
| `void` | Cancelled; cannot be sent, paid, or edited |

## A/R aging

```bash
nigel invoice aging                    # print the table
nigel report aging                     # the same report, browsable
nigel report aging --mode export       # PDF into <data_dir>/exports/
```

Buckets the outstanding balance of every open invoice by how long it has been due
— `current`, `1-30`, `31-60`, `61-90`, `90+` days past due. Invoices with no due
date age from their issue date. Drafts, void invoices and anything settled in
full are left out. Both commands are always as of today; there is no as-of date
to pass. The JSON API's `GET /api/invoices/aging` does take an optional `asOf`,
which is the one place that differs — see [`api.md`](api.md).

Both commands print the same numbers — `invoice aging` prints the report's own
table:

```
Acme Consulting LLC

A/R Aging — as of 2026-08-07

Summary
+-------------------+----------+-----------+
| Bucket            | Invoices | Amount    |
+==========================================+
| current           | 1        | $1,500.00 |
|-------------------+----------+-----------|
| 1-30              | 1        | $1,500.00 |
|-------------------+----------+-----------|
| 31-60             | 0        | $0.00     |
|-------------------+----------+-----------|
| 61-90             | 0        | $0.00     |
|-------------------+----------+-----------|
| 90+               | 1        | $3,200.00 |
|-------------------+----------+-----------|
| Total Outstanding | 3        | $6,200.00 |
+-------------------+----------+-----------+

Open Invoices
+---------+---------+------------+------+-----------+
| Invoice | Client  | Due        | Days | Balance   |
+===================================================+
| #1250   | Initech | 2026-05-04 | 95   | $3,200.00 |
|---------+---------+------------+------+-----------|
| #1249   | Acme Co | 2026-07-20 | 18   | $1,500.00 |
|---------+---------+------------+------+-----------|
| #1248   | Acme Co | 2026-08-27 | —    | $1,500.00 |
+---------+---------+------------+------+-----------+
```

`nigel report aging` opens the interactive view on a terminal (scroll with
↑/↓, `q` closes) and falls back to this text when piped. The dashboard offers
it under both `v` (view a report) and `e` (export a report), and the home
screen shows the outstanding total whenever any invoice is open.

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
