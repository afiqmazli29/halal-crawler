use reqwest::Client;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

use crate::http;
use crate::paginate;
use crate::parser::{extract_onclick_urls, extract_table_data};
use crate::types::{Error, SubStrategy};

pub async fn scrape_companies(
    client: &Arc<Client>,
    semaphore: &Arc<Semaphore>,
    base_url: &str,
    strategy: &SubStrategy,
) -> Result<Vec<serde_json::Value>, Error> {
    let listing_url = format!(
        "{base_url}?data={}&category={}",
        strategy.data_param, strategy.category_code
    );

    println!("│    fetching listing page...");
    let html = http::scrape_html(client, semaphore, &listing_url).await?;

    let detail_urls: Vec<String> = extract_onclick_urls(&html, base_url);
    println!("│    found {} detail links", detail_urls.len());

    if detail_urls.is_empty() {
        println!("│    (no onclick links — falling back to table scrape)");
        return paginate::scrape_sub(client, semaphore, base_url, strategy, 1).await;
    }

    let mut records = Vec::new();
    let mut set = tokio::task::JoinSet::new();

    for (i, url) in detail_urls.iter().enumerate() {
        let c = Arc::clone(client);
        let s = Arc::clone(semaphore);
        let u = url.clone();
        let idx = i + 1;
        let total = detail_urls.len();

        set.spawn(async move {
            let t = Instant::now();
            let html = http::scrape_html(&c, &s, &u).await?;
            let data = extract_table_data(&html);
            let n = data.len();
            println!(
                "│    [{idx}/{total}] ✓  {n} fields  {:.1}s",
                t.elapsed().as_secs_f32()
            );
            Ok::<_, Error>(data)
        });
    }

    while let Some(result) = set.join_next().await {
        match result? {
            Ok(data) => records.push(serde_json::Value::Object(data)),
            Err(e) => eprintln!("│    ✗ detail page: {e}"),
        }
    }

    Ok(records)
}

pub async fn scrape_products(
    client: &Arc<Client>,
    semaphore: &Arc<Semaphore>,
    base_url: &str,
    strategy: &SubStrategy,
) -> Result<Vec<serde_json::Value>, Error> {
    paginate::scrape_sub(client, semaphore, base_url, strategy, 1).await
}
