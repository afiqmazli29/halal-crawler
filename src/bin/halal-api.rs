use std::net::SocketAddr;

use axum::http::{HeaderValue, Method};
use halal_crawler::types::Error;
use halal_crawler::{api, db};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

/// The read-only HTTP API over the crawled directory.
///
/// Deliberately separate from the crawler binary: this process only reads.
/// It connects with [`db::connect`], never [`db::init`], so it works against a
/// role that cannot create or alter tables.
#[tokio::main]
async fn main() -> Result<(), Error> {
    if std::path::Path::new(".env").exists() {
        dotenvy::dotenv().ok();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug")),
        )
        .init();

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/halal".to_string());

    // Default bind is loopback port 8080; set API_BIND to override.
    let addr: SocketAddr = match std::env::var("API_BIND").ok() {
        Some(value) => value
            .parse()
            .map_err(|e| format!("invalid API_BIND {value:?}: {e}"))?,
        None => SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 8080)),
    };

    let pool = db::connect(&db_url).await?;

    // The API does not own the schema. Fail fast with an actionable message
    // instead of a throwaway SQL error on the first request.
    let missing: Option<String> = sqlx::query_scalar(
        "SELECT name
         FROM (VALUES ('companies'), ('products'),
                      ('product_categories'), ('scrap_log')) AS t(name)
         WHERE to_regclass('public.' || name) IS NULL
         ORDER BY name
         LIMIT 1",
    )
    .fetch_optional(&pool)
    .await?;

    if let Some(table) = missing {
        return Err(format!(
            "database schema is incomplete (missing `{table}`): run the crawler once \
             (cargo run) to create it"
        )
        .into());
    }

    let app = api::router(pool)
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("halal-api listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// `API_CORS_ORIGINS` is a comma-separated allowlist; `*` (the default)
/// allows any origin. The API is read-only and credential-free, so a wide
/// default is reasonable for public directory data.
fn cors_layer() -> CorsLayer {
    let configured = std::env::var("API_CORS_ORIGINS").unwrap_or_else(|_| "*".to_string());
    let base = CorsLayer::new()
        .allow_methods([Method::GET, Method::HEAD])
        .allow_headers(Any);

    if configured.trim() == "*" {
        return base.allow_origin(Any);
    }

    let origins: Vec<HeaderValue> = configured
        .split(',')
        .filter_map(|origin| origin.trim().parse().ok())
        .collect();

    if origins.is_empty() {
        // An explicitly configured but unparseable allowlist must not fall back
        // to allowing every origin.
        tracing::warn!("API_CORS_ORIGINS contains no valid origins; denying cross-origin requests");
    }

    base.allow_origin(AllowOrigin::list(origins))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutting down");
}
