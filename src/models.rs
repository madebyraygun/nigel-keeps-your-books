use serde::Serialize;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub account_type: String,
    pub institution: Option<String>,
    pub last_four: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub category_type: String,
    pub parent_id: Option<i64>,
    pub tax_line: Option<String>,
    pub form_line: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: Option<i64>,
    pub account_id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub category_id: Option<i64>,
    pub vendor: Option<String>,
    pub notes: Option<String>,
    pub is_flagged: bool,
    pub flag_reason: Option<String>,
    pub import_id: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: Option<i64>,
    pub pattern: String,
    pub category_id: i64,
    pub match_type: String,
    pub vendor: Option<String>,
    pub priority: i64,
    pub hit_count: i64,
    pub is_active: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRecord {
    pub id: Option<i64>,
    pub filename: String,
    pub account_id: i64,
    pub record_count: Option<i64>,
    pub date_range_start: Option<String>,
    pub date_range_end: Option<String>,
    pub checksum: Option<String>,
}

/// Intermediate representation from a CSV/XLSX parser before DB insert.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRow {
    pub date: String,
    pub description: String,
    pub amount: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvoiceStatus {
    Draft,
    Sent,
    Partial,
    Paid,
    Overdue,
    Void,
}

impl InvoiceStatus {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Sent => "sent",
            Self::Partial => "partial",
            Self::Paid => "paid",
            Self::Overdue => "overdue",
            Self::Void => "void",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub billing_address: Option<String>,
    pub notes: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceLineItem {
    pub id: Option<i64>,
    pub invoice_id: Option<i64>,
    pub description: String,
    pub quantity: f64,
    pub unit_amount: f64,
    pub line_total: f64,
    pub position: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invoice {
    pub id: i64,
    pub number: i64,
    pub client_id: i64,
    pub issue_date: String,
    pub due_date: Option<String>,
    pub status: String,
    pub currency: String,
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
    pub notes: Option<String>,
    pub terms: Option<String>,
    /// The only access control on a published invoice, so it never crosses the
    /// wire: a list endpoint carrying one token per row would put every
    /// invoice's access control into devtools history and any response cache.
    #[serde(skip_serializing)]
    pub token: String,
    pub stripe_payment_link_id: Option<String>,
    pub stripe_payment_link_url: Option<String>,
    pub published_at: Option<String>,
    pub voided_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicePayment {
    pub id: Option<i64>,
    pub invoice_id: i64,
    pub amount: f64,
    pub paid_date: String,
    pub method: String,
    pub stripe_checkout_session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoice() -> Invoice {
        Invoice {
            id: 7,
            number: 1250,
            client_id: 3,
            issue_date: "2026-03-01".into(),
            due_date: Some("2026-03-31".into()),
            status: "partial".into(),
            currency: "USD".into(),
            subtotal: 3200.0,
            tax: 0.0,
            total: 3200.0,
            notes: Some("Thanks".into()),
            terms: Some("Net 30".into()),
            token: "aBc123".into(),
            stripe_payment_link_id: Some("plink_1".into()),
            stripe_payment_link_url: Some("https://buy.stripe.test/x".into()),
            published_at: Some("2026-03-01".into()),
            voided_at: None,
        }
    }

    #[test]
    fn the_invoicing_structs_serialize_as_camel_case() {
        let value = serde_json::to_value(invoice()).unwrap();
        for key in [
            "clientId",
            "issueDate",
            "dueDate",
            "stripePaymentLinkId",
            "stripePaymentLinkUrl",
            "publishedAt",
            "voidedAt",
        ] {
            assert!(value.get(key).is_some(), "missing {key} in {value}");
        }

        let client = Client {
            id: 3,
            name: "Acme Co".into(),
            email: Some("ap@acme.test".into()),
            billing_address: Some("1 Main St".into()),
            notes: None,
        };
        let value = serde_json::to_value(client).unwrap();
        assert!(value.get("billingAddress").is_some(), "got {value}");

        let item = InvoiceLineItem {
            id: Some(1),
            invoice_id: Some(7),
            description: "Consulting".into(),
            quantity: 10.0,
            unit_amount: 150.0,
            line_total: 1500.0,
            position: 0,
        };
        let value = serde_json::to_value(item).unwrap();
        for key in ["invoiceId", "unitAmount", "lineTotal"] {
            assert!(value.get(key).is_some(), "missing {key} in {value}");
        }

        let payment = InvoicePayment {
            id: Some(2),
            invoice_id: 7,
            amount: 2000.0,
            paid_date: "2026-03-10".into(),
            method: "stripe".into(),
            stripe_checkout_session_id: Some("cs_1".into()),
        };
        let value = serde_json::to_value(payment).unwrap();
        for key in ["invoiceId", "paidDate", "stripeCheckoutSessionId"] {
            assert!(value.get(key).is_some(), "missing {key} in {value}");
        }
    }

    #[test]
    fn an_invoice_never_serializes_its_token() {
        let value = serde_json::to_value(invoice()).unwrap();
        assert!(value.get("token").is_none(), "token leaked: {value}");
        assert!(
            !serde_json::to_string(&invoice())
                .unwrap()
                .contains("aBc123"),
            "the token value appears under some other key"
        );
    }

    #[test]
    fn invoice_status_serializes_as_the_word_as_str_returns() {
        for status in [
            InvoiceStatus::Draft,
            InvoiceStatus::Sent,
            InvoiceStatus::Partial,
            InvoiceStatus::Paid,
            InvoiceStatus::Overdue,
            InvoiceStatus::Void,
        ] {
            let word = status.as_str();
            assert_eq!(
                serde_json::to_value(&status).unwrap(),
                serde_json::Value::String(word.to_string())
            );
        }
    }
}
