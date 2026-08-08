use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use comfy_table::{Cell, Table};
use rusqlite::Connection;

use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::invoicing::clients::get_client;
use crate::invoicing::import_invoiceshelf::import as import_invoiceshelf;
use crate::invoicing::invoices::{
    create_invoice, ensure_voidable, get_invoice, get_invoice_by_number, line_items, paid_amount,
    record_payment, update_invoice, void_invoice, InvoiceUpdate, NewLineItem,
};
use crate::invoicing::mailgun::MailgunClient;
use crate::invoicing::r2::R2Publisher;
use crate::invoicing::render::render_invoice;
use crate::invoicing::render_html::{
    load_template, template_path, Branding, PayButton, DEFAULT_TEMPLATE,
};
use crate::invoicing::send::send_invoice;
use crate::invoicing::stripe::StripeClient;
use crate::invoicing::sync::sync_all;
use crate::models::{Invoice, InvoiceStatus};
use crate::settings::{get_data_dir, invoicing_config, InvoicingConfig};

fn parse_item(s: &str) -> Result<NewLineItem> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(NigelError::Other(format!(
            "bad --item '{s}', want desc:qty:unit"
        )));
    }
    Ok(NewLineItem {
        description: parts[0].to_string(),
        quantity: parts[1].parse().map_err(|_| {
            NigelError::Other(format!("bad quantity '{}' in --item '{s}'", parts[1]))
        })?,
        unit_amount: parts[2].parse().map_err(|_| {
            NigelError::Other(format!("bad unit amount '{}' in --item '{s}'", parts[2]))
        })?,
    })
}

fn parse_items(items: &[String]) -> Result<Vec<NewLineItem>> {
    if items.is_empty() {
        return Err(NigelError::Other(
            "an invoice needs at least one --item \"desc:qty:unit\"".into(),
        ));
    }
    items.iter().map(|s| parse_item(s)).collect()
}

/// `--item` on an edit is all-or-nothing: none supplied leaves the existing
/// lines alone, any supplied replaces the whole set.
fn optional_items(items: &[String]) -> Result<Option<Vec<NewLineItem>>> {
    if items.is_empty() {
        Ok(None)
    } else {
        parse_items(items).map(Some)
    }
}

fn void_summary(invoice: &Invoice, client_name: &str) -> String {
    format!(
        "Invoice #{} — {client_name}, {:.2} {}, {} {}.",
        invoice.number, invoice.total, invoice.currency, invoice.status, invoice.issue_date
    )
}

fn find_invoice(conn: &Connection, number: i64) -> Result<Invoice> {
    get_invoice_by_number(conn, number).map_err(|e| match e {
        NigelError::Db(rusqlite::Error::QueryReturnedNoRows) => NigelError::NotFound(format!(
            "No invoice #{number}. Run `nigel invoice list` to see invoice numbers."
        )),
        other => other,
    })
}

/// What voiding does *not* undo. Void leaves the published page and the Stripe
/// payment link live, so every front end that voids says so.
pub(crate) const PUBLISHED_VOID_WARNING: &str =
    "Warning: this invoice was already published. Its page and Stripe payment link stay live — \
     deactivate the link in Stripe if you do not want it paid.";

/// `voided_at` is the fact; `status` is derived from it. Reading the timestamp
/// first means a void whose status write did not land still reads as void.
pub(crate) fn is_void(invoice: &Invoice) -> bool {
    invoice.voided_at.is_some() || invoice.status == InvoiceStatus::Void.as_str()
}

pub(crate) fn ensure_not_void(invoice: &Invoice, action: &str) -> Result<()> {
    if is_void(invoice) {
        return Err(NigelError::Conflict {
            code: "void",
            message: format!(
                "Invoice #{} is void and cannot be {action}.",
                invoice.number
            ),
        });
    }
    Ok(())
}

/// Resolve the amount to record: the explicit `--amount`, or the whole
/// outstanding balance. Rejects amounts that would write a junk payment row.
pub(crate) fn payment_amount(invoice: &Invoice, paid: f64, requested: Option<f64>) -> Result<f64> {
    match requested {
        // Negated positive test, not `amount <= 0.0`: NaN compares false against
        // every bound, and a NaN payment row poisons every later SUM.
        Some(amount) if !(amount.is_finite() && amount > 0.0) => Err(NigelError::Other(format!(
            "--amount must be a finite number greater than zero, got {amount:.2}."
        ))),
        Some(amount) => Ok(amount),
        None => {
            let outstanding = invoice.total - paid;
            // Same half-cent slack `refresh_status` settles with: anything under
            // it is already paid in full, not a balance worth recording.
            if outstanding < 0.005 {
                return Err(NigelError::Other(format!(
                    "Invoice #{} has no outstanding balance (total {:.2}, paid {:.2}). Pass --amount to record a payment anyway.",
                    invoice.number, invoice.total, paid
                )));
            }
            Ok(outstanding)
        }
    }
}

fn require(value: Option<String>, what: &str) -> Result<String> {
    value.ok_or_else(|| {
        NigelError::Other(format!(
            "missing invoicing config: {what} (set it in settings.json or the matching NIGEL_ env var)"
        ))
    })
}

/// The business name the settings screen writes, as the invoice page and the
/// email subject want it: a plain string, empty when nobody has set one.
pub(crate) fn company_name(conn: &Connection) -> String {
    crate::db::get_metadata(conn, "company_name").unwrap_or_default()
}

fn build_gateway(cfg: &InvoicingConfig) -> Result<StripeClient> {
    Ok(StripeClient {
        secret_key: require(cfg.stripe_secret_key.clone(), "stripe_secret_key")?,
    })
}

pub(crate) fn build_clients(
    cfg: InvoicingConfig,
) -> Result<(StripeClient, R2Publisher, MailgunClient)> {
    let stripe = build_gateway(&cfg)?;
    let r2 = R2Publisher {
        account_id: require(cfg.r2_account_id, "r2_account_id")?,
        access_key: require(cfg.r2_access_key, "r2_access_key")?,
        secret_key: require(cfg.r2_secret_key, "r2_secret_key")?,
        bucket: require(cfg.r2_bucket, "r2_bucket")?,
        public_base_url: require(cfg.public_base_url, "public_base_url")?,
    };
    let mail = MailgunClient {
        api_key: require(cfg.mailgun_api_key, "mailgun_api_key")?,
        domain: require(cfg.mailgun_domain, "mailgun_domain")?,
        from: require(cfg.from_email, "from_email")?,
    };
    Ok((stripe, r2, mail))
}

pub fn new(
    client_id: i64,
    issue_date: &str,
    due_date: Option<&str>,
    currency: &str,
    items: &[String],
    notes: Option<&str>,
    terms: Option<&str>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let parsed = parse_items(items)?;
    let id = create_invoice(
        &conn, client_id, issue_date, due_date, currency, &parsed, notes, terms,
    )?;
    let invoice = get_invoice(&conn, id)?;
    println!(
        "Created draft invoice #{} for {:.2} {}",
        invoice.number, invoice.total, invoice.currency
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn edit(
    number: i64,
    issue_date: Option<String>,
    due_date: Option<String>,
    clear_due: bool,
    currency: Option<String>,
    notes: Option<String>,
    terms: Option<String>,
    items: &[String],
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let had_link = invoice.stripe_payment_link_id.is_some();

    let update = InvoiceUpdate {
        issue_date,
        due_date: if clear_due {
            Some(None)
        } else {
            due_date.map(Some)
        },
        currency,
        notes: notes.map(Some),
        terms: terms.map(Some),
        items: optional_items(items)?,
    };
    update_invoice(&conn, invoice.id, &update)?;

    let updated = get_invoice(&conn, invoice.id)?;
    println!(
        "Updated draft invoice #{number} — {:.2} {}",
        updated.total, updated.currency
    );
    if had_link && updated.stripe_payment_link_id.is_none() {
        println!(
            "Cleared the stale Stripe payment link; `nigel invoice send {number}` will create a new one."
        );
    }
    Ok(())
}

pub fn void(number: i64, yes: bool, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    ensure_voidable(&conn, &invoice)?;
    let client = get_client(&conn, invoice.client_id)?;

    println!("{}", void_summary(&invoice, &client.name));
    if !confirm_void(&invoice, yes)? {
        println!("Aborted.");
        return Ok(());
    }

    void_invoice(&conn, invoice.id, today)?;
    println!("Voided invoice #{number}.");
    if invoice.published_at.is_some() {
        println!("{PUBLISHED_VOID_WARNING}");
    }
    Ok(())
}

fn confirm_void(invoice: &Invoice, yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        return Err(NigelError::Other(format!(
            "Refusing to void invoice #{} without confirmation. Pass --yes.",
            invoice.number
        )));
    }
    print!("Void it? [y/N] ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut stmt = conn.prepare(
        "SELECT i.number, i.status, c.name, i.total, COALESCE(i.due_date, '')
         FROM invoices i JOIN clients c ON c.id = i.client_id
         ORDER BY i.number DESC",
    )?;
    let rows: Vec<(i64, String, String, f64, String)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut table = Table::new();
    table.set_header(vec!["#", "Status", "Client", "Total", "Due"]);
    for (number, status, client, total, due) in rows {
        table.add_row(vec![
            Cell::new(number),
            Cell::new(status),
            Cell::new(client),
            Cell::new(format!("{total:.2}")),
            Cell::new(due),
        ]);
    }
    println!("Invoices\n{table}");
    Ok(())
}

pub fn show(number: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    let client = get_client(&conn, invoice.client_id)?;

    println!(
        "Invoice #{}  [{}]  {} {:.2}",
        invoice.number, invoice.status, invoice.currency, invoice.total
    );
    println!("Client:   {}", client.name);
    println!("Issued:   {}", invoice.issue_date);
    println!("Due:      {}", invoice.due_date.as_deref().unwrap_or("-"));

    let mut table = Table::new();
    table.set_header(vec!["Description", "Qty", "Unit", "Amount"]);
    for item in line_items(&conn, invoice.id)? {
        table.add_row(vec![
            Cell::new(item.description),
            Cell::new(format!("{:.2}", item.quantity)),
            Cell::new(format!("{:.2}", item.unit_amount)),
            Cell::new(format!("{:.2}", item.line_total)),
        ]);
    }
    println!("{table}");

    let paid = paid_amount(&conn, invoice.id)?;
    println!("Paid:     {paid:.2}");
    println!("Balance:  {:.2}", invoice.total - paid);
    if let Some(url) = invoice.stripe_payment_link_url {
        println!("Pay:      {url}");
    }
    Ok(())
}

/// The direct-deposit contact line when `from_email` is not configured. Preview
/// is the one invoicing command that runs without any configuration, so it
/// renders a visible stand-in rather than refusing.
const PREVIEW_CONTACT_PLACEHOLDER: &str = "(from_email not configured)";

fn preview_dir(output_dir: Option<String>) -> (PathBuf, bool) {
    match output_dir {
        Some(dir) => (
            PathBuf::from(crate::settings::shellexpand_path(&dir)),
            false,
        ),
        None => (get_data_dir().join("previews"), true),
    }
}

fn preview_paths(dir: &Path, number: i64) -> (PathBuf, PathBuf) {
    (
        dir.join(format!("invoice-{number}.html")),
        dir.join(format!("invoice-{number}.pdf")),
    )
}

fn pay_button_for(invoice: &Invoice) -> PayButton<'_> {
    // A voided invoice can still carry a live Stripe URL, and rendering a
    // working Pay button on a cancelled invoice is the one way this command
    // could cost someone money.
    if is_void(invoice) {
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

    if is_void(&invoice) {
        eprintln!("notice: invoice #{number} is void — this preview is for reference only.");
    }
    let (contact_email, is_placeholder) = contact_email_for_preview(&invoicing_config());
    if is_placeholder {
        eprintln!(
            "notice: from_email is not configured — the direct-deposit contact line is a placeholder"
        );
    }

    let template = load_template(&get_data_dir())?;
    let company = company_name(&conn);
    let branding = Branding {
        template: &template,
        company: &company,
        contact_email: &contact_email,
    };

    // Both artifacts are rendered before either is written, so a PDF failure
    // cannot leave fresh HTML beside a stale PDF.
    let rendered = render_invoice(
        &conn,
        &invoice,
        &client,
        pay_button_for(&invoice),
        &branding,
    )?;

    let (dir, is_default) = preview_dir(output_dir);
    std::fs::create_dir_all(&dir)?;
    if is_default {
        // Only the directory Nigel chose. A directory the user named may be
        // shared on purpose, and tightening it would be a surprise.
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

pub fn send(number: i64, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    ensure_not_void(&invoice, "sent")?;
    // The template is loaded before anything is built or created, so a broken
    // one fails the send with no Stripe link made and nothing published.
    let template = load_template(&get_data_dir())?;
    let (stripe, r2, mail) = build_clients(invoicing_config())?;
    let company = company_name(&conn);
    let branding = Branding {
        template: &template,
        company: &company,
        contact_email: &mail.from,
    };
    let url = send_invoice(&conn, invoice.id, today, &branding, &stripe, &r2, &mail)?;
    println!("Sent invoice #{number}: {url}");
    Ok(())
}

pub fn sync(today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let stripe = build_gateway(&invoicing_config())?;
    let recorded = sync_all(&conn, today, &stripe)?;
    println!("Recorded {recorded} new payment(s)");
    Ok(())
}

pub fn pay(number: i64, amount: Option<f64>, date: &str, method: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = find_invoice(&conn, number)?;
    ensure_not_void(&invoice, "paid")?;
    let paid = paid_amount(&conn, invoice.id)?;
    let amount = payment_amount(&invoice, paid, amount)?;
    record_payment(&conn, invoice.id, amount, date, method, None)?;
    let invoice = get_invoice(&conn, invoice.id)?;
    println!(
        "Recorded {amount:.2} against invoice #{number} ({})",
        invoice.status
    );
    Ok(())
}

pub fn aging(today: &str) -> Result<()> {
    println!("{}", crate::cli::report::text::aging(today)?);
    Ok(())
}

pub fn template_export(output: Option<&str>, force: bool) -> Result<()> {
    let destination = match output {
        Some(path) => PathBuf::from(crate::settings::shellexpand_path(path)),
        None => template_path(&get_data_dir()),
    };

    if destination.exists() && !force {
        return Err(NigelError::Invalid(format!(
            "{} already exists. Pass --force to overwrite it.",
            destination.display()
        )));
    }
    let write_error = |e: std::io::Error| {
        NigelError::Invalid(format!(
            "Cannot write invoice template to {}: {e}",
            destination.display()
        ))
    };
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(write_error)?;
    }
    std::fs::write(&destination, DEFAULT_TEMPLATE).map_err(write_error)?;

    println!("Wrote invoice template to {}", destination.display());
    println!(
        "Edit it, then check it with `nigel invoice preview <number>` — see docs/invoicing.md."
    );
    Ok(())
}

pub fn template_show_path() -> Result<()> {
    let path = template_path(&get_data_dir());
    println!("{}", path.display());

    if !path.exists() {
        println!("No custom template — the built-in one is in use.");
        return Ok(());
    }
    load_template(&get_data_dir())?;
    println!("Custom template in effect.");
    Ok(())
}

pub fn import(db: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let summary = import_invoiceshelf(&conn, Path::new(db))?;
    println!(
        "Imported {} clients, {} invoices, {} payments. Next invoice number: {}",
        summary.clients, summary.invoices, summary.payments, summary.next_number
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::invoicing::clients::add_client;
    use crate::invoicing::invoices::{create_invoice, set_payment_link};
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    /// Insert one 100.00 draft invoice (number 1248) and return its row id.
    fn seed_invoice(conn: &Connection) -> i64 {
        let cid = add_client(conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
    }

    #[test]
    fn parse_item_splits_desc_qty_unit() {
        let item = parse_item("Design:2:150.50").unwrap();
        assert_eq!(item.description, "Design");
        assert_eq!(item.quantity, 2.0);
        assert_eq!(item.unit_amount, 150.50);
    }

    #[test]
    fn parse_item_rejects_wrong_shape_and_bad_numbers() {
        assert!(parse_item("Design:2").is_err());
        assert!(parse_item("Design:two:150").is_err());
        assert!(parse_item("Design:2:free").is_err());
    }

    #[test]
    fn an_invoice_needs_at_least_one_item() {
        let err = parse_items(&[]).map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("--item"), "got: {err}");
        assert_eq!(parse_items(&["W:1:10".to_string()]).unwrap().len(), 1);
    }

    #[test]
    fn parse_items_is_optional_for_an_edit() {
        assert!(optional_items(&[]).unwrap().is_none());

        let parsed = optional_items(&["Rework:2:250".to_string()])
            .unwrap()
            .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].description, "Rework");
        assert_eq!(parsed[0].unit_amount, 250.0);

        assert!(optional_items(&["Rework:2".to_string()]).is_err());
    }

    #[test]
    fn confirm_prompt_names_the_invoice() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        let line = void_summary(&invoice, "Acme Co");
        assert!(line.contains("#1248"), "got: {line}");
        assert!(line.contains("Acme Co"), "got: {line}");
        assert!(line.contains("100.00 USD"), "got: {line}");
        assert!(line.contains("draft"), "got: {line}");
    }

    fn test_config() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: None,
            mailgun_api_key: None,
            mailgun_domain: None,
            from_email: None,
            r2_account_id: None,
            r2_access_key: None,
            r2_secret_key: None,
            r2_bucket: None,
            public_base_url: None,
        }
    }

    #[test]
    fn missing_secret_names_the_setting() {
        let err = build_clients(test_config())
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("stripe_secret_key"), "got: {err}");
    }

    #[test]
    fn missing_public_base_url_names_the_setting() {
        let cfg = InvoicingConfig {
            stripe_secret_key: Some("sk_test".into()),
            r2_account_id: Some("acct".into()),
            r2_access_key: Some("ak".into()),
            r2_secret_key: Some("sk".into()),
            r2_bucket: Some("billing".into()),
            ..test_config()
        };
        let err = build_clients(cfg).map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("public_base_url"), "got: {err}");
    }

    fn config_up_to_mailgun() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: Some("sk_test".into()),
            r2_account_id: Some("acct".into()),
            r2_access_key: Some("ak".into()),
            r2_secret_key: Some("sk".into()),
            r2_bucket: Some("billing".into()),
            public_base_url: Some("https://billing.example.test/i".into()),
            mailgun_api_key: Some("key".into()),
            ..test_config()
        }
    }

    #[test]
    fn missing_mailgun_domain_names_the_setting() {
        let err = build_clients(config_up_to_mailgun())
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(err.contains("mailgun_domain"), "got: {err}");
    }

    #[test]
    fn missing_from_email_names_the_setting() {
        let cfg = InvoicingConfig {
            mailgun_domain: Some("mail.example.test".into()),
            ..config_up_to_mailgun()
        };
        let err = build_clients(cfg).map(|_| ()).unwrap_err().to_string();
        assert!(err.contains("from_email"), "got: {err}");
    }

    #[test]
    fn unknown_invoice_number_gets_a_readable_error() {
        let (_d, conn) = test_conn();
        let err = find_invoice(&conn, 9999).map(|_| ()).unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err:?}");
        assert!(err.to_string().contains("No invoice #9999"), "got: {err}");
    }

    #[test]
    fn find_invoice_returns_the_matching_invoice() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);

        let invoice = find_invoice(&conn, 1248).unwrap();
        assert_eq!(invoice.number, 1248);
        assert_eq!(invoice.total, 100.0);
    }

    #[test]
    fn void_invoices_are_refused_before_any_network_call_or_payment() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        conn.execute("UPDATE invoices SET status='void' WHERE id=?1", [id])
            .unwrap();
        let invoice = find_invoice(&conn, 1248).unwrap();

        let send_err = ensure_not_void(&invoice, "sent").unwrap_err();
        assert!(
            matches!(send_err, NigelError::Conflict { code: "void", .. }),
            "got: {send_err:?}"
        );
        assert!(
            send_err.to_string().contains("void and cannot be sent"),
            "got: {send_err}"
        );
        let pay_err = ensure_not_void(&invoice, "paid").unwrap_err();
        assert!(
            matches!(pay_err, NigelError::Conflict { code: "void", .. }),
            "got: {pay_err:?}"
        );
        assert!(
            pay_err.to_string().contains("void and cannot be paid"),
            "got: {pay_err}"
        );
    }

    #[test]
    fn a_voided_at_row_whose_status_is_stale_is_still_refused() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        // A void whose status write did not land: the timestamp is the fact.
        conn.execute(
            "UPDATE invoices SET voided_at='2026-08-06', status='draft' WHERE id=?1",
            [id],
        )
        .unwrap();
        let invoice = find_invoice(&conn, 1248).unwrap();

        for action in ["sent", "paid"] {
            let err = ensure_not_void(&invoice, action).unwrap_err();
            assert!(
                matches!(err, NigelError::Conflict { code: "void", .. }),
                "got: {err:?}"
            );
        }
    }

    #[test]
    fn draft_invoices_are_sendable_and_payable() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();
        assert!(ensure_not_void(&invoice, "sent").is_ok());
        assert!(ensure_not_void(&invoice, "paid").is_ok());
    }

    #[test]
    fn preview_paths_are_stable_and_undated() {
        let (html, pdf) = preview_paths(Path::new("/tmp/p"), 1248);
        assert_eq!(html, Path::new("/tmp/p/invoice-1248.html"));
        assert_eq!(pdf, Path::new("/tmp/p/invoice-1248.pdf"));
    }

    #[test]
    fn explicit_output_dir_wins_and_is_not_the_default() {
        let (dir, is_default) = preview_dir(Some("/tmp/elsewhere".into()));
        assert_eq!(dir, PathBuf::from("/tmp/elsewhere"));
        assert!(
            !is_default,
            "a directory the user named is not re-permissioned"
        );

        let (dir, is_default) = preview_dir(None);
        assert!(is_default && dir.ends_with("previews"), "got: {dir:?}");
    }

    #[test]
    fn a_draft_with_no_link_gets_the_placeholder_button() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();
        assert!(matches!(pay_button_for(&invoice), PayButton::Placeholder));
    }

    #[test]
    fn a_sent_invoice_previews_with_its_real_link() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        let invoice = find_invoice(&conn, 1248).unwrap();
        assert!(matches!(
            pay_button_for(&invoice),
            PayButton::Link("https://pay/x")
        ));
    }

    #[test]
    fn a_void_invoice_never_renders_a_pay_button_even_with_a_live_link() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        void_invoice(&conn, id, "2026-08-06").unwrap();
        let invoice = find_invoice(&conn, 1248).unwrap();

        assert!(
            matches!(pay_button_for(&invoice), PayButton::Omitted),
            "a cancelled invoice must not offer a working payment link"
        );
    }

    #[test]
    fn a_stale_void_status_still_omits_the_pay_button() {
        let (_d, conn) = test_conn();
        let id = seed_invoice(&conn);
        // A void whose status write did not land: the timestamp is the fact,
        // the same reading `ensure_not_void` takes.
        conn.execute(
            "UPDATE invoices SET voided_at='2026-08-06', status='draft',
                                 stripe_payment_link_url='https://pay/x' WHERE id=?1",
            [id],
        )
        .unwrap();
        let invoice = find_invoice(&conn, 1248).unwrap();
        assert!(matches!(pay_button_for(&invoice), PayButton::Omitted));
    }

    #[test]
    fn missing_from_email_becomes_a_flagged_placeholder() {
        let (value, placeholder) = contact_email_for_preview(&test_config());
        assert!(placeholder && value.contains("from_email"), "got: {value}");

        let cfg = InvoicingConfig {
            from_email: Some("billing@example.test".into()),
            ..test_config()
        };
        assert_eq!(
            contact_email_for_preview(&cfg),
            ("billing@example.test".to_string(), false)
        );
    }

    #[test]
    fn preview_requires_no_invoicing_config_at_all() {
        assert!(build_clients(test_config()).is_err()); // send cannot run
        assert!(!contact_email_for_preview(&test_config()).0.is_empty()); // preview can
    }

    #[test]
    fn default_payment_is_the_outstanding_balance() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        assert_eq!(payment_amount(&invoice, 0.0, None).unwrap(), 100.0);
        assert_eq!(payment_amount(&invoice, 40.0, None).unwrap(), 60.0);
    }

    #[test]
    fn settled_invoices_have_nothing_left_to_pay() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        for paid in [100.0, 100.001, 150.0] {
            let err = payment_amount(&invoice, paid, None)
                .unwrap_err()
                .to_string();
            assert!(err.contains("no outstanding balance"), "got: {err}");
        }
    }

    #[test]
    fn explicit_amount_must_be_positive() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        for amount in [0.0, -25.0] {
            let err = payment_amount(&invoice, 0.0, Some(amount))
                .unwrap_err()
                .to_string();
            assert!(err.contains("greater than zero"), "got: {err}");
        }
        // An overpayment is a real thing a bank does; only zero and negative are junk.
        assert_eq!(payment_amount(&invoice, 0.0, Some(250.0)).unwrap(), 250.0);
    }

    #[test]
    fn explicit_amount_must_be_finite() {
        let (_d, conn) = test_conn();
        seed_invoice(&conn);
        let invoice = find_invoice(&conn, 1248).unwrap();

        for amount in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = payment_amount(&invoice, 0.0, Some(amount))
                .unwrap_err()
                .to_string();
            assert!(err.contains("finite number"), "got: {err}");
        }
    }
}
