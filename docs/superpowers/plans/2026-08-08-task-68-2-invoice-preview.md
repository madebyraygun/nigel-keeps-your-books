# `nigel invoice preview` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `nigel invoice preview <number>` renders the same HTML (and PDF, when the `pdf` feature is on) that `send` would publish, to local files, with zero network calls and no invoicing config required — per `docs/superpowers/specs/2026-08-08-task-68-2-invoice-preview-design.md`.

**Architecture:** A three-state `PayButton` enum replaces `render_invoice_html`'s `Option<&str>` pay parameter. A new `src/invoicing/render.rs` holds `render_invoice` — the seam that turns an invoice into `RenderedInvoice { html, pdf: Option<Vec<u8>> }`, extracted out of `send.rs` so `send` and `preview` cannot drift. `src/cli/invoice.rs` gains `preview()`, which resolves paths, calls the seam, writes files, and prints. `src/cli/mod.rs` gains the clap variant; `src/main.rs` dispatches it **and adds it to the launch-sync skip list**.

**Tech Stack:** Rust, clap (derive), rusqlite, printpdf (feature-gated), assert_cmd/predicates/tempfile.

## Global Constraints

- After every task: `cargo test -- --test-threads=1`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` clean.
- **Every task must also pass without the `pdf` feature** — `cargo test --no-default-features --features gusto -- --test-threads=1`. That is the whole reason `pdf` is an `Option` in the seam.
- `src/invoicing/` never reads `settings` and never reaches into `src/cli/`. Config resolution stays in `src/cli/invoice.rs`.
- Preview writes nothing to the database and makes no network call: no `refresh_status`, no `mark_published`, no gateway/publisher/mailer construction.
- The seam takes data, never a template — TASK-68.3 must land entirely below it.
- `src/invoicing/templates/invoice.html` is **not touched** by this task.

---

### Task 1: `PayButton` — a third state for the pay element

**Files:** modify `src/invoicing/render_html.rs` (enum, signature, tests); `src/invoicing/send.rs` (one call site).

**Interface produced** (consumed by Tasks 2 and 3):

```rust
pub enum PayButton<'a> { Link(&'a str), Placeholder, Omitted }

pub fn render_invoice_html(
    invoice: &Invoice, client: &Client, items: &[InvoiceLineItem],
    pay: PayButton<'_>, contact_email: &str,
) -> String;
```

- [ ] **Step 1: Write failing tests** in the existing `mod tests` (the `sample()` helper is already there). First migrate the five existing tests to the new signature — `Some(url)` → `PayButton::Link(url)`, `None` → `PayButton::Omitted` — with **no change to their assertions**. That invariance is the point: the two old states behave exactly as before. Then add:

```rust
#[test]
fn placeholder_renders_an_inert_span_not_a_link() {
    let (inv, client, items) = sample();
    let html = render_invoice_html(&inv, &client, &items, PayButton::Placeholder, "b@e.test");
    assert!(html.contains("<span class=\"pay pay-placeholder\""));
    assert!(html.contains("link created when the invoice is sent"), "got: {html}");
    assert!(!html.contains("<a class=\"pay\""));
    assert!(!html.contains("href"), "a placeholder must not be clickable");
}

#[test]
fn only_the_pay_element_differs_between_link_and_placeholder() {
    let (inv, client, items) = sample();
    let linked = render_invoice_html(&inv, &client, &items, PayButton::Link("https://pay.test/x"), "b@e.test");
    let pending = render_invoice_html(&inv, &client, &items, PayButton::Placeholder, "b@e.test");
    let strip = |s: &str| s.lines().filter(|l| !l.contains("class=\"pay")).collect::<Vec<_>>().join("\n");
    assert_eq!(strip(&linked), strip(&pending)); // AC #3 in one assertion
}
```

`{{PAY}}` sits alone on its own line in the template, which is what makes the line filter work — if that changes, this test is the thing that notices.

- [ ] **Step 2: Verify they fail.** `cargo test --lib render_html 2>&1 | tail -20` — compile errors, the enum does not exist.

- [ ] **Step 3: Implement.** Add the enum above `render_invoice_html` with a doc comment naming when each variant applies, and replace the `let pay = match pay_url {…}` block:

```rust
let pay = match pay {
    PayButton::Link(url) => format!("<a class=\"pay\" href=\"{}\">Pay online</a>", esc(url)),
    PayButton::Placeholder => "<span class=\"pay pay-placeholder\" style=\"background:#777;cursor:default\">Pay online — link created when the invoice is sent</span>".to_string(),
    PayButton::Omitted => String::new(),
};
```

The style is inline rather than a new rule in the template, so the placeholder renders correctly against the custom templates TASK-68.3 will allow. See the spec's "The Pay button".

- [ ] **Step 4: Fix the `send.rs` call site** so the tree compiles and every send test still passes at this commit (this shim disappears in Task 2):

```rust
let pay = match pay_url.as_deref() {
    Some(url) => PayButton::Link(url),
    None => PayButton::Omitted,
};
let html = render_invoice_html(&invoice, &client, &items, pay, contact_email);
```

- [ ] **Step 5: Verify.** Both feature builds green; clippy and fmt clean.

---

### Task 2: Extract the render seam (`src/invoicing/render.rs`)

**Files:** create `src/invoicing/render.rs`; modify `src/invoicing/mod.rs` (`pub mod render;`, alphabetical, between `r2` and `render_html`); modify `src/invoicing/send.rs` (call the seam, delete its private `render_pdf` pair).

**Interface produced** (consumed by Task 3):

```rust
pub struct RenderedInvoice { pub html: String, pub pdf: Option<Vec<u8>> }

pub fn render_invoice(
    conn: &Connection, invoice: &Invoice, client: &Client,
    pay: PayButton<'_>, contact_email: &str,
) -> Result<RenderedInvoice>;
```

- [ ] **Step 1: Write failing tests** in a new `#[cfg(test)] mod tests` in `render.rs`. Copy `test_conn()` and `seed()` from `send.rs`'s test module (tempdir + `get_connection` + `init_db` + `run_migrations`; `add_client` + `create_invoice` with one 100.00 line item). **These tests are not gated on `pdf`** — that is the deliberate difference from `send.rs`'s module: the seam must be exercised in both builds.

```rust
#[test] fn renders_the_html_send_would_publish() {
    // out.html contains "Invoice #1248", "Contact ap@acme.test", "100.00"
}
#[test] fn line_items_come_from_the_database_in_position_order() {
    // seed extra items at positions 1 and 2; assert their find() indices ascend in out.html
}
#[test] fn rendering_writes_nothing_to_the_invoice() {
    // after render: status still "draft", published_at none, stripe_payment_link_url none
}
#[cfg(feature = "pdf")]
#[test] fn pdf_is_rendered_when_the_feature_is_on() { /* out.pdf.unwrap().starts_with(b"%PDF") */ }
#[cfg(not(feature = "pdf"))]
#[test] fn pdf_is_none_without_the_feature_and_html_still_renders() { /* out.pdf.is_none(), html non-empty */ }
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib invoicing::render 2>&1 | tail -20`

- [ ] **Step 3: Implement `render.rs`.** Move both `#[cfg]` halves of `render_pdf` out of `send.rs`, changing the `not(feature = "pdf")` half from `Err(...)` to `Ok(None)`:

```rust
/// Everything `invoice send` publishes for one invoice.
pub struct RenderedInvoice {
    pub html: String,
    /// `None` only in a build without the `pdf` feature; each caller decides
    /// whether that is fatal (`send`) or a notice (`preview`).
    pub pdf: Option<Vec<u8>>,
}

/// Render an invoice exactly as `send` publishes it. Reads the database, makes
/// no network call, and writes nothing — the single seam `send` and `preview`
/// share so a preview can never disagree with what a client receives.
pub fn render_invoice(
    conn: &Connection, invoice: &Invoice, client: &Client,
    pay: PayButton<'_>, contact_email: &str,
) -> Result<RenderedInvoice> {
    let items = line_items(conn, invoice.id)?;
    let html = render_invoice_html(invoice, client, &items, pay, contact_email);
    let pdf = render_pdf(invoice, client, &items)?;
    Ok(RenderedInvoice { html, pdf })
}

#[cfg(feature = "pdf")]
fn render_pdf(i: &Invoice, c: &Client, items: &[InvoiceLineItem]) -> Result<Option<Vec<u8>>> {
    crate::pdf::render_invoice_pdf(i, c, items).map(Some)
}

#[cfg(not(feature = "pdf"))]
fn render_pdf(_i: &Invoice, _c: &Client, _items: &[InvoiceLineItem]) -> Result<Option<Vec<u8>>> {
    Ok(None)
}
```

The seam loads line items itself so both callers get the same rows in the same order.

- [ ] **Step 4: Rewrite `send_invoice`'s middle** (replacing today's lines 33–40) and delete the Task 1 shim plus `send.rs`'s own `render_pdf` pair:

```rust
let pay = match pay_url.as_deref() {
    Some(url) => PayButton::Link(url),
    None => PayButton::Omitted,
};
let rendered = render_invoice(conn, &invoice, &client, pay, contact_email)?;
let pdf = rendered.pdf.ok_or_else(|| {
    NigelError::Other("PDF support not compiled in (build with --features pdf)".into())
})?;

let public_url = publisher.publish(&invoice.token, rendered.html.as_bytes(), &pdf)?;

let subject = format!("Invoice #{} from Raygun", invoice.number);
mailer.send_invoice(&email, &subject, &rendered.html, &pdf)?;
```

Send keeps its own error sentence verbatim, so behavior in a `pdf`-less build is unchanged. Drop the now-unused `render_invoice_html` and `line_items` imports from `send.rs`.

- [ ] **Step 5: Verify.** All four existing `send.rs` tests must pass **untouched** — especially `published_html_carries_the_supplied_contact_email` and `publish_failure_leaves_draft_and_sends_no_email`. If either needed editing, the extraction changed behavior and is wrong. Then both feature builds, clippy, fmt.

---

### Task 3: The `preview` command

**Files:** modify `src/cli/invoice.rs`, `src/cli/mod.rs`, `src/main.rs`.

**Interfaces** (the first four are pure and unit-tested):

```rust
fn preview_dir(output_dir: Option<String>) -> (PathBuf, bool);  // (dir, is_default)
fn preview_paths(dir: &Path, number: i64) -> (PathBuf, PathBuf); // (html, pdf)
fn pay_button_for(invoice: &Invoice) -> PayButton<'_>;
fn contact_email_for_preview(cfg: &InvoicingConfig) -> (String, bool); // (value, is_placeholder)
pub fn preview(number: i64, output_dir: Option<String>) -> Result<()>;
```

- [ ] **Step 1: Write failing unit tests** in `src/cli/invoice.rs`'s `mod tests` (helpers `test_conn()`, `seed_invoice()`, `test_config()` already exist):

```rust
#[test] fn preview_paths_are_stable_and_undated() {
    let (html, pdf) = preview_paths(Path::new("/tmp/p"), 1248);
    assert_eq!(html, Path::new("/tmp/p/invoice-1248.html"));
    assert_eq!(pdf,  Path::new("/tmp/p/invoice-1248.pdf"));
}

#[test] fn explicit_output_dir_wins_and_is_not_the_default() {
    let (dir, is_default) = preview_dir(Some("/tmp/elsewhere".into()));
    assert_eq!(dir, PathBuf::from("/tmp/elsewhere"));
    assert!(!is_default, "a directory the user named is not re-permissioned");
    let (dir, is_default) = preview_dir(None);
    assert!(is_default && dir.ends_with("previews"));
}

#[test] fn a_draft_with_no_link_gets_the_placeholder_button() { /* matches!(…, PayButton::Placeholder) */ }

#[test] fn a_sent_invoice_previews_with_its_real_link() {
    // set_payment_link(&conn, id, "pl_1", "https://pay/x") → PayButton::Link("https://pay/x")
}

#[test] fn a_void_invoice_never_renders_a_pay_button_even_with_a_live_link() {
    // set_payment_link, then UPDATE invoices SET status='void'
    // assert!(matches!(pay_button_for(&invoice), PayButton::Omitted),
    //         "a cancelled invoice must not offer a working payment link");
}

#[test] fn missing_from_email_becomes_a_flagged_placeholder() {
    let (value, placeholder) = contact_email_for_preview(&test_config());
    assert!(placeholder && value.contains("from_email"), "got: {value}");
    let cfg = InvoicingConfig { from_email: Some("billing@example.test".into()), ..test_config() };
    assert_eq!(contact_email_for_preview(&cfg), ("billing@example.test".to_string(), false));
}

#[test] fn preview_requires_no_invoicing_config_at_all() {
    assert!(build_clients(test_config()).is_err());               // send cannot run
    assert!(!contact_email_for_preview(&test_config()).0.is_empty()); // preview can
}
```

- [ ] **Step 2: Verify they fail.** `cargo test --lib cli::invoice 2>&1 | tail -20`

- [ ] **Step 3: Implement** in `src/cli/invoice.rs` (new imports: `std::path::PathBuf`, `crate::invoicing::render::render_invoice`, `crate::invoicing::render_html::PayButton`):

```rust
const PREVIEW_CONTACT_PLACEHOLDER: &str = "(from_email not configured)";

fn preview_dir(output_dir: Option<String>) -> (PathBuf, bool) {
    match output_dir {
        Some(dir) => (PathBuf::from(crate::settings::shellexpand_path(&dir)), false),
        None => (get_data_dir().join("previews"), true),
    }
}

fn preview_paths(dir: &Path, number: i64) -> (PathBuf, PathBuf) {
    (dir.join(format!("invoice-{number}.html")), dir.join(format!("invoice-{number}.pdf")))
}

fn pay_button_for(invoice: &Invoice) -> PayButton<'_> {
    if invoice.status == InvoiceStatus::Void.as_str() {
        return PayButton::Omitted;
    }
    match invoice.stripe_payment_link_url.as_deref() {
        Some(url) => PayButton::Link(url),
        None => PayButton::Placeholder,
    }
}

fn contact_email_for_preview(cfg: &InvoicingConfig) -> (String, bool) {
    match cfg.from_email.as_deref() {
        Some(email) => (email.to_string(), false),
        None => (PREVIEW_CONTACT_PLACEHOLDER.to_string(), true),
    }
}

pub fn preview(number: i64, output_dir: Option<String>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let client = get_client(&conn, invoice.client_id)?;

    if invoice.status == InvoiceStatus::Void.as_str() {
        eprintln!("notice: invoice #{number} is void — this preview is for reference only.");
    }
    let (contact_email, is_placeholder) = contact_email_for_preview(&invoicing_config());
    if is_placeholder {
        eprintln!("notice: from_email is not configured — the direct-deposit contact line is a placeholder");
    }

    // Render both before writing either, so a PDF failure cannot leave fresh
    // HTML beside a stale PDF.
    let rendered = render_invoice(&conn, &invoice, &client, pay_button_for(&invoice), &contact_email)?;

    let (dir, is_default) = preview_dir(output_dir);
    std::fs::create_dir_all(&dir)?;
    if is_default {
        crate::settings::restrict_dir_permissions(&dir)?;
    }
    let (html_path, pdf_path) = preview_paths(&dir, number);

    std::fs::write(&html_path, &rendered.html)?;
    crate::settings::restrict_file_permissions(&html_path)?;
    println!("Wrote {}", html_path.display());

    match rendered.pdf {
        Some(bytes) => {
            std::fs::write(&pdf_path, &bytes)?;
            crate::settings::restrict_file_permissions(&pdf_path)?;
            println!("Wrote {}", pdf_path.display());
        }
        None => eprintln!("notice: {}", crate::cli::report::PDF_DISABLED_MESSAGE),
    }
    Ok(())
}
```

- [ ] **Step 4: Add the clap variant** in `src/cli/mod.rs`'s `InvoiceCommands`, after `Show` and before `Send`:

```rust
/// Render an invoice to local files without sending it.
Preview {
    /// Invoice number (shown in `nigel invoice list`)
    number: i64,
    /// Directory to write into (default: <data_dir>/previews)
    #[arg(long)]
    output_dir: Option<String>,
},
```

- [ ] **Step 5: Dispatch in `src/main.rs`**, beside `Show`/`Send`:

```rust
InvoiceCommands::Preview { number, output_dir } => cli::invoice::preview(number, output_dir),
```

- [ ] **Step 6: Add it to the launch-sync skip list** in `src/main.rs` (the `if !matches!(command, …)` block above `sync_invoice_payments()`). Widen the existing arm:

```rust
    | Commands::Invoice {
        command: InvoiceCommands::Sync | InvoiceCommands::Preview { .. }
    }
```

and extend the comment above it: `…, and \`invoice preview\` because it is defined to make no network call at all`. **This step is acceptance criterion #2** — without it, a machine with `stripe_secret_key` set polls Stripe before every preview.

- [ ] **Step 7: Verify.** Both feature builds, clippy, fmt, plus `cargo run -- invoice preview --help`.

---

### Task 4: End-to-end integration tests

**Files:** modify `tests/cli_dispatch.rs`.

`TestEnv` already clears all nine `NIGEL_*` invoicing vars per command and points `HOME` at a temp dir, so **every test here runs with no invoicing config** — exactly AC #2's condition. Anything that reaches the network hangs into `TEST_TIMEOUT`.

- [ ] **Step 1: Add a seeding helper** near `client_add_and_list_roundtrip`:

```rust
/// init + demo, then one client and one 100.00 draft invoice (number 1248).
fn seed_invoice(env: &TestEnv) {
    env.init_and_demo();
    env.cmd().args(["client", "add", "Acme Co", "--email", "ap@acme.test"])
        .timeout(TEST_TIMEOUT).assert().success();
    env.cmd().args(["invoice", "new", "--client", "1", "--issue", "2026-08-04",
                    "--item", "Consulting:1:100"])
        .timeout(TEST_TIMEOUT).assert().success();
}
```

- [ ] **Step 2: Write the ungated tests.**

```rust
#[test] fn invoice_preview_writes_html_to_the_data_dir() {
    // stdout contains "invoice-1248.html"; the file contains "Invoice #1248" and "100.00"
}
#[test] fn invoice_preview_of_a_draft_shows_an_inert_pay_placeholder() {
    // html contains "pay-placeholder" and not "<a class=\"pay\""
}
#[test] fn invoice_preview_needs_no_invoicing_config_and_makes_no_network_call() {
    // success, and .stderr(predicate::str::contains("missing invoicing config").not())
}
#[test] fn invoice_preview_leaves_the_invoice_a_draft() {
    // SELECT status, published_at FROM invoices WHERE number = 1248 → ("draft", None)
}
#[test] fn invoice_preview_honors_output_dir() {
    // --output-dir <temp>/elsewhere: files land there, stdout names it,
    // and <data_dir>/previews does not exist
}
#[test] fn invoice_preview_overwrites_in_place_on_a_second_run() {
    // run twice, then read_dir(previews) → exactly the expected file count, no dated names
}
#[test] fn invoice_preview_of_an_unknown_number_fails_with_the_shared_message() {
    // ["invoice","preview","9999"] → failure, stderr contains "No invoice #9999"
}
```

- [ ] **Step 3: Write the void test.** TASK-68.1 has not shipped `invoice void` yet, so set the status through `env.db()` — the same trick the unit test `void_invoices_are_refused_before_any_network_call_or_payment` uses:

```rust
#[test]
fn invoice_preview_of_a_void_invoice_warns_and_omits_the_pay_button() {
    let env = TestEnv::new();
    seed_invoice(&env);
    env.db().execute(
        "UPDATE invoices SET status='void', stripe_payment_link_url='https://pay/x' WHERE number=1248", []
    ).unwrap();
    env.cmd().args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT).assert().success()
        .stderr(predicate::str::contains("is void"));
    let html = std::fs::read_to_string(env.data_dir().join("previews/invoice-1248.html")).unwrap();
    assert!(!html.contains("https://pay/x"), "a void invoice must not publish a live payment link");
    assert!(!html.contains("Pay online"));
}
```

- [ ] **Step 4: Write the two feature-gated tests.**

```rust
#[cfg(feature = "pdf")]
#[test] fn invoice_preview_writes_a_real_pdf() {
    // stdout contains "invoice-1248.pdf"; the bytes start with b"%PDF"
}

#[cfg(not(feature = "pdf"))]
#[test] fn invoice_preview_without_the_pdf_feature_still_writes_html_and_says_why() {
    // .assert().success()  ← exit 0, not a failure
    // .stderr(contains("PDF export requires the 'pdf' feature"))
    // previews/invoice-1248.html exists; previews/invoice-1248.pdf does not
}
```

- [ ] **Step 5: Verify both builds** — the gate means each runs a different subset, and both must be green:

```bash
cargo test --test cli_dispatch -- --test-threads=1
cargo test --no-default-features --features gusto --test cli_dispatch -- --test-threads=1
```

---

### Task 5: Documentation

Per CLAUDE.md's Documentation Policy the work is not complete until these land.

- [ ] **Step 1: `CLAUDE.md` Commands block** — after the `nigel invoice show 1248` line:

```
nigel invoice preview 1248                        # Render HTML/PDF locally, no network (<data_dir>/previews)
nigel invoice preview 1248 --output-dir /tmp      # Write the preview somewhere else
```

- [ ] **Step 2: `CLAUDE.md` Architecture** — amend the **Invoicing** bullet so the seam is described where someone will look for it: `render_html.rs` + `templates/invoice.html` (`{{KEY}}` expansion; `PayButton` renders a live link, an inert placeholder, or nothing), `render.rs` (`render_invoice` — the one seam turning an invoice into the HTML+PDF pair `send` publishes and `preview` writes locally; `pdf: None` without the feature), `send.rs` (Stripe link → render → R2 publish → …).

- [ ] **Step 3: `CLAUDE.md` Key Design Constraints** — add: *`invoice preview` renders through the same `invoicing::render::render_invoice` seam `send` publishes through, so the two cannot drift; it makes no network call (it is in `main.rs`'s launch-sync skip list), needs no invoicing config, and writes nothing to the database. The only differences from a published invoice are the Pay placeholder on an unsent draft and the absent PDF in a build without the `pdf` feature.*

- [ ] **Step 4: `README.md`** — add `nigel invoice preview 1248` to the invoicing command list with a one-line gloss.

- [ ] **Step 5: `docs/invoicing.md`** — new `## Previewing` section between "Creating an invoice" and "Sending", covering: the command and both flag forms; the default location and the stable, undated filenames; the Pay-button table (link / placeholder / omitted-when-void); that it needs no configuration and makes no network call — the one invoicing command that works on a fresh install; that an unset `from_email` renders a placeholder in the direct-deposit line; and that a build without `pdf` writes HTML only, with the same sentence `nigel report` prints. Also amend the intro line "Sending requires a build with the `pdf` feature (the default)" to note `preview` is the exception that degrades to HTML instead of stopping.

- [ ] **Step 6: Verify.** `git diff --stat` shows all three docs touched; re-read the "Sending" section to confirm its numbered publish steps still read correctly now that step 2 is a named seam.

---

## Final verification

- [ ] `cargo test -- --test-threads=1`
- [ ] `cargo test --no-default-features --features gusto -- --test-threads=1`
- [ ] `cargo test --no-default-features -- --test-threads=1`
- [ ] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- [ ] Manual, against a scratch data dir with **no** invoicing config exported:
```bash
cargo run -- invoice preview 1248
xdg-open ~/Documents/nigel/previews/invoice-1248.html   # eyeball the placeholder button
```
- [ ] `git diff src/invoicing/send.rs` changes *only* the render block and its imports — nothing in the gateway call, the publish, the email, or `mark_published`.
- [ ] `git diff src/invoicing/templates/invoice.html` is empty (TASK-68.3 owns that file).

## Acceptance criteria mapping

| AC | Verified by |
|---|---|
| #1 writes HTML (and PDF when the feature is on) and prints the paths | Task 4 `invoice_preview_writes_html_to_the_data_dir`, `invoice_preview_writes_a_real_pdf`, `invoice_preview_without_the_pdf_feature_still_writes_html_and_says_why` |
| #2 no network calls, works with no invoicing config | Task 3 Step 6 (launch-sync skip list) and `preview_requires_no_invoicing_config_at_all`; Task 4 `invoice_preview_needs_no_invoicing_config_and_makes_no_network_call` (a network call hits `TEST_TIMEOUT`) |
| #3 output matches what send would publish, modulo the payment-link placeholder | Task 2 (one shared seam — `send` has no render code of its own left); Task 1 `only_the_pay_element_differs_between_link_and_placeholder` |
