use serde_json::Value;
use sqlx::PgPool;

use crate::types::{Error, pick_str};

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

    println!("  DB ready: {db_url}");
    Ok(pool)
}

pub async fn insert_companies(
    pool: &PgPool,
    records: &[Value],
    category_code: &str,
) -> Result<usize, Error> {
    if records.is_empty() {
        return Ok(0);
    }
    let mut tx = pool.begin().await?;
    for r in records {
        let name = pick_str(r, &["nama_syarikat", "name", "company_name", "nama"]);
        let address = pick_str(r, &["alamat", "address"]);
        let postcode = pick_str(r, &["postcode", "poskod"]);
        let state = pick_str(r, &["negeri", "state"]);
        let phone = pick_str(r, &["no_telefon", "phone_no", "phone", "telefon"]);
        let fax = pick_str(r, &["no_fax", "fax_no", "fax"]);
        let email = pick_str(r, &["e_mel", "email", "emel"]);
        let website = pick_str(r, &["laman_web", "website", "web"]);
        let ref_no = pick_str(r, &["no_rujukan", "reference_no", "reference"]);
        let officer = pick_str(r, &["pegawai", "officer", "nama_pegawai"]);

        sqlx::query(
            "INSERT INTO companies (category_code, name, address, postcode, state, phone_no, fax_no, email, website, reference_no, officer)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT(category_code, name) DO UPDATE SET
                address = excluded.address,
                postcode = excluded.postcode,
                state = excluded.state,
                phone_no = excluded.phone_no,
                fax_no = excluded.fax_no,
                email = excluded.email,
                website = excluded.website,
                reference_no = excluded.reference_no,
                officer = excluded.officer,
                scraped_at = NOW()",
        )
        .bind(category_code)
        .bind(&name)
        .bind(&address)
        .bind(&postcode)
        .bind(&state)
        .bind(&phone)
        .bind(&fax)
        .bind(&email)
        .bind(&website)
        .bind(&ref_no)
        .bind(&officer)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(records.len())
}

pub async fn insert_products(
    pool: &PgPool,
    records: &[Value],
    category_code: &str,
    subcategory_code: &str,
) -> Result<usize, Error> {
    if records.is_empty() {
        return Ok(0);
    }
    let mut tx = pool.begin().await?;
    for r in records {
        let name = pick_str(r, &["name", "nama", "product_name"]);
        let brand = pick_str(r, &["brand", "jenama"]);
        let expiry = pick_str(r, &["expiry_date", "tarikh_tamat", "tempoh_sah_laku"]);

        sqlx::query(
            "INSERT INTO products (name, brand, category_code, subcategory_code, expiry_date)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT(category_code, subcategory_code, name, brand) DO UPDATE SET
                expiry_date = excluded.expiry_date,
                scraped_at = NOW()",
        )
        .bind(&name)
        .bind(&brand)
        .bind(category_code)
        .bind(subcategory_code)
        .bind(&expiry)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(records.len())
}
