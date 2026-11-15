use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use sqlx::PgPool;

use crate::api::AppState;
use crate::api::dto::{CategoryRef, Data, List, Meta, ProductDto, ProductRow};
use crate::api::error::ApiError;
use crate::api::extract::{ApiPath, ApiQuery};
use crate::api::params::{PageParams, ProductQuery, non_blank};

/// The filters shared by the standalone product list and a company's product
/// list. A `None` field means "no filter"; the SQL predicates are static.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProductFilter<'a> {
    pub q: Option<&'a str>,
    pub company_id: Option<i32>,
    pub category: Option<&'a str>,
    pub subcategory: Option<&'a str>,
}

/// `GET /api/v1/products`
pub async fn list(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ProductQuery>,
) -> Result<Json<List<ProductDto>>, ApiError> {
    let filter = ProductFilter {
        q: non_blank(query.q.as_deref()),
        company_id: query.company_id,
        category: non_blank(query.category.as_deref()),
        subcategory: non_blank(query.subcategory.as_deref()),
    };

    let list = list_page(&state.pool, filter, query.pages()).await?;
    Ok(Json(list))
}

/// `GET /api/v1/products/{id}`
pub async fn detail(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<Data<ProductDto>>, ApiError> {
    let row: Option<ProductRow> = sqlx::query_as(
        "SELECT p.id,
                p.name,
                COALESCE(p.brand, '') AS brand,
                COALESCE(p.holder, '') AS holder,
                COALESCE(p.expiry_date, '') AS expiry_date,
                p.company_id,
                COALESCE(c.name, '') AS company_name,
                p.updated_at
         FROM products p
         JOIN companies c ON c.id = p.company_id
         WHERE p.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("product"))?;
    let mut categories = load_categories(&state.pool, &[id]).await?;
    let categories = categories.remove(&id).unwrap_or_default();

    Ok(Json(Data {
        data: ProductDto::from_row(row, categories),
    }))
}

/// Shared paging used by both product endpoints.
pub async fn list_page(
    pool: &PgPool,
    filter: ProductFilter<'_>,
    page: PageParams,
) -> Result<List<ProductDto>, ApiError> {
    // Bind order: $1 q, $2 company_id, $3 category, $4 subcategory,
    // then $5 limit / $6 offset on the list query only.
    const WHERE: &str = "WHERE (
            $1::text IS NULL
            OR p.name ILIKE '%' || $1 || '%'
            OR COALESCE(p.brand, '') ILIKE '%' || $1 || '%'
            OR COALESCE(p.holder, '') ILIKE '%' || $1 || '%'
            OR c.name ILIKE '%' || $1 || '%'
         )
         AND ($2::int IS NULL OR p.company_id = $2)
         AND (
            ($3::text IS NULL AND $4::text IS NULL)
            OR EXISTS (
                SELECT 1 FROM product_categories pc
                WHERE pc.product_id = p.id
                  AND ($3::text IS NULL OR pc.category_code = $3)
                  AND ($4::text IS NULL OR pc.subcategory_code = $4)
            )
         )";

    let count_sql = format!(
        "SELECT COUNT(*)
         FROM products p
         JOIN companies c ON c.id = p.company_id
         {WHERE}"
    );

    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(filter.q)
        .bind(filter.company_id)
        .bind(filter.category)
        .bind(filter.subcategory)
        .fetch_one(pool)
        .await?;

    let list_sql = format!(
        "SELECT p.id,
                p.name,
                COALESCE(p.brand, '') AS brand,
                COALESCE(p.holder, '') AS holder,
                COALESCE(p.expiry_date, '') AS expiry_date,
                p.company_id,
                COALESCE(c.name, '') AS company_name,
                p.updated_at
         FROM products p
         JOIN companies c ON c.id = p.company_id
         {WHERE}
         ORDER BY p.name, p.id
         LIMIT $5 OFFSET $6"
    );

    let rows: Vec<ProductRow> = sqlx::query_as(&list_sql)
        .bind(filter.q)
        .bind(filter.company_id)
        .bind(filter.category)
        .bind(filter.subcategory)
        .bind(page.limit())
        .bind(page.offset())
        .fetch_all(pool)
        .await?;

    let ids: Vec<i32> = rows.iter().map(|row| row.id).collect();
    let mut categories = load_categories(pool, &ids).await?;

    let data = rows
        .into_iter()
        .map(|row| {
            let categories = categories.remove(&row.id).unwrap_or_default();
            ProductDto::from_row(row, categories)
        })
        .collect();

    Ok(List {
        data,
        meta: Meta::new(page.page, page.per_page, total),
    })
}

/// Load memberships for a whole page in one query, keyed by product id.
async fn load_categories(
    pool: &PgPool,
    ids: &[i32],
) -> Result<HashMap<i32, Vec<CategoryRef>>, ApiError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<(i32, String, String)> = sqlx::query_as(
        "SELECT product_id, category_code, subcategory_code
         FROM product_categories
         WHERE product_id = ANY($1)
         ORDER BY category_code, subcategory_code",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;

    let mut by_product: HashMap<i32, Vec<CategoryRef>> = HashMap::new();
    for (product_id, category_code, subcategory_code) in rows {
        by_product.entry(product_id).or_default().push(CategoryRef {
            category_code,
            subcategory_code,
        });
    }

    Ok(by_product)
}
