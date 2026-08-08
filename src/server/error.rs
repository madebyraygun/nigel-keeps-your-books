//! The API error envelope: `{"error": {"code", "message", "details"?}}`.

use std::fmt::Display;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::Value;

use crate::error::NigelError;
use crate::invoicing::send::{SendFailure, SendStep, PDF_REQUIRED_MESSAGE};

/// Machine-readable error codes. The wire form is snake_case; the HTTP status
/// is derived from the code so the two can never drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorCode {
    BadRequest,
    Unauthorized,
    InvalidPassword,
    Forbidden,
    NotFound,
    Conflict,
    PayloadTooLarge,
    Locked,
    Internal,
    /// A service this server depends on refused or could not be reached —
    /// Stripe, R2 or Mailgun. Separate from `internal` because the two call for
    /// opposite responses: one is ours to fix, the other is theirs, and a screen
    /// that offers "try again" has to know which it is looking at.
    UpstreamFailed,
    FeatureDisabled,
}

impl ApiErrorCode {
    pub fn status(self) -> StatusCode {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized | Self::InvalidPassword => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Locked => StatusCode::LOCKED,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UpstreamFailed => StatusCode::BAD_GATEWAY,
            Self::FeatureDisabled => StatusCode::NOT_IMPLEMENTED,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::BadRequest => "bad_request",
            Self::Unauthorized => "unauthorized",
            Self::InvalidPassword => "invalid_password",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::PayloadTooLarge => "payload_too_large",
            Self::Locked => "locked",
            Self::Internal => "internal",
            Self::UpstreamFailed => "upstream_failed",
            Self::FeatureDisabled => "feature_disabled",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiError {
    code: ApiErrorCode,
    message: String,
    details: Option<Value>,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::BadRequest, message)
    }

    pub fn unauthorized() -> Self {
        Self::new(
            ApiErrorCode::Unauthorized,
            "Not signed in. Open the URL printed by `nigel serve` to start a session.",
        )
    }

    /// A rejected unlock attempt. `details` carries the attempt budget and the
    /// delay the server already applied before answering.
    pub fn invalid_password(attempts_remaining: u32, retry_after_ms: u128) -> Self {
        Self::new(ApiErrorCode::InvalidPassword, "Wrong password.").with_details(
            serde_json::json!({
                "attemptsRemaining": attempts_remaining,
                "retryAfterMs": retry_after_ms,
            }),
        )
    }

    pub fn forbidden() -> Self {
        Self::new(
            ApiErrorCode::Forbidden,
            "Request rejected: this server only accepts local requests.",
        )
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::NotFound, message)
    }

    /// A `404` that says *which* thing was missing.
    ///
    /// A route that looks up more than one thing answers the same status for
    /// each, so a client branching on the status alone has to guess — and the
    /// review screen guessed "the transaction is gone" for a category that had
    /// merely been deactivated, skipping a transaction it had not reviewed.
    pub fn not_found_because(message: impl Into<String>, reason: &str) -> Self {
        Self::not_found(message).with_details(serde_json::json!({ "reason": reason }))
    }

    pub fn conflict(message: impl Into<String>, details: Value) -> Self {
        Self::new(ApiErrorCode::Conflict, message).with_details(details)
    }

    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::PayloadTooLarge, message)
    }

    pub fn locked() -> Self {
        Self::new(
            ApiErrorCode::Locked,
            "This database is encrypted and has not been unlocked yet.",
        )
    }

    pub fn feature_disabled(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::FeatureDisabled, message)
    }

    pub fn internal(err: impl Display) -> Self {
        Self::new(ApiErrorCode::Internal, err.to_string())
    }

    pub fn code(&self) -> ApiErrorCode {
        self.code
    }

    /// Tag a refusal raised outside the orchestration with the step it belongs
    /// to, so every answer a send can give carries `details.step`.
    pub(crate) fn at_step(mut self, step: SendStep) -> Self {
        self.merge_details(serde_json::json!({ "step": step.as_str() }));
        self
    }

    /// Add context to whatever `details` this error already carries, leaving
    /// what is there alone. A send failure keeps the data layer's own reason and
    /// gains the step it stopped at.
    fn merge_details(&mut self, extra: Value) {
        self.details = match (self.details.take(), extra) {
            (Some(Value::Object(mut existing)), Value::Object(added)) => {
                for (key, value) in added {
                    existing.entry(key).or_insert(value);
                }
                Some(Value::Object(existing))
            }
            // Nothing structured to merge into, so the context is the details.
            (_, extra) => Some(extra),
        };
    }
}

impl From<NigelError> for ApiError {
    fn from(err: NigelError) -> Self {
        match err {
            NigelError::UnknownAccount(_) => {
                Self::not_found_because(err.to_string(), "account_not_found")
            }
            NigelError::UnknownCategory(_) => {
                Self::not_found_because(err.to_string(), "category_not_found")
            }
            NigelError::NotFound(_) => Self::not_found(err.to_string()),
            NigelError::UnknownFormat(_) | NigelError::NoImporter(_) | NigelError::Invalid(_) => {
                Self::bad_request(err.to_string())
            }
            // The account exists and the request was well formed; there is
            // simply nothing in that month to reconcile against.
            NigelError::NoTransactions {
                ref account,
                ref month,
            } => {
                let details = serde_json::json!({
                    "reason": "no_transactions",
                    "account": account,
                    "month": month,
                });
                Self::conflict(err.to_string(), details)
            }
            NigelError::DuplicateName { kind: _, ref name } => {
                let details = serde_json::json!({ "reason": "duplicate_name", "name": name });
                Self::conflict(err.to_string(), details)
            }
            NigelError::Blocked(block) => Self::conflict(
                err.to_string(),
                serde_json::json!({ "reason": block.reason_code(), "count": block.count }),
            ),
            NigelError::Conflict { code, .. } => {
                Self::conflict(err.to_string(), serde_json::json!({ "reason": code }))
            }
            NigelError::Db(_) => Self::internal(redact_key_pragma(err.to_string())),
            other => Self::internal(other),
        }
    }
}

/// A send that stopped, as the one response that says where and whether the
/// email had already gone out.
///
/// The step-to-code decision lives here and nowhere else — a `match` on
/// `SendStep`, so a step added to the vocabulary cannot be added without
/// deciding what it answers with. The upstream's own message is passed through
/// untouched: `r2 403: SignatureDoesNotMatch` is the only information anyone
/// has about why R2 refused, and a sentence we invented instead would be a
/// worse bug report.
impl From<SendFailure> for ApiError {
    fn from(failure: SendFailure) -> Self {
        let SendFailure {
            step,
            completed,
            email_sent,
            invoice_status,
            source,
        } = failure;

        // Enumerated rather than defaulted, for the reason the code match below
        // is: a step added to the vocabulary has to say whose service it is.
        let service = match step {
            SendStep::PaymentLink => Some("stripe"),
            SendStep::Publish => Some("r2"),
            SendStep::Email => Some("mailgun"),
            SendStep::Config
            | SendStep::Load
            | SendStep::Precheck
            | SendStep::Render
            | SendStep::Record => None,
        };

        // A rusqlite failure is never an upstream failure, whichever step it
        // lands on. Telling an R2 outage from a database write that did not
        // land is the whole point of tagging the step, and a 502 for the second
        // one would send the operator to Cloudflare's status page.
        let database_failed = matches!(source, NigelError::Db(_));
        let code = match step {
            _ if database_failed => Some(ApiErrorCode::Internal),
            SendStep::PaymentLink | SendStep::Publish | SendStep::Email => {
                Some(ApiErrorCode::UpstreamFailed)
            }
            // The one render failure a different build would not have.
            SendStep::Render if is_pdf_missing(&source) => Some(ApiErrorCode::FeatureDisabled),
            SendStep::Render | SendStep::Record => Some(ApiErrorCode::Internal),
            // Config, load and precheck refuse for reasons the data layer
            // already names — a missing client is a 404, a void invoice a 409 —
            // so those keep their own code and reason and only gain the step.
            SendStep::Config | SendStep::Load | SendStep::Precheck => None,
        };

        let mut step_details = serde_json::json!({
            "step": step.as_str(),
            "completed": completed.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "emailSent": email_sent,
        });
        if let Some(status) = invoice_status {
            step_details["invoiceStatus"] = serde_json::json!(status);
        }
        if let Some(service) = service {
            step_details["service"] = serde_json::json!(service);
        }

        match code {
            Some(code) => {
                step_details["reason"] = serde_json::json!("send_failed");
                let message = match database_failed {
                    true => redact_key_pragma(source.to_string()),
                    false => source.to_string(),
                };
                Self::new(code, message).with_details(step_details)
            }
            None => {
                let mut error = Self::from(source);
                error.merge_details(step_details);
                error
            }
        }
    }
}

/// The failure a build without the `pdf` feature answers `501` for, told apart
/// from a genuine render failure by the sentence `send.rs` raises it with.
fn is_pdf_missing(source: &NigelError) -> bool {
    matches!(source, NigelError::Other(message) if message == PDF_REQUIRED_MESSAGE)
}

/// rusqlite renders the statement it failed on, and the statement that carries
/// the database password is `PRAGMA key = '…'`. Any rusqlite error is therefore
/// one bad key away from putting the key in a response body, so the text is
/// dropped rather than filtered whenever it mentions that statement.
fn redact_key_pragma(message: String) -> String {
    if message.to_ascii_uppercase().contains("PRAGMA KEY") {
        return "Database error while opening the database.".to_string();
    }
    message
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code.as_str(),
                message: &self.message,
                details: self.details.as_ref(),
            },
        };
        (self.code.status(), Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn codes_map_to_the_documented_statuses() {
        let pairs = [
            (ApiErrorCode::BadRequest, 400, "bad_request"),
            (ApiErrorCode::Unauthorized, 401, "unauthorized"),
            (ApiErrorCode::InvalidPassword, 401, "invalid_password"),
            (ApiErrorCode::Forbidden, 403, "forbidden"),
            (ApiErrorCode::NotFound, 404, "not_found"),
            (ApiErrorCode::Conflict, 409, "conflict"),
            (ApiErrorCode::PayloadTooLarge, 413, "payload_too_large"),
            (ApiErrorCode::Locked, 423, "locked"),
            (ApiErrorCode::Internal, 500, "internal"),
            (ApiErrorCode::UpstreamFailed, 502, "upstream_failed"),
            (ApiErrorCode::FeatureDisabled, 501, "feature_disabled"),
        ];
        for (code, status, name) in pairs {
            assert_eq!(code.status().as_u16(), status, "status for {name}");
            assert_eq!(code.as_str(), name);
        }
    }

    #[test]
    fn envelope_omits_details_when_absent() {
        let err = ApiError::not_found("no such account");
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: err.code.as_str(),
                message: &err.message,
                details: err.details.as_ref(),
            },
        };
        let json = serde_json::to_value(&body).expect("serializes");
        assert_eq!(
            json,
            json!({"error": {"code": "not_found", "message": "no such account"}})
        );
    }

    #[test]
    fn envelope_includes_details_when_present() {
        let err = ApiError::conflict("in use", json!({"reason": "has_transactions", "count": 3}));
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: err.code.as_str(),
                message: &err.message,
                details: err.details.as_ref(),
            },
        };
        let json = serde_json::to_value(&body).expect("serializes");
        assert_eq!(
            json,
            json!({"error": {
                "code": "conflict",
                "message": "in use",
                "details": {"reason": "has_transactions", "count": 3}
            }})
        );
    }

    #[test]
    fn invalid_password_carries_the_attempt_budget() {
        let err = ApiError::invalid_password(2, 1000);
        assert_eq!(err.code(), ApiErrorCode::InvalidPassword);
        assert_eq!(
            err.details,
            Some(json!({"attemptsRemaining": 2, "retryAfterMs": 1000}))
        );
    }

    #[test]
    fn nigel_errors_map_to_sensible_codes() {
        assert_eq!(
            ApiError::from(NigelError::UnknownAccount("Checking".into())).code(),
            ApiErrorCode::NotFound
        );
        assert_eq!(
            ApiError::from(NigelError::NotFound("no rule".into())).code(),
            ApiErrorCode::NotFound
        );
        assert_eq!(
            ApiError::from(NigelError::UnknownFormat("nope".into())).code(),
            ApiErrorCode::BadRequest
        );
        assert_eq!(
            ApiError::from(NigelError::Invalid("bad type".into())).code(),
            ApiErrorCode::BadRequest
        );
        assert_eq!(
            ApiError::from(NigelError::NotInitialized).code(),
            ApiErrorCode::Internal
        );
    }

    /// Every guardrail answers 409 with a reason a client can branch on — the
    /// contract the manager screens render from instead of parsing messages.
    #[test]
    fn guardrails_carry_a_machine_readable_reason() {
        use crate::error::DeleteBlock;

        let cases: [(NigelError, serde_json::Value); 5] = [
            (
                NigelError::Blocked(DeleteBlock::transactions("account", 3)),
                json!({"reason": "has_transactions", "count": 3}),
            ),
            (
                NigelError::Blocked(DeleteBlock::active_rules("category", 2)),
                json!({"reason": "has_active_rules", "count": 2}),
            ),
            (
                NigelError::DuplicateName {
                    kind: "Account",
                    name: "BofA Checking".into(),
                },
                json!({"reason": "duplicate_name", "name": "BofA Checking"}),
            ),
            (
                NigelError::Conflict {
                    code: "already_inactive",
                    message: "Rule 7 is already inactive".into(),
                },
                json!({"reason": "already_inactive"}),
            ),
            (
                NigelError::NoTransactions {
                    account: "BofA Checking".into(),
                    month: "2025-07".into(),
                },
                json!({"reason": "no_transactions", "account": "BofA Checking", "month": "2025-07"}),
            ),
        ];

        for (err, expected) in cases {
            let message = err.to_string();
            let api = ApiError::from(err);
            assert_eq!(api.code(), ApiErrorCode::Conflict, "for {message}");
            assert_eq!(api.details, Some(expected), "for {message}");
            assert_eq!(api.message, message, "the human message is preserved");
        }
    }

    /// rusqlite renders the statement it failed on, and the statement that
    /// carries the database password is `PRAGMA key = '…'`.
    #[test]
    fn a_database_error_never_carries_the_key_back() {
        let leaky = redact_key_pragma(
            "unrecognized token: \"'ab\" in PRAGMA key='ab cd' at offset 11".to_string(),
        );
        assert!(!leaky.contains("ab cd"), "{leaky}");
        assert!(!leaky.to_ascii_uppercase().contains("PRAGMA"), "{leaky}");

        let ordinary = redact_key_pragma("no such table: transactions".to_string());
        assert_eq!(ordinary, "no such table: transactions");
    }

    /// One send failure, with the step and the source that would have produced
    /// it, and everything else as a real one carries it.
    fn failure(step: SendStep, source: NigelError) -> SendFailure {
        SendFailure {
            step,
            completed: vec![SendStep::Load, SendStep::Precheck],
            email_sent: step == SendStep::Record,
            invoice_status: Some("draft".to_string()),
            source,
        }
    }

    #[test]
    fn an_upstream_send_failure_is_a_502_naming_the_step_and_the_service() {
        let cases = [
            (SendStep::PaymentLink, "stripe", "stripe 402: card_declined"),
            (SendStep::Publish, "r2", "r2 403: SignatureDoesNotMatch"),
            (
                SendStep::Email,
                "mailgun",
                "mailgun 401: Invalid private key",
            ),
        ];

        for (step, service, message) in cases {
            let api = ApiError::from(failure(step, NigelError::Other(message.into())));
            assert_eq!(api.code(), ApiErrorCode::UpstreamFailed, "for {service}");
            assert_eq!(api.code.status().as_u16(), 502, "for {service}");
            assert_eq!(
                api.message, message,
                "the upstream's own words, never reconstructed"
            );
            let details = api.details.expect("details");
            assert_eq!(details["reason"], "send_failed");
            assert_eq!(details["step"], step.as_str());
            assert_eq!(details["service"], service);
            assert_eq!(details["completed"], json!(["load", "precheck"]));
            assert_eq!(details["invoiceStatus"], "draft");
        }
    }

    /// An R2 outage and a database write that did not land must not read the
    /// same: one sends the operator to Cloudflare, the other to their disk.
    #[test]
    fn a_database_failure_inside_a_send_stays_a_500() {
        let db = NigelError::Db(rusqlite::Error::QueryReturnedNoRows);
        let api = ApiError::from(failure(SendStep::Publish, db));
        assert_eq!(api.code(), ApiErrorCode::Internal);
        assert_eq!(api.details.expect("details")["step"], "publish");
    }

    #[test]
    fn a_send_that_failed_after_the_email_says_so() {
        let api = ApiError::from(failure(
            SendStep::Record,
            NigelError::Other("disk full".into()),
        ));
        assert_eq!(api.code(), ApiErrorCode::Internal);
        let details = api.details.expect("details");
        assert_eq!(details["step"], "record");
        assert_eq!(
            details["emailSent"], true,
            "the client already has the invoice, so this is not safe to retry"
        );
    }

    #[test]
    fn a_render_failure_without_the_pdf_feature_is_a_501() {
        let api = ApiError::from(failure(
            SendStep::Render,
            NigelError::Other(PDF_REQUIRED_MESSAGE.into()),
        ));
        assert_eq!(api.code(), ApiErrorCode::FeatureDisabled);
        assert_eq!(api.message, PDF_REQUIRED_MESSAGE);

        // Any other render failure is ours, not the build's.
        let other = ApiError::from(failure(
            SendStep::Render,
            NigelError::Other("template expansion failed".into()),
        ));
        assert_eq!(other.code(), ApiErrorCode::Internal);
    }

    /// The refusals that predate the send path keep their own code and reason —
    /// a void invoice reads the same here as it does on `PATCH` — and only gain
    /// the step they stopped at.
    #[test]
    fn a_precheck_refusal_keeps_the_data_layers_own_reason() {
        let api = ApiError::from(failure(
            SendStep::Precheck,
            NigelError::Conflict {
                code: "void",
                message: "Invoice #1247 is void and cannot be sent".into(),
            },
        ));
        assert_eq!(api.code(), ApiErrorCode::Conflict);
        let details = api.details.expect("details");
        assert_eq!(details["reason"], "void");
        assert_eq!(details["step"], "precheck");

        let missing = ApiError::from(failure(
            SendStep::Load,
            NigelError::NotFound("Client 9 not found".into()),
        ));
        assert_eq!(missing.code(), ApiErrorCode::NotFound);
        assert_eq!(missing.details.expect("details")["step"], "load");
    }

    /// A send failure carries the step, not the settings the step was
    /// configured with: the keys never leave the process, and `missing` (on the
    /// unconfigured refusal) is key names only.
    #[test]
    fn a_send_failure_body_never_carries_a_secret() {
        let steps = [
            SendStep::Config,
            SendStep::Load,
            SendStep::Precheck,
            SendStep::PaymentLink,
            SendStep::Render,
            SendStep::Publish,
            SendStep::Email,
            SendStep::Record,
        ];
        let allowed = [
            "reason",
            "step",
            "service",
            "completed",
            "emailSent",
            "invoiceStatus",
        ];

        for step in steps {
            let api = ApiError::from(failure(
                step,
                NigelError::Other("upstream said no".to_string()),
            ));
            let details = api.details.expect("details");
            for key in details.as_object().expect("an object").keys() {
                assert!(
                    allowed.contains(&key.as_str()),
                    "{step:?} put an unexpected key in details: {key}"
                );
            }
        }
    }

    #[test]
    fn not_found_variants_say_which_thing_was_missing() {
        let account = ApiError::from(NigelError::UnknownAccount("Books".into()));
        assert_eq!(
            account.details,
            Some(json!({ "reason": "account_not_found" }))
        );

        let category = ApiError::from(NigelError::UnknownCategory("Meals".into()));
        assert_eq!(
            category.details,
            Some(json!({ "reason": "category_not_found" }))
        );
    }
}
