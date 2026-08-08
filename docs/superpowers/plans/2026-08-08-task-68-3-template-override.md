# Invoice template override — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to work this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `<data_dir>/templates/invoice.html` overrides the embedded invoice page, the default is exportable as a starting point, the vocabulary is documented, and expansion stays injection-safe with an untrusted template — per `docs/superpowers/specs/2026-08-08-task-68-3-template-override-design.md`.

**Architecture:** All loading and validation lives in `src/invoicing/render_html.rs` behind `load_template(data_dir)`; the invoicing module never reads settings, so `cli::invoice` resolves the data directory and the company name and passes them down in a `Branding` struct. `send_invoice` takes `&Branding` in place of `contact_email: &str`, and the load happens in the CLI before `build_clients`, so a broken template fails a send before Stripe is touched.

**Tech stack:** Rust, clap (derive), rusqlite, assert_cmd/predicates/tempfile.

## Global constraints

- After every task: `cargo test -- --test-threads=1`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. Also `cargo test --no-default-features -- --test-threads=1` after Task 4 (the `send.rs` signature change is behind the `pdf` feature's test gate).
- `expand` stays **single-pass**. No second pass, no expand-until-stable loop, no regex replace-all. Substituted values are never rescanned; that is what keeps a client named `Acme {{ROWS}} Co` literal text.
- `esc` is unchanged. Every value entering the vars table is escaped or is a fragment built from escaped parts.
- No sanitization of template markup. The operator's `<script>` is the feature.
- Validation runs at load, never at render. `render_invoice_html` stays infallible.
- Tasks 7 and 8 are severable (see spec open question 2); tasks 1–6 and 9 are the task's acceptance criteria.

---

### Task 1: Vocabulary constant and load-time validation (`src/invoicing/render_html.rs`)

**Files:** modify `src/invoicing/render_html.rs`.

**Interfaces produced:**

```rust
pub const DEFAULT_TEMPLATE: &str = include_str!("templates/invoice.html");
pub const PLACEHOLDERS: &[&str] = &["NUMBER", "CLIENT", "ISSUE", "DUE", "ROWS",
                                    "CURRENCY", "TOTAL", "PAY", "CONTACT"];
const REQUIRED: &[&str] = &["NUMBER", "CLIENT", "ROWS", "TOTAL"];
const MAX_TEMPLATE_BYTES: usize = 1024 * 1024;

/// Errors naming `path` when the template is empty, oversized, missing a
/// required placeholder, or uses one that is not in `PLACEHOLDERS`.
fn validate_template(source: &str, path: &Path) -> Result<()>;

/// Placeholder tokens present in `source`: `{{` + SCREAMING_SNAKE + `}}` only.
fn placeholder_tokens(source: &str) -> Vec<&str>;
```

- [ ] **Step 1 — failing tests.** In `render_html.rs` `mod tests`, using `Path::new("/tmp/t.html")` as the reported path:

```rust
#[test] fn the_default_template_validates() // validate_template(DEFAULT_TEMPLATE, p).is_ok()
#[test] fn an_empty_or_whitespace_template_is_rejected()      // "", "\n \t\n" -> err contains "is empty" and the path
#[test] fn an_oversized_template_is_rejected()                // "x".repeat(MAX+1) -> err contains "1 MiB" and the path
#[test] fn a_template_missing_a_required_placeholder_is_rejected()
    // "<p>{{NUMBER}} {{CLIENT}} {{ROWS}}</p>" -> err contains "{{TOTAL}}" and "missing required"
#[test] fn a_template_with_an_unknown_placeholder_is_rejected()
    // "...{{TOTL}}..." -> err contains "{{TOTL}}" and lists known placeholders
#[test] fn non_placeholder_braces_are_left_alone()
    // "{{ not a key }} {{lower}} {{ }} {{" alongside the four required -> Ok
#[test] fn placeholder_tokens_finds_each_key_once_per_occurrence()
```

- [ ] **Step 2 — run, verify failure to compile.** `cargo test --lib render_html 2>&1 | tail -20`
- [ ] **Step 3 — implement.** `placeholder_tokens` scans for `{{`, requires the next byte to be `A-Z` or `_`, accepts `[A-Z0-9_]*`, and requires `}}` immediately after; anything else is skipped past the `{{` and ignored. `validate_template` checks, in order: byte length, `trim().is_empty()`, required-set difference, unknown-token difference. All failures are `NigelError::Invalid` with the path in the sentence.
- [ ] **Step 4 — verify.** `cargo test --lib render_html -- --test-threads=1`, clippy, fmt.

**Verify:** every rejection message contains the path; the shipped default passes.

---

### Task 2: `load_template` and `template_path` (`src/invoicing/render_html.rs`)

**Files:** modify `src/invoicing/render_html.rs`.

**Interfaces produced:**

```rust
pub fn template_path(data_dir: &Path) -> PathBuf;          // data_dir/templates/invoice.html
pub fn load_template(data_dir: &Path) -> Result<Cow<'static, str>>;
```

- [ ] **Step 1 — failing tests.** With `tempfile::tempdir()` as the data dir:

```rust
#[test] fn no_override_falls_back_to_the_embedded_default()
    // load_template(dir) == DEFAULT_TEMPLATE, and is Cow::Borrowed
#[test] fn an_override_file_wins_over_the_default()
    // write templates/invoice.html with the four required keys -> loaded content is that file
#[test] fn an_unreadable_override_errors_naming_the_path()
    // create templates/invoice.html as a *directory* -> err contains the path, is not the default
#[test] fn an_invalid_override_errors_rather_than_falling_back()
    // write "<p>hello</p>" -> err mentions "{{NUMBER}}"; assert the result is Err, not DEFAULT_TEMPLATE
#[test] fn template_path_is_templates_invoice_html()
```

- [ ] **Step 2 — run, verify failure.**
- [ ] **Step 3 — implement.** `load_template`: if `!path.exists()` return `Cow::Borrowed(DEFAULT_TEMPLATE)`. Otherwise `fs::read_to_string`, mapping the io error to `NigelError::Invalid(format!("Cannot read invoice template {}: {e}", path.display()))` — an unadorned `NigelError::Io` would print no path. Then `validate_template(&source, &path)?` and return `Cow::Owned`.

  Read-then-check order matters for the size cap: check `fs::metadata(&path)?.len()` **before** reading, so an accidental 2 GB file is never pulled into memory. Do the length check in `load_template` and keep the string-length check in `validate_template` for the direct-call tests.
- [ ] **Step 4 — verify.** Tests, clippy, fmt.

**Verify:** an existing-but-broken override never yields the default. Assert that explicitly, not just that it errors.

---

### Task 3: `Branding` and the render signature (`src/invoicing/render_html.rs`)

**Files:** modify `src/invoicing/render_html.rs`.

**Interfaces produced:**

```rust
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

`company` is carried now and consumed in Task 7; if Task 7 is dropped, `Branding` still earns its place by collapsing the parameter list.

- [ ] **Step 1 — failing tests.** Update the five existing `render_invoice_html` call sites in `mod tests` to the new signature (they will not compile until Step 3), and add:

```rust
#[test] fn a_custom_template_renders_instead_of_the_default()
    // template "<h1>{{NUMBER}}</h1>{{CLIENT}}{{ROWS}}{{TOTAL}}" -> no "Direct deposit"
#[test] fn a_client_name_that_looks_like_a_placeholder_stays_literal_in_a_custom_template()
    // custom template containing {{ROWS}} once; client.name = "Acme {{ROWS}} {{PAY}} Co"
    // -> html contains that name literally; html.matches("<tr>").count() == items.len()
#[test] fn a_custom_template_can_place_a_value_in_a_quoted_attribute()
    // template: <span title="{{CLIENT}}">; client.name = r#"a" onmouseover="x"#
    // -> contains "&quot;" and does NOT contain `" onmouseover="`
#[test] fn operator_markup_in_a_template_is_not_sanitized()
    // template containing <script>alert(1)</script> -> renders verbatim
```

- [ ] **Step 2 — run, verify failure.**
- [ ] **Step 3 — implement.** Replace `TEMPLATE` with `branding.template` in the `expand` call and thread `branding.contact_email` into `CONTACT`. **Do not touch `expand` or `esc`.** Keep `DEFAULT_TEMPLATE` public so `send`'s tests and the export command can reach it.
- [ ] **Step 4 — verify.** Tests, clippy, fmt.

**Verify:** the literal-placeholder test is the injection-safety regression guard — it must assert the row count, not just the substring.

---

### Task 4: Thread `Branding` through `send_invoice` (`src/invoicing/send.rs`)

**Files:** modify `src/invoicing/send.rs`.

- [ ] **Step 1 — failing tests.** Update the four tests in `send.rs`'s `#[cfg(all(test, feature = "pdf"))] mod tests` to build a `Branding { template: DEFAULT_TEMPLATE, company: "", contact_email: "billing@example.test" }`, and add:

```rust
#[test] fn send_renders_with_the_supplied_template()
    // Branding.template = "<p>CUSTOM {{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>"
    // CapturePub sees "CUSTOM" and does not see "Direct deposit"
```

- [ ] **Step 2 — run, verify failure.**
- [ ] **Step 3 — implement.** `send_invoice(conn, invoice_id, today, branding: &Branding<'_>, gateway, publisher, mailer)`. Nothing else in the function changes.
- [ ] **Step 4 — verify.** `cargo test -- --test-threads=1` **and** `cargo test --no-default-features -- --test-threads=1`.

**Verify:** `send_invoice` still opens no files and reads no settings — the template arrives as a `&str`.

---

### Task 5: CLI wiring for `send` (`src/cli/invoice.rs`)

**Files:** modify `src/cli/invoice.rs`.

- [ ] **Step 1 — failing tests.** In `tests/cli_dispatch.rs`, using the existing `TestEnv` (which already strips every `NIGEL_*` credential, so no send can reach the network):

```rust
#[test] fn send_with_a_broken_template_fails_before_touching_stripe()
    // init, client add, invoice new, write an invalid templates/invoice.html,
    // then `invoice send 1248` -> failure whose stderr names the template path,
    // and the invoice row is still status 'draft'
```

- [ ] **Step 2 — run, verify failure.**
- [ ] **Step 3 — implement.** In `cli::invoice::send`, before `build_clients`:

```rust
let template = load_template(&get_data_dir())?;
let cfg = invoicing_config();
let (stripe, r2, mail) = build_clients(cfg)?;
let branding = Branding { template: &template, company: "", contact_email: &mail.from };
```

  Order matters: the template load comes first so a broken template errors even when invoicing config is also missing, and so nothing is constructed before the failure.
- [ ] **Step 4 — verify.** Integration test passes; `cargo test -- --test-threads=1`.

**Verify:** stderr names the path, and the DB still shows `draft`.

---

### Task 6: `invoice template export|path` (`src/cli/mod.rs`, `src/cli/invoice.rs`, `src/main.rs`)

**Files:** modify `src/cli/mod.rs`, `src/cli/invoice.rs`, `src/main.rs`.

**Interfaces produced:**

```rust
// src/cli/mod.rs
pub enum InvoiceCommands { /* … */ Template {
    #[command(subcommand)] command: InvoiceTemplateCommands,
} }

#[derive(Subcommand)]
pub enum InvoiceTemplateCommands {
    /// Write the built-in invoice template out as a starting point.
    Export {
        /// Destination (default: <data_dir>/templates/invoice.html)
        #[arg(long)] output: Option<String>,
        /// Overwrite an existing file
        #[arg(long)] force: bool,
    },
    /// Show where Nigel looks for a custom invoice template.
    Path,
}

// src/cli/invoice.rs
pub fn template_export(output: Option<&str>, force: bool) -> Result<()>;
pub fn template_show_path() -> Result<()>;
```

- [ ] **Step 1 — failing tests.** In `tests/cli_dispatch.rs`:

```rust
#[test] fn template_export_writes_the_default_and_reports_where()
    // fresh HOME, NO `nigel init` -> exits 0, prints the path, file == DEFAULT_TEMPLATE bytes
#[test] fn template_export_refuses_to_clobber_without_force()
    // export, overwrite file with "MINE", export again -> fails, file still "MINE";
    // export --force -> succeeds, file is the default again
#[test] fn template_export_honors_output()
    // --output <tmp>/custom.html
#[test] fn template_path_reports_absent_then_present_then_broken()
    // no file -> "no custom template"; valid file -> "in effect";
    // broken file -> names the validation error, exit code non-zero
```

  The no-`nigel init` case is the point of the first test: it proves the command is exempt from the `needs_existing_db` / `needs_password` pre-flight.

- [ ] **Step 2 — run, verify failure.**
- [ ] **Step 3 — implement.**
  - `cli/mod.rs`: the two enums above.
  - `cli/invoice.rs`: `template_export` resolves the destination (`output` via `settings::shellexpand_path`, else `template_path(&get_data_dir())`), errors when it exists and `!force` (`NigelError::Invalid` naming the path and suggesting `--force`), `fs::create_dir_all` the parent, writes `DEFAULT_TEMPLATE`, prints `Wrote invoice template to <path>` plus a one-line pointer to `docs/invoicing.md`. `template_show_path` prints the path, then either `No custom template — the built-in one is in use.` or, on a present file, calls `load_template` and prints `Custom template in effect.` or propagates the validation error.
  - `main.rs`: dispatch the two variants, and add `Commands::Invoice { command: InvoiceCommands::Template { .. } }` to the `needs_existing_db` guard (`main.rs:66`), the `needs_password` guard (`main.rs:74`), and the Stripe-sync exemption (`main.rs:115`). All three are `matches!` blocks that already carry a nested `InvoiceCommands::Sync` arm to copy.
- [ ] **Step 4 — verify.** Integration tests, `cargo test -- --test-threads=1`, clippy, fmt.

**Verify:** run `nigel invoice template export` by hand against a scratch `HOME` and confirm the exported file round-trips: `nigel invoice template path` reports it in effect, unedited.

---

### Task 7 (severable): `{{COMPANY}}` and the email subject

**Files:** modify `src/invoicing/render_html.rs`, `src/invoicing/send.rs`, `src/cli/invoice.rs`.

Skip this task and Task 8 if the orchestrator answers spec question 2 with "ship the existing nine".

- [ ] **Step 1 — failing tests.**

```rust
// render_html.rs
#[test] fn company_renders_and_is_escaped()      // Branding.company = "A & B <Co>" -> "A &amp; B"
#[test] fn company_renders_empty_when_unset()
// send.rs
#[test] fn the_subject_names_the_company_when_there_is_one()   // "Invoice #1248 from Acme LLC"
#[test] fn the_subject_omits_the_from_clause_when_there_is_none() // "Invoice #1248"
```

- [ ] **Step 2 — run, verify failure.**
- [ ] **Step 3 — implement.** Add `"COMPANY"` to `PLACEHOLDERS` and `("COMPANY", &esc(branding.company))` to the vars table. In `send.rs` replace the hardcoded `format!("Invoice #{} from Raygun", …)` with a company-aware subject. In `cli::invoice::send`, resolve `db::get_metadata(&conn, "company_name").unwrap_or_default()` into `Branding.company`.
- [ ] **Step 4 — verify.** Full test run.

**Verify:** the string `Raygun` no longer appears in `src/` — `rg -n 'Raygun' src/` should return nothing outside comments.

---

### Task 8 (severable): the rest of the vocabulary

**Files:** modify `src/invoicing/render_html.rs`, `src/invoicing/templates/invoice.html`.

Adds `CLIENT_EMAIL`, `CLIENT_ADDRESS`, `DUE_DATE`, `SUBTOTAL`, `TAX`, `NOTES`, `TERMS`, `PAY_URL`.

- [ ] **Step 1 — failing tests.**

```rust
#[test] fn every_placeholder_in_the_vocabulary_expands()
    // template built by joining PLACEHOLDERS as "{{K}}" -> no "{{" survives in the output
#[test] fn optional_values_render_empty_rather_than_an_empty_tag()
    // client with no email/address, invoice with no due date/notes/terms/pay link
    // -> the fragments contribute nothing; assert no "<p></p>" and no "Due:"
#[test] fn notes_and_terms_are_escaped()          // notes = "<b>x</b>" -> "&lt;b&gt;"
#[test] fn due_date_is_the_bare_date_and_due_is_the_fragment()
#[test] fn the_default_template_still_validates() // guards the invoice.html edit
```

- [ ] **Step 2 — run, verify failure.**
- [ ] **Step 3 — implement.** Extend `PLACEHOLDERS` and the vars table. `NOTES`/`TERMS` follow the `{{DUE}}` pattern — a pre-built block (`<h3>Notes</h3><p>…</p>`) built from `esc`'d text, empty `String` when the field is `None`. `SUBTOTAL`/`TAX` use `format!("{:.2}", …)` like `TOTAL`. Add `{{NOTES}}` and `{{TERMS}}` to `src/invoicing/templates/invoice.html` after the total block.
- [ ] **Step 4 — verify.** Full test run, plus an eyeball: export the template, render a preview, open it.

**Verify:** the vocabulary constant, the vars table, and the docs table in Task 9 list the same keys. A test that iterates `PLACEHOLDERS` (the first test above) makes the first two agree mechanically.

---

### Task 9: Documentation

**Files:** modify `docs/invoicing.md`, `CLAUDE.md`.

- [ ] **Step 1 — write `docs/invoicing.md`.** A "Customizing the invoice page" section after "Sending", covering:
  - the override path, and that absence falls back to the built-in page;
  - `nigel invoice template export` / `nigel invoice template path`, with the `--output`/`--force` flags;
  - the full placeholder table from the spec, keeping the **text vs fragment** column;
  - the placement contract, verbatim from the spec: safe in element content and quoted attribute values; never inside `<script>`, `<style>`, an unquoted attribute, or a URL-scheme position; the fragment placeholders are content-only;
  - that a template which exists but is empty, oversized, missing `{{NUMBER}}`/`{{CLIENT}}`/`{{ROWS}}`/`{{TOTAL}}`, or using an unknown `{{KEY}}` is an error naming the path — Nigel will not quietly mail the stock page instead;
  - that values are always HTML-escaped and that the template itself is not sanitized, because whoever can write it already runs as you;
  - the preview loop (`export` → edit → `nigel invoice preview 1248`), if 68.2 has landed.
- [ ] **Step 2 — update `CLAUDE.md`.** Extend the invoicing bullet (line 33): `render_html.rs` gains the `<data_dir>/templates/invoice.html` override with load-time validation against a fixed placeholder vocabulary, a hard error rather than a fallback on a broken override, and `Branding` carrying template/company/contact into `send_invoice`. Add to the Key Behaviors list: *a custom invoice template is validated when it is loaded, not when it is rendered, so `preview` and `invoice template path` catch a typo before a client does.*
- [ ] **Step 3 — describe current state only.** No "added in 68.3", no migration notes, no before/after. `git log` is the audit trail.
- [ ] **Step 4 — verify.** Follow the doc literally in a scratch `HOME`: export, edit the heading, `invoice template path`, preview. Every command in the doc must run as written.

---

### Task 10: Wire `invoice preview` (conditional on 68.2)

**Files:** modify `src/cli/invoice.rs`.

Skip entirely if `cli::invoice::preview` does not exist yet — in that case note on task 68.2 that preview must call `load_template` rather than `DEFAULT_TEMPLATE`.

- [ ] **Step 1 — failing test.** `tests/cli_dispatch.rs`: with a custom template in place, `invoice preview 1248` writes HTML containing the custom marker; with a broken template it fails naming the path and writes nothing.
- [ ] **Step 2 — run, verify failure.**
- [ ] **Step 3 — implement.** Build the same `Branding` as `send` does, minus the mailer: `contact_email` from `invoicing_config().from_email.unwrap_or_default()`, since preview must work with no config set (68.2 AC #2).
- [ ] **Step 4 — verify.** Both preview tests, plus the whole suite.

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo fmt --check`
- [ ] Manual round trip against a scratch `HOME`: `nigel init`, `nigel demo`, `invoice template export`, edit the exported file, `invoice template path`, `invoice preview` (or a `send` against Stripe test keys), and confirm the edit appears.
- [ ] Confirm against the acceptance criteria: #1 override wins and absence falls back (Task 2); #2 vocabulary documented and default exportable (Tasks 6, 9); #3 expansion injection-safe with user templates (Task 3's literal-placeholder and quoted-attribute tests, plus the unchanged single-pass `expand`).
- [ ] `backlog task edit 68.3` to check off the criteria and set status — orchestrator's call, on `main`, not on a PR branch.
