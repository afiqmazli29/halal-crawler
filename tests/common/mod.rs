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
    // product_categories is deleted via ON DELETE CASCADE from products.
    for name in product_names {
        sqlx::query("DELETE FROM products WHERE name = $1")
            .bind(name)
            .execute(pool)
            .await
            .ok();
    }
}

/// A directory search response: span-based listing plus the portal's
/// "Total Record … From N" line that announces the total page count.
pub fn listing_html(records: &[(&str, &str)], total_pages: u32) -> String {
    let mut spans = String::new();
    for (name, address) in records {
        spans.push_str(&format!(
            "<span class=\"company-name\">{name}</span>\n\
             <span class=\"company-address\">{address}</span>\n"
        ));
    }
    let total_pages_line = format!("Total Record : 955 - Page 1 From {total_pages}");
    format!("<html><body>\n{total_pages_line}\n{spans}</body></html>")
}

/// A product/premise listing page in the portal's table-row shape:
/// name span, optional JENAMA brand span, certificate holder, and an
/// expiry date cell. Pages past the first need only the Total Record line.
pub fn product_listing_html(records: &[(&str, &str, &str, &str)], total_pages: u32) -> String {
    let mut rows = String::new();
    for (i, (name, brand, company, expiry)) in records.iter().enumerate() {
        let brand_span = if brand.is_empty() {
            String::new()
        } else {
            format!("<span class=\"company-brand\"><br><b>JENAMA:</b>{brand}<br></span>\n")
        };
        rows.push_str(&format!(
            "<tr>\n\
             <td class=\"text-center font-semibold\">{}</td>\n\
             <td>\n\
             <span class=\"company-name\">{name}</span>\n\
             {brand_span}\
             <span class=\"company-address\"><i>{company}</i></span>\n\
             </td>\n\
             <td class=\"text-center\">{expiry}</td>\n\
             </tr>\n",
            i + 1
        ));
    }
    let total_pages_line = format!("Total Record : 999 - Page 1 From {total_pages}");

    format!(
        "<html><body>\n\
         {total_pages_line}\n\
         <table>\n\
         {rows}\
         </table>\n\
         </body></html>"
    )
}
