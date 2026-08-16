use sqlx::PgPool;

use crate::records::{Company, Product};
use crate::types::Error;

pub async fn init(db_url: &str) -> Result<PgPool, Error> {
    let pool = PgPool::connect(db_url).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS companies (
            id SERIAL PRIMARY KEY,
            category_code TEXT NOT NULL,
            name TEXT,
            address TEXT,
            postcode TEXT,
            state TEXT,
            phone_no TEXT,
            fax_no TEXT,
            email TEXT,
            website TEXT,
            reference_no TEXT,
            officer TEXT,
            scraped_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(category_code, name)
        )",
    )
    .execute(&pool)
    .await?;

    // Migrate: add columns that may be missing from older schema
    for col in ["postcode"] {
        sqlx::query(&format!(
            "ALTER TABLE companies ADD COLUMN IF NOT EXISTS {col} TEXT"
        ))
        .execute(&pool)
        .await
        .ok();
    }

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS products (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            brand TEXT,
            holder TEXT,
            category_code TEXT NOT NULL,
            subcategory_code TEXT NOT NULL,
            company_id INTEGER REFERENCES companies(id),
            expiry_date TEXT,
            scraped_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(category_code, subcategory_code, name, brand)
        )",
    )
    .execute(&pool)
    .await?;

    for col in ["holder"] {
        sqlx::query(&format!(
            "ALTER TABLE products ADD COLUMN IF NOT EXISTS {col} TEXT"
        ))
        .execute(&pool)
        .await
        .ok();
    }

    println!("  DB ready: {db_url}");
    Ok(pool)
}

pub async fn insert_companies(
    pool: &PgPool,
    records: &[Company],
    category_code: &str,
) -> Result<usize, Error> {
    if records.is_empty() {
        return Ok(0);
    }
    let mut tx = pool.begin().await?;
    for r in records {
        sqlx::query(
            "INSERT INTO companies (category_code, name, address, postcode, state)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT(category_code, name) DO UPDATE SET
                address = excluded.address,
                postcode = excluded.postcode,
                state = excluded.state,
                scraped_at = NOW()",
        )
        .bind(category_code)
        .bind(&r.name)
        .bind(&r.address)
        .bind(&r.postcode)
        .bind(&r.state)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(records.len())
}

pub async fn insert_products(
    pool: &PgPool,
    records: &[Product],
    category_code: &str,
    subcategory_code: &str,
) -> Result<usize, Error> {
    if records.is_empty() {
        return Ok(0);
    }
    let mut tx = pool.begin().await?;
    for r in records {
        sqlx::query(
            "INSERT INTO products (name, brand, holder, category_code, subcategory_code, expiry_date)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT(category_code, subcategory_code, name, brand) DO UPDATE SET
                holder = excluded.holder,
                expiry_date = excluded.expiry_date,
                scraped_at = NOW()",
        )
        .bind(&r.name)
        .bind(&r.brand)
        .bind(&r.holder)
        .bind(category_code)
        .bind(subcategory_code)
        .bind(&r.expiry_date)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(records.len())
}

/// A few random companies for the post-scrape sanity check.
pub async fn sample_companies(pool: &PgPool) -> Result<Vec<(String, String)>, Error> {
    let rows = sqlx::query_as("SELECT name, phone_no FROM companies ORDER BY RANDOM() LIMIT 3")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}
