use crate::error::Result;
use crate::models::{Client, Invoice};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PaymentLink {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PaidSession {
    pub session_id: String,
    pub amount: f64,
}

#[allow(dead_code)]
pub trait PaymentGateway {
    fn create_payment_link(&self, invoice: &Invoice, client: &Client) -> Result<PaymentLink>;
    fn paid_sessions(&self, payment_link_id: &str) -> Result<Vec<PaidSession>>;
}

#[allow(dead_code)]
pub trait AssetPublisher {
    fn publish(&self, token: &str, html: &[u8], pdf: &[u8]) -> Result<String>;
}

#[allow(dead_code)]
pub trait Mailer {
    fn send_invoice(&self, to: &str, subject: &str, html: &str, pdf: &[u8]) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ok1;
    impl AssetPublisher for Ok1 {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> crate::error::Result<String> {
            Ok(format!("https://billing.rygn.io/i/{token}/"))
        }
    }

    #[test]
    fn publisher_trait_returns_url() {
        let url = Ok1.publish("tok", b"<html>", b"%PDF").unwrap();
        assert_eq!(url, "https://billing.rygn.io/i/tok/");
    }
}
