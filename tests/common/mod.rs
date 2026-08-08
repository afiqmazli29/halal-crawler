use std::sync::Arc;

use httpmock::MockServer;
use sqlx::PgPool;
use tokio::sync::Semaphore;

use halal_crawler::{config, db};

pub struct TestCtx {
    pub pool: PgPool,
    pub server: MockServer,
    pub client: Arc<reqwest::Client>,
    pub semaphore: Arc<Semaphore>,
}

pub async fn setup() -> TestCtx {
    let db_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/halal".to_string());

    let pool = {
        let mut attempts = 0u32;
        loop {
            match db::init(&db_url).await {
                Ok(p) => break p,
                Err(e) if attempts < 5 => {
                    attempts += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    eprintln!("DB init retry {attempts}: {e}");
                }
                Err(e) => panic!("DB init failed after {attempts} retries: {e}"),
            }
        }
    };

    let server = MockServer::start();

    TestCtx {
        pool,
        server,
        client: Arc::new(reqwest::Client::new()),
        semaphore: config::semaphore(),
    }
}

pub async fn cleanup(ctx: &TestCtx, company_names: &[&str], product_names: &[&str]) {
    for name in company_names {
        sqlx::query("DELETE FROM companies WHERE name = $1")
            .bind(name)
            .execute(&ctx.pool)
            .await
            .ok();
    }
    for name in product_names {
        sqlx::query("DELETE FROM products WHERE name = $1")
            .bind(name)
            .execute(&ctx.pool)
            .await
            .ok();
    }
}

pub fn mock_base(server: &MockServer) -> String {
    server.url("/index.php")
}

pub fn company_detail_html(name: &str, phone: &str, state: &str) -> String {
    format!(
        "<html><body><table>\n\
         <tr>\n<td>Nama Syarikat</td>\n<td>{name}</td>\n</tr>\n\
         <tr>\n<td>No. Telefon</td>\n<td>{phone}</td>\n</tr>\n\
         <tr>\n<td>Negeri</td>\n<td>{state}</td>\n</tr>\n\
         </table></body></html>"
    )
}

pub fn company_listing_html(detail_links: &[&str]) -> String {
    let mut links = String::new();
    for (i, link) in detail_links.iter().enumerate() {
        links.push_str(&format!(
            "<tr><td>{}</td><td><a onclick=\"openModal('{}', 'modal')\">view</a></td></tr>\n",
            i + 1,
            link
        ));
    }
    format!(
        "<html><body><table>\n\
         <tr><td>Bil</td><td>Tindakan</td></tr>\n\
         {links}\
         </table></body></html>"
    )
}

pub fn product_listing_html(records: &[(u32, &str, &str, &str)], total_pages: u32) -> String {
    let mut rows = String::new();
    for (bil, name, address, expiry) in records {
        rows.push_str(&format!(
            "<tr>\n\
             <td>{bil}</td>\n\
             <td>{name}<br>{address}</td>\n\
             <td>{expiry}</td>\n\
             </tr>\n"
        ));
    }
    let total_pages_line = format!("Total Record : 99999 From {total_pages}");

    format!(
        "<html><body>\n\
         {total_pages_line}\n\
         <table>\n\
         <tr>\n<td>Bil</td>\n<td>Name</td>\n<td>Expiry</td>\n</tr>\n\
         {rows}\
         </table>\n\
         </body></html>"
    )
}
