use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::types::{Error, error_chain};

/// Everything a handler can fail with. `sqlx` errors map to 500 through the
/// `From` impl; handlers use [`ApiError::not_found`] for missing resources so
/// the response stays specific.
#[derive(Debug)]
pub enum ApiError {
    NotFound(&'static str),
    BadRequest(String),
    MethodNotAllowed,
    Internal(Error),
}

impl ApiError {
    pub fn not_found(what: &'static str) -> Self {
        Self::NotFound(what)
    }

    pub fn internal(error: impl Into<Error>) -> Self {
        Self::Internal(error.into())
    }
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    status: u16,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(what) => (StatusCode::NOT_FOUND, format!("{what} not found")),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                "method not allowed".to_string(),
            ),
            Self::Internal(error) => {
                // The chain is the useful part; the client only gets the status.
                tracing::error!("api error: {}", error_chain(&error));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };

        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    status: status.as_u16(),
                    message,
                },
            }),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(error)
    }
}
