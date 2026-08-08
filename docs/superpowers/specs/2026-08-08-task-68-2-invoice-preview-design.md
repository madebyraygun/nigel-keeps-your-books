# `nigel invoice preview` — see the invoice before it goes out

Task: TASK-68.2 (epic TASK-68, *Invoicing management surface*).

## Problem

The first time an invoice is rendered is *inside* `send_invoice`, after the Stripe
Payment Link has already been created and immediately before the artifacts are
uploaded to R2 and emailed. There is no way to look at what the client will
receive without committing to all three network calls, and a mistake found after
the fact is a resend to a client's inbox.

`invoice preview <number>` renders the same HTML — and the same PDF, when the
`pdf` feature is compiled in — to local files and prints where they landed. It
touches no network, requires no invoicing configuration, and writes nothing to
the database.

## Command

```
nigel invoice preview <NUMBER> [--output-dir <DIR>]
```

- `<NUMBER>` is the invoice number (`nigel invoice list`), the same positional
  argument `show`, `send`, and `pay` take. An unknown number gets `find_invoice`'s
  existing sentence: `No invoice #9999. Run \`nigel invoice list\` to see invoice numbers.`
- `--output-dir <DIR>` writes into a directory of the caller's choosing. Absent,
  previews land in `<data_dir>/previews/`.

There is no `--format` flag: preview always writes everything the build can
render, because the point is to see what `send` would publish, and `send`
publishes both.

### Why `--output-dir` and not `--output`

`nigel report` already splits these two: `--output` names a **file** (one report,
one artifact) and `report all --output-dir` names a **directory** (many
artifacts). A preview is always two artifacts, so it is the `--output-dir` shape.
Reusing `--output` for a directory would make the same flag mean two different
things in one binary.

### Output location and naming

| | Path |
|---|---|
| Default | `<data_dir>/previews/invoice-<number>.html` and `.pdf` |
| `--output-dir DIR` | `DIR/invoice-<number>.html` and `.pdf` |

- **Data dir, not cwd.** An invoice is client-confidential and derived from the
  books; `<data_dir>` is already where Nigel puts derived artifacts
  (`exports/`, `backups/`, `snapshots/`) and is already mode 0700. The cwd is
  frequently a git checkout.
- **No date stamp.** `cli::report::export_file_stem` appends the export date
  because reports are period snapshots you want to keep side by side. A preview
  is a scratch view of one invoice; re-previewing after an edit should overwrite
  in place so a browser reload shows the new render. `invoice-1248.html` is
  stable and guessable.
- Permissions mirror `report`'s export path exactly: the directory is created if
  missing, `restrict_dir_permissions` (0700) is applied **only when it is the
  default `<data_dir>/previews`** (Nigel does not re-permission a directory the
  user named), and `restrict_file_permissions` (0600) is applied to every file
  written.
- **Both artifacts are rendered before either is written.** The seam returns both
  in memory, so a PDF failure leaves no freshly-written HTML beside a stale PDF.

### Output

Paths go to stdout, one per line, in `report`'s existing `Wrote <path>` wording.
Everything else — void warnings, missing `from_email`, a build with no PDF — goes
to stderr with the `notice:` prefix `main.rs` already uses for the launch sync.

```
$ nigel invoice preview 1248
Wrote /home/you/Documents/nigel/previews/invoice-1248.html
Wrote /home/you/Documents/nigel/previews/invoice-1248.pdf
```

No browser is opened and no `--open` flag ships in this task. `serve` opens a
browser because a tokenized URL is unusable if you cannot click it; a file path
in a terminal is already copyable, and every modern terminal linkifies it.
Mechanically, `open` is an optional dependency gated behind the `serve` feature,
so an `--open` flag would either be absent from some builds or drag `serve`'s
dependency into a CLI command. If opening is wanted later, the clean move is to
promote `open` to an unconditional dependency and add an opt-in flag — never
open-by-default, which cannot be taken back.

## The render seam

Today `send.rs` inlines the whole render between the gateway call and the
publish. Preview must run the *same* code or it will drift.

### New: `src/invoicing/render.rs`

```rust
pub struct RenderedInvoice {
    pub html: String,
    /// `None` only in a build without the `pdf` feature. Each caller decides
    /// whether that is fatal.
    pub pdf: Option<Vec<u8>>,
}

/// Render an invoice exactly as `send` publishes it. Reads the database, makes
/// no network call, and writes nothing.
pub fn render_invoice(
    conn: &Connection,
    invoice: &Invoice,
    client: &Client,
    pay: PayButton<'_>,
    contact_email: &str,
) -> Result<RenderedInvoice>;
```

`render_invoice` loads the line items itself (`invoices::line_items`), so both
callers get the same rows in the same order — the seam takes an invoice, not a
pre-assembled bundle. The two `#[cfg]` halves of `send.rs`'s private `render_pdf`
move here unchanged in shape, except that the `not(feature = "pdf")` half now
returns `Ok(None)` instead of an error.

The split against the existing `render_html.rs` is: **`render_html.rs` renders one
artifact and is a pure function of its arguments; `render.rs` renders the artifact
*set* `send` publishes, and is the one place that knows the `pdf` feature exists.**

### `send.rs` after the extraction

```rust
let rendered = render_invoice(conn, &invoice, &client, pay, contact_email)?;
let pdf = rendered.pdf.ok_or_else(|| NigelError::Other(
    "PDF support not compiled in (build with --features pdf)".into(),
))?;
publisher.publish(&invoice.token, rendered.html.as_bytes(), &pdf)?;
mailer.send_invoice(&email, &subject, &rendered.html, &pdf)?;
```

Send keeps its own sentence (it is a hard failure there and is already covered by
tests); preview words its own. `src/invoicing/` does not reach into `src/cli/`.

### Forward compatibility with TASK-68.3

68.3 makes the HTML template overridable from `<data_dir>/templates/invoice.html`.
That change lands **inside `render_html.rs`**, below the seam, so preview picks it
up with no edit. Two rules keep it that way:

1. **The seam takes data, never a template.** No template path, no template
   string, no "which template" flag crosses `render_invoice`.
2. **`render_invoice` already returns `Result`.** When 68.3 makes
   `render_invoice_html` fallible (it will start reading a file), the signature
   above absorbs it without changing anything above the seam.

## The Pay button

`render_invoice_html` currently takes `pay_url: Option<&str>` and renders either
an anchor or nothing. Preview needs a third rendering, so the parameter becomes an
explicit three-state enum in `render_html.rs`:

```rust
pub enum PayButton<'a> {
    /// A real Stripe link. `<a class="pay" href="…">Pay online</a>` — unchanged.
    Link(&'a str),
    /// A draft that will get a link when it is sent. Inert placeholder.
    Placeholder,
    /// No link, and none coming. Nothing rendered — today's `None` behavior.
    Omitted,
}
```

Placeholder markup — a `<span>`, so there is nothing to click and nothing to 404:

```html
<span class="pay pay-placeholder" style="background:#777;cursor:default">Pay online — link created when the invoice is sent</span>
```

The style is **inline rather than a new rule in `templates/invoice.html`**. The
placeholder is generated entirely by `render_html.rs`, so it renders correctly
against any template with a `{{PAY}}` slot — including the custom templates 68.3
will let users write, which will not know about a `.pay-placeholder` class. The
`.pay` class rides along so a custom stylesheet can still restyle it, and
`templates/invoice.html` is not touched by this task at all.

### Which button, when

| Invoice state | Preview renders | Send renders |
|---|---|---|
| Has `stripe_payment_link_url`, not void | `Link(url)` | `Link(url)` |
| No link, not void | `Placeholder` | `Link(url)` — send creates it first |
| Void | `Omitted` | n/a — `ensure_not_void` refuses |

**Void always omits the button, even when the invoice already carries a link.**
An invoice voided after it was sent still has a live Stripe URL in the row;
rendering a working Pay button on a cancelled invoice is the one way this command
could cause real damage.

This table *is* acceptance criterion #3: apart from that single element (and the
absent PDF in a build without the feature), a preview is byte-identical to what
`send` uploads. There is deliberately **no "PREVIEW" watermark or banner** in the
artifact — that would defeat the point. The framing lives in the terminal output.

## Configuration and network

`send` requires nine settings by name. Preview requires none:

- It reads `settings::invoicing_config()` but calls no `require()` and builds no
  `StripeClient`/`R2Publisher`/`MailgunClient`.
- `contact_email` (the direct-deposit line) comes from `from_email` when set. When
  unset, it renders the literal `(from_email not configured)` and preview prints
  `notice: from_email is not configured — the direct-deposit contact line is a placeholder`
  to stderr. The value is escaped by `esc()` like any other.
- **`main.rs` must add `InvoiceCommands::Preview { .. }` to the list of commands
  that skip `sync_invoice_payments()`.** That launch hook polls Stripe before the
  subcommand runs whenever `stripe_secret_key` is configured, and without this it
  would make acceptance criterion #2 false on any configured machine — while the
  preview code itself is provably offline.

Preview goes through the normal migration and password pre-flight, like every
other `invoice` subcommand.

## Void invoices

Preview **renders** a void invoice and warns; it does not refuse. `send` and `pay`
refuse because they change the world. Preview is read-only, and looking at what
you cancelled is a legitimate thing to want.

```
notice: invoice #1248 is void — this preview is for reference only.
```

Combined with the `Omitted` pay button above, a void preview cannot be mistaken
for a payable document.

## Builds without the `pdf` feature

The HTML is written, no PDF is written, **exit status is 0** — the acceptance
criterion is "HTML, and PDF *when the feature is on*", so this is the documented
outcome and not a failure. The notice is `cli::report::PDF_DISABLED_MESSAGE`
verbatim, the same sentence `nigel report --format pdf` and the HTTP export
endpoints already print:

```
Wrote /home/you/Documents/nigel/previews/invoice-1248.html
notice: PDF export requires the 'pdf' feature — build with `cargo build --features pdf`
```

A stale `invoice-1248.pdf` from an earlier `pdf`-enabled run is left alone rather
than deleted — deleting a file the user may have moved or kept is a worse
surprise than the notice explaining why no PDF was written.

## Out of scope

- Opening a browser (`--open`), and any change to the `open` dependency's gating.
- Template or PDF-layout customization — that is TASK-68.3.
- Previewing the *email* body (subject line, Mailgun rendering).
- A `preview` endpoint on the HTTP API or a TUI screen — TASK-68.4 / 68.6.
- Previewing an invoice that does not exist yet (a dry-run of `invoice new`).

## Open questions for the orchestrator

1. **Flag name.** Settled here as `--output-dir`, matching `report all`. The task
   text says "a `--output` path". If you prefer the literal wording, renaming is a
   one-line clap change and nothing else in this design moves.
2. **Browser opening.** Deferred entirely. Confirm that printing paths is enough
   for now, or say the word and `open` gets promoted out of the `serve` feature
   for an opt-in `--open`.
3. **Void posture.** Warn-and-render (with the button omitted) rather than refuse.
   TASK-68.1 is what makes `void` reachable in the first place; confirm this is
   the posture you want before 68.1 lands and the state becomes common.
4. **Seam visibility.** `render_invoice` is `pub` in `src/invoicing/render.rs` but
   is only called from `send.rs` and `cli/invoice.rs` in this task. TASK-68.6 will
   want it from an HTTP handler. Leaving it `pub` now costs nothing; flag it if
   you would rather it were `pub(crate)` until there is a second caller.
5. **Placeholder wording.** `(from_email not configured)` renders inside the
   sentence "Contact `(from_email not configured)` for account details." If you
   have a preferred phrasing for a document a user might screenshot, name it now —
   it is one string constant.
