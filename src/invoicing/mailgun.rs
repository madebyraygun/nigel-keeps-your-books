use crate::error::{NigelError, Result};
use crate::invoicing::gateway::Mailer;

pub fn message_fields(from: &str, to: &str, subject: &str, html: &str) -> Vec<(String, String)> {
    vec![
        ("from".into(), from.to_string()),
        ("to".into(), to.to_string()),
        ("subject".into(), subject.to_string()),
        ("html".into(), html.to_string()),
    ]
}

fn ensure_success(status: reqwest::StatusCode, body: &str) -> Result<()> {
    if status.is_success() {
        return Ok(());
    }
    Err(NigelError::Other(format!("mailgun {status}: {body}")))
}

pub struct MailgunClient {
    pub api_key: String,
    pub domain: String,
    pub from: String,
}

impl Mailer for MailgunClient {
    fn send_invoice(&self, to: &str, subject: &str, html: &str, pdf: &[u8]) -> Result<()> {
        let url = format!("https://api.mailgun.net/v3/{}/messages", self.domain);

        let mut form = reqwest::blocking::multipart::Form::new();
        for (name, value) in message_fields(&self.from, to, subject, html) {
            form = form.text(name, value);
        }
        let part = reqwest::blocking::multipart::Part::bytes(pdf.to_vec())
            .file_name("invoice.pdf")
            .mime_str("application/pdf")
            .map_err(|e| NigelError::Other(format!("mailgun attachment: {e}")))?;
        form = form.part("attachment", part);

        let resp = crate::invoicing::http_client()
            .post(&url)
            .basic_auth("api", Some(&self.api_key))
            .multipart(form)
            .send()
            .map_err(|e| NigelError::Other(format!("mailgun request: {e}")))?;
        let status = resp.status();
        let body = resp.text().map_err(|e| NigelError::Other(e.to_string()))?;
        ensure_success(status, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_fields_include_from_to_subject_html() {
        let f = message_fields("billing@rygn.io", "a@b.test", "Invoice #1248", "<p>hi</p>");
        assert!(f.contains(&("from".into(), "billing@rygn.io".into())));
        assert!(f.contains(&("to".into(), "a@b.test".into())));
        assert!(f.contains(&("subject".into(), "Invoice #1248".into())));
        assert!(f.contains(&("html".into(), "<p>hi</p>".into())));
    }

    #[test]
    fn ensure_success_rejects_non_2xx_and_keeps_mailgun_message() {
        let err = ensure_success(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"message":"Invalid private key"}"#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("401"), "status missing from {msg:?}");
        assert!(
            msg.contains("Invalid private key"),
            "mailgun message missing from {msg:?}"
        );
    }

    #[test]
    fn ensure_success_accepts_2xx() {
        assert!(ensure_success(reqwest::StatusCode::OK, "{}").is_ok());
    }
}
