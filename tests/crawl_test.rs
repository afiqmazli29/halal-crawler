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

    let (inserted, updated) = db::insert_companies(&ctx.pool, &records)
        .await
        .expect("insert");
    assert_eq!((inserted, updated), (2, 0));

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

    let (inserted, updated) = db::insert_products(&ctx.pool, &products, "PR", "PR")
        .await
        .expect("insert");
    assert_eq!((inserted, updated), (2, 0));

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
async fn test_db_insert_products_links_holder_case_insensitively() {
    let ctx = common::setup_db().await;

    let company_name = "t3_Holder Co";
    db::insert_companies(
        &ctx.pool,
        &[Company::from_value(&json!({"nama_syarikat": company_name}))],
    )
    .await
    .unwrap();

    let product_name = "t3_Product A";
    let products = vec![Product::from_value(
        &json!({"name": product_name, "brand": "BrandX", "company": "  T3_HOLDER co  "}),
    )];
    db::insert_products(&ctx.pool, &products, "PR", "PR")
        .await
        .expect("insert");

    let company_id: Option<i32> =
        sqlx::query_scalar("SELECT company_id FROM products WHERE name = $1")
            .bind(product_name)
            .fetch_one(&ctx.pool)
            .await
            .expect("product company_id");
    let expected: i32 = sqlx::query_scalar("SELECT id FROM companies WHERE name = $1")
        .bind(company_name)
        .fetch_one(&ctx.pool)
        .await
        .expect("company id");
    assert_eq!(company_id, Some(expected));

    common::cleanup(&ctx.pool, &[company_name], &[product_name]).await;
}

#[tokio::test]
async fn test_db_insert_products_skips_unresolvable_holder() {
    let ctx = common::setup_db().await;

    let holder = "t3_Brand New Holder Co";
    let product_name = "t3_Product B";
    let products = vec![Product::from_value(
        &json!({"name": product_name, "brand": "BrandY", "company": holder}),
    )];
    let (inserted, updated) = db::insert_products(&ctx.pool, &products, "PR", "PR")
        .await
        .expect("insert");
    assert_eq!((inserted, updated), (0, 0));

    let product_count: i64 = sqlx::query_scalar("SELECT count(*) FROM products WHERE name = $1")
        .bind(product_name)
        .fetch_one(&ctx.pool)
        .await
        .expect("count");
    assert_eq!(product_count, 0);

    let company_count: i64 = sqlx::query_scalar("SELECT count(*) FROM companies WHERE name = $1")
        .bind(holder)
        .fetch_one(&ctx.pool)
        .await
        .expect("count");
    assert_eq!(company_count, 0);
}

#[tokio::test]
async fn test_db_insert_empty_returns_zero() {
    let ctx = common::setup_db().await;

    let (inserted, updated) = db::insert_companies(&ctx.pool, &[]).await.expect("ok");
    assert_eq!((inserted, updated), (0, 0));

    let (inserted, updated) = db::insert_products(&ctx.pool, &[], "PR", "PR")
        .await
        .expect("ok");
    assert_eq!((inserted, updated), (0, 0));
}

#[tokio::test]
async fn test_db_upsert_companies() {
    let ctx = common::setup_db().await;

    let names = &["t4_Foo"];
    let first = vec![Company::from_value(
        &json!({"nama_syarikat": names[0], "state": "KL"}),
    )];
    db::insert_companies(&ctx.pool, &first).await.unwrap();

    let second = vec![Company::from_value(
        &json!({"nama_syarikat": names[0], "state": "Selangor"}),
    )];
    let (inserted, updated) = db::insert_companies(&ctx.pool, &second).await.unwrap();
    assert_eq!((inserted, updated), (0, 1));

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

#[tokio::test]
async fn test_db_scrap_log_records_inserts() {
    let ctx = common::setup_db().await;

    let log_id = db::start_scrap(&ctx.pool, "BG", "companies")
        .await
        .expect("start");

    let names = &["tSL_Co A", "tSL_Co B"];
    let records = vec![
        Company::from_value(&json!({"nama_syarikat": names[0]})),
        Company::from_value(&json!({"nama_syarikat": names[1]})),
    ];
    let (inserted, updated) = db::insert_companies(&ctx.pool, &records)
        .await
        .expect("insert");
    db::finish_scrap(&ctx.pool, log_id, inserted, updated)
        .await
        .expect("finish");

    let (phase, ins, upd, finished): (String, i32, i32, bool) = sqlx::query_as(
        "SELECT phase, inserted_count, updated_count, finished_at IS NOT NULL
         FROM scrap_log WHERE id = $1",
    )
    .bind(log_id)
    .fetch_one(&ctx.pool)
    .await
    .expect("log row");
    assert_eq!(phase, "companies");
    assert_eq!((ins, upd, finished), (2, 0, true));

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
async fn test_fetch_subcategory_dedups_by_name_brand_holder_expiry() {
    let ctx = common::setup_mock().await;

    let page1 = common::product_listing_html(
        &[
            ("tD_Dup", "BrandX", "Holder Co", "2026-01-01"),
            ("tD_Dup", "BrandX", "Holder Co", "2026-01-01"),
            ("tD_Dup", "BrandX", "Holder Co", "2026-02-02"),
            ("tD_Other", "BrandY", "Holder Co", "2026-01-01"),
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

    assert_eq!(records.len(), 3);

    let expiries: Vec<&str> = records
        .iter()
        .filter(|r| r.name == "tD_Dup")
        .map(|r| r.expiry_date.as_str())
        .collect();
    assert!(expiries.contains(&"2026-01-01"), "got: {expiries:?}");
    assert!(expiries.contains(&"2026-02-02"), "got: {expiries:?}");
    assert!(records.iter().any(|r| r.name == "tD_Other"));
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

// ── Modal detail fetching (httpmock, no database) ────────────────

#[tokio::test]
async fn test_fetch_company_modals_enriches_and_returns_products() {
    let ctx = common::setup_mock().await;

    // The modal endpoint: company detail + product list in the live shape.
    ctx.server.mock(|when, then| {
        when.method(GET)
            .path("/directory/slm_viewdetail.php")
            .query_param("comp_code", "COMP-20230804-000001");
        then.status(200).header("content-type", "text/html").body(
            "<html><body><table>\
                 <tr><td><b><div align=\"right\">Name :</div></b></td>\
                 <td>tM_Enriched Co</td></tr>\
                 <tr><td><b><div align=\"right\">Phone No :</div></b></td>\
                 <td>03-1234567</td></tr>\
                 <tr><td><b><div align=\"right\">e-mail :</div></b></td>\
                 <td>a@b.example</td></tr>\
                 <tr><td colspan=\"2\"><b>Product / Menu List :</b>\
                 <table border=\"1\">\
                 <tr><td align=\"center\">1.</td>\
                 <td class=\"txt\">HK1 TEST PRODUCT A</td>\
                 <td class=\"txt\">tM_Enriched Co</td>\
                 <td align=\"center\">15/07/2029</td></tr>\
                 </table></td></tr>\
                 </table></body></html>",
        );
    });

    let companies = vec![Company::from_value(&json!({
        "nama_syarikat": "tM_Listing Co",
        "alamat": "1 Jalan, 50000 KL, Kuala Lumpur",
        "comp_code": "COMP-20230804-000001",
    }))];

    let entries = listing::fetch_company_modals(&ctx.portal, &companies, 2)
        .await
        .expect("modals");

    assert_eq!(entries.len(), 1);
    let (company, products) = &entries[0];
    // Enriched fields came from the modal
    assert_eq!(company.name, "tM_Enriched Co");
    assert_eq!(company.phone_no, "03-1234567");
    assert_eq!(company.email, "a@b.example");
    // Fields missing from the modal fall back to the listing values
    assert_eq!(company.address, "1 Jalan, 50000 KL, Kuala Lumpur");
    assert_eq!(company.comp_code, "COMP-20230804-000001");
    // Products were parsed from the modal's Product / Menu List
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].name, "HK1 TEST PRODUCT A");
    assert_eq!(products[0].expiry_date, "15/07/2029");
}

#[tokio::test]
async fn test_fetch_company_modals_skips_companies_without_comp_code() {
    let ctx = common::setup_mock().await;

    let companies = vec![Company::from_value(&json!({
        "nama_syarikat": "tM_NoModal Co",
    }))];

    let entries = listing::fetch_company_modals(&ctx.portal, &companies, 2)
        .await
        .expect("modals");

    assert_eq!(entries.len(), 1);
    let (company, products) = &entries[0];
    assert_eq!(company.name, "tM_NoModal Co");
    assert_eq!(company.phone_no, "");
    assert!(products.is_empty());
}

// ── DB persistence of enriched fields (needs live PostgreSQL) ────

#[tokio::test]
async fn test_db_persists_modal_enriched_fields() {
    let ctx = common::setup_db().await;

    let name = "tDB_Enriched Co";
    let enriched = vec![halal_crawler::records::Company {
        name: name.to_string(),
        address: "99 Jalan Uji, 43650 Bangi, Selangor".to_string(),
        postcode: "43650".to_string(),
        state: "Selangor".to_string(),
        phone_no: "03-9876543".to_string(),
        fax_no: String::new(),
        email: "x@y.example".to_string(),
        website: String::new(),
        reference_no: "JAKIM.700-1/1/1 100-1/2025".to_string(),
        officer: "Officer A".to_string(),
        comp_code: "COMP-20240101-999999".to_string(),
    }];

    let (inserted, updated) = halal_crawler::db::insert_companies(&ctx.pool, &enriched)
        .await
        .unwrap();
    assert_eq!((inserted, updated), (1, 0));

    let (phone, email, ref_no, comp): (String, String, String, String) = sqlx::query_as(
        "SELECT phone_no, email, reference_no, comp_code FROM companies WHERE name = $1",
    )
    .bind(name)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(phone, "03-9876543");
    assert_eq!(email, "x@y.example");
    assert_eq!(ref_no, "JAKIM.700-1/1/1 100-1/2025");
    assert_eq!(comp, "COMP-20240101-999999");

    common::cleanup(&ctx.pool, &[name], &[]).await;
}
