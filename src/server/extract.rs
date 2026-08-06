//! Extractors that answer inside the error envelope.
//!
//! axum's own `Json` and `Path` rejections are `text/plain`, which would make
//! them the only responses on the API a client cannot parse the same way as
//! every other failure. These wrappers are the same extractors with the
//! rejection translated.
//!
//! `POST /api/unlock` deliberately does not use `ApiJson`: its rejection text
//! would quote the offending value, and that value is a password.

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{FromRequest, FromRequestParts, Path, Request};
use axum::http::request::Parts;
use axum::Json;
use serde::de::DeserializeOwned;

use super::error::ApiError;

/// `Json<T>`, with a 400 in the error envelope instead of a plain-text body.
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(bad_json(&rejection)),
        }
    }
}

fn bad_json(rejection: &JsonRejection) -> ApiError {
    ApiError::bad_request(match rejection {
        JsonRejection::MissingJsonContentType(_) => {
            "Expected a JSON body with Content-Type: application/json.".to_string()
        }
        other => other.body_text(),
    })
}

/// `Path<T>`, with a 400 in the error envelope instead of a plain-text body.
pub struct ApiPath<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Path::<T>::from_request_parts(parts, state).await {
            Ok(Path(value)) => Ok(Self(value)),
            Err(rejection) => Err(bad_path(&rejection)),
        }
    }
}

fn bad_path(rejection: &PathRejection) -> ApiError {
    ApiError::bad_request(match rejection {
        PathRejection::FailedToDeserializePathParams(_) => {
            "Expected a numeric id in the path.".to_string()
        }
        other => other.body_text(),
    })
}
