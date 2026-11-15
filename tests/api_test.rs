use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use halal_crawler::records::Company;
use halal_crawler::{api, db};

#[allow(dead_code)]
mod common;

/// Drive the router without a listener, exactly as a client would.
async fn request(app: &axum::Router, method: Method, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request");

    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json body")
    };

    (status, value)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    request(app, Method::GET, uri).await
}

#[tokio::test]
async fn test_health_reports_ok() {
    let ctx = common::setup_db().await;
    let app = api::router(ctx.pool.clone());

    let (status, body) = get(&app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], "ok");
}

#[tokio::test]
async fn test_invalid_query_is_json_bad_request() {
    let ctx = common::setup_db().await;
    let app = api::router(ctx.pool.clone());

    let (status, body) = get(&app, "/api/v1/companies?page=not-a-number").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["status"].as_i64(), Some(400));
}

#[tokio::test]
async fn test_bad_path_unknown_route_and_wrong_method_are_json() {
    let ctx = common::setup_db().await;
    let app = api::router(ctx.pool.clone());

    let (status, body) = get(&app, "/api/v1/companies/not-a-number").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["status"].as_i64(), Some(400));

    let (status, body) = get(&app, "/api/v1/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["status"].as_i64(), Some(404));

    let (status, body) = request(&app, Method::POST, "/api/v1/companies").await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(body["error"]["status"].as_i64(), Some(405));
}

#[tokio::test]
async fn test_companies_list_paginates_filters_and_details() {
    let ctx = common::setup_db().await;
    let names = ["tAPIC_Alpha Sdn Bhd", "tAPIC_Beta Sdn Bhd"];

    let companies = vec![
        Company {
            name: names[0].to_string(),
            state: "Selangor".to_string(),
            ..Default::default()
        },
        Company {
            name: names[1].to_string(),
            state: "Kedah".to_string(),
            ..Default::default()
        },
    ];
    db::insert_companies(&ctx.pool, &companies)
        .await
        .expect("insert companies");

    let app = api::router(ctx.pool.clone());

    let (status, body) = get(&app, "/api/v1/companies?q=tAPIC_&per_page=1&page=1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"].as_i64(), Some(2));
    assert_eq!(body["meta"]["per_page"].as_i64(), Some(1));
    assert_eq!(body["meta"]["total_pages"].as_i64(), Some(2));
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    let (status, body) = get(&app, "/api/v1/companies?q=tAPIC_&state=Selangor").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"].as_i64(), Some(1));
    assert_eq!(body["data"][0]["state"], "Selangor");

    let id: i32 = sqlx::query_scalar("SELECT id FROM companies WHERE name = $1")
        .bind(names[0])
        .fetch_one(&ctx.pool)
        .await
        .expect("company id");

    let (status, body) = get(&app, &format!("/api/v1/companies/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["name"], names[0]);
    assert_eq!(body["data"]["state"], "Selangor");

    let (status, body) = get(&app, "/api/v1/companies/999999999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["status"].as_i64(), Some(404));

    common::cleanup(&ctx.pool, &names, &[]).await;
}

#[tokio::test]
async fn test_products_lists_memberships_and_company_scope() {
    let ctx = common::setup_db().await;
    let company_name = "tAPIP_Products Co";

    db::insert_companies(&ctx.pool, &[common::company(company_name)])
        .await
        .expect("insert company");

    let mut product_a = common::product("tAPIP_Product A", "BrandA", company_name, "2027-01-01");
    product_a.category_code = "PR".to_string();
    product_a.subcategory_code = "PR".to_string();

    let mut product_b = common::product("tAPIP_Product B", "BrandB", company_name, "2028-01-01");
    product_b.category_code = "BG".to_string();
    product_b.subcategory_code = "BG".to_string();

    db::insert_products(&ctx.pool, &[product_a, product_b])
        .await
        .expect("insert products");

    let company_id: i32 = sqlx::query_scalar("SELECT id FROM companies WHERE name = $1")
        .bind(company_name)
        .fetch_one(&ctx.pool)
        .await
        .expect("company id");

    let app = api::router(ctx.pool.clone());

    let (status, body) = get(&app, "/api/v1/products?q=tAPIP_Product&category=PR").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"].as_i64(), Some(1));
    assert_eq!(body["data"][0]["name"], "tAPIP_Product A");
    assert_eq!(body["data"][0]["company"]["name"], company_name);
    assert_eq!(body["data"][0]["categories"][0]["category_code"], "PR");

    // Category and subcategory must match the same membership row.
    let (status, body) = get(
        &app,
        "/api/v1/products?q=tAPIP_Product&category=PR&subcategory=BG",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"].as_i64(), Some(0));

    let (status, body) = get(
        &app,
        &format!("/api/v1/products?company_id={company_id}&per_page=1"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"].as_i64(), Some(2));
    assert_eq!(body["data"].as_array().unwrap().len(), 1);

    let product_id: i32 = sqlx::query_scalar("SELECT id FROM products WHERE name = $1")
        .bind("tAPIP_Product A")
        .fetch_one(&ctx.pool)
        .await
        .expect("product id");

    let (status, body) = get(&app, &format!("/api/v1/products/{product_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["id"].as_i64(), Some(product_id as i64));
    assert_eq!(
        body["data"]["company"]["id"].as_i64(),
        Some(company_id as i64)
    );
    assert_eq!(body["data"]["categories"].as_array().unwrap().len(), 1);

    let (status, body) = get(&app, &format!("/api/v1/companies/{company_id}/products")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"].as_i64(), Some(2));

    let (status, body) = get(&app, "/api/v1/companies/999999999/products").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["status"].as_i64(), Some(404));

    common::cleanup(
        &ctx.pool,
        &[company_name],
        &["tAPIP_Product A", "tAPIP_Product B"],
    )
    .await;
}

#[tokio::test]
async fn test_categories_and_scrape_runs() {
    let ctx = common::setup_db().await;
    let company_name = "tAPIR_Runs Co";
    let product_name = "tAPIR_Run Product";

    db::insert_companies(&ctx.pool, &[common::company(company_name)])
        .await
        .expect("insert company");
    db::insert_products(
        &ctx.pool,
        &[common::product(
            product_name,
            "RunBrand",
            company_name,
            "2027-01-01",
        )],
    )
    .await
    .expect("insert product");

    let log_id = db::start_scrap(&ctx.pool, "ZA", "companies")
        .await
        .expect("start scrap log");
    db::finish_scrap(&ctx.pool, log_id, 3, 1)
        .await
        .expect("finish scrap log");

    let app = api::router(ctx.pool.clone());

    let (status, body) = get(&app, "/api/v1/categories").await;
    assert_eq!(status, StatusCode::OK);
    let pr = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["category_code"] == "PR")
        .expect("PR category");
    assert!(
        pr["subcategories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|sub| sub["code"] == "PR" && sub["product_count"].as_i64().unwrap() >= 1)
    );

    assert!(
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|category| category["subcategories"]
                .as_array()
                .unwrap()
                .iter()
                .all(|sub| sub["code"] != "CO")),
        "legacy CO memberships must not appear as product categories"
    );

    let (status, body) = get(&app, "/api/v1/scrape-runs?limit=200").await;
    assert_eq!(status, StatusCode::OK);
    let run = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|run| run["category_code"] == "ZA")
        .expect("ZA run");
    assert_eq!(run["phase"], "companies");
    assert_eq!(run["inserted_count"].as_i64(), Some(3));
    assert_eq!(run["updated_count"].as_i64(), Some(1));
    assert!(!run["finished_at"].is_null());

    sqlx::query("DELETE FROM scrap_log WHERE category_code = 'ZA'")
        .execute(&ctx.pool)
        .await
        .ok();
    common::cleanup(&ctx.pool, &[company_name], &[product_name]).await;
}
