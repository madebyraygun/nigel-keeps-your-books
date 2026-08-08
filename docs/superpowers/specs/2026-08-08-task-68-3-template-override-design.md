# Invoice template customization without rebuilding — design

Task 68.3 of epic 68 (Invoicing management surface).

## Problem

`src/invoicing/render_html.rs:3` bakes the client-facing invoice page into the
binary with `include_str!("templates/invoice.html")`. Changing a colour, a
heading, or adding a company name means editing `src/` and rebuilding. The page
is the first thing a client sees, and it is the one artifact in the whole
invoicing pipeline whose look is nobody's business but the operator's.

The PDF has the same problem in a harder form: `src/pdf.rs:787` builds the
document imperatively against a layout DSL — there is no template to override,
only code.

## Shape of the change

A template file in the data directory replaces the embedded default:

```
<data_dir>/templates/invoice.html
```

Absent → the embedded default renders, exactly as today. Present → it renders
instead, after validation. `nigel invoice template export` writes the embedded
default to that path as a starting point.

## Where the filesystem read lives

`src/invoicing/` never reads settings — `from_email` reaches `send_invoice` as a
parameter for exactly this reason (see CLAUDE.md, "No invoicing config has a
built-in default"). The template follows that rule: the invoicing module offers
loading and rendering functions that take an explicit path or an explicit
string, and the CLI layer decides where the data directory is.

```rust
// src/invoicing/render_html.rs
pub const DEFAULT_TEMPLATE: &str = include_str!("templates/invoice.html");

/// `<data_dir>/templates/invoice.html`
pub fn template_path(data_dir: &Path) -> PathBuf;

/// The override if the file exists and validates, the embedded default if it
/// does not exist. An override that exists but cannot be read or does not
/// validate is an error naming the path.
pub fn load_template(data_dir: &Path) -> Result<Cow<'static, str>>;

pub struct Branding<'a> {
    pub template: &'a str,
    pub company: &'a str,
    pub contact_email: &'a str,
}

pub fn render_invoice_html(
    branding: &Branding<'_>,
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
    pay_url: Option<&str>,
) -> String;
```

`send_invoice` takes `&Branding` in place of today's `contact_email: &str`.
`cli::invoice::send` builds it: `load_template(&get_data_dir())?`,
`db::get_metadata(&conn, "company_name")`, and `mail.from`. Loading happens
**before** `build_clients` and before the Stripe link is created, so a broken
template fails a send with nothing published, nothing charged, and no email out.

## Malformed override: hard error, never silent fallback

A template that exists but does not load is an error naming the path. It does
not fall back to the default.

The reasoning: `send` mails a real client. Someone who put a file at
`<data_dir>/templates/invoice.html` has decided what their invoice looks like.
Silently mailing the stock Nigel page instead means the client receives a
document the operator did not approve and has no way to notice — the command
prints `Sent invoice #1248: …` and everything looks fine. An error costs one
retry; a silent fallback costs a wrong document in a client's inbox. This also
matches how the rest of Nigel treats invoicing config, where a missing value is
reported by name rather than defaulted.

An override is rejected when it:

| Condition | Message |
|---|---|
| Cannot be read (permissions, is a directory, invalid UTF-8) | `Cannot read invoice template <path>: <io error>` |
| Is empty or whitespace only | `Invoice template <path> is empty.` |
| Exceeds 1 MiB | `Invoice template <path> is <n> bytes; the limit is 1 MiB.` |
| Omits a required placeholder | `Invoice template <path> is missing required placeholder(s): {{TOTAL}}. …` |
| Uses a placeholder that is not in the vocabulary | `Invoice template <path> uses unknown placeholder(s): {{TOTL}}. Known placeholders: …` |

Validation errors are `NigelError::Invalid`; read failures keep the underlying
`std::io::Error` in the message but are raised as `Invalid` too, so the path is
always in the sentence (a bare `NigelError::Io` would print `IO error: Permission
denied (os error 13)` with no clue which file).

The 1 MiB cap is not a security boundary — the operator owns the file — it stops
a mistake (a directory of images pasted in as data URIs, a wrong file copied over
the template) from becoming a 200 MB Mailgun attachment.

## Missing and unknown placeholders

Both are caught at **load** time, not render time, so `invoice preview` and
`invoice template path` surface them before a send does.

**Required:** `{{NUMBER}}`, `{{CLIENT}}`, `{{ROWS}}`, `{{TOTAL}}`. These four are
what an invoice is — which invoice, who owes, for what, how much. A template
missing one of them produces a document that is wrong about money, and rendering
it empty would be worse than refusing.

Everything else is optional and renders empty when absent from the template —
`{{CURRENCY}}` because a single-currency shop may hardcode `USD`, `{{DUE}}`
because not every invoice has a due date, `{{PAY}}` because not everyone uses
Stripe, `{{CONTACT}}` because not everyone offers bank transfer.

**Unknown:** a token matching `{{[A-Z_][A-Z0-9_]*}}` that is not in the
vocabulary is an error. The alternative — today's behaviour, emitting it
verbatim — puts the literal text `{{TOTL}}` on a page a client reads. It is
always a typo, and refusing is how the operator finds out.

The scan is deliberately narrow: only `{{`+`SCREAMING_SNAKE`+`}}` is treated as a
placeholder token. Any other brace sequence (`{{ not a key }}`, a CSS `{{`
accident, a JS template literal) passes through as literal text and does not
trip validation. That keeps false positives near zero at the cost of not catching
`{{ TOTAL }}` (spaced) as a typo — which is fine, because it renders as visible
literal text the operator sees in preview.

## Injection safety with an untrusted template

The task's constraint is that expansion stays injection-safe when the template
itself is user content. Working through it:

**The substitution algorithm needs no change.** `expand` is single-pass: on a
hit it appends the value to `out` and advances `rest` past the closing `}}`, so
substituted bytes are never rescanned. That invariant is a property of the
algorithm, not of where the template came from, and it is what makes a client
named `Acme {{ROWS}} Co` render as literal text rather than a second copy of the
line-item table (`render_html.rs:167` already tests this). It holds identically
for a template loaded from disk. The one thing this change must not do is
introduce a second pass, a "expand until stable" loop, or a regex-replace-all
implementation — any of those would re-expand injected values and turn a client
name into markup.

**Values stay escaped.** `esc` runs on every client-controlled value before it
enters the vars table. That is unchanged and is the actual XSS boundary: client
name, line-item descriptions, notes, terms, billing address, contact email, pay
URL. Adding vocabulary means adding `esc` calls, not exceptions to them.

**The template is not a privilege boundary, and must not be sanitized.** Anyone
who can write `<data_dir>/templates/invoice.html` can already run code as the
operator. A template containing `<script>` is the feature working, not an
attack. No sanitizer, no allowlist of tags, no CSP synthesis — those would break
legitimate customization while defending nothing. The path is fixed and takes no
user input, so there is no traversal surface either.

**What genuinely changes is placement.** Today the template is written by the
same people who wrote `esc`, so every placeholder sits in a context `esc` is
correct for. A custom template can put one anywhere. `esc` escapes
`& < > " '` — correct for element content and for quoted attribute values, and
not correct for a `<script>` body, a `<style>` body, an unquoted attribute, or a
URL-scheme position. This is a documented contract, not an enforced one:

> Placeholders are safe in element content and inside quoted attribute values.
> Do not put one inside `<script>`, `<style>`, an unquoted attribute, or a
> position where the value becomes a URL scheme.

Three placeholders — `{{ROWS}}`, `{{PAY}}`, `{{DUE}}` — are pre-built HTML
fragments rather than escaped text, and are content-only: putting one in an
attribute produces broken markup. Documented alongside the vocabulary table,
with `{{DUE_DATE}}` and `{{PAY_URL}}` offered as the escaped-text alternatives
for authors who want to place those values themselves.

The pre-existing `javascript:`-scheme question on `pay_url` is out of scope:
that value comes from Stripe, not from a client, and nothing about this change
alters it.

## Placeholder vocabulary

Today's nine, plus a bounded addition. A template you cannot put your own company
name or payment terms into is not customizable in any useful sense, and `notes`
and `terms` are already collected and already render in the PDF while the HTML
page silently drops them.

| Placeholder | Kind | Value |
|---|---|---|
| `{{NUMBER}}` | text | Invoice number (required) |
| `{{CLIENT}}` | text | Client name (required) |
| `{{CLIENT_EMAIL}}` | text | Client billing email, empty when unset |
| `{{CLIENT_ADDRESS}}` | text | Client billing address, empty when unset |
| `{{COMPANY}}` | text | `company_name` from the database, empty when unset |
| `{{ISSUE}}` | text | Issue date |
| `{{DUE_DATE}}` | text | Due date, empty when there is none |
| `{{DUE}}` | fragment | `<br>Due: …`, empty when there is none |
| `{{ROWS}}` | fragment | Line-item `<tr>` rows (required) |
| `{{CURRENCY}}` | text | Currency code |
| `{{SUBTOTAL}}` | text | Subtotal, two decimals |
| `{{TAX}}` | text | Tax, two decimals |
| `{{TOTAL}}` | text | Total, two decimals (required) |
| `{{NOTES}}` | fragment | Notes block, empty when unset |
| `{{TERMS}}` | fragment | Terms block, empty when unset |
| `{{PAY_URL}}` | text | Stripe payment link, empty when there is none |
| `{{PAY}}` | fragment | Pay button `<a>`, empty when there is none |
| `{{CONTACT}}` | text | Direct-deposit contact address (`from_email`) |

The default template picks up `{{NOTES}}` and `{{TERMS}}` so the HTML page stops
being the only surface that drops them.

`{{COMPANY}}` is the one value that needs a database read, so `cli::invoice`
resolves it the same way it resolves `contact_email`. The same value fixes the
hardcoded `"Invoice #{} from Raygun"` email subject at `send.rs:39` — with a
company name it reads `Invoice #1248 from Acme LLC`, without one it is just
`Invoice #1248`.

## Command grammar

```
nigel invoice template export [--output <PATH>] [--force]
nigel invoice template path
```

- `export` writes `DEFAULT_TEMPLATE` to `<data_dir>/templates/invoice.html`,
  creating the directory, and prints where it landed. It refuses to overwrite an
  existing file without `--force`, because the file it would clobber is the
  operator's own work. `--output <PATH>` writes somewhere else instead.
- `path` prints the path Nigel reads, says whether an override is in effect, and
  reports the validation error if the file is there and broken. It is the answer
  to "where do I put it" and to "why is my template not showing up", in five
  lines of code.

Nested subcommands are a small break with the existing grammar — `InvoiceCommands`
is flat today and nothing in the CLI nests three deep. It is worth it here
because `template` is a noun with more than one verb attached, and the flat
alternatives (`invoice export-template` plus `invoice template-path`) spend two
top-level verbs on a rarely used corner of the invoice surface. The flat form is
the fallback if the orchestrator would rather not set the precedent.

`invoice template` touches no database, so it joins `completions` in the
`needs_existing_db`, `needs_password`, and Stripe-sync exemption lists in
`main.rs:66-127`. Exporting a template on a machine that has never run
`nigel init` should work.

## Iterating on a template

`invoice preview` (task 68.2) is the loop:

```bash
nigel invoice template export
$EDITOR ~/Documents/nigel/templates/invoice.html
nigel invoice preview 1248 && open …/invoice-1248.html
```

Preview makes no network calls and needs no invoicing config, so a template can
be taken through twenty revisions without a Stripe key. This is the documented
workflow, and it is why validation runs at load rather than at send: preview
surfaces a missing `{{TOTAL}}` before a client ever could.

The dependency is one-directional and small: 68.2's preview must call
`load_template` rather than reaching for `DEFAULT_TEMPLATE`. If 68.2 has landed
when 68.3 is implemented, 68.3 rewires it in one line; if it has not, 68.3
leaves the seam and 68.2 uses it. 68.2 landing first is the cheaper order.

## PDF: deferred, explicitly

Out of scope for 68.3, and the task's "PDF customization can stay minimal
(company block, logo)" should be its own task rather than a rider on this one.

`src/pdf.rs` has no template — `render_invoice_pdf` emits a document by calling
layout primitives. "Customizable" there means one of two much larger things: a
structured settings file describing a company block and its typography, which is
a new file format with its own validation and its own export command; or
HTML-to-PDF rendering, which is a dependency decision (headless browser, Typst,
Weasyprint) with real weight in a tool that ships a single static binary. Logo
support additionally means image embedding in the PDF writer.

Folding either into 68.3 would triple it and delay the HTML page, which is the
artifact clients actually open — the email links to it and the PDF rides along as
an attachment. Recommended follow-up: a separate task for a PDF company block and
logo sourced from `company_name` and a settings path, sized on its own.

## Testing

Unit tests in `render_html.rs`:

- Override wins over the default; absent override falls back; the fallback is
  byte-identical to today's output.
- Each rejection path names the path: unreadable, empty, oversized, missing
  required placeholder, unknown placeholder.
- A whitespace-only file is empty; a file with only `{{FOO}}`-shaped noise fails
  on unknown placeholders, not on emptiness.
- `{{ not a key }}` and stray `{{` pass validation and render literally.
- **Injection under a custom template:** a client name of `Acme {{ROWS}} Co`
  renders literally when the template comes from disk, with exactly one copy of
  the line-item table — the same assertion as `render_html.rs:167`, now with the
  untrusted-template path.
- A custom template placing `{{CLIENT}}` in a quoted attribute keeps a
  quote-carrying client name inside the attribute.
- A template containing `<script>alert(1)</script>` renders it verbatim — the
  operator's markup is not sanitized.
- Every new placeholder escapes its value, and the empty forms of `{{DUE}}`,
  `{{PAY}}`, `{{NOTES}}`, `{{TERMS}}` render nothing rather than an empty tag.

Integration tests in `tests/cli_dispatch.rs`, using the existing `TestEnv`:

- `invoice template export` in a fresh HOME with no `nigel init` writes the file
  and prints the path; a second run without `--force` fails and leaves the file
  untouched; with `--force` it overwrites.
- `invoice template path` reports no override, then an override, then names the
  validation error for a broken one.
- A broken override makes `invoice send` fail before any network call — the
  invoice stays a draft. (`TestEnv::cmd` already strips every `NIGEL_*` credential,
  so a send in the test environment cannot reach Stripe regardless.)

## Documentation

`docs/invoicing.md` gains a "Customizing the invoice page" section: the override
path, the export and path commands, the full vocabulary table above, the
text-versus-fragment distinction and the placement contract, what a rejected
template says, and the preview loop. `CLAUDE.md`'s invoicing architecture bullet
gains the override path and the load-time validation rule.

## Out of scope

- PDF customization of any kind (see above).
- A template language — no conditionals, loops, includes, or partials. The
  fragment placeholders exist precisely so that "empty when absent" needs no
  `if`.
- Per-client or per-invoice template selection.
- Customizing the line-item row markup, which stays a Rust-built fragment behind
  `{{ROWS}}`.
- The email body template — `send` mails the same HTML it publishes, and that
  stays true.
- Any change to `esc` or to the single-pass structure of `expand`.

## Open questions for the orchestrator

1. **Nested subcommand precedent.** `invoice template export|path` is the first
   three-level command in the CLI. Accept it, or take the flat
   `invoice export-template` and drop `path`?
2. **Vocabulary expansion.** Tasks 7–8 of the plan add `{{COMPANY}}`,
   `{{NOTES}}`, `{{TERMS}}`, `{{SUBTOTAL}}`, `{{TAX}}`, `{{CLIENT_EMAIL}}`,
   `{{CLIENT_ADDRESS}}`, `{{DUE_DATE}}`, `{{PAY_URL}}` and are severable. Ship
   68.3 with the existing nine and file the rest, or take them here?
3. **Default template changes.** Adding `{{NOTES}}`/`{{TERMS}}` to the default
   changes what clients see on the stock page — from nothing to the notes and
   terms the operator already entered. Correct fix or unwanted surprise?
4. **The hardcoded `"from Raygun"` email subject** (`send.rs:39`) is a bug in a
   general-purpose tool and `{{COMPANY}}` plumbs the value that fixes it. Fold
   it in, or file separately?
5. **Ordering against 68.2.** Landing preview first makes template iteration
   usable on day one and saves a rewire. Confirm 68.2 → 68.3?
6. **PDF follow-up task.** Should this spec's recommendation be filed as a new
   subtask under epic 68, and does it want the settings-file shape or the
   HTML-to-PDF shape? (Not filed here — no `backlog` writes were made.)
7. **Unknown placeholders as a hard error.** The strictest of the choices here.
   The alternative is a warning on stderr plus verbatim output, which keeps a
   typo from blocking a send at the cost of letting `{{TOTL}}` reach a client.
