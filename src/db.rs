use sqlx::PgPool;

use crate::records::{Company, Product};
use crate::types::Error;

pub async fn init(db_url: &str) -> Result<PgPool, Error> {
    let pool = PgPool::connect(db_url).await?;

    // `companies` holds only company data. Category membership and scrape
    // timing live in `scrap_log`, so a company is unique by `name` alone —
    // the same name appearing in multiple categories collapses into one row.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS companies (
            id SERIAL PRIMARY KEY,
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
            comp_code TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(name)
        )",
    )
    .execute(&pool)
    .await?;

    // Migrate: add columns that may be missing from older schema
    for col in ["postcode", "comp_code"] {
        sqlx::query(&format!(
            "ALTER TABLE companies ADD COLUMN IF NOT EXISTS {col} TEXT"
        ))
        .execute(&pool)
        .await
        .ok();
    }
    for col in ["created_at", "updated_at"] {
        sqlx::query(&format!(
            "ALTER TABLE companies ADD COLUMN IF NOT EXISTS {col} TIMESTAMPTZ NOT NULL DEFAULT NOW()"
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
            company_id INTEGER NOT NULL REFERENCES companies(id),
            expiry_date TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(company_id, name, brand)
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
    for col in ["created_at", "updated_at"] {
        sqlx::query(&format!(
            "ALTER TABLE products ADD COLUMN IF NOT EXISTS {col} TIMESTAMPTZ NOT NULL DEFAULT NOW()"
        ))
        .execute(&pool)
        .await
        .ok();
    }

    // company_id is mandatory — existing rows are always inserted with a
    // resolved company_id, so SET NOT NULL is safe on a healthy table.
    sqlx::query("ALTER TABLE products ALTER COLUMN company_id SET NOT NULL")
        .execute(&pool)
        .await
        .ok();

    // Migrate: drop old products columns if they exist (moved to product_categories).
    // The syntax "DROP COLUMN IF EXISTS col" (without type) works in PostgreSQL 9.4+.
    for col in ["category_code", "subcategory_code", "scraped_at"] {
        sqlx::query(&format!("ALTER TABLE products DROP COLUMN IF EXISTS {col}"))
            .execute(&pool)
            .await
            .ok();
    }

    // `product_categories` maps each product to the (category, subcategory) it
    // was seen in. A product is unique by (company_id, name, brand); the mapping
    // table allows many-to-many: one product can appear in multiple categories,
    // and one category can contain many products.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS product_categories (
            product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
            category_code TEXT NOT NULL,
            subcategory_code TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            PRIMARY KEY (product_id, category_code, subcategory_code)
        )",
    )
    .execute(&pool)
    .await?;

    // `scrap_log` records scrape runs generally: which category+phase was
    // crawled, when it ran, and how many rows were inserted/updated. It does
    // not track individual company/product rows.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS scrap_log (
            id BIGSERIAL PRIMARY KEY,
            category_code TEXT NOT NULL,
            phase TEXT NOT NULL,
            started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            finished_at TIMESTAMPTZ,
            inserted_count INT NOT NULL DEFAULT 0,
            updated_count INT NOT NULL DEFAULT 0
        )",
    )
    .execute(&pool)
    .await?;

    println!("  DB ready: {db_url}");
    Ok(pool)
}

/// Open a scrape-log entry for a category+phase crawl. Returns its id; call
/// [`finish_scrap`] once the crawl finishes.
pub async fn start_scrap(pool: &PgPool, category_code: &str, phase: &str) -> Result<i64, Error> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO scrap_log (category_code, phase) VALUES ($1, $2) RETURNING id",
    )
    .bind(category_code)
    .bind(phase)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Close a scrape-log entry with the number of rows inserted and updated.
pub async fn finish_scrap(
    pool: &PgPool,
    id: i64,
    inserted: usize,
    updated: usize,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE scrap_log
         SET finished_at = NOW(), inserted_count = $1, updated_count = $2
         WHERE id = $3",
    )
    .bind(inserted as i64)
    .bind(updated as i64)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert companies. `companies` has no category, so uniqueness is by `name`
/// alone. All modal-enriched fields (phone, fax, email, …) are persisted;
/// empty values never clobber existing non-empty ones. Returns
/// `(inserted, updated)` per record via the Postgres `xmax = 0` trick.
pub async fn insert_companies(pool: &PgPool, records: &[Company]) -> Result<(usize, usize), Error> {
    if records.is_empty() {
        return Ok((0, 0));
    }
    let mut tx = pool.begin().await?;
    let mut inserted = 0usize;
    let mut updated = 0usize;
    for r in records {
        let (is_ins,): (bool,) = sqlx::query_as(
            "WITH ins AS (
                INSERT INTO companies (name, address, postcode, state,
                                       phone_no, fax_no, email, website,
                                       reference_no, officer, comp_code)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT(name) DO UPDATE SET
                    address = CASE WHEN excluded.address <> '' THEN excluded.address ELSE companies.address END,
                    postcode = CASE WHEN excluded.postcode <> '' THEN excluded.postcode ELSE companies.postcode END,
                    state = CASE WHEN excluded.state <> '' THEN excluded.state ELSE companies.state END,
                    phone_no = CASE WHEN excluded.phone_no <> '' THEN excluded.phone_no ELSE companies.phone_no END,
                    fax_no = CASE WHEN excluded.fax_no <> '' THEN excluded.fax_no ELSE companies.fax_no END,
                    email = CASE WHEN excluded.email <> '' THEN excluded.email ELSE companies.email END,
                    website = CASE WHEN excluded.website <> '' THEN excluded.website ELSE companies.website END,
                    reference_no = CASE WHEN excluded.reference_no <> '' THEN excluded.reference_no ELSE companies.reference_no END,
                    officer = CASE WHEN excluded.officer <> '' THEN excluded.officer ELSE companies.officer END,
                    comp_code = CASE WHEN excluded.comp_code <> '' THEN excluded.comp_code ELSE companies.comp_code END,
                    updated_at = NOW()
                RETURNING (xmax = 0) AS inserted
             )
             SELECT inserted FROM ins",
        )
        .bind(&r.name)
        .bind(&r.address)
        .bind(&r.postcode)
        .bind(&r.state)
        .bind(&r.phone_no)
        .bind(&r.fax_no)
        .bind(&r.email)
        .bind(&r.website)
        .bind(&r.reference_no)
        .bind(&r.officer)
        .bind(&r.comp_code)
        .fetch_one(&mut *tx)
        .await?;
        if is_ins {
            inserted += 1;
        } else {
            updated += 1;
        }
    }
    tx.commit().await?;
    Ok((inserted, updated))
}

/// Resolve a product's `holder` (a company name) to a `companies.id`,
/// matching case-insensitively and preferring an exact-name match. Returns
/// `None` when the holder is empty or no company matches — the caller then
/// skips the product rather than fabricating a company.
async fn resolve_company(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    holder: &str,
) -> Result<Option<i32>, Error> {
    let holder = holder.trim();
    if holder.is_empty() {
        return Ok(None);
    }

    let existing: Option<i32> = sqlx::query_scalar(
        "SELECT id FROM companies
         WHERE name = $1 OR lower(name) = lower($1)
         ORDER BY (name = $1) DESC, id
         LIMIT 1",
    )
    .bind(holder)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(existing)
}

/// Upsert products. The product's `holder` is resolved to a `company_id`
/// via the companies table (matching by name). Each product is then linked
/// to its (category_code, subcategory_code) via the `product_categories`
/// mapping table. Returns `(inserted, updated)` product rows.
pub async fn insert_products(
    pool: &PgPool,
    records: &[Product],
    category_code: &str,
    subcategory_code: &str,
) -> Result<(usize, usize), Error> {
    if records.is_empty() {
        return Ok((0, 0));
    }
    let mut tx = pool.begin().await?;
    let mut inserted = 0usize;
    let mut updated = 0usize;

    for r in records {
        // company_id is NOT NULL: a product whose holder doesn't resolve to a
        // real company has nowhere to link, so it's skipped.
        let Some(company_id) = resolve_company(&mut tx, &r.holder).await? else {
            eprintln!(
                "│    skipping product with unresolvable holder: {:?}",
                r.name
            );
            continue;
        };

        // Upsert the product by (company_id, name, brand).
        let (product_id, is_ins): (i32, bool) = sqlx::query_as(
            "WITH ins AS (
                INSERT INTO products (name, brand, holder, company_id, expiry_date)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT(company_id, name, brand) DO UPDATE SET
                    holder = excluded.holder,
                    expiry_date = excluded.expiry_date,
                    updated_at = NOW()
                RETURNING id, (xmax = 0) AS inserted
             )
             SELECT id, inserted FROM ins",
        )
        .bind(&r.name)
        .bind(&r.brand)
        .bind(&r.holder)
        .bind(company_id)
        .bind(&r.expiry_date)
        .fetch_one(&mut *tx)
        .await?;

        if is_ins {
            inserted += 1;
        } else {
            updated += 1;
        }

        // Link the product to its category/subcategory (idempotent).
        sqlx::query(
            "INSERT INTO product_categories (product_id, category_code, subcategory_code)
             VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(product_id)
        .bind(category_code)
        .bind(subcategory_code)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok((inserted, updated))
}

/// A few random companies for the post-scrape sanity check.
pub async fn sample_companies(pool: &PgPool) -> Result<Vec<(String, String)>, Error> {
    let rows = sqlx::query_as("SELECT name, phone_no FROM companies ORDER BY RANDOM() LIMIT 3")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}
