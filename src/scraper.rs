use crate::parser;
use crate::portal::Portal;
use crate::types::{Error, SubStrategy};

/// Scrape companies by POST-searching each letter a–z for the category.
/// All letters run concurrently; each letter paginates through all result pages.
pub async fn scrape_companies(
    portal: &Portal,
    strategy: &SubStrategy,
) -> Result<Vec<serde_json::Value>, Error> {
    let mut set = tokio::task::JoinSet::new();
    let code = strategy.category_code.to_string();
    let ty = strategy.sub_code;

    for letter in 'a'..='z' {
        let cat = code.clone();
        let portal = Portal::clone(portal);

        set.spawn(async move {
            let mut records = Vec::new();
            let mut page = 1u32;
            let mut counter = String::new();

            loop {
                let html = portal.search(&cat, ty, letter, page, &counter).await?;

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
    portal: &Portal,
    strategy: &SubStrategy,
) -> Result<Vec<serde_json::Value>, Error> {
    let base_url = portal.base();
    crate::paginate::scrape_sub(portal, base_url, strategy, 1).await
}
