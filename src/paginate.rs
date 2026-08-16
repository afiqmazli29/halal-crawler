use std::sync::Arc;
use std::time::Instant;

use crate::parser;
use crate::portal::Portal;
use crate::types::{Error, SubStrategy};

pub(crate) async fn scrape_sub(
    portal: &Portal,
    base_url: &str,
    strategy: &SubStrategy,
    max_pages: u32,
) -> Result<Vec<serde_json::Value>, Error> {
    let param_candidates: &[&str] = if strategy.sub_code.is_empty() {
        &[""]
    } else {
        &["subcategory", "menu", "sub", "type"]
    };

    for pname in param_candidates {
        let sp = if pname.is_empty() {
            String::new()
        } else {
            format!("&{pname}={}", strategy.sub_code)
        };
        let url1 = format!(
            "{base_url}/index.php?data={}&category={}{sp}",
            crate::constants::DATA_PARAM,
            strategy.category_code
        );

        let t0 = Instant::now();
        let html = portal.get_retry(&url1, 3).await?;

        if html.contains("Total Record") {
            let total_pages = parser::extract_total_pages(&html).min(max_pages);
            let page1_count = parser::parse_table(&html, 1).len();
            println!(
                "│    page 1/{total_pages} ✓  {page1_count} records  {:.1}s",
                t0.elapsed().as_secs_f32()
            );

            let mut records = parser::parse_table(&html, 1);
            if total_pages > 1 {
                records
                    .extend(fetch_remaining(portal, base_url, strategy, &sp, total_pages).await?);
            }
            return Ok(records);
        }
    }
    println!("│    (no records)");
    Ok(vec![])
}

async fn fetch_remaining(
    portal: &Portal,
    base_url: &str,
    strategy: &SubStrategy,
    sub_param: &str,
    total_pages: u32,
) -> Result<Vec<serde_json::Value>, Error> {
    use std::sync::atomic::{AtomicU32, Ordering};

    let done = Arc::new(AtomicU32::new(1));
    let failed = Arc::new(AtomicU32::new(0));
    let mut set = tokio::task::JoinSet::new();
    let mut records = Vec::new();

    for page in 2..=total_pages {
        let url = format!(
            "{base_url}/index.php?data={}&category={}{sub_param}&page={page}",
            crate::constants::DATA_PARAM,
            strategy.category_code
        );
        let d = Arc::clone(&done);
        let portal = Portal::clone(portal);

        set.spawn(async move {
            let t = Instant::now();
            let html = portal.get_retry(&url, 3).await?;
            let r = parser::parse_table(&html, page);
            let n = r.len();
            d.fetch_add(1, Ordering::Relaxed);
            println!(
                "│    page {page}/{total_pages} ✓  {n} records  {:.1}s",
                t.elapsed().as_secs_f32()
            );
            Ok::<_, Error>(r)
        });
    }
    while let Some(result) = set.join_next().await {
        match result? {
            Ok(mut r) => records.append(&mut r),
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                eprintln!("│    ✗ {e}");
            }
        }
    }
    Ok(records)
}
