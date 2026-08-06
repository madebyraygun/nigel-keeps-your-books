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
    Forbidden,
    NotFound,
    Conflict,
    Locked,
    Internal,
    FeatureDisabled,
}

impl ApiErrorCode {
    pub fn status(self) -> StatusCode {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Locked => StatusCode::LOCKED,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Self::FeatureDisabled => StatusCode::NOT_IMPLEMENTED,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::BadRequest => "bad_request",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
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

    pub fn forbidden() -> Self {
        Self::new(
            ApiErrorCode::Forbidden,
            "Request rejected: this server only accepts local requests.",
        )
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::NotFound, message)
    }

    pub fn conflict(message: impl Into<String>, details: Value) -> Self {
        Self::new(ApiErrorCode::Conflict, message).with_details(details)
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
            NigelError::UnknownAccount(_)
            | NigelError::UnknownCategory(_)
            | NigelError::NoTransactions { .. } => Self::not_found(err.to_string()),
            NigelError::UnknownFormat(_) | NigelError::NoImporter(_) => {
                Self::bad_request(err.to_string())
            }
            other => Self::internal(other),
        }
    }
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
            (ApiErrorCode::Forbidden, 403, "forbidden"),
            (ApiErrorCode::NotFound, 404, "not_found"),
            (ApiErrorCode::Conflict, 409, "conflict"),
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
    fn nigel_errors_map_to_sensible_codes() {
        assert_eq!(
            ApiError::from(NigelError::UnknownAccount("Checking".into())).code(),
            ApiErrorCode::NotFound
        );
        assert_eq!(
            ApiError::from(NigelError::UnknownFormat("nope".into())).code(),
            ApiErrorCode::BadRequest
        );
        assert_eq!(
            ApiError::from(NigelError::NotInitialized).code(),
            ApiErrorCode::Internal
        );
    }
}
