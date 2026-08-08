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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone)]
pub struct Client {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub billing_address: Option<String>,
    pub notes: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
    pub token: String,
    pub stripe_payment_link_id: Option<String>,
    pub stripe_payment_link_url: Option<String>,
    pub published_at: Option<String>,
    pub voided_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InvoicePayment {
    pub id: Option<i64>,
    pub invoice_id: i64,
    pub amount: f64,
    pub paid_date: String,
    pub method: String,
    pub stripe_checkout_session_id: Option<String>,
}
