use reqwest::Client;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::types::Error;

pub async fn scrape_html(
    client: &Client,
    semaphore: &Semaphore,
    url: &str,
) -> Result<String, Error> {
    let _permit = semaphore.acquire().await?;
    let resp = client
        .get(url)
        .timeout(Duration::from_secs(90))
        .send()
        .await?;
    Ok(resp.text().await?)
}

pub async fn scrape_with_retry(
    client: &Client,
    semaphore: &Semaphore,
    url: &str,
    max_retries: u32,
) -> Result<String, Error> {
    for attempt in 1..=max_retries {
        let _permit = semaphore.acquire().await?;
        match client
            .get(url)
            .timeout(Duration::from_secs(90))
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_server_error() && attempt < max_retries {
                    sleep(Duration::from_secs(2u64.pow(attempt))).await;
                    continue;
                }
                return Ok(resp.text().await?);
            }
            Err(e) => {
                if (e.is_timeout() || e.is_connect()) && attempt < max_retries {
                    sleep(Duration::from_secs(2u64.pow(attempt))).await;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
    Err("all retries exhausted".into())
}
