use httpmock::prelude::*;
use serde_json::json;

use halal_crawler::{db, scraper, types};

mod common;

fn company_strategy() -> types::SubStrategy {
    types::SubStrategy {
        category_code: "BG",
        category_name: "Barang Gunaan",
        sub_code: "CO",
        sub_name: "Syarikat",
    }
}

fn product_strategy() -> types::SubStrategy {
    types::SubStrategy {
        category_code: "PR",
        category_name: "Produk Makanan",
        sub_code: "PR",
        sub_name: "Produk",
    }
}

// ── DB insert/upsert (needs live PostgreSQL) ────────────────────

#[tokio::test]
async fn test_db_insert_and_query_companies() {
    let ctx = common::setup_db().await;

    let names = &["t1_ABC Sdn Bhd", "t1_XYZ Sdn Bhd"];
    let records = vec![
        json!({"nama_syarikat": names[0], "no_telefon": "03-111", "negeri": "Selangor"}),
        json!({"nama_syarikat": names[1], "no_telefon": "03-222", "negeri": "KL"}),
    ];

    let n = db::insert_companies(&ctx.pool, &records, "BG")
        .await
        .expect("insert");
    assert_eq!(n, 2);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM companies WHERE name LIKE 't1_%'")
        .fetch_one(&ctx.pool)
        .await
        .expect("query");
    assert_eq!(count, 2);

    let (name,): (String,) =
        sqlx::query_as("SELECT name FROM companies WHERE phone_no = '03-111' AND name LIKE 't1_%'")
            .fetch_one(&ctx.pool)
            .await
            .expect("query");
    assert_eq!(name, names[0]);

    common::cleanup(&ctx.pool, names, &[]).await;
}

#[tokio::test]
async fn test_db_insert_and_query_products() {
    let ctx = common::setup_db().await;

    let company_names = &["t2_Parent Co"];
    db::insert_companies(
        &ctx.pool,
        &[json!({"nama_syarikat": company_names[0]})],
        "PR",
    )
    .await
    .unwrap();

    let product_names = &["t2_Product A", "t2_Product B"];
    let products = vec![
        json!({"name": product_names[0], "brand": "BrandA", "expiry_date": "2026-12-31"}),
        json!({"name": product_names[1], "brand": "BrandB", "expiry_date": "2027-06-15"}),
    ];

    let n = db::insert_products(&ctx.pool, &products, "PR", "PR")
        .await
        .expect("insert");
    assert_eq!(n, 2);

    let (name, brand, expiry): (String, String, String) =
        sqlx::query_as("SELECT name, brand, expiry_date FROM products WHERE name LIKE 't2_%' ORDER BY name LIMIT 1")
            .fetch_one(&ctx.pool)
            .await
            .expect("query");
    assert_eq!(name, product_names[0]);
    assert_eq!(brand, "BrandA");
    assert_eq!(expiry, "2026-12-31");

    common::cleanup(&ctx.pool, company_names, product_names).await;
}

#[tokio::test]
async fn test_db_insert_empty_returns_zero() {
    let ctx = common::setup_db().await;

    let n = db::insert_companies(&ctx.pool, &[], "BG")
        .await
        .expect("ok");
    assert_eq!(n, 0);

    let n = db::insert_products(&ctx.pool, &[], "PR", "PR")
        .await
        .expect("ok");
    assert_eq!(n, 0);
}

#[tokio::test]
async fn test_db_upsert_companies() {
    let ctx = common::setup_db().await;

    let names = &["t4_Foo"];
    let first = vec![json!({"nama_syarikat": names[0], "no_telefon": "111", "negeri": "KL"})];
    db::insert_companies(&ctx.pool, &first, "BG").await.unwrap();

    let second =
        vec![json!({"nama_syarikat": names[0], "no_telefon": "222", "negeri": "Selangor"})];
    let n = db::insert_companies(&ctx.pool, &second, "BG")
        .await
        .unwrap();
    assert_eq!(n, 1);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM companies WHERE name LIKE 't4_%'")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let (phone,): (String,) = sqlx::query_as("SELECT phone_no FROM companies WHERE name = $1")
        .bind(names[0])
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(phone, "222");

    common::cleanup(&ctx.pool, names, &[]).await;
}

// ── Crawl tests (httpmock only, no database) ────────────────────

#[tokio::test]
async fn test_scrape_companies_dedups_across_letters() {
    let ctx = common::setup_mock().await;

    // Every letter gets the same listing; the crawl must dedup by name.
    let html = common::listing_html(
        &[
            ("t5_ABC Sdn Bhd", "123 Jalan, 50000 KL, Selangor"),
            ("t5_XYZ Sdn Bhd", "456 Jalan, 47000 Shah Alam, Selangor"),
        ],
        0,
    );
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body(html);
    });

    let records = scraper::scrape_companies(&ctx.portal, &company_strategy())
        .await
        .expect("scrape");

    assert_eq!(records.len(), 2);
    let names: Vec<&str> = records
        .iter()
        .map(|r| r["name"].as_str().unwrap_or(""))
        .collect();
    assert!(names.contains(&"t5_ABC Sdn Bhd"));
    assert!(names.contains(&"t5_XYZ Sdn Bhd"));
}

#[tokio::test]
async fn test_scrape_companies_paginates_via_counter() {
    let ctx = common::setup_mock().await;

    // Page 1 returns counter=41, page 2 returns counter=0 (end).
    ctx.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("cari", "a")
            .query_param("page", "1");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::listing_html(
                &[("t6_Alpha One", "1 Jalan, 50000 KL, Kuala Lumpur")],
                41,
            ));
    });
    ctx.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("cari", "a")
            .query_param("page", "2");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::listing_html(
                &[("t6_Alpha Two", "2 Jalan, 47000 Shah Alam, Selangor")],
                0,
            ));
    });
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>empty</body></html>");
    });

    let records = scraper::scrape_companies(&ctx.portal, &company_strategy())
        .await
        .expect("scrape");

    let names: Vec<&str> = records
        .iter()
        .map(|r| r["name"].as_str().unwrap_or(""))
        .collect();
    assert!(names.contains(&"t6_Alpha One"), "got: {names:?}");
    assert!(names.contains(&"t6_Alpha Two"), "got: {names:?}");
    assert_eq!(records.len(), 2);
}

#[tokio::test]
async fn test_scrape_companies_parses_address_fields() {
    let ctx = common::setup_mock().await;

    let html = common::listing_html(&[("t7_Addr Co", "12 Jalan, 63000 Cyberjaya, Selangor")], 0);
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body(html);
    });

    let records = scraper::scrape_companies(&ctx.portal, &company_strategy())
        .await
        .expect("scrape");

    assert_eq!(records[0]["name"], "t7_Addr Co");
    assert_eq!(records[0]["postcode"], "63000");
    assert_eq!(records[0]["state"], "Selangor");
}

#[tokio::test]
async fn test_scrape_products_mocked() {
    let ctx = common::setup_mock().await;

    let page1 = common::product_listing_html(
        &[
            ("t6_Biskut A", "Alamat A", "2026-12-31"),
            ("t6_Biskut B", "Alamat B", "2027-06-15"),
        ],
        1,
    );

    ctx.server.mock(|when, then| {
        when.method(GET)
            .path("/index.php")
            .query_param("category", "PR")
            .query_param("subcategory", "PR");
        then.status(200)
            .header("content-type", "text/html")
            .body(page1);
    });

    let records = scraper::scrape_products(&ctx.portal, &product_strategy())
        .await
        .expect("scrape");

    assert_eq!(records.len(), 2);

    let names: Vec<&str> = records
        .iter()
        .map(|r| r["name"].as_str().unwrap_or(""))
        .collect();
    assert!(names.contains(&"t6_Biskut A"));
    assert!(names.contains(&"t6_Biskut B"));
}

#[tokio::test]
async fn test_scrape_companies_empty_listing_returns_empty() {
    let ctx = common::setup_mock().await;

    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>No spans here</body></html>");
    });

    let records = scraper::scrape_companies(&ctx.portal, &company_strategy())
        .await
        .expect("scrape");

    assert_eq!(records.len(), 0);
}

#[tokio::test]
async fn test_scrape_products_no_total_record_returns_empty() {
    let ctx = common::setup_mock().await;

    let strategy = types::SubStrategy {
        category_code: "ZZ",
        category_name: "Unknown",
        sub_code: "ZZ",
        sub_name: "Unknown",
    };

    ctx.server.mock(|when, then| {
        when.method(GET).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>Nothing here</body></html>");
    });

    let records = scraper::scrape_products(&ctx.portal, &strategy)
        .await
        .expect("scrape");
    assert_eq!(records.len(), 0);
}
