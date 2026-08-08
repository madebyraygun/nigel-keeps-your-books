use std::time::Duration;

use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

use crate::error::{NigelError, Result};
use crate::invoicing::gateway::AssetPublisher;

pub fn object_key(token: &str, filename: &str) -> String {
    format!("i/{token}/{filename}")
}

pub fn public_url(public_base_url: &str, token: &str) -> String {
    format!("{}/{}/", public_base_url.trim_end_matches('/'), token)
}

fn ensure_success(status: reqwest::StatusCode, body: &str) -> Result<()> {
    if status.is_success() {
        return Ok(());
    }
    Err(NigelError::Other(format!("r2 {status}: {body}")))
}

pub struct R2Publisher {
    pub account_id: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub public_base_url: String,
}

impl R2Publisher {
    fn put(&self, key: &str, body: &[u8], content_type: &str) -> Result<()> {
        let endpoint = format!("https://{}.r2.cloudflarestorage.com", self.account_id)
            .parse()
            .map_err(|e| NigelError::Other(format!("r2 endpoint: {e}")))?;
        let bucket = Bucket::new(endpoint, UrlStyle::Path, self.bucket.clone(), "auto")
            .map_err(|e| NigelError::Other(format!("r2 bucket: {e}")))?;
        let creds = Credentials::new(self.access_key.clone(), self.secret_key.clone());

        let action = bucket.put_object(Some(&creds), key);
        let signed = action.sign(Duration::from_secs(300));

        let resp = crate::invoicing::http_client()
            .put(signed)
            .header("content-type", content_type)
            .body(body.to_vec())
            .send()
            .map_err(|e| NigelError::Other(format!("r2 put: {e}")))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| NigelError::Other(e.to_string()))?;
        ensure_success(status, &text)
    }
}

impl AssetPublisher for R2Publisher {
    fn publish(&self, token: &str, html: &[u8], pdf: &[u8]) -> Result<String> {
        self.put(
            &object_key(token, "index.html"),
            html,
            "text/html; charset=utf-8",
        )?;
        self.put(&object_key(token, "invoice.pdf"), pdf, "application/pdf")?;
        Ok(public_url(&self.public_base_url, token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_key_layout() {
        assert_eq!(object_key("abc", "index.html"), "i/abc/index.html");
        assert_eq!(object_key("abc", "invoice.pdf"), "i/abc/invoice.pdf");
    }

    #[test]
    fn public_url_joins_base_and_token_with_single_slash() {
        assert_eq!(
            public_url("https://billing.rygn.io/i", "abc"),
            "https://billing.rygn.io/i/abc/"
        );
        assert_eq!(
            public_url("https://billing.rygn.io/i/", "abc"),
            "https://billing.rygn.io/i/abc/"
        );
    }

    #[test]
    fn ensure_success_rejects_non_2xx_and_keeps_r2_message() {
        let err = ensure_success(
            reqwest::StatusCode::FORBIDDEN,
            "<Error><Code>SignatureDoesNotMatch</Code></Error>",
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("403"), "status missing from {msg:?}");
        assert!(
            msg.contains("SignatureDoesNotMatch"),
            "r2 message missing from {msg:?}"
        );
    }

    #[test]
    fn ensure_success_accepts_2xx() {
        assert!(ensure_success(reqwest::StatusCode::OK, "").is_ok());
    }
}
