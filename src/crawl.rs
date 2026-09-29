use sqlx::PgPool;

use crate::config;
use crate::listing;
use crate::portal::Portal;
use crate::types::{CrawlTarget, Error, Phase, error_chain};
use crate::{db, types};

/// What a completed [`run`] produced. `failures` records targets that errored;
/// a failed target never aborts the run, so incompleteness is explicit here
/// rather than only in the log output.
#[derive(Debug, Default)]
pub struct RunReport {
    pub companies_inserted: usize,
    pub companies_updated: usize,
    pub products_inserted: usize,
    pub products_updated: usize,
    pub failures: Vec<TargetFailure>,
}

/// One crawl target that failed, with the error chain for the log.
#[derive(Debug)]
pub struct TargetFailure {
    pub target: CrawlTarget,
    pub error: String,
}

/// Per-target counts. `companies` is meaningful for phase `Companies`,
/// `products` for phase `Products`; the other stays zero.
#[derive(Debug, Default)]
struct TargetOutcome {
    companies: (usize, usize),
    products: (usize, usize),
}

/// Run every crawl target: phase `Companies` first (letter search + modal
/// enrichment), then phase `Products` (subcategory listings). Each target's
/// `scrap_log` row is opened and closed here, whatever the outcome — the
/// lifecycle is owned by this module, not the caller.
///
/// `max_pages` caps how far a single letter paginates (`None` = full crawl).
/// A failing target is recorded in the report and skipped; the run continues.
pub async fn run(
    portal: &Portal,
    pool: &PgPool,
    targets: &[CrawlTarget],
    max_pages: Option<u32>,
) -> Result<RunReport, Error> {
    let mut report = RunReport::default();

    for phase in [Phase::Companies, Phase::Products] {
        let phase_targets: Vec<&CrawlTarget> =
            targets.iter().filter(|t| t.phase == phase).collect();
        if phase_targets.is_empty() {
            continue;
        }

        println!(
            "\n═══ {} ═══",
            match phase {
                Phase::Companies => "PHASE 1: COMPANIES (a–z search + detail modals)",
                Phase::Products => "PHASE 2: SUBCATEGORY LISTINGS",
            }
        );

        for (idx, target) in phase_targets.iter().enumerate() {
            println!(
                "\n┌─ [{}/{}] {}",
                idx + 1,
                phase_targets.len(),
                config::label(target)
            );

            match run_target(portal, pool, target, max_pages).await {
                Ok(outcome) => {
                    report.companies_inserted += outcome.companies.0;
                    report.companies_updated += outcome.companies.1;
                    report.products_inserted += outcome.products.0;
                    report.products_updated += outcome.products.1;
                    let (ins, upd) = match phase {
                        Phase::Companies => outcome.companies,
                        Phase::Products => outcome.products,
                    };
                    println!("└─ {ins} inserted, {upd} updated → DB");
                }
                Err(e) => {
                    let error = error_chain(&e);
                    eprintln!("└─ ✗ {error}");
                    report.failures.push(TargetFailure {
                        target: **target,
                        error,
                    });
                }
            }
        }
    }

    Ok(report)
}

/// Run one target, owning its `scrap_log` open/close on every branch.
async fn run_target(
    portal: &Portal,
    pool: &PgPool,
    target: &CrawlTarget,
    max_pages: Option<u32>,
) -> Result<TargetOutcome, Error> {
    let log_id = db::start_scrap(pool, target.category_code, target.phase.as_str()).await?;

    let outcome = match target.phase {
        Phase::Companies => run_companies(portal, pool, target, max_pages).await,
        Phase::Products => run_products(portal, pool, target, max_pages).await,
    };

    match outcome {
        Ok(outcome) => {
            let (ins, upd) = match target.phase {
                Phase::Companies => outcome.companies,
                Phase::Products => outcome.products,
            };
            db::finish_scrap(pool, log_id, ins, upd).await?;
            Ok(outcome)
        }
        Err(e) => {
            db::finish_scrap(pool, log_id, 0, 0).await?;
            Err(e)
        }
    }
}

/// Phase `Companies`: discover the listing, upsert it so every company exists,
/// then enrich each from its modal page and persist the enriched companies and
/// their products. A failed modal pass is logged, not fatal — the listing
/// records are already stored.
async fn run_companies(
    portal: &Portal,
    pool: &PgPool,
    target: &CrawlTarget,
    max_pages: Option<u32>,
) -> Result<TargetOutcome, Error> {
    let records = listing::fetch_companies(portal, target, max_pages).await?;

    let (mut ins_total, mut upd_total) = db::insert_companies(pool, &records).await?;
    println!("│  {ins_total} inserted, {upd_total} updated from listing");

    let mut products = (0, 0);
    match listing::fetch_company_modals(portal, target, &records).await {
        Ok(entries) => {
            let mut companies = Vec::with_capacity(entries.len());
            let mut all_products = Vec::new();
            for (company, ps) in entries {
                all_products.extend(ps);
                companies.push(company);
            }
            let (ins2, upd2) = db::insert_companies(pool, &companies).await?;
            println!("│  {ins2} inserted, {upd2} updated after modal enrich");
            ins_total += ins2;
            upd_total += upd2;

            if !all_products.is_empty() {
                let (pins, pupd) = db::insert_products(pool, &all_products).await?;
                println!("│  {pins} products inserted, {pupd} products updated");
                products = (pins, pupd);
            }
        }
        Err(e) => eprintln!("│  ✗ modal pass failed: {}", types::error_chain(&e)),
    }

    Ok(TargetOutcome {
        companies: (ins_total, upd_total),
        products,
    })
}

/// Phase `Products`: sweep a subcategory listing and upsert its records.
async fn run_products(
    portal: &Portal,
    pool: &PgPool,
    target: &CrawlTarget,
    max_pages: Option<u32>,
) -> Result<TargetOutcome, Error> {
    let records = listing::fetch_subcategory(portal, target, max_pages).await?;
    let (inserted, updated) = db::insert_products(pool, &records).await?;
    Ok(TargetOutcome {
        companies: (0, 0),
        products: (inserted, updated),
    })
}
