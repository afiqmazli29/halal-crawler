use std::time::Instant;

use halal_crawler::{config, db, listing, portal, types};
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

    // Seed PHP session
    println!("  getting session...");
    portal.init_session().await?;

    let company_strategies = config::company_strategies();
    let mut total_companies = 0usize;

    // ── Phase 1: Companies — letter-by-letter search ──────────────────
    println!("\n═══ PHASE 1: COMPANIES (a–z search) ═══");
    for (idx, s) in company_strategies.iter().enumerate() {
        println!(
            "\n┌─ [{}/{}] {} ({})",
            idx + 1,
            company_strategies.len(),
            s.category_name,
            s.category_code
        );

        match listing::fetch_companies(&portal, s).await {
            Ok(records) => {
                let n = db::insert_companies(&pool, &records, s.category_code).await?;
                total_companies += n;
                println!("└─ {n} companies → DB");
            }
            Err(e) => eprintln!("└─ ✗ {e}"),
        }
    }

    // ── Summary ───────────────────────────────────────────────────────
    let elapsed = started.elapsed();
    println!("\n╔══════════════════════════════════════════╗");
    println!("║  DONE: {} companies          ║", total_companies);
    println!(
        "║  in {:.1}s                          ║",
        elapsed.as_secs_f32()
    );
    println!("╚══════════════════════════════════════════╝");

    println!("\n── Sample companies ──");
    let rows = db::sample_companies(&pool).await?;
    for (name, phone) in &rows {
        println!("  {name} — {}", phone.as_str());
    }

    Ok(())
}
