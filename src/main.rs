use std::time::Instant;

use halal_crawler::{config, constants, crawl, db, portal, types};
use types::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let started = Instant::now();

    if std::path::Path::new(".env").exists() {
        dotenvy::dotenv().ok();
    }

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/halal".to_string());

    let portal = portal::Portal::new(portal::DEFAULT_BASE_URL)?;
    let pool = db::init(&db_url).await?;

    // Debug builds crawl at most DEBUG_MAX_PAGES_PER_LETTER pages per letter so
    // a local `cargo run` doesn't chew through the portal's thousands of pages.
    // Release builds run the full crawl unless HALAL_MAX_PAGES is set.
    let max_pages = constants::max_pages_per_letter(cfg!(debug_assertions));

    // Seed PHP session
    println!("  getting session...");
    portal.init_session().await?;

    let report = crawl::run(&portal, &pool, &config::targets(), max_pages).await?;

    // ── Summary ───────────────────────────────────────────────────────
    let elapsed = started.elapsed();
    let companies = report.companies_inserted + report.companies_updated;
    let products = report.products_inserted + report.products_updated;
    println!("\n╔══════════════════════════════════════════╗");
    println!("║  DONE: {companies} companies          ║");
    println!("║        {products} products           ║");
    println!(
        "║  in {:.1}s                          ║",
        elapsed.as_secs_f32()
    );
    println!("╚══════════════════════════════════════════╝");

    if !report.failures.is_empty() {
        println!("\n── {} target(s) failed ──", report.failures.len());
        for f in &report.failures {
            println!("  {}: {}", config::label(&f.target), f.error);
        }
    }

    println!("\n── Sample companies ──");
    let rows = db::sample_companies(&pool).await?;
    for (name, phone) in &rows {
        println!("  {name} — {}", phone.as_str());
    }

    Ok(())
}
