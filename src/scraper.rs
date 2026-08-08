use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::http;
use crate::parser;
use crate::types::{Error, SubStrategy};

/// Scrape companies by POST-searching each letter a–z for the category.
pub async fn scrape_companies(
    client: &Arc<Client>,
    semaphore: &Arc<Semaphore>,
    strategy: &SubStrategy,
) -> Result<Vec<serde_json::Value>, Error> {
    let mut all = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for letter in 'a'..='z' {
        let html =
            http::search_directory(client, semaphore, strategy.category_code, letter).await?;

        let records = parser::parse_table(&html, 0);
        if !records.is_empty() {
            let new_count = records.len();
            for r in records {
                let name = crate::types::pick_str(&r, &["name", "nama", "company_name"]);
                if name.is_empty() || !seen.insert(name) {
                    continue;
                }
                all.push(r);
            }
            println!(
                "│    [{letter}] {new_count} rows  (total unique: {})",
                all.len()
            );
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
