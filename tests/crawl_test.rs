use httpmock::prelude::*;
use serde_json::json;

use halal_crawler::{db, scraper, types};

mod common;

#[tokio::test]
async fn test_db_insert_and_query_companies() {
    let ctx = common::setup().await;

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

    common::cleanup(&ctx, names, &[]).await;
}

#[tokio::test]
async fn test_db_insert_and_query_products() {
    let ctx = common::setup().await;

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

    common::cleanup(&ctx, company_names, product_names).await;
}

#[tokio::test]
async fn test_db_insert_empty_returns_zero() {
    let ctx = common::setup().await;

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
    let ctx = common::setup().await;

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

    common::cleanup(&ctx, names, &[]).await;
}

#[tokio::test]
async fn test_scrape_companies_mocked() {
    let ctx = common::setup().await;
    let base = common::mock_base(&ctx.server);

    let strategy = types::SubStrategy {
        category_code: "BG",
        category_name: "Barang Gunaan",
        data_param: "testdata",
        sub_code: "CO",
        sub_name: "Syarikat",
    };

    let detail1 = format!("{}/detail/t5_1", ctx.server.base_url());
    let detail2 = format!("{}/detail/t5_2", ctx.server.base_url());

    let listing_html = common::company_listing_html(&[&detail1, &detail2]);

    ctx.server.mock(|when, then| {
        when.method(GET)
            .path("/index.php")
            .query_param("data", "testdata")
            .query_param("category", "BG");
        then.status(200)
            .header("content-type", "text/html")
            .body(listing_html);
    });

    ctx.server.mock(|when, then| {
        when.method(GET).path("/detail/t5_1");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::company_detail_html(
                "t5_ABC Sdn Bhd",
                "03-111",
                "Selangor",
            ));
    });
    ctx.server.mock(|when, then| {
        when.method(GET).path("/detail/t5_2");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::company_detail_html(
                "t5_XYZ Sdn Bhd",
                "03-222",
                "KL",
            ));
    });

    let records = scraper::scrape_companies(&ctx.client, &ctx.semaphore, &base, &strategy)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 2);

    let names: Vec<&str> = records
        .iter()
        .map(|r| r["nama_syarikat"].as_str().unwrap_or(""))
        .collect();
    assert!(names.contains(&"t5_ABC Sdn Bhd"));
    assert!(names.contains(&"t5_XYZ Sdn Bhd"));

    let inserted = db::insert_companies(&ctx.pool, &records, "BG")
        .await
        .expect("insert");
    assert_eq!(inserted, 2);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM companies WHERE name LIKE 't5_%'")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(count, 2);

    common::cleanup(&ctx, &["t5_ABC Sdn Bhd", "t5_XYZ Sdn Bhd"], &[]).await;
}

#[tokio::test]
async fn test_scrape_products_mocked() {
    let ctx = common::setup().await;
    let base = common::mock_base(&ctx.server);

    let strategy = types::SubStrategy {
        category_code: "PR",
        category_name: "Produk Makanan",
        data_param: "testdata",
        sub_code: "PR",
        sub_name: "Produk",
    };

    let page1 = common::product_listing_html(
        &[
            (1, "t6_Biskut A", "Alamat A", "2026-12-31"),
            (2, "t6_Biskut B", "Alamat B", "2027-06-15"),
        ],
        1,
    );

    ctx.server.mock(|when, then| {
        when.method(GET)
            .path("/index.php")
            .query_param("data", "testdata")
            .query_param("category", "PR")
            .query_param("subcategory", "PR");
        then.status(200)
            .header("content-type", "text/html")
            .body(page1);
    });

    let records = scraper::scrape_products(&ctx.client, &ctx.semaphore, &base, &strategy)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 2);

    let names: Vec<&str> = records
        .iter()
        .map(|r| r["name"].as_str().unwrap_or(""))
        .collect();
    assert!(names.contains(&"t6_Biskut A"));
    assert!(names.contains(&"t6_Biskut B"));

    let inserted = db::insert_products(&ctx.pool, &records, "PR", "PR")
        .await
        .expect("insert");
    assert_eq!(inserted, 2);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM products WHERE name LIKE 't6_%'")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(count, 2);

    common::cleanup(&ctx, &[], &["t6_Biskut A", "t6_Biskut B"]).await;
}

#[tokio::test]
async fn test_scrape_companies_empty_listing_falls_back() {
    let ctx = common::setup().await;
    let base = common::mock_base(&ctx.server);

    let strategy = types::SubStrategy {
        category_code: "BG",
        category_name: "Barang Gunaan",
        data_param: "testdata",
        sub_code: "CO",
        sub_name: "Syarikat",
    };

    ctx.server.mock(|when, then| {
        when.method(GET)
            .path("/index.php")
            .query_param("data", "testdata")
            .query_param("category", "BG");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>No onclick links here</body></html>");
    });

    let records = scraper::scrape_companies(&ctx.client, &ctx.semaphore, &base, &strategy)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 0);
}

#[tokio::test]
async fn test_scrape_products_no_total_record_returns_empty() {
    let ctx = common::setup().await;
    let base = common::mock_base(&ctx.server);

    let strategy = types::SubStrategy {
        category_code: "ZZ",
        category_name: "Unknown",
        data_param: "testdata",
        sub_code: "ZZ",
        sub_name: "Unknown",
    };

    ctx.server.mock(|when, then| {
        when.method(GET).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>Nothing here</body></html>");
    });

    let records = scraper::scrape_products(&ctx.client, &ctx.semaphore, &base, &strategy)
        .await
        .expect("scrape");
    assert_eq!(records.len(), 0);
}
