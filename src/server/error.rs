//! The API error envelope: `{"error": {"code", "message", "details"?}}`.

use std::fmt::Display;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::Value;

use crate::error::NigelError;

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
