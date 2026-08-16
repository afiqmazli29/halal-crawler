use std::time::Duration;

use httpmock::MockServer;
use sqlx::PgPool;

use halal_crawler::{db, portal::Portal};

/// Context for tests that need a real PostgreSQL database.
pub struct DbCtx {
    pub pool: PgPool,
}

pub async fn setup_db() -> DbCtx {
    let db_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/halal".to_string());

    let pool = {
        let mut attempts = 0u32;
        loop {
            match db::init(&db_url).await {
                Ok(p) => break p,
                Err(e) if attempts < 5 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    eprintln!("DB init retry {attempts}: {e}");
                }
                Err(e) => panic!("DB init failed after {attempts} retries: {e}"),
            }
        }
    };

    DbCtx { pool }
}

/// Context for crawl tests: an httpmock server with a Portal rooted at it.
/// No database needed — the Portal is the only adapter under test.
pub struct MockCtx {
    pub server: MockServer,
    pub portal: Portal,
}

pub async fn setup_mock() -> MockCtx {
    let server = MockServer::start();
    let portal = Portal::new(server.base_url()).expect("portal");
    MockCtx { server, portal }
}

pub async fn cleanup(pool: &PgPool, company_names: &[&str], product_names: &[&str]) {
    for name in company_names {
        sqlx::query("DELETE FROM companies WHERE name = $1")
            .bind(name)
            .execute(pool)
            .await
            .ok();
    }
    for name in product_names {
        sqlx::query("DELETE FROM products WHERE name = $1")
            .bind(name)
            .execute(pool)
            .await
            .ok();
    }
}

/// A directory search response: span-based listing plus the hdnCounter
/// hidden input, as the live portal renders it (value may be unquoted).
pub fn listing_html(records: &[(&str, &str)], counter: u32) -> String {
    let mut spans = String::new();
    for (name, address) in records {
        spans.push_str(&format!(
            "<span class=\"company-name\">{name}</span>\n\
             <span class=\"company-address\">{address}</span>\n"
        ));
    }
    format!(
        "<html><body>\n{spans}<input type=\"hidden\" name=\"hdnCounter\" value={counter}>\n</body></html>"
    )
}

/// First page of a GET product listing (old flow) with a Total Record line.
pub fn product_listing_html(records: &[(&str, &str, &str)], total_pages: u32) -> String {
    let mut spans = String::new();
    for (name, address, _expiry) in records {
        spans.push_str(&format!(
            "<span class=\"company-name\">{name}</span>\n\
             <span class=\"company-address\">{address}</span>\n"
        ));
    }
    let total_pages_line = format!("Total Record : 99999 From {total_pages}");

    format!(
        "<html><body>\n\
         {total_pages_line}\n\
         {spans}\
         </body></html>"
    )
}
