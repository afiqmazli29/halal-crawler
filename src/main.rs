use std::sync::Arc;
use std::time::Instant;

use halal_crawler::{config, db, scraper, types};
use types::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let started = Instant::now();

    if std::path::Path::new(".env").exists() {
        dotenvy::dotenv().ok();
    }

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/halal".to_string());

    let client = Arc::new(reqwest::Client::new());
    let semaphore = config::semaphore();
    let pool = db::init(&db_url).await?;

    let base = "https://myehalal.halal.gov.my/portal-halal/v1/index.php";
    let company_strategies = config::company_strategies();
    let other_strategies = config::other_strategies();

    let mut total_companies = 0usize;
    let mut total_products = 0usize;

    // ── Phase 1: Companies (CO) — two-level scrape ───────────────────
    println!("\n═══ PHASE 1: COMPANIES (CO) ═══");
    for (idx, s) in company_strategies.iter().enumerate() {
        println!(
            "\n┌─ [{}/{}] {} ({})",
            idx + 1,
            company_strategies.len(),
            s.category_name,
            s.category_code
        );

        match scraper::scrape_companies(&client, &semaphore, base, s).await {
            Ok(records) => {
                let n = db::insert_companies(&pool, &records, s.category_code).await?;
                total_companies += n;
                println!("└─ {n} companies → DB");
            }
            Err(e) => eprintln!("└─ ✗ {e}"),
        }
    }

    // ── Phase 2: Products — paginated table scrape ───────────────────
    println!("\n═══ PHASE 2: PRODUCTS & OTHERS ═══");
    for (idx, s) in other_strategies.iter().enumerate() {
        println!(
            "\n┌─ [{}/{}] {} › {} ({})",
            idx + 1,
            other_strategies.len(),
            s.category_name,
            s.sub_name,
            s.sub_code
        );

        match scraper::scrape_products(&client, &semaphore, base, s).await {
            Ok(records) => {
                let n = db::insert_products(&pool, &records, s.category_code, s.sub_code).await?;
                total_products += n;
                println!("└─ {n} products → DB");
            }
            Err(e) => eprintln!("└─ ✗ {e}"),
        }
    }

    // ── Summary ───────────────────────────────────────────────────────
    let elapsed = started.elapsed();
    println!("\n╔══════════════════════════════════════════╗");
    println!(
        "║  DONE: {} companies + {} others  ║",
        total_companies, total_products
    );
    println!(
        "║  in {:.1}s                          ║",
        elapsed.as_secs_f32()
    );
    println!("╚══════════════════════════════════════════╝");

    println!("\n── Sample companies ──");
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT name, phone_no FROM companies ORDER BY RANDOM() LIMIT 3")
            .fetch_all(&pool)
            .await?;
    for (name, phone) in &rows {
        println!("  {name} — {}", phone.as_str());
    }

    println!("\n── Sample products ──");
    let rows: Vec<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT name, brand, expiry_date FROM products ORDER BY RANDOM() LIMIT 3")
            .fetch_all(&pool)
            .await?;
    for (name, brand, expiry) in &rows {
        println!(
            "  {name} — {} — {}",
            brand.as_deref().unwrap_or("?"),
            expiry.as_deref().unwrap_or("?")
        );
    }

    Ok(())
}
