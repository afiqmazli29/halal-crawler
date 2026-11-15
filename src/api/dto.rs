use chrono::{DateTime, Utc};
use serde::Serialize;

/// A single success payload. Detail endpoints use this wrapper.
#[derive(Debug, Serialize)]
pub struct Data<T> {
    pub data: T,
}

/// A paginated success payload.
#[derive(Debug, Serialize)]
pub struct List<T> {
    pub data: Vec<T>,
    pub meta: Meta,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
    pub total_pages: i64,
}

impl Meta {
    pub fn new(page: u32, per_page: u32, total: i64) -> Self {
        let per_page = per_page.max(1) as i64;
        Self {
            page,
            per_page: per_page as u32,
            total,
            total_pages: (total + per_page - 1) / per_page,
        }
    }
}

/// The list shape for `GET /companies` — the fields a directory consumer
/// scans, without the modal-only detail fields.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CompanySummary {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub postcode: String,
    pub state: String,
    pub phone_no: String,
    pub email: String,
    pub website: String,
    pub product_count: i64,
    pub updated_at: DateTime<Utc>,
}

/// The full company row, including modal-enriched fields.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CompanyDetail {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub postcode: String,
    pub state: String,
    pub phone_no: String,
    pub fax_no: String,
    pub email: String,
    pub website: String,
    pub reference_no: String,
    pub officer: String,
    pub comp_code: String,
    pub product_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal product row: the list query's flat shape, mapped to
/// [`ProductDto`] once memberships are loaded.
#[derive(Debug, sqlx::FromRow)]
pub struct ProductRow {
    pub id: i32,
    pub name: String,
    pub brand: String,
    pub holder: String,
    pub expiry_date: String,
    pub company_id: i32,
    pub company_name: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProductDto {
    pub id: i32,
    pub name: String,
    pub brand: String,
    pub holder: String,
    pub expiry_date: String,
    pub company: CompanyRef,
    pub categories: Vec<CategoryRef>,
    pub updated_at: DateTime<Utc>,
}

impl ProductDto {
    pub fn from_row(row: ProductRow, categories: Vec<CategoryRef>) -> Self {
        Self {
            id: row.id,
            name: row.name,
            brand: row.brand,
            holder: row.holder,
            expiry_date: row.expiry_date,
            company: CompanyRef {
                id: row.company_id,
                name: row.company_name,
            },
            categories,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CompanyRef {
    pub id: i32,
    pub name: String,
}

/// One (category, subcategory) membership of a product.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CategoryRef {
    pub category_code: String,
    pub subcategory_code: String,
}

#[derive(Debug, Serialize)]
pub struct CategoryDto {
    pub category_code: String,
    pub category_name: String,
    pub subcategories: Vec<SubcategoryDto>,
}

#[derive(Debug, Serialize)]
pub struct SubcategoryDto {
    pub code: String,
    pub name: String,
    pub product_count: i64,
}

/// One `scrap_log` row, newest first.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScrapeRun {
    pub id: i64,
    pub category_code: String,
    pub phase: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub inserted_count: i32,
    pub updated_count: i32,
}

#[derive(Debug, Serialize)]
pub struct Health {
    pub status: &'static str,
    pub db: &'static str,
}
