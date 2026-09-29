use httpmock::prelude::*;

use halal_crawler::records::Company;
use halal_crawler::types::{CrawlTarget, Phase};
use halal_crawler::{crawl, db, listing};

mod common;

fn company_target() -> CrawlTarget {
    CrawlTarget {
        category_code: "BG",
        ty: "CO",
        phase: Phase::Companies,
    }
}

fn product_target() -> CrawlTarget {
    CrawlTarget {
        category_code: "PR",
        ty: "PR",
        phase: Phase::Products,
    }
}

// ── DB insert/upsert (needs live PostgreSQL) ────────────────────

#[tokio::test]
async fn test_db_insert_and_query_companies() {
    let ctx = common::setup_db().await;

    let names = &["t1_ABC Sdn Bhd", "t1_XYZ Sdn Bhd"];
    let records = vec![
        Company {
            name: names[0].to_string(),
            state: "Selangor".to_string(),
            ..Default::default()
        },
        Company {
            name: names[1].to_string(),
            state: "KL".to_string(),
            ..Default::default()
        },
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
    db::insert_companies(&ctx.pool, &[common::company(company_names[0])])
        .await
        .unwrap();

    let product_names = &["t2_Product A", "t2_Product B"];
    let products = vec![
        common::product(product_names[0], "BrandA", "t2_Parent Co", "2026-12-31"),
        common::product(product_names[1], "BrandB", "t2_Parent Co", "2027-06-15"),
    ];

    let (inserted, updated) = db::insert_products(&ctx.pool, &products)
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
async fn test_db_insert_products_writes_category_mapping() {
    let ctx = common::setup_db().await;

    let company_name = "tPC_Holder Co";
    db::insert_companies(&ctx.pool, &[common::company(company_name)])
        .await
        .unwrap();

    let product_name = "tPC_Product A";
    let products = vec![common::product(
        product_name,
        "BrandPC",
        company_name,
        "2027-01-01",
    )];
    db::insert_products(&ctx.pool, &products)
        .await
        .expect("insert");

    let (cat, sub): (String, String) = sqlx::query_as(
        "SELECT pc.category_code, pc.subcategory_code
         FROM product_categories pc
         JOIN products p ON p.id = pc.product_id
         WHERE p.name = $1",
    )
    .bind(product_name)
    .fetch_one(&ctx.pool)
    .await
    .expect("mapping row");
    assert_eq!((cat.as_str(), sub.as_str()), ("PR", "PR"));

    common::cleanup(&ctx.pool, &[company_name], &[product_name]).await;
}

#[tokio::test]
async fn test_db_insert_products_links_holder_case_insensitively() {
    let ctx = common::setup_db().await;

    let company_name = "t3_Holder Co";
    db::insert_companies(&ctx.pool, &[common::company(company_name)])
        .await
        .unwrap();

    let product_name = "t3_Product A";
    let products = vec![common::product(
        product_name,
        "BrandX",
        "  T3_HOLDER co  ",
        "2026-01-01",
    )];
    db::insert_products(&ctx.pool, &products)
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
    let products = vec![common::product(
        product_name,
        "BrandY",
        holder,
        "2026-01-01",
    )];
    let (inserted, updated) = db::insert_products(&ctx.pool, &products)
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

    let (inserted, updated) = db::insert_products(&ctx.pool, &[]).await.expect("ok");
    assert_eq!((inserted, updated), (0, 0));
}

#[tokio::test]
async fn test_db_upsert_companies() {
    let ctx = common::setup_db().await;

    let names = &["t4_Foo"];
    let first = vec![Company {
        name: names[0].to_string(),
        state: "KL".to_string(),
        ..Default::default()
    }];
    db::insert_companies(&ctx.pool, &first).await.unwrap();

    let second = vec![Company {
        name: names[0].to_string(),
        state: "Selangor".to_string(),
        ..Default::default()
    }];
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
    let records = vec![common::company(names[0]), common::company(names[1])];
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

    let records = listing::fetch_companies(&ctx.portal, &company_target(), None)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 2);
    let names: Vec<&str> = records.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"t5_ABC Sdn Bhd"));
    assert!(names.contains(&"t5_XYZ Sdn Bhd"));
}

#[tokio::test]
async fn test_scrape_companies_caps_pages_per_letter() {
    let ctx = common::setup_mock().await;

    // Page 1 announces 5 total pages, but the cap is 1: only page 1 is
    // fetched, and page 2 is never requested.
    let page1 = ctx.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("cari", "a")
            .query_param("page", "1");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::listing_html(
                &[("tCap_Alpha", "1 Jalan, 50000 KL, Kuala Lumpur")],
                5,
            ));
    });
    let page2 = ctx.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("cari", "a")
            .query_param("page", "2");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::listing_html(
                &[("tCap_Beta", "2 Jalan, 50000 KL, Kuala Lumpur")],
                5,
            ));
    });
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>empty</body></html>");
    });

    let records = listing::fetch_companies(&ctx.portal, &company_target(), Some(1))
        .await
        .expect("scrape");

    let names: Vec<&str> = records.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"tCap_Alpha"), "got: {names:?}");
    assert!(!names.contains(&"tCap_Beta"), "page 2 should be capped");
    page1.assert_hits(1);
    page2.assert_hits(0);
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

    let records = listing::fetch_companies(&ctx.portal, &company_target(), None)
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

    let records = listing::fetch_companies(&ctx.portal, &company_target(), None)
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

    let records = listing::fetch_subcategory(&ctx.portal, &product_target(), None)
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
async fn test_fetch_subcategory_dedups_by_name_brand_holder() {
    let ctx = common::setup_mock().await;

    // Two tD_Dup rows share (name, brand, holder) but differ in expiry: the
    // in-memory identity must match the persistence key (company_id, name,
    // brand), so only one survives. tD_Other has a different brand.
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

    let records = listing::fetch_subcategory(&ctx.portal, &product_target(), None)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 2);
    assert_eq!(records.iter().filter(|r| r.name == "tD_Dup").count(), 1);
    assert!(records.iter().any(|r| r.name == "tD_Other"));
}

#[tokio::test]
async fn test_fetch_subcategory_stamps_target_membership() {
    let ctx = common::setup_mock().await;

    let page1 =
        common::product_listing_html(&[("tS_Prod", "BrandZ", "Holder Co", "2026-01-01")], 1);
    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body(page1);
    });

    let records = listing::fetch_subcategory(&ctx.portal, &product_target(), None)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].category_code, "PR");
    assert_eq!(records[0].subcategory_code, "PR");
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

    let records = listing::fetch_subcategory(&ctx.portal, &product_target(), None)
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

    let records = listing::fetch_companies(&ctx.portal, &company_target(), None)
        .await
        .expect("scrape");

    assert_eq!(records.len(), 0);
}

#[tokio::test]
async fn test_fetch_subcategory_no_records_returns_empty() {
    let ctx = common::setup_mock().await;

    let target = CrawlTarget {
        category_code: "ZZ",
        ty: "ZZ",
        phase: Phase::Products,
    };

    ctx.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>Nothing here</body></html>");
    });

    let records = listing::fetch_subcategory(&ctx.portal, &target, None)
        .await
        .expect("scrape");
    assert_eq!(records.len(), 0);
}

// ── Modal detail fetching (httpmock, no database) ────────────────

#[tokio::test]
async fn test_fetch_modal_retries_server_errors() {
    let ctx = common::setup_mock().await;

    // The modal endpoint fails every time: the seam must retry 3× and then
    // surface the error rather than silently returning the 500 body.
    let mock = ctx.server.mock(|when, then| {
        when.method(GET).path("/directory/slm_viewdetail.php");
        then.status(500).body("boom");
    });

    let result = ctx.portal.fetch_modal("COMP-1").await;
    assert!(result.is_err(), "exhausted retries should error");
    mock.assert_hits(3);
}

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

    let companies = vec![Company {
        name: "tM_Listing Co".to_string(),
        address: "1 Jalan, 50000 KL, Kuala Lumpur".to_string(),
        comp_code: "COMP-20230804-000001".to_string(),
        ..Default::default()
    }];

    let entries = listing::fetch_company_modals(&ctx.portal, &company_target(), &companies)
        .await
        .expect("modals");

    assert_eq!(entries.len(), 1);
    let (company, products) = &entries[0];
    // Enriched fields came from the modal …
    assert_eq!(company.phone_no, "03-1234567");
    assert_eq!(company.email, "a@b.example");
    // … but the listing name is the identity and is never rewritten by the
    // modal, even when the modal spells it differently.
    assert_eq!(company.name, "tM_Listing Co");
    // Fields missing from the modal fall back to the listing values
    assert_eq!(company.address, "1 Jalan, 50000 KL, Kuala Lumpur");
    assert_eq!(company.comp_code, "COMP-20230804-000001");
    // Products were parsed from the modal's Product / Menu List and stamped
    // with the target's membership.
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].name, "HK1 TEST PRODUCT A");
    assert_eq!(products[0].expiry_date, "15/07/2029");
    assert_eq!(products[0].category_code, "BG");
    assert_eq!(products[0].subcategory_code, "CO");
}

#[tokio::test]
async fn test_fetch_company_modals_skips_companies_without_comp_code() {
    let ctx = common::setup_mock().await;

    let companies = vec![common::company("tM_NoModal Co")];

    let entries = listing::fetch_company_modals(&ctx.portal, &company_target(), &companies)
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

#[tokio::test]
async fn test_db_enrichment_does_not_split_company_on_name_spelling() {
    let ctx = common::setup_db().await;

    // Run 1: the listing discovers "tN_ABC Sdn Bhd".
    let listing = Company {
        name: "tN_ABC Sdn Bhd".to_string(),
        address: "1 Jalan, 50000 KL, Kuala Lumpur".to_string(),
        comp_code: "COMP-N-1".to_string(),
        ..Default::default()
    };
    db::insert_companies(&ctx.pool, &[listing.clone()])
        .await
        .unwrap();

    // Run 2's modal spells the name differently. Enrichment must keep the
    // listing identity, so this upserts the same row — not a second one.
    let mut enriched = Company {
        name: "tN_ABC SDN. BHD.".to_string(),
        phone_no: "03-5555".to_string(),
        ..Default::default()
    };
    enriched.name = listing.name.clone();
    enriched.fill_from(&listing);
    db::insert_companies(&ctx.pool, &[enriched]).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM companies WHERE name LIKE 'tN_%'")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "one company, not a duplicate row");

    let phone: String = sqlx::query_scalar("SELECT phone_no FROM companies WHERE name = $1")
        .bind("tN_ABC Sdn Bhd")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(phone, "03-5555");

    common::cleanup(&ctx.pool, &["tN_ABC Sdn Bhd"], &[]).await;
}

// ── crawl::run orchestration (httpmock + live PostgreSQL) ────────

#[tokio::test]
async fn test_run_reports_counts_and_closes_scrap_log() {
    let mock = common::setup_mock().await;
    let ctx = common::setup_db().await;

    let company_name = "tRun_Alpha Co";
    let product_name = "tRun_Product A";

    // Unique category codes so this run's scrap_log rows are identifiable
    // even while other tests write to scrap_log concurrently.
    let targets = vec![
        CrawlTarget {
            category_code: "ZQ",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "ZR",
            ty: "ZR",
            phase: Phase::Products,
        },
    ];

    // Phase 1: the letter search returns one company with a modal comp_code.
    mock.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("category", "ZQ")
            .query_param("cari", "a")
            .query_param("page", "1");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::listing_html(
                &[(company_name, "1 Jalan, 50000 KL, Kuala Lumpur")],
                1,
            ));
    });
    // Its modal enriches the company and yields one product.
    mock.server.mock(|when, then| {
        when.method(GET)
            .path("/directory/slm_viewdetail.php")
            .query_param("comp_code", "COMP-20230804-000001");
        then.status(200)
            .header("content-type", "text/html")
            .body(format!(
                "<html><body><table>\
             <tr><td><b><div align=\"right\">Phone No :</div></b></td>\
             <td>03-1111</td></tr>\
             <tr><td colspan=\"2\"><b>Product / Menu List :</b>\
             <table border=\"1\">\
             <tr><td align=\"center\">1.</td>\
             <td class=\"txt\">{product_name}</td>\
             <td class=\"txt\">{company_name}</td>\
             <td align=\"center\">15/07/2029</td></tr>\
             </table></td></tr>\
             </table></body></html>"
            ));
    });
    // Phase 2: the subcategory listing returns one product.
    mock.server.mock(|when, then| {
        when.method(POST)
            .path("/index.php")
            .query_param("category", "ZR")
            .query_param("cari", "a")
            .query_param("page", "1");
        then.status(200)
            .header("content-type", "text/html")
            .body(common::product_listing_html(
                &[("tRun_Prod B", "BrandRun", company_name, "2027-01-01")],
                1,
            ));
    });
    // Every other letter/category returns an empty listing.
    mock.server.mock(|when, then| {
        when.method(POST).path("/index.php");
        then.status(200)
            .header("content-type", "text/html")
            .body("<html><body>empty</body></html>");
    });

    let report = crawl::run(&mock.portal, &ctx.pool, &targets, None)
        .await
        .expect("run");

    assert!(
        report.failures.is_empty(),
        "failures: {:?}",
        report.failures
    );
    assert!(report.companies_inserted >= 1, "report: {report:?}");
    assert!(report.products_inserted >= 1, "report: {report:?}");

    // The company was enriched from the modal.
    let phone: String = sqlx::query_scalar("SELECT phone_no FROM companies WHERE name = $1")
        .bind(company_name)
        .fetch_one(&ctx.pool)
        .await
        .expect("company row");
    assert_eq!(phone, "03-1111");

    // This run opened one scrap_log row per target and closed every one.
    let (total, open): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE finished_at IS NULL)
         FROM scrap_log WHERE category_code IN ('ZQ', 'ZR')",
    )
    .fetch_one(&ctx.pool)
    .await
    .expect("scrap_log");
    assert_eq!(total, 2, "one row per target");
    assert_eq!(open, 0, "every scrap_log row was closed");

    let phases: Vec<String> = sqlx::query_scalar(
        "SELECT phase FROM scrap_log WHERE category_code IN ('ZQ', 'ZR') ORDER BY id",
    )
    .fetch_all(&ctx.pool)
    .await
    .expect("scrap_log phases");
    assert_eq!(phases, vec!["companies", "products"]);

    // Clean up the log rows this test created.
    sqlx::query("DELETE FROM scrap_log WHERE category_code IN ('ZQ', 'ZR')")
        .execute(&ctx.pool)
        .await
        .ok();

    common::cleanup(&ctx.pool, &[company_name], &[product_name, "tRun_Prod B"]).await;
}
