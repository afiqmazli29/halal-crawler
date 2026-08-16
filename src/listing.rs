use std::collections::HashSet;
use tokio::task::JoinSet;

use crate::parser;
use crate::portal::Portal;
use crate::records::{Company, Product};
use crate::types::{Error, SubStrategy, error_chain};

/// The directory listing as one deep module: both crawl modes live here,
/// hiding the pagination protocol, concurrency, page caps, and dedup
/// behind two small functions. Callers hand over a strategy and get
/// records — they never learn about hdnCounter or Total Record lines.

/// Crawl a category's companies: each letter a–z is searched, pages
/// advance via the hdnCounter echoed back by the portal, and records
/// are deduped by name. A failing letter is logged and skipped — it
/// never aborts the rest of the category.
pub async fn fetch_companies(
    portal: &Portal,
    strategy: &SubStrategy,
) -> Result<Vec<Company>, Error> {
    let mut set = JoinSet::new();
    let category = strategy.category_code.to_string();
    let ty = strategy.sub_code;

    for letter in 'a'..='z' {
        let portal = Portal::clone(portal);
        let category = category.clone();
        set.spawn(async move {
            letter_crawl(portal, category, ty, letter)
                .await
                .map_err(|e| format!("[{letter}] {}", error_chain(&e)))
        });
    }

    let mut all = Vec::new();
    let mut seen = HashSet::new();

    while let Some(result) = set.join_next().await {
        match result {
            Ok(Ok((letter, records))) => {
                let letter_count = records.len();
                for r in records {
                    let name = r.name.clone();
                    if name.is_empty() || !seen.insert(name) {
                        continue;
                    }
                    all.push(r);
                }
                println!(
                    "│    [{letter}] total {letter_count} (unique so far: {})",
                    all.len()
                );
            }
            Ok(Err(e)) => eprintln!("│    ✗ {e}"),
            Err(join) => eprintln!("│    ✗ letter task failed: {join}"),
        }
    }

    Ok(all)
}

/// Crawl a subcategory listing (products, premises, …): each letter
/// announces its total page count on page 1, then the remaining pages
/// are fetched concurrently. No dedup — records may share names.
pub async fn fetch_subcategory(
    portal: &Portal,
    strategy: &SubStrategy,
) -> Result<Vec<Product>, Error> {
    let mut set = JoinSet::new();
    let category = strategy.category_code.to_string();
    let ty = strategy.sub_code;

    for letter in 'a'..='z' {
        let portal = Portal::clone(portal);
        let category = category.clone();
        set.spawn(async move {
            letter_sub_crawl(portal, category, ty, letter)
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

/// One letter of a company crawl: POST pages, advancing the hdnCounter
/// each time until the portal echoes 0 or the counter stops growing.
async fn letter_crawl(
    portal: Portal,
    category: String,
    ty: &'static str,
    letter: char,
) -> Result<(char, Vec<Company>), Error> {
    let mut records = Vec::new();
    let mut page = 1u32;
    let mut counter = String::new();

    loop {
        let html = portal.search(&category, ty, letter, page, &counter).await?;

        let page_records = parser::parse_table(&html);
        if page_records.is_empty() {
            break;
        }

        let new_on_page = page_records.len();
        records.extend(page_records);

        println!("│    [{letter}] page {page}: {new_on_page} records");

        let prev: u32 = counter.parse().unwrap_or(0);
        let next = parser::extract_counter(&html);
        if next == 0 || (prev > 0 && next >= prev) || page > 100 {
            break;
        }
        counter = next.to_string();
        page += 1;
    }

    Ok((letter, records))
}

/// One letter of a subcategory crawl: page 1 announces the total page
/// count, the rest are fetched concurrently (the portal ignores the
/// counter here and advances on the page parameter alone).
async fn letter_sub_crawl(
    portal: Portal,
    category: String,
    ty: &'static str,
    letter: char,
) -> Result<(char, Vec<Product>), Error> {
    let html = portal.search(&category, ty, letter, 1, "0").await?;
    let total_pages = parser::extract_total_pages(&html);
    let page1_count = parser::parse_product_table(&html).len();
    println!("│    [{letter}] page 1/{total_pages} ✓  {page1_count} records");

    let mut records = parser::parse_product_table(&html);
    if total_pages > 1 {
        let mut set = JoinSet::new();
        for page in 2..=total_pages {
            let portal = Portal::clone(&portal);
            let category = category.clone();
            set.spawn(async move {
                let html = portal.search(&category, ty, letter, page, "0").await?;
                let r = parser::parse_product_table(&html);
                let n = r.len();
                println!("│    [{letter}] page {page}/{total_pages} ✓  {n} records");
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
