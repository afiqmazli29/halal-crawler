use std::time::Instant;

use halal_crawler::{config, constants, db, listing, portal, types};
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

    let company_strategies = config::company_strategies();
    let mut total_companies = 0usize;
    let mut total_products = 0usize;

    // ── Phase 1: Companies — letter search discovers comp_codes, then each
    //    company's modal detail page is fetched for enriched fields and
    //    its product list ────────────────────────────────────────────────
    println!("\n═══ PHASE 1: COMPANIES (a–z search + detail modals) ═══");
    for (idx, s) in company_strategies.iter().enumerate() {
        println!(
            "\n┌─ [{}/{}] {} ({})",
            idx + 1,
            company_strategies.len(),
            s.category_name,
            s.category_code
        );

        let log_id = db::start_scrap(&pool, s.category_code, "companies").await?;
        match listing::fetch_companies(&portal, s, max_pages).await {
            Ok(records) => {
                // Upsert what the listing page gave us first so every
                // discovered company exists before modal enrichment.
                let (mut ins_total, mut upd_total) = db::insert_companies(&pool, &records).await?;
                println!("│  {ins_total} inserted, {upd_total} updated from listing");

                // Fetch each company's modal detail page (concurrently,
                // capped) for phone/fax/email/etc. and its product list.
                match listing::fetch_company_modals(&portal, &records, constants::MAX_CONCURRENT)
                    .await
                {
                    Ok(entries) => {
                        let mut companies = Vec::with_capacity(entries.len());
                        let mut all_products = Vec::new();
                        for (company, products) in entries {
                            all_products.extend(products);
                            companies.push(company);
                        }
                        let (ins2, upd2) = db::insert_companies(&pool, &companies).await?;
                        println!("│  {ins2} inserted, {upd2} updated after modal enrich");
                        ins_total += ins2;
                        upd_total += upd2;

                        if !all_products.is_empty() {
                            let (pins, pupd) = db::insert_products(
                                &pool,
                                &all_products,
                                s.category_code,
                                s.sub_code,
                            )
                            .await?;
                            println!("│  {pins} products inserted, {pupd} products updated");
                            total_products += pins + pupd;
                        }
                    }
                    Err(e) => eprintln!("│  ✗ modal pass failed: {}", types::error_chain(&e)),
                }

                total_companies += ins_total + upd_total;
                db::finish_scrap(&pool, log_id, ins_total, upd_total).await?;
                println!("└─ {ins_total} inserted, {upd_total} updated → DB");
            }
            Err(e) => {
                db::finish_scrap(&pool, log_id, 0, 0).await?;
                eprintln!("└─ ✗ {}", types::error_chain(&e));
            }
        }
    }

    // ── Phase 2: Subcategory listings (products, premises, …) ─────────
    let other_strategies = config::other_strategies();

    println!("\n═══ PHASE 2: SUBCATEGORY LISTINGS ═══");
    for (idx, s) in other_strategies.iter().enumerate() {
        println!(
            "\n┌─ [{}/{}] {} — {}",
            idx + 1,
            other_strategies.len(),
            s.category_name,
            s.sub_name
        );

        let log_id = db::start_scrap(&pool, s.category_code, "products").await?;
        match listing::fetch_subcategory(&portal, s, max_pages).await {
            Ok(records) => {
                let (inserted, updated) =
                    db::insert_products(&pool, &records, s.category_code, s.sub_code).await?;
                db::finish_scrap(&pool, log_id, inserted, updated).await?;
                total_products += inserted + updated;
                println!("└─ {inserted} inserted, {updated} updated → DB");
            }
            Err(e) => {
                db::finish_scrap(&pool, log_id, 0, 0).await?;
                eprintln!("└─ ✗ {}", types::error_chain(&e));
            }
        }
    }

    // ── Summary ───────────────────────────────────────────────────────
    let elapsed = started.elapsed();
    println!("\n╔══════════════════════════════════════════╗");
    println!("║  DONE: {} companies          ║", total_companies);
    println!("║        {} products           ║", total_products);
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
