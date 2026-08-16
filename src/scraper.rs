use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::http;
use crate::parser;
use crate::types::{Error, SubStrategy};

/// Scrape companies by POST-searching each letter a–z for the category.
/// All letters run concurrently; each letter paginates through all result pages.
pub async fn scrape_companies(
    client: &Arc<Client>,
    semaphore: &Arc<Semaphore>,
    strategy: &SubStrategy,
) -> Result<Vec<serde_json::Value>, Error> {
    let mut set = tokio::task::JoinSet::new();
    let code = strategy.category_code.to_string();

    for letter in 'a'..='z' {
        let c = Arc::clone(client);
        let s = Arc::clone(semaphore);
        let cat = code.clone();

        set.spawn(async move {
            let mut records = Vec::new();
            let mut page = 1u32;
            let mut counter = String::new();

            loop {
                let html = http::search_directory(&c, &s, &cat, letter, page, &counter).await?;

                let page_records = parser::parse_table(&html, 0);
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

            Ok::<_, Error>((letter, records))
        });
    }

    let mut all = Vec::new();
    let mut seen = std::collections::HashSet::new();

    while let Some(result) = set.join_next().await {
        match result? {
            Ok((letter, records)) => {
                let letter_count = records.len();
                for r in records {
                    let name = crate::types::pick_str(&r, &["name", "nama", "company_name"]);
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
            Err(e) => eprintln!("│    ✗ {e}"),
        }
    }

    Ok(all)
}

pub async fn scrape_products(
    client: &Arc<Client>,
    semaphore: &Arc<Semaphore>,
    base_url: &str,
    strategy: &SubStrategy,
) -> Result<Vec<serde_json::Value>, Error> {
    crate::paginate::scrape_sub(client, semaphore, base_url, strategy, 1).await
}
