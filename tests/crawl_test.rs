use httpmock::prelude::*;
use serde_json::json;

use halal_crawler::records::{Company, Product};
use halal_crawler::{db, listing, types};

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
        Company::from_value(&json!({"nama_syarikat": names[0], "negeri": "Selangor"})),
        Company::from_value(&json!({"nama_syarikat": names[1], "negeri": "KL"})),
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
        sqlx::query_as("SELECT name FROM companies WHERE state = 'Selangor' AND name LIKE 't1_%'")
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
        &[Company::from_value(
            &json!({"nama_syarikat": company_names[0]}),
        )],
        "PR",
    )
    .await
    .unwrap();

    let product_names = &["t2_Product A", "t2_Product B"];
    let products = vec![
        Product::from_value(
            &json!({"name": product_names[0], "brand": "BrandA", "expiry_date": "2026-12-31", "company": "t2_Parent Co"}),
        ),
        Product::from_value(
            &json!({"name": product_names[1], "brand": "BrandB", "expiry_date": "2027-06-15", "company": "t2_Parent Co"}),
        ),
    ];

    let n = db::insert_products(&ctx.pool, &products, "PR", "PR")
        .await
        .expect("insert");
    assert_eq!(n, 2);

    let (name, brand, expiry, holder): (String, String, String, String) =
        sqlx::query_as("SELECT name, brand, expiry_date, holder FROM products WHERE name LIKE 't2_%' ORDER BY name LIMIT 1")
            .fetch_one(&ctx.pool)
            .await
            .expect("query");
    assert_eq!(name, product_names[0]);
    assert_eq!(brand, "BrandA");
    assert_eq!(expiry, "2026-12-31");
    assert_eq!(holder, "t2_Parent Co");

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
    let first = vec![Company::from_value(
        &json!({"nama_syarikat": names[0], "state": "KL"}),
    )];
    db::insert_companies(&ctx.pool, &first, "BG").await.unwrap();

    let second = vec![Company::from_value(
        &json!({"nama_syarikat": names[0], "state": "Selangor"}),
    )];
    let n = db::insert_companies(&ctx.pool, &second, "BG")
        .await
        .unwrap();
    assert_eq!(n, 1);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM companies WHERE name LIKE 't4_%'")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let (state,): (String,) = sqlx::query_as("SELECT state FROM companies WHERE name = $1")
        .bind(names[0])
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(state, "Selangor");

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
        1,
    );
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body(html);
    });

    let records = listing::fetch_companies(&ctx.portal, &company_strategy(), None)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 2);
    let names: Vec<&str> = records.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"t5_ABC Sdn Bhd"));
    assert!(names.contains(&"t5_XYZ Sdn Bhd"));
}

#[tokio::test]
async fn test_scrape_companies_paginates_via_page_param() {
    let ctx = common::setup_mock().await;

    // Page 1 announces 2 total pages; page 2 is fetched next.
    ctx.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("cari", "a")
            .query_param("page", "1");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::listing_html(
                &[("t6_Alpha One", "1 Jalan, 50000 KL, Kuala Lumpur")],
                2,
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
                2,
            ));
    });
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>empty</body></html>");
    });

    let records = listing::fetch_companies(&ctx.portal, &company_strategy(), None)
        .await
        .expect("scrape");

    let names: Vec<&str> = records.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"t6_Alpha One"), "got: {names:?}");
    assert!(names.contains(&"t6_Alpha Two"), "got: {names:?}");
    assert_eq!(records.len(), 2);
}

#[tokio::test]
async fn test_scrape_companies_parses_address_fields() {
    let ctx = common::setup_mock().await;

    let html = common::listing_html(&[("t7_Addr Co", "12 Jalan, 63000 Cyberjaya, Selangor")], 1);
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body(html);
    });

    let records = listing::fetch_companies(&ctx.portal, &company_strategy(), None)
        .await
        .expect("scrape");

    assert_eq!(records[0].name, "t7_Addr Co");
    assert_eq!(records[0].postcode, "63000");
    assert_eq!(records[0].state, "Selangor");
}

#[tokio::test]
async fn test_fetch_subcategory_mocked() {
    let ctx = common::setup_mock().await;

    let page1 = common::product_listing_html(
        &[
            ("t6_Biskut A", "BrandA", "t6_Parent Co", "2026-12-31"),
            ("t6_Biskut B", "BrandB", "t6_Parent Co", "2027-06-15"),
        ],
        1,
    );

    ctx.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("category", "PR")
            .query_param("cari", "a")
            .query_param("page", "1");
        then.status(200)
            .header("content-type", "text/html")
            .body(page1);
    });
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>empty</body></html>");
    });

    let records = listing::fetch_subcategory(&ctx.portal, &product_strategy(), None)
        .await
        .expect("scrape");

    let names: Vec<&str> = records.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"t6_Biskut A"), "got: {names:?}");
    assert!(names.contains(&"t6_Biskut B"), "got: {names:?}");
    assert_eq!(records.len(), 2);

    let biscuit_a = records
        .iter()
        .find(|r| r.name == "t6_Biskut A")
        .expect("record");
    assert_eq!(biscuit_a.brand, "BrandA");
    assert_eq!(biscuit_a.holder, "t6_Parent Co");
    assert_eq!(biscuit_a.expiry_date, "2026-12-31");
}

#[tokio::test]
async fn test_fetch_subcategory_paginates_via_page_param() {
    let ctx = common::setup_mock().await;

    ctx.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("cari", "a")
            .query_param("page", "1");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::product_listing_html(
                &[("t6_Page One", "BrandX", "Co X", "2028-01-01")],
                2,
            ));
    });
    ctx.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("cari", "a")
            .query_param("page", "2");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::product_listing_html(
                &[("t6_Page Two", "BrandY", "Co Y", "2028-02-02")],
                2,
            ));
    });
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>empty</body></html>");
    });

    let records = listing::fetch_subcategory(&ctx.portal, &product_strategy(), None)
        .await
        .expect("scrape");

    let names: Vec<&str> = records.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"t6_Page One"), "got: {names:?}");
    assert!(names.contains(&"t6_Page Two"), "got: {names:?}");
    assert_eq!(records.len(), 2);
}

#[tokio::test]
async fn test_fetch_companies_empty_listing_returns_empty() {
    let ctx = common::setup_mock().await;

    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>No spans here</body></html>");
    });

    let records = listing::fetch_companies(&ctx.portal, &company_strategy(), None)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 0);
}

#[tokio::test]
async fn test_fetch_subcategory_no_records_returns_empty() {
    let ctx = common::setup_mock().await;

    let strategy = types::SubStrategy {
        category_code: "ZZ",
        category_name: "Unknown",
        sub_code: "ZZ",
        sub_name: "Unknown",
    };

    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>Nothing here</body></html>");
    });

    let records = listing::fetch_subcategory(&ctx.portal, &strategy, None)
        .await
        .expect("scrape");
    assert_eq!(records.len(), 0);
}
