use axum::extract::{FromRequestParts, Path, Query};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::api::error::ApiError;

/// `Query` with the API's uniform JSON error body.
///
/// Axum's built-in `Query` rejection is a plain-text 400; this wrapper keeps
/// every failure in the `{ "error": { ... } }` shape clients already parse.
pub struct ApiQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiQuery<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|rejection| ApiError::BadRequest(rejection.body_text()))?;
        Ok(Self(value))
    }
}

/// `Path` with the same JSON error body as [`ApiQuery`]. Without this, a
/// non-numeric id would fall back to axum's plain-text 400.
pub struct ApiPath<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(|rejection| ApiError::BadRequest(rejection.body_text()))?;
        Ok(Self(value))
    }
}
