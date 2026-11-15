use axum::Json;
use axum::extract::State;

use crate::api::AppState;
use crate::api::dto::{CompanyDetail, CompanySummary, Data, List, Meta, ProductDto};
use crate::api::error::ApiError;
use crate::api::extract::{ApiPath, ApiQuery};
use crate::api::params::{CompanyQuery, ProductQuery, non_blank};
use crate::api::products::{self as product_api, ProductFilter};

/// `GET /api/v1/companies`
pub async fn list(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CompanyQuery>,
) -> Result<Json<List<CompanySummary>>, ApiError> {
    let page = query.pages();
    let search = non_blank(query.q.as_deref());
    let region = non_blank(query.state.as_deref());

    // Shared by the count and the page query so the two can never drift.
    const WHERE: &str = "WHERE (
            $1::text IS NULL
            OR COALESCE(c.name, '') ILIKE '%' || $1 || '%'
            OR COALESCE(c.address, '') ILIKE '%' || $1 || '%'
         )
       AND ($2::text IS NULL OR c.state = $2)";

    let count_sql = format!("SELECT COUNT(*) FROM companies c {WHERE}");
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(search)
        .bind(region)
        .fetch_one(&state.pool)
        .await?;

    let list_sql = format!(
        "SELECT c.id,
                COALESCE(c.name, '') AS name,
                COALESCE(c.address, '') AS address,
                COALESCE(c.postcode, '') AS postcode,
                COALESCE(c.state, '') AS state,
                COALESCE(c.phone_no, '') AS phone_no,
                COALESCE(c.email, '') AS email,
                COALESCE(c.website, '') AS website,
                COUNT(p.id)::bigint AS product_count,
                c.updated_at
         FROM companies c
         LEFT JOIN products p ON p.company_id = c.id
         {WHERE}
         GROUP BY c.id
         ORDER BY c.name, c.id
         LIMIT $3 OFFSET $4"
    );

    let rows: Vec<CompanySummary> = sqlx::query_as(&list_sql)
        .bind(search)
        .bind(region)
        .bind(page.limit())
        .bind(page.offset())
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(List {
        data: rows,
        meta: Meta::new(page.page, page.per_page, total),
    }))
}

/// `GET /api/v1/companies/{id}`
pub async fn detail(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i32>,
) -> Result<Json<Data<CompanyDetail>>, ApiError> {
    let row: Option<CompanyDetail> = sqlx::query_as(
        "SELECT c.id,
                COALESCE(c.name, '') AS name,
                COALESCE(c.address, '') AS address,
                COALESCE(c.postcode, '') AS postcode,
                COALESCE(c.state, '') AS state,
                COALESCE(c.phone_no, '') AS phone_no,
                COALESCE(c.fax_no, '') AS fax_no,
                COALESCE(c.email, '') AS email,
                COALESCE(c.website, '') AS website,
                COALESCE(c.reference_no, '') AS reference_no,
                COALESCE(c.officer, '') AS officer,
                COALESCE(c.comp_code, '') AS comp_code,
                (SELECT COUNT(*) FROM products p WHERE p.company_id = c.id)::bigint
                    AS product_count,
                c.created_at,
                c.updated_at
         FROM companies c
         WHERE c.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("company"))?;
    Ok(Json(Data { data: row }))
}

/// `GET /api/v1/companies/{id}/products`
pub async fn products(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i32>,
    ApiQuery(query): ApiQuery<ProductQuery>,
) -> Result<Json<List<ProductDto>>, ApiError> {
    let exists: Option<i32> = sqlx::query_scalar("SELECT id FROM companies WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(ApiError::not_found("company"));
    }

    let filter = ProductFilter {
        q: non_blank(query.q.as_deref()),
        company_id: Some(id),
        category: non_blank(query.category.as_deref()),
        subcategory: non_blank(query.subcategory.as_deref()),
    };

    let list = product_api::list_page(&state.pool, filter, query.pages()).await?;
    Ok(Json(list))
}
