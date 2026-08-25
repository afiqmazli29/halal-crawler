use std::collections::HashSet;
use tokio::task::JoinSet;

use crate::parser;
use crate::portal::Portal;
use crate::records::{Company, Product};
use crate::types::{Error, SubStrategy, error_chain};

/// The directory listing as one deep module: both crawl modes live here,
/// hiding the pagination protocol, concurrency, and dedup behind two
/// small functions. Callers hand over a strategy and get records — they
/// never learn about Total Record lines or the page parameter.

/// Crawl a category's companies: each letter a–z is searched, page 1
/// announces the total page count, and the remaining pages are fetched
/// concurrently. Records are deduped by name. A failing letter is
/// logged and skipped — it never aborts the rest of the category.
///
/// `max_pages` optionally caps how far a single letter paginates (debug runs);
/// `None` means the full crawl.
pub async fn fetch_companies(
    portal: &Portal,
    strategy: &SubStrategy,
    max_pages: Option<u32>,
) -> Result<Vec<Company>, Error> {
    let records = crawl(portal, strategy, max_pages, parser::parse_table).await?;

    let mut seen = HashSet::new();
    let deduped = records
        .into_iter()
        .filter(|r| !r.name.is_empty() && seen.insert(r.name.clone()))
        .collect();

    Ok(deduped)
}

/// Crawl a subcategory listing (products, premises, …) — same crawl,
/// product rows, no dedup (records may share names).
///
/// `max_pages` optionally caps how far a single letter paginates (debug runs);
/// `None` means the full crawl.
pub async fn fetch_subcategory(
    portal: &Portal,
    strategy: &SubStrategy,
    max_pages: Option<u32>,
) -> Result<Vec<Product>, Error> {
    crawl(portal, strategy, max_pages, parser::parse_product_table).await
}

/// The shared crawl: one task per letter, each letter paginating from
/// page 1 to the total announced by the portal on page 1.
async fn crawl<T>(
    portal: &Portal,
    strategy: &SubStrategy,
    max_pages: Option<u32>,
    parse: fn(&str) -> Vec<T>,
) -> Result<Vec<T>, Error>
where
    T: Send + 'static,
{
    let mut set = JoinSet::new();
    let category = strategy.category_code.to_string();
    let ty = strategy.sub_code;

    for letter in 'a'..='z' {
        let portal = Portal::clone(portal);
        let category = category.clone();
        set.spawn(async move {
            letter_crawl(portal, category, ty, letter, max_pages, parse)
                .await
                .map_err(|e| format!("[{letter}] {}", error_chain(&e)))
        });
    }

    let mut all = Vec::new();
    while let Some(result) = set.join_next().await {
        match result {
            Ok(Ok((letter, records))) => {
                let letter_count = records.len();
                all.extend(records);
                println!(
                    "│    [{letter}] total {letter_count} (overall so far: {})",
                    all.len()
                );
            }
            Ok(Err(e)) => eprintln!("│    ✗ {e}"),
            Err(join) => eprintln!("│    ✗ letter task failed: {join}"),
        }
    }

    Ok(all)
}

/// One letter: page 1 announces the total page count, the rest are
/// fetched concurrently. The portal ignores hdnCounter here — the page
/// parameter alone advances the listing.
async fn letter_crawl<T>(
    portal: Portal,
    category: String,
    ty: &'static str,
    letter: char,
    max_pages: Option<u32>,
    parse: fn(&str) -> Vec<T>,
) -> Result<(char, Vec<T>), Error>
where
    T: Send + 'static,
{
    let html = portal.search(&category, ty, letter, 1, "0").await?;
    let total_pages = parser::extract_total_pages(&html);
    // A page cap (set in debug runs, or via env) stops a letter from crawling
    // the portal's thousands of pages.
    let fetch_up_to = match max_pages {
        Some(m) if m < total_pages => m,
        _ => total_pages,
    };
    let capped = fetch_up_to < total_pages;
    let page1_count = parse(&html).len();
    println!(
        "│    [{letter}] page 1/{fetch_up_to} ✓  {page1_count} records{}",
        if capped {
            format!(" (debug: capped from {total_pages})")
        } else {
            String::new()
        }
    );

    let mut records = parse(&html);
    if fetch_up_to > 1 {
        let mut set = JoinSet::new();
        for page in 2..=fetch_up_to {
            let portal = Portal::clone(&portal);
            let category = category.clone();
            set.spawn(async move {
                let html = portal.search(&category, ty, letter, page, "0").await?;
                let r = parse(&html);
                let n = r.len();
                println!("│    [{letter}] page {page}/{fetch_up_to} ✓  {n} records");
                Ok::<_, Error>(r)
            });
        }
        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok(mut r)) => records.append(&mut r),
                Ok(Err(e)) => eprintln!("│    [{letter}] ✗ {}", error_chain(&e)),
                Err(join) => eprintln!("│    [{letter}] ✗ page task failed: {join}"),
            }
        }
    }

    Ok((letter, records))
}
