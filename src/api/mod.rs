//! Read-only HTTP API over the crawled directory.
//!
//! The router is deliberately layer-free so tests can call it directly:
//! `api::router(pool)` returns a `Router`, and the `halal-api` binary adds
//! tracing, CORS, and graceful shutdown around it.

pub mod categories;
pub mod companies;
pub mod dto;
pub mod error;
pub mod extract;
pub mod params;
pub mod products;
pub mod runs;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sqlx::PgPool;

use crate::api::dto::Health;
use crate::api::error::ApiError;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

/// Build the API router. No state, middleware, or I/O beyond the pool.
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/companies", get(companies::list))
        .route("/api/v1/companies/{id}", get(companies::detail))
        .route("/api/v1/companies/{id}/products", get(companies::products))
        .route("/api/v1/products", get(products::list))
        .route("/api/v1/products/{id}", get(products::detail))
        .route("/api/v1/categories", get(categories::list))
        .route("/api/v1/scrape-runs", get(runs::list))
        .with_state(AppState { pool })
        .fallback(route_not_found)
        .method_not_allowed_fallback(method_not_allowed)
}

/// Unmatched paths get the same JSON error shape as handlers.
async fn route_not_found() -> ApiError {
    ApiError::NotFound("route")
}

/// Wrong methods get a JSON 405; keep the `Allow` header browsers and clients
/// expect.
async fn method_not_allowed() -> Response {
    let mut response = ApiError::MethodNotAllowed.into_response();
    response.headers_mut().insert(
        axum::http::header::ALLOW,
        axum::http::HeaderValue::from_static("GET, HEAD"),
    );
    response
}

/// Liveness plus a DB and schema round trip. Returns 503 when Postgres is down
/// or the `companies` table is missing so a load balancer or scheduler sees the
/// API as unhealthy. An empty table is healthy.
async fn health(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    match sqlx::query_scalar::<_, i32>("SELECT 1 FROM companies LIMIT 1")
        .fetch_optional(&state.pool)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(Health {
                status: "ok",
                db: "ok",
            }),
        ),
        Err(error) => {
            let error: crate::types::Error = error.into();
            tracing::error!("health check failed: {}", crate::types::error_chain(&error));
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Health {
                    status: "degraded",
                    db: "unavailable",
                }),
            )
        }
    }
}
