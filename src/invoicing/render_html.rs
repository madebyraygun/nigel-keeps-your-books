use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::error::{NigelError, Result};
use crate::models::{Client, Invoice, InvoiceLineItem};

/// The page Nigel renders when the data directory holds no template of its own.
pub const DEFAULT_TEMPLATE: &str = include_str!("templates/invoice.html");

/// Every `{{KEY}}` a template may use. Anything else shaped like a placeholder
/// is a typo and is refused at load time.
pub const PLACEHOLDERS: &[&str] = &[
    "NUMBER",
    "CLIENT",
    "CLIENT_EMAIL",
    "CLIENT_ADDRESS",
    "COMPANY",
    "ISSUE",
    "DUE_DATE",
    "DUE",
    "ROWS",
    "CURRENCY",
    "SUBTOTAL",
    "TAX",
    "TOTAL",
    "NOTES",
    "TERMS",
    "PAY_URL",
    "PAY",
    "CONTACT",
];

/// What an invoice is: which invoice, who owes, for what, how much. A template
/// without these renders a document that is wrong about money.
const REQUIRED: &[&str] = &["NUMBER", "CLIENT", "ROWS", "TOTAL"];

const MAX_TEMPLATE_BYTES: usize = 1024 * 1024;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Expands `{{KEY}}` placeholders in a single left-to-right pass, so substituted
/// values are never re-scanned for further placeholders. Unknown placeholders are
/// emitted verbatim.
fn expand(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(open) = rest.find("{{") {
        let after_open = &rest[open + 2..];
        match after_open.find("}}").and_then(|close| {
            vars.iter()
                .find(|(k, _)| *k == &after_open[..close])
                .map(|(_, v)| (close, *v))
        }) {
            Some((close, value)) => {
                out.push_str(&rest[..open]);
                out.push_str(value);
                rest = &after_open[close + 2..];
            }
            None => {
                out.push_str(&rest[..open + 2]);
                rest = after_open;
            }
        }
    }

    out.push_str(rest);
    out
}

/// The placeholder keys `source` uses, in order, once per occurrence. The scan
/// is deliberately narrow — `{{` + SCREAMING_SNAKE + `}}` and nothing else — so
/// a CSS brace or a `{{ not a key }}` aside is literal text rather than a
/// validation failure.
fn placeholder_tokens(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;

    while let Some(open) = source[at..].find("{{") {
        let start = at + open + 2;
        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_uppercase()
                || bytes[end] == b'_'
                || (end > start && bytes[end].is_ascii_digit()))
        {
            end += 1;
        }
        if end > start && source[end..].starts_with("}}") {
            out.push(&source[start..end]);
            at = end + 2;
        } else {
            at = start;
        }
    }
    out
}

fn braced(keys: &[&str]) -> String {
    keys.iter()
        .map(|k| format!("{{{{{k}}}}}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Checked when a template is loaded, never when one is rendered, so a typo
/// surfaces on `invoice template path` or `invoice preview` rather than in a
/// client's inbox. `path` is named in every failure.
fn validate_template(source: &str, path: &Path) -> Result<()> {
    if source.len() > MAX_TEMPLATE_BYTES {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} is {} bytes; the limit is 1 MiB.",
            path.display(),
            source.len()
        )));
    }
    if source.trim().is_empty() {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} is empty.",
            path.display()
        )));
    }

    let found = placeholder_tokens(source);

    let missing: Vec<&str> = REQUIRED
        .iter()
        .filter(|k| !found.contains(*k))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} is missing required placeholder(s): {}. Known placeholders: {}.",
            path.display(),
            braced(&missing),
            braced(PLACEHOLDERS)
        )));
    }

    let mut unknown: Vec<&str> = Vec::new();
    for key in found {
        if !PLACEHOLDERS.contains(&key) && !unknown.contains(&key) {
            unknown.push(key);
        }
    }
    if !unknown.is_empty() {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} uses unknown placeholder(s): {}. Known placeholders: {}.",
            path.display(),
            braced(&unknown),
            braced(PLACEHOLDERS)
        )));
    }

    Ok(())
}

/// Where Nigel looks for an operator's own invoice page.
pub fn template_path(data_dir: &Path) -> PathBuf {
    data_dir.join("templates").join("invoice.html")
}

/// The operator's template when the file is there and valid, the embedded
/// default when it is not there at all. A file that exists but cannot be read
/// or does not validate is an error naming the path — never a silent fallback,
/// because the stock page would then reach a client nobody chose to send it to.
pub fn load_template(data_dir: &Path) -> Result<Cow<'static, str>> {
    let path = template_path(data_dir);
    let read_error = |e: std::io::Error| {
        NigelError::Invalid(format!(
            "Cannot read invoice template {}: {e}",
            path.display()
        ))
    };

    // Only a genuine "no such file" is an absent override. `Path::exists` would
    // answer false for a dangling symlink or an unreadable directory too, and
    // each of those would then render the stock page for someone who put a
    // template there on purpose.
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Cow::Borrowed(DEFAULT_TEMPLATE))
        }
        Err(e) => return Err(read_error(e)),
        Ok(_) => {}
    }

    // Sized before it is read, so a wrong file copied over the template cannot
    // be pulled into memory whole just to be rejected.
    let size = std::fs::metadata(&path).map_err(read_error)?.len();
    if size > MAX_TEMPLATE_BYTES as u64 {
        return Err(NigelError::Invalid(format!(
            "Invoice template {} is {size} bytes; the limit is 1 MiB.",
            path.display()
        )));
    }

    let source = std::fs::read_to_string(&path).map_err(read_error)?;
    validate_template(&source, &path)?;
    Ok(Cow::Owned(source))
}

/// Which pay element the page carries.
pub enum PayButton<'a> {
    /// A live payment link, as a sent invoice renders it.
    Link(&'a str),
    /// A draft that gets its link when it is sent: an inert stand-in showing
    /// where the button goes, with nothing to click.
    Placeholder,
    /// No link, and none coming — a void invoice, or a page that never had one.
    Omitted,
}

/// What the page says about the sender, as opposed to what it says about the
/// invoice. `src/invoicing/` reads no settings and no database, so the CLI layer
/// resolves all three and passes them in.
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
    pay: PayButton<'_>,
) -> String {
    let rows: String = items
        .iter()
        .map(|i| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                esc(&i.description),
                i.quantity,
                i.unit_amount,
                i.line_total
            )
        })
        .collect();

    // The placeholder styles itself inline instead of adding a rule to
    // `templates/invoice.html`, so it renders correctly against a custom
    // template that knows nothing about a `.pay-placeholder` class. The grey
    // carries `.pay`'s white text at 4.5:1, the WCAG AA floor.
    let pay_url = match &pay {
        PayButton::Link(url) => *url,
        PayButton::Placeholder | PayButton::Omitted => "",
    };
    let pay = match pay {
        PayButton::Link(url) => format!("<a class=\"pay\" href=\"{}\">Pay online</a>", esc(url)),
        PayButton::Placeholder => "<span class=\"pay pay-placeholder\" style=\"background:#767676;cursor:default\">Pay online — link created when the invoice is sent</span>".to_string(),
        PayButton::Omitted => String::new(),
    };
    let due_date = invoice.due_date.as_deref().map(esc).unwrap_or_default();
    let due = match due_date.as_str() {
        "" => String::new(),
        date => format!("<br>Due: {date}"),
    };

    // A block rather than a bare value, so "empty when unset" needs no
    // conditional in a template language that has none.
    let block = |heading: &str, text: Option<&String>| match text {
        Some(t) if !t.trim().is_empty() => format!("<h3>{heading}</h3><p>{}</p>", esc(t)),
        _ => String::new(),
    };

    expand(
        branding.template,
        &[
            ("NUMBER", &invoice.number.to_string()),
            ("CLIENT", &esc(&client.name)),
            (
                "CLIENT_EMAIL",
                &esc(client.email.as_deref().unwrap_or_default()),
            ),
            (
                "CLIENT_ADDRESS",
                &esc(client.billing_address.as_deref().unwrap_or_default()),
            ),
            ("COMPANY", &esc(branding.company)),
            ("ISSUE", &esc(&invoice.issue_date)),
            ("DUE_DATE", &due_date),
            ("DUE", &due),
            ("ROWS", &rows),
            ("CURRENCY", &esc(&invoice.currency)),
            ("SUBTOTAL", &format!("{:.2}", invoice.subtotal)),
            ("TAX", &format!("{:.2}", invoice.tax)),
            ("TOTAL", &format!("{:.2}", invoice.total)),
            ("NOTES", &block("Notes", invoice.notes.as_ref())),
            ("TERMS", &block("Terms", invoice.terms.as_ref())),
            ("PAY_URL", &esc(pay_url)),
            ("PAY", &pay),
            ("CONTACT", &esc(branding.contact_email)),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Client, Invoice, InvoiceLineItem};

    fn sample() -> (Invoice, Client, Vec<InvoiceLineItem>) {
        let inv = Invoice {
            id: 1,
            number: 1248,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: Some("2026-09-03".into()),
            status: "sent".into(),
            currency: "USD".into(),
            subtotal: 250.0,
            tax: 0.0,
            total: 250.0,
            notes: None,
            terms: None,
            token: "abc123".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: Some("2026-08-04".into()),
            voided_at: None,
        };
        let client = Client {
            id: 1,
            name: "Acme <Co>".into(),
            email: Some("a@b.test".into()),
            billing_address: None,
            notes: None,
        };
        let items = vec![InvoiceLineItem {
            id: None,
            invoice_id: Some(1),
            description: "Design".into(),
            quantity: 2.0,
            unit_amount: 100.0,
            line_total: 200.0,
            position: 0,
        }];
        (inv, client, items)
    }

    #[test]
    fn renders_number_total_items_and_pay_button() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("billing@example.test"),
            &inv,
            &client,
            &items,
            PayButton::Link("https://pay.stripe.test/x"),
        );
        assert!(html.contains("1248"));
        assert!(html.contains("Design"));
        assert!(html.contains("250.00"));
        assert!(html.contains("https://pay.stripe.test/x"));
        assert!(html.contains("Direct deposit"));
        assert!(html.contains("Acme &lt;Co&gt;")); // escaped
    }

    #[test]
    fn direct_deposit_line_uses_the_supplied_contact_email() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("ap@acme.test"),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(html.contains("Contact ap@acme.test for account details"));
        assert!(!html.contains("rygn.io"));
    }

    #[test]
    fn contact_email_is_escaped() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("<script>alert(1)</script>"),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn client_name_containing_a_placeholder_stays_literal_text() {
        let (inv, mut client, items) = sample();
        client.name = "Acme {{ROWS}} {{PAY}} Co".into();
        let html = render_invoice_html(
            &brand("billing@example.test"),
            &inv,
            &client,
            &items,
            PayButton::Link("https://pay.stripe.test/x"),
        );
        assert!(html.contains("Acme {{ROWS}} {{PAY}} Co"));
        assert_eq!(html.matches("Design").count(), 1);
        assert_eq!(html.matches("Pay online").count(), 1);
    }

    #[test]
    fn pay_url_cannot_break_out_of_the_href_attribute() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("billing@example.test"),
            &inv,
            &client,
            &items,
            PayButton::Link("https://pay.stripe.test/x\"onmouseover=\"alert(1)"),
        );
        assert!(html.contains("&quot;onmouseover"));
        assert!(!html.contains("\"onmouseover"));
    }

    #[test]
    fn placeholder_renders_an_inert_span_not_a_link() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Placeholder,
        );
        assert!(html.contains("<span class=\"pay pay-placeholder\""));
        assert!(
            html.contains("link created when the invoice is sent"),
            "got: {html}"
        );
        assert!(!html.contains("<a class=\"pay\""));
        assert!(
            !html.contains("href"),
            "a placeholder must not be clickable"
        );
    }

    #[test]
    fn only_the_pay_element_differs_between_link_and_placeholder() {
        let (inv, client, items) = sample();
        let linked = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Link("https://pay.test/x"),
        );
        let pending = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Placeholder,
        );
        // `{{PAY}}` sits alone on its own line in the template, which is what
        // lets a line filter isolate it. If that moves, this test is what notices.
        let strip = |s: &str| {
            s.lines()
                .filter(|l| !l.contains("class=\"pay"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(strip(&linked), strip(&pending));
    }

    const MINIMAL: &str = "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>";

    fn brand(contact_email: &str) -> Branding<'_> {
        Branding {
            template: DEFAULT_TEMPLATE,
            company: "",
            contact_email,
        }
    }

    fn brand_with<'a>(template: &'a str, contact_email: &'a str) -> Branding<'a> {
        Branding {
            template,
            company: "",
            contact_email,
        }
    }

    fn write_override(dir: &std::path::Path, source: &str) {
        let path = template_path(dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }

    #[test]
    fn template_path_is_templates_invoice_html() {
        assert_eq!(
            template_path(Path::new("/books")),
            Path::new("/books/templates/invoice.html")
        );
    }

    #[test]
    fn no_override_falls_back_to_the_embedded_default() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_template(dir.path()).unwrap();
        assert!(matches!(loaded, std::borrow::Cow::Borrowed(_)));
        assert_eq!(loaded, DEFAULT_TEMPLATE);
    }

    #[test]
    fn an_override_file_wins_over_the_default() {
        let dir = tempfile::tempdir().unwrap();
        write_override(dir.path(), MINIMAL);
        assert_eq!(load_template(dir.path()).unwrap(), MINIMAL);
    }

    #[test]
    fn an_unreadable_override_errors_naming_the_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(template_path(dir.path())).unwrap();
        let err = load_template(dir.path()).unwrap_err().to_string();
        assert!(
            err.contains(&template_path(dir.path()).display().to_string()),
            "got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_errors_rather_than_falling_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = template_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(dir.path().join("elsewhere.html"), &path).unwrap();

        let loaded = load_template(dir.path());
        assert!(
            loaded.is_err(),
            "a template symlinked to nothing is a broken override, not an absent one"
        );
        assert!(loaded
            .unwrap_err()
            .to_string()
            .contains(&path.display().to_string()));
    }

    #[test]
    fn an_invalid_override_errors_rather_than_falling_back() {
        let dir = tempfile::tempdir().unwrap();
        write_override(dir.path(), "<p>hello</p>");
        let loaded = load_template(dir.path());
        assert!(loaded.is_err(), "a broken override must never render");
        assert!(
            loaded.unwrap_err().to_string().contains("{{NUMBER}}"),
            "the failure must name what is missing"
        );
    }

    #[test]
    fn the_default_template_validates() {
        assert!(validate_template(DEFAULT_TEMPLATE, Path::new("/tmp/t.html")).is_ok());
    }

    #[test]
    fn an_empty_or_whitespace_template_is_rejected() {
        for source in ["", "\n \t\n"] {
            let err = validate_template(source, Path::new("/tmp/t.html"))
                .unwrap_err()
                .to_string();
            assert!(err.contains("is empty"), "got: {err}");
            assert!(err.contains("/tmp/t.html"), "got: {err}");
        }
    }

    #[test]
    fn an_oversized_template_is_rejected() {
        let source = "x".repeat(MAX_TEMPLATE_BYTES + 1);
        let err = validate_template(&source, Path::new("/tmp/t.html"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("1 MiB"), "got: {err}");
        assert!(err.contains("/tmp/t.html"), "got: {err}");
    }

    #[test]
    fn a_template_missing_a_required_placeholder_is_rejected() {
        let err = validate_template(
            "<p>{{NUMBER}} {{CLIENT}} {{ROWS}}</p>",
            Path::new("/tmp/t.html"),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("missing required"), "got: {err}");
        assert!(err.contains("{{TOTAL}}"), "got: {err}");
    }

    #[test]
    fn a_template_with_an_unknown_placeholder_is_rejected() {
        let err = validate_template(
            "<p>{{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}} {{TOTL}}</p>",
            Path::new("/tmp/t.html"),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("{{TOTL}}"), "got: {err}");
        assert!(
            err.contains("{{NUMBER}}"),
            "the known list is missing: {err}"
        );
    }

    #[test]
    fn non_placeholder_braces_are_left_alone() {
        let source = "{{ not a key }} {{lower}} {{ }} {{ {{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}";
        assert!(validate_template(source, Path::new("/tmp/t.html")).is_ok());
    }

    #[test]
    fn placeholder_tokens_finds_each_key_once_per_occurrence() {
        assert_eq!(
            placeholder_tokens("{{NUMBER}} x {{NUMBER}} {{ROWS}} {{lower}} {{"),
            vec!["NUMBER", "NUMBER", "ROWS"]
        );
    }

    #[test]
    fn omits_pay_button_when_no_url() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand("billing@example.test"),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(!html.contains("Pay online"));
        assert!(html.contains("Direct deposit"));
    }

    #[test]
    fn a_custom_template_renders_instead_of_the_default() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with("<h1>{{NUMBER}}</h1>{{CLIENT}}{{ROWS}}{{TOTAL}}", "b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(html.starts_with("<h1>1248</h1>"), "got: {html}");
        assert!(!html.contains("Direct deposit"));
    }

    #[test]
    fn a_client_name_that_looks_like_a_placeholder_stays_literal_in_a_custom_template() {
        let (inv, mut client, items) = sample();
        client.name = "Acme {{ROWS}} {{PAY}} Co".into();
        let html = render_invoice_html(
            &brand_with(MINIMAL, "b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Link("https://pay.test/x"),
        );
        assert!(html.contains("Acme {{ROWS}} {{PAY}} Co"), "got: {html}");
        assert_eq!(html.matches("<tr>").count(), items.len());
    }

    #[test]
    fn a_custom_template_can_place_a_value_in_a_quoted_attribute() {
        let (inv, mut client, items) = sample();
        client.name = r#"a" onmouseover="x"#.into();
        let html = render_invoice_html(
            &brand_with(
                r#"<span title="{{CLIENT}}">{{NUMBER}}{{ROWS}}{{TOTAL}}</span>"#,
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(html.contains("&quot;"), "got: {html}");
        assert!(!html.contains(r#"" onmouseover=""#), "got: {html}");
    }

    #[test]
    fn company_renders_and_is_escaped() {
        let (inv, client, items) = sample();
        let branding = Branding {
            template: "<h1>{{COMPANY}}</h1>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
            company: "A & B <Co>",
            contact_email: "b@e.test",
        };
        let html = render_invoice_html(&branding, &inv, &client, &items, PayButton::Omitted);
        assert!(html.contains("A &amp; B &lt;Co&gt;"), "got: {html}");
    }

    #[test]
    fn company_renders_empty_when_unset() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "<h1>{{COMPANY}}</h1>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(html.starts_with("<h1></h1>"), "got: {html}");
    }

    #[test]
    fn every_placeholder_in_the_vocabulary_expands() {
        let (inv, client, items) = sample();
        let template = PLACEHOLDERS
            .iter()
            .map(|k| format!("{{{{{k}}}}}"))
            .collect::<Vec<_>>()
            .join("\n");
        let html = render_invoice_html(
            &brand_with(&template, "b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Link("https://pay.test/x"),
        );
        assert!(!html.contains("{{"), "unexpanded placeholder in: {html}");
    }

    #[test]
    fn optional_values_render_empty_rather_than_an_empty_tag() {
        let (mut inv, mut client, items) = sample();
        inv.due_date = None;
        inv.notes = None;
        inv.terms = None;
        client.email = None;
        client.billing_address = None;

        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(!html.contains("Due:"), "got: {html}");
        assert!(!html.contains("<p></p>"), "got: {html}");
        assert!(!html.contains("Notes"), "got: {html}");
        assert!(!html.contains("Terms"), "got: {html}");
    }

    #[test]
    fn notes_and_terms_are_escaped() {
        let (mut inv, client, items) = sample();
        inv.notes = Some("<b>thanks</b>".into());
        inv.terms = Some("<i>net 30</i>".into());

        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(html.contains("&lt;b&gt;thanks&lt;/b&gt;"), "got: {html}");
        assert!(html.contains("&lt;i&gt;net 30&lt;/i&gt;"), "got: {html}");
        assert!(!html.contains("<b>thanks"), "got: {html}");
    }

    #[test]
    fn due_date_is_the_bare_date_and_due_is_the_fragment() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "[{{DUE_DATE}}][{{DUE}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(
            html.starts_with("[2026-09-03][<br>Due: 2026-09-03]"),
            "got: {html}"
        );
    }

    #[test]
    fn the_pay_url_is_the_link_only_when_there_is_one() {
        let (inv, client, items) = sample();
        let template = "[{{PAY_URL}}]{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}";
        let linked = render_invoice_html(
            &brand_with(template, "b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Link("https://pay.test/x"),
        );
        assert!(linked.starts_with("[https://pay.test/x]"), "got: {linked}");

        for pay in [PayButton::Placeholder, PayButton::Omitted] {
            let html = render_invoice_html(
                &brand_with(template, "b@e.test"),
                &inv,
                &client,
                &items,
                pay,
            );
            assert!(html.starts_with("[]"), "got: {html}");
        }
    }

    #[test]
    fn the_default_template_renders_notes_and_terms() {
        let (mut inv, client, items) = sample();
        inv.notes = Some("Thanks for the work".into());
        inv.terms = Some("Net 30".into());

        let html = render_invoice_html(
            &brand("b@e.test"),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(html.contains("Thanks for the work"), "got: {html}");
        assert!(html.contains("Net 30"), "got: {html}");
    }

    #[test]
    fn operator_markup_in_a_template_is_not_sanitized() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(
            &brand_with(
                "<script>alert(1)</script>{{NUMBER}}{{CLIENT}}{{ROWS}}{{TOTAL}}",
                "b@e.test",
            ),
            &inv,
            &client,
            &items,
            PayButton::Omitted,
        );
        assert!(html.contains("<script>alert(1)</script>"));
    }
}
