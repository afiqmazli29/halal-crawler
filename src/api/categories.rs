use std::collections::BTreeMap;

use axum::Json;
use axum::extract::State;

use crate::api::AppState;
use crate::api::dto::{CategoryDto, Data, SubcategoryDto};
use crate::api::error::ApiError;
use crate::config;

/// `GET /api/v1/categories`
///
/// The list is seeded from [`config::targets`] so every configured category is
/// present before a crawl has produced products; counts are overlaid from
/// `product_categories`. Codes found only in the database (never in config) are
/// appended rather than dropped.
pub async fn list(State(state): State<AppState>) -> Result<Json<Data<Vec<CategoryDto>>>, ApiError> {
    let counts: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT category_code, subcategory_code, COUNT(*)::bigint
         FROM product_categories
         GROUP BY category_code, subcategory_code",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut by_category: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for (category_code, subcategory_code, count) in counts {
        by_category
            .entry(category_code)
            .or_default()
            .insert(subcategory_code, count);
    }

    for target in config::targets() {
        by_category
            .entry(target.category_code.to_string())
            .or_default()
            .entry(target.ty.to_string())
            .or_insert(0);
    }

    let data = by_category
        .into_iter()
        .map(|(category_code, subcategories)| {
            let category_name = config::category_name(&category_code).to_string();
            let subcategories = subcategories
                .into_iter()
                .map(|(code, product_count)| {
                    let name = config::subcategory_name(&code).to_string();
                    SubcategoryDto {
                        code,
                        name,
                        product_count,
                    }
                })
                .collect();

            CategoryDto {
                category_code,
                category_name,
                subcategories,
            }
        })
        .collect();

    Ok(Json(Data { data }))
}
