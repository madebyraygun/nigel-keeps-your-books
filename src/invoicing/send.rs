use rusqlite::Connection;
use serde::{Serialize, Serializer};

use crate::error::{NigelError, Result};
use crate::invoicing::clients::get_client;
use crate::invoicing::gateway::{AssetPublisher, Mailer, PaymentGateway};
use crate::invoicing::invoices::{ensure_not_void, get_invoice, mark_published, set_payment_link};
use crate::invoicing::render::render_invoice;
use crate::invoicing::render_html::{Branding, PayButton};
use crate::models::Client;

/// What a send that reaches the render step needs and a build without the `pdf`
/// feature cannot give it. A constant because the API answers this one failure
/// as `501 feature_disabled` while every other render failure is a `500`.
pub const PDF_REQUIRED_MESSAGE: &str = "PDF support not compiled in (build with --features pdf)";

/// The stages of a send, in execution order.
///
/// `Config` belongs to the caller — resolving the invoicing settings and
/// building the three clients happens before the orchestration is reachable —
/// and is named here so that the one failure vocabulary covers the whole
/// operation a user pressed one button for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendStep {
    Config,
    Load,
    Precheck,
    PaymentLink,
    Render,
    Publish,
    Email,
    Record,
}

impl SendStep {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Load => "load",
            Self::Precheck => "precheck",
            Self::PaymentLink => "payment_link",
            Self::Render => "render",
            Self::Publish => "publish",
            Self::Email => "email",
            Self::Record => "record",
        }
    }
}

/// How a step that did not fail turned out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Ok,
    /// A resend: the Stripe payment link this invoice already carries is the one
    /// the client was given, so no second link is created.
    Reused,
}

impl StepOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Reused => "reused",
        }
    }
}

// Serialized through `as_str` rather than a derive, so the wire word and the
// word an error message carries cannot drift apart.
impl Serialize for SendStep {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl Serialize for StepOutcome {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// A send that went all the way through, with what each step did.
#[derive(Debug, Clone)]
pub struct SendOutcome {
    pub public_url: String,
    pub payment_link_url: Option<String>,
    /// The invoice's status once the send was recorded.
    pub status: String,
    pub steps: Vec<(SendStep, StepOutcome)>,
}

/// A send that stopped, and where.
///
/// `email_sent` is the field a front end's wording turns on: every failure
/// before it is safe to retry, and a failure at `Record` after the mail went out
/// is not — the client already has the invoice.
#[derive(Debug)]
pub struct SendFailure {
    pub step: SendStep,
    pub completed: Vec<SendStep>,
    pub email_sent: bool,
    /// The invoice's status as it stands after the failure, or `None` when the
    /// invoice could not be read at all.
    pub invoice_status: Option<String>,
    pub source: NigelError,
}

/// The address a send needs, or the refusal both front ends print.
///
/// A missing address is a conflict rather than a plain error because it is a
/// fact about the client record that a screen can act on — over HTTP it becomes
/// a `409` naming the client, not a `500`.
pub fn require_email(client: &Client) -> Result<String> {
    client.email.clone().ok_or_else(|| NigelError::Conflict {
        code: "client_missing_email",
        message: format!("client '{}' has no email", client.name),
    })
}

/// An invoice worth sending has something to charge for.
pub fn ensure_payable(invoice: &crate::models::Invoice) -> Result<()> {
    if invoice.total > 0.0 {
        return Ok(());
    }
    Err(NigelError::Conflict {
        code: "invoice_not_payable",
        message: format!(
            "Invoice #{} has no amount to charge and cannot be sent",
            invoice.number
        ),
    })
}

#[derive(Default)]
struct Trace {
    steps: Vec<(SendStep, StepOutcome)>,
}

impl Trace {
    fn done(&mut self, step: SendStep, outcome: StepOutcome) {
        self.steps.push((step, outcome));
    }

    fn into_failure(
        self,
        conn: &Connection,
        invoice_id: i64,
        step: SendStep,
        source: NigelError,
    ) -> SendFailure {
        SendFailure {
            step,
            email_sent: self.steps.iter().any(|(s, _)| *s == SendStep::Email),
            completed: self.steps.iter().map(|(s, _)| *s).collect(),
            // Re-read rather than remembered: a failure at `Record` can land
            // either side of the row being written, and the status a screen
            // renders has to be the one in the database.
            invoice_status: get_invoice(conn, invoice_id).ok().map(|i| i.status),
            source,
        }
    }
}

/// Send an invoice, reporting what each step did.
///
/// The orchestration itself: Stripe link, render, publish, email, mark
/// published. Any failure leaves the invoice a draft, and names the step it
/// stopped at.
pub fn send_invoice_traced<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
    conn: &Connection,
    invoice_id: i64,
    today: &str,
    branding: &Branding<'_>,
    gateway: &G,
    publisher: &P,
    mailer: &M,
) -> std::result::Result<SendOutcome, SendFailure> {
    let mut trace = Trace::default();
    match run(
        conn, invoice_id, today, branding, gateway, publisher, mailer, &mut trace,
    ) {
        Ok(outcome) => Ok(outcome),
        Err((step, source)) => Err(trace.into_failure(conn, invoice_id, step, source)),
    }
}

/// [`send_invoice_traced`]'s body, written with `?` — every fallible call tags
/// itself with the step it belongs to, and the trace turns the pair into the
/// failure the caller sees.
#[allow(clippy::too_many_arguments)]
fn run<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
    conn: &Connection,
    invoice_id: i64,
    today: &str,
    branding: &Branding<'_>,
    gateway: &G,
    publisher: &P,
    mailer: &M,
    trace: &mut Trace,
) -> std::result::Result<SendOutcome, (SendStep, NigelError)> {
    use SendStep::*;

    let mut invoice = get_invoice(conn, invoice_id).map_err(|e| (Load, e))?;
    let client = get_client(conn, invoice.client_id).map_err(|e| (Load, e))?;
    trace.done(Load, StepOutcome::Ok);

    // Before the Stripe link, so a void invoice or a client with no address
    // costs no network call.
    ensure_not_void(&invoice, "sent").map_err(|e| (Precheck, e))?;
    let email = require_email(&client).map_err(|e| (Precheck, e))?;
    ensure_payable(&invoice).map_err(|e| (Precheck, e))?;
    trace.done(Precheck, StepOutcome::Ok);

    // Create the Stripe link once; reuse on resend.
    let link_outcome = if invoice.stripe_payment_link_url.is_none() {
        let link = gateway
            .create_payment_link(&invoice, &client)
            .map_err(|e| (PaymentLink, e))?;
        set_payment_link(conn, invoice_id, &link.id, &link.url).map_err(|e| (PaymentLink, e))?;
        invoice = get_invoice(conn, invoice_id).map_err(|e| (PaymentLink, e))?;
        StepOutcome::Ok
    } else {
        StepOutcome::Reused
    };
    trace.done(PaymentLink, link_outcome);

    let pay_url = invoice.stripe_payment_link_url.clone();
    let pay = match pay_url.as_deref() {
        Some(url) => PayButton::Link(url),
        None => PayButton::Omitted,
    };
    let rendered =
        render_invoice(conn, &invoice, &client, pay, branding).map_err(|e| (Render, e))?;
    let pdf = rendered
        .pdf
        .ok_or((Render, NigelError::Other(PDF_REQUIRED_MESSAGE.into())))?;
    trace.done(Render, StepOutcome::Ok);

    let public_url = publisher
        .publish(&invoice.token, rendered.html.as_bytes(), &pdf)
        .map_err(|e| (Publish, e))?;
    trace.done(Publish, StepOutcome::Ok);

    let subject = match branding.company.trim() {
        "" => format!("Invoice #{}", invoice.number),
        company => format!("Invoice #{} from {company}", invoice.number),
    };
    mailer
        .send_invoice(&email, &subject, &rendered.html, &pdf)
        .map_err(|e| (Email, e))?;
    trace.done(Email, StepOutcome::Ok);

    mark_published(conn, invoice_id, today).map_err(|e| (Record, e))?;
    let status = get_invoice(conn, invoice_id)
        .map_err(|e| (Record, e))?
        .status;
    trace.done(Record, StepOutcome::Ok);

    Ok(SendOutcome {
        public_url,
        payment_link_url: pay_url,
        status,
        steps: std::mem::take(&mut trace.steps),
    })
}

/// The URL of the published page, for callers that only need to say where the
/// invoice went — the CLI and the TUI, whose wording predates the step trace.
pub fn send_invoice<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
    conn: &Connection,
    invoice_id: i64,
    today: &str,
    branding: &Branding<'_>,
    gateway: &G,
    publisher: &P,
    mailer: &M,
) -> Result<String> {
    send_invoice_traced(
        conn, invoice_id, today, branding, gateway, publisher, mailer,
    )
    .map(|outcome| outcome.public_url)
    .map_err(|failure| failure.source)
}

// Sending needs a real PDF to publish and attach, so these exercise the orchestration
// only in the `pdf` build; without the feature the seam renders no PDF and send refuses.
#[cfg(all(test, feature = "pdf"))]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::{NigelError, Result};
    use crate::invoicing::clients::add_client;
    use crate::invoicing::gateway::{
        AssetPublisher, Mailer, PaidSession, PaymentGateway, PaymentLink,
    };
    use crate::invoicing::invoices::{create_invoice, get_invoice, NewLineItem};
    use crate::invoicing::render_html::DEFAULT_TEMPLATE;
    use crate::migrations::run_migrations;
    use crate::models::{Client, Invoice};
    use std::cell::RefCell;

    fn brand(contact_email: &str) -> Branding<'_> {
        Branding {
            template: DEFAULT_TEMPLATE,
            company: "",
            contact_email,
        }
    }

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    struct FakeGw {
        create_calls: RefCell<u32>,
    }
    impl PaymentGateway for FakeGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
            *self.create_calls.borrow_mut() += 1;
            Ok(PaymentLink {
                id: "pl_1".into(),
                url: "https://pay/x".into(),
            })
        }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> {
            Ok(vec![])
        }
    }
    struct FakePub;
    impl AssetPublisher for FakePub {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(format!("https://billing.rygn.io/i/{token}/"))
        }
    }
    struct CapturePub {
        html: RefCell<String>,
    }
    impl AssetPublisher for CapturePub {
        fn publish(&self, token: &str, h: &[u8], _p: &[u8]) -> Result<String> {
            *self.html.borrow_mut() = String::from_utf8(h.to_vec()).unwrap();
            Ok(format!("https://billing.example.test/i/{token}/"))
        }
    }
    struct FailPub;
    impl AssetPublisher for FailPub {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Err(NigelError::Other("upload down".into()))
        }
    }
    #[derive(Default)]
    struct FakeMail {
        sent: RefCell<u32>,
        subject: RefCell<String>,
    }
    impl Mailer for FakeMail {
        fn send_invoice(&self, _to: &str, s: &str, _h: &str, _p: &[u8]) -> Result<()> {
            *self.sent.borrow_mut() += 1;
            *self.subject.borrow_mut() = s.to_string();
            Ok(())
        }
    }

    fn seed(conn: &rusqlite::Connection) -> i64 {
        let cid = add_client(conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
    }

    #[test]
    fn happy_path_publishes_emails_and_marks_sent() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();
        let url = send_invoice(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap();
        assert!(url.starts_with("https://billing.rygn.io/i/"));
        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.status, "sent");
        assert_eq!(inv.stripe_payment_link_id.as_deref(), Some("pl_1"));
        assert_eq!(*mail.sent.borrow(), 1);
    }

    #[test]
    fn send_invoice_refuses_a_void_invoice_without_the_cli_wrapper() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        crate::invoicing::invoices::void_invoice(&conn, id, "2026-08-06").unwrap();
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();

        let err = send_invoice(
            &conn,
            id,
            "2026-08-07",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap_err();

        assert!(
            matches!(err, NigelError::Conflict { code: "void", .. }),
            "got: {err:?}"
        );
        // Refused before any network call, so no payment link was created.
        assert_eq!(*gw.create_calls.borrow(), 0);
        assert_eq!(*mail.sent.borrow(), 0);
    }

    #[test]
    fn publish_failure_leaves_draft_and_sends_no_email() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();
        let err = send_invoice(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &FailPub,
            &mail,
        );
        assert!(err.is_err());
        assert_eq!(get_invoice(&conn, id).unwrap().status, "draft");
        assert_eq!(*mail.sent.borrow(), 0);
    }

    #[test]
    fn published_html_carries_the_supplied_contact_email() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();
        let publisher = CapturePub {
            html: RefCell::new(String::new()),
        };
        send_invoice(
            &conn,
            id,
            "2026-08-04",
            &brand("ap@acme.test"),
            &gw,
            &publisher,
            &mail,
        )
        .unwrap();
        assert!(publisher.html.borrow().contains("Contact ap@acme.test"));
    }

    #[test]
    fn send_renders_with_the_supplied_template() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();
        let publisher = CapturePub {
            html: RefCell::new(String::new()),
        };
        let branding = Branding {
            template: "<p>CUSTOM {{NUMBER}} {{CLIENT}} {{ROWS}} {{TOTAL}}</p>",
            company: "",
            contact_email: "billing@example.test",
        };
        send_invoice(&conn, id, "2026-08-04", &branding, &gw, &publisher, &mail).unwrap();

        let html = publisher.html.borrow();
        assert!(html.contains("CUSTOM"), "got: {html}");
        assert!(!html.contains("Direct deposit"));
    }

    #[test]
    fn the_subject_names_the_company_when_there_is_one() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();
        let branding = Branding {
            template: DEFAULT_TEMPLATE,
            company: "Acme LLC",
            contact_email: "billing@example.test",
        };
        send_invoice(&conn, id, "2026-08-04", &branding, &gw, &FakePub, &mail).unwrap();
        assert_eq!(*mail.subject.borrow(), "Invoice #1248 from Acme LLC");
    }

    #[test]
    fn the_subject_omits_the_from_clause_when_there_is_no_company() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();
        send_invoice(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap();
        assert_eq!(*mail.subject.borrow(), "Invoice #1248");
    }

    // -----------------------------------------------------------------------
    // The step trace
    // -----------------------------------------------------------------------

    struct FailMail;
    impl Mailer for FailMail {
        fn send_invoice(&self, _t: &str, _s: &str, _h: &str, _p: &[u8]) -> Result<()> {
            Err(NigelError::Other("mailgun 401: Invalid private key".into()))
        }
    }

    /// A publisher that answers the way R2 does when the signature is wrong.
    struct ForbiddenPub;
    impl AssetPublisher for ForbiddenPub {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Err(NigelError::Other(
                "r2 403: <Error><Code>SignatureDoesNotMatch</Code></Error>".into(),
            ))
        }
    }

    fn steps(outcome: &SendOutcome) -> Vec<(SendStep, StepOutcome)> {
        outcome.steps.clone()
    }

    #[test]
    fn a_successful_send_reports_every_step_and_marks_a_resent_link_reused() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();

        let first = send_invoice_traced(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .expect("sends");

        assert_eq!(
            steps(&first),
            vec![
                (SendStep::Load, StepOutcome::Ok),
                (SendStep::Precheck, StepOutcome::Ok),
                (SendStep::PaymentLink, StepOutcome::Ok),
                (SendStep::Render, StepOutcome::Ok),
                (SendStep::Publish, StepOutcome::Ok),
                (SendStep::Email, StepOutcome::Ok),
                (SendStep::Record, StepOutcome::Ok),
            ]
        );
        assert_eq!(first.status, "sent");
        assert_eq!(first.payment_link_url.as_deref(), Some("https://pay/x"));
        assert!(first.public_url.starts_with("https://billing.rygn.io/i/"));

        let again = send_invoice_traced(
            &conn,
            id,
            "2026-08-05",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .expect("resends");
        assert!(
            steps(&again).contains(&(SendStep::PaymentLink, StepOutcome::Reused)),
            "got: {:?}",
            steps(&again)
        );
        assert_eq!(*gw.create_calls.borrow(), 1);
    }

    #[test]
    fn a_publish_failure_names_the_step_and_says_no_email_went_out() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();

        let failure = send_invoice_traced(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &ForbiddenPub,
            &mail,
        )
        .unwrap_err();

        assert_eq!(failure.step, SendStep::Publish);
        assert!(!failure.email_sent);
        assert_eq!(
            failure.completed,
            vec![
                SendStep::Load,
                SendStep::Precheck,
                SendStep::PaymentLink,
                SendStep::Render,
            ]
        );
        assert_eq!(failure.invoice_status.as_deref(), Some("draft"));
        assert!(
            failure.source.to_string().contains("SignatureDoesNotMatch"),
            "the upstream's own words: {}",
            failure.source
        );
        assert_eq!(get_invoice(&conn, id).unwrap().status, "draft");
        assert_eq!(*mail.sent.borrow(), 0);
    }

    /// The one failure that is not safe to retry.
    #[test]
    fn a_mail_failure_is_the_last_step_a_retry_is_still_safe_after() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };

        let failure = send_invoice_traced(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &FailMail,
        )
        .unwrap_err();

        assert_eq!(failure.step, SendStep::Email);
        assert!(!failure.email_sent, "the step that failed did not happen");
        assert!(failure.completed.contains(&SendStep::Publish));
        // Nothing was recorded, so the invoice is still a draft to resend.
        assert_eq!(get_invoice(&conn, id).unwrap().status, "draft");
    }

    #[test]
    fn a_client_with_no_email_fails_at_precheck_before_any_network_call() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Globex", None, None, None).unwrap();
        let items = vec![NewLineItem {
            description: "W".into(),
            quantity: 1.0,
            unit_amount: 100.0,
        }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();
        let failure = send_invoice_traced(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap_err();

        assert_eq!(failure.step, SendStep::Precheck);
        assert_eq!(failure.completed, vec![SendStep::Load]);
        assert!(
            matches!(
                failure.source,
                NigelError::Conflict {
                    code: "client_missing_email",
                    ..
                }
            ),
            "got: {:?}",
            failure.source
        );
        assert_eq!(*gw.create_calls.borrow(), 0, "no link was created");
        assert_eq!(*mail.sent.borrow(), 0);
    }

    #[test]
    fn a_void_invoice_fails_at_precheck_with_the_data_layers_own_conflict() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        crate::invoicing::invoices::void_invoice(&conn, id, "2026-08-06").unwrap();
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };

        let failure = send_invoice_traced(
            &conn,
            id,
            "2026-08-07",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &FakeMail::default(),
        )
        .unwrap_err();

        assert_eq!(failure.step, SendStep::Precheck);
        assert!(matches!(
            failure.source,
            NigelError::Conflict { code: "void", .. }
        ));
        assert_eq!(failure.invoice_status.as_deref(), Some("void"));
    }

    /// The wrapper's contract: `cli/invoice.rs` and every test above it see the
    /// public URL and the same error text they always did.
    #[test]
    fn send_invoice_still_returns_the_public_url_and_the_same_error_text() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();

        let url = send_invoice(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap();
        assert!(url.starts_with("https://billing.rygn.io/i/"));

        let (_d2, other) = test_conn();
        let second = seed(&other);
        let err = send_invoice(
            &other,
            second,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &ForbiddenPub,
            &mail,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("SignatureDoesNotMatch"),
            "got: {err}"
        );
    }

    #[test]
    fn resend_reuses_existing_payment_link() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw {
            create_calls: RefCell::new(0),
        };
        let mail = FakeMail::default();
        send_invoice(
            &conn,
            id,
            "2026-08-04",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap();
        send_invoice(
            &conn,
            id,
            "2026-08-05",
            &brand("billing@example.test"),
            &gw,
            &FakePub,
            &mail,
        )
        .unwrap();
        assert_eq!(*gw.create_calls.borrow(), 1); // created once, reused second time
    }
}
