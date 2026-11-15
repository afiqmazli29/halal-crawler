use axum::Json;
use axum::extract::State;

use crate::api::AppState;
use crate::api::dto::{Data, ScrapeRun};
use crate::api::error::ApiError;
use crate::api::extract::ApiQuery;
use crate::api::params::RunQuery;

const MAX_LIMIT: usize = 200;

/// `GET /api/v1/scrape-runs?limit=20`
///
/// Newest first. A `finished_at` of `null` means the run is still open (or
/// crashed before closing its row).
pub async fn list(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<RunQuery>,
) -> Result<Json<Data<Vec<ScrapeRun>>>, ApiError> {
    let limit = query.limit.clamp(1, MAX_LIMIT) as i64;

    let rows: Vec<ScrapeRun> = sqlx::query_as(
        "SELECT id, category_code, phase, started_at, finished_at,
                inserted_count, updated_count
         FROM scrap_log
         ORDER BY started_at DESC, id DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(Data { data: rows }))
}
