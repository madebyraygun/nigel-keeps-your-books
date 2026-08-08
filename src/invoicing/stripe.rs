use serde::Deserialize;

use crate::error::{NigelError, Result};
use crate::invoicing::gateway::{PaidSession, PaymentGateway, PaymentLink};
use crate::models::{Client, Invoice};

fn to_cents(amount: f64) -> i64 {
    (amount * 100.0).round() as i64
}

pub fn price_params(invoice: &Invoice) -> Vec<(String, String)> {
    vec![
        ("currency".into(), invoice.currency.to_lowercase()),
        ("unit_amount".into(), to_cents(invoice.total).to_string()),
        (
            "product_data[name]".into(),
            format!("Invoice #{}", invoice.number),
        ),
    ]
}

pub fn payment_link_params(price_id: &str, invoice: &Invoice) -> Vec<(String, String)> {
    vec![
        ("line_items[0][price]".into(), price_id.to_string()),
        ("line_items[0][quantity]".into(), "1".into()),
        ("metadata[invoice_id]".into(), invoice.number.to_string()),
    ]
}

#[derive(Deserialize)]
struct SessionList {
    data: Vec<Session>,
}

#[derive(Deserialize)]
struct Session {
    id: String,
    status: String,
    payment_status: String,
    amount_total: i64,
}

pub fn parse_paid_sessions(json: &str) -> Result<Vec<PaidSession>> {
    let list: SessionList =
        serde_json::from_str(json).map_err(|e| NigelError::Other(format!("stripe parse: {e}")))?;
    Ok(list
        .data
        .into_iter()
        .filter(|s| s.status == "complete" && s.payment_status == "paid")
        .map(|s| PaidSession {
            session_id: s.id,
            amount: s.amount_total as f64 / 100.0,
        })
        .collect())
}

fn ensure_success(status: reqwest::StatusCode, body: &str) -> Result<()> {
    if status.is_success() {
        return Ok(());
    }
    Err(NigelError::Other(format!("stripe {status}: {body}")))
}

fn required_str(value: &serde_json::Value, field: &str) -> Result<String> {
    value[field]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| NigelError::Other(format!("stripe response missing {field}")))
}

fn payment_link_from_value(value: &serde_json::Value) -> Result<PaymentLink> {
    Ok(PaymentLink {
        id: required_str(value, "id")?,
        url: required_str(value, "url")?,
    })
}

pub struct StripeClient {
    pub secret_key: String,
}

impl StripeClient {
    fn post_form(&self, url: &str, form: &[(String, String)]) -> Result<serde_json::Value> {
        let resp = crate::invoicing::http_client()
            .post(url)
            .bearer_auth(&self.secret_key)
            .form(form)
            .send()
            .map_err(|e| NigelError::Other(format!("stripe request: {e}")))?;
        let status = resp.status();
        let body = resp.text().map_err(|e| NigelError::Other(e.to_string()))?;
        ensure_success(status, &body)?;
        serde_json::from_str(&body).map_err(|e| NigelError::Other(e.to_string()))
    }
}

impl PaymentGateway for StripeClient {
    fn create_payment_link(&self, invoice: &Invoice, _client: &Client) -> Result<PaymentLink> {
        let price = self.post_form("https://api.stripe.com/v1/prices", &price_params(invoice))?;
        let price_id = required_str(&price, "id")?;
        let link = self.post_form(
            "https://api.stripe.com/v1/payment_links",
            &payment_link_params(&price_id, invoice),
        )?;
        payment_link_from_value(&link)
    }

    fn paid_sessions(&self, payment_link_id: &str) -> Result<Vec<PaidSession>> {
        let url = format!(
            "https://api.stripe.com/v1/checkout/sessions?payment_link={payment_link_id}&limit=100"
        );
        let resp = crate::invoicing::http_client()
            .get(&url)
            .bearer_auth(&self.secret_key)
            .send()
            .map_err(|e| NigelError::Other(format!("stripe request: {e}")))?;
        let status = resp.status();
        let body = resp.text().map_err(|e| NigelError::Other(e.to_string()))?;
        ensure_success(status, &body)?;
        parse_paid_sessions(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Invoice;

    fn inv() -> Invoice {
        Invoice {
            id: 1,
            number: 1248,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: None,
            status: "draft".into(),
            currency: "USD".into(),
            subtotal: 250.0,
            tax: 0.0,
            total: 250.0,
            notes: None,
            terms: None,
            token: "t".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: None,
            voided_at: None,
        }
    }

    #[test]
    fn price_params_are_lowercase_currency_and_cents() {
        let p = price_params(&inv());
        assert!(p.contains(&("currency".into(), "usd".into())));
        assert!(p.contains(&("unit_amount".into(), "25000".into())));
        assert!(p.iter().any(|(k, _)| k == "product_data[name]"));
    }

    #[test]
    fn payment_link_params_carry_invoice_metadata() {
        let p = payment_link_params("price_123", &inv());
        assert!(p.contains(&("line_items[0][price]".into(), "price_123".into())));
        assert!(p.contains(&("line_items[0][quantity]".into(), "1".into())));
        assert!(p.contains(&("metadata[invoice_id]".into(), "1248".into())));
    }

    #[test]
    fn ensure_success_rejects_non_2xx_and_keeps_stripe_message() {
        let err = ensure_success(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"error":{"message":"Invalid API Key provided"}}"#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("401"), "status missing from {msg:?}");
        assert!(
            msg.contains("Invalid API Key provided"),
            "stripe message missing from {msg:?}"
        );
    }

    #[test]
    fn ensure_success_accepts_2xx() {
        assert!(ensure_success(reqwest::StatusCode::OK, "{}").is_ok());
    }

    #[test]
    fn payment_link_from_value_requires_id_and_url() {
        let ok = serde_json::json!({"id": "plink_1", "url": "https://buy.stripe.com/x"});
        let link = payment_link_from_value(&ok).unwrap();
        assert_eq!(link.id, "plink_1");
        assert_eq!(link.url, "https://buy.stripe.com/x");

        assert!(payment_link_from_value(&serde_json::json!({"id": "plink_1"})).is_err());
        assert!(
            payment_link_from_value(&serde_json::json!({"url": "https://buy.stripe.com/x"}))
                .is_err()
        );
        assert!(payment_link_from_value(&serde_json::json!({})).is_err());
    }

    #[test]
    fn parse_paid_sessions_filters_unpaid() {
        let json = r#"{"object":"list","data":[
            {"id":"cs_1","status":"complete","payment_status":"paid","amount_total":25000},
            {"id":"cs_2","status":"open","payment_status":"unpaid","amount_total":25000},
            {"id":"cs_3","status":"complete","payment_status":"no_payment_required","amount_total":0}
        ]}"#;
        let sessions = parse_paid_sessions(json).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "cs_1");
        assert_eq!(sessions[0].amount, 250.0);
    }
}
