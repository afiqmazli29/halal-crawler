use reqwest::Client;
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT,
};
use std::time::Duration;
use tokio::sync::Semaphore;

use crate::types::Error;

/// Build a browser-like client with cookie store. Call once.
pub fn build_client() -> Result<Client, Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (X11; Linux x86_64; rv:153.0) Gecko/20100101 Firefox/153.0",
        ),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

    let client = Client::builder()
        .cookie_store(true)
        .default_headers(headers)
        .timeout(Duration::from_secs(90))
        .build()?;
    Ok(client)
}

/// Seed a PHP session by visiting the homepage.
pub async fn init_session(client: &Client) -> Result<(), Error> {
    client
        .get("https://www.halal.gov.my/index.php")
        .send()
        .await?;
    Ok(())
}

/// GET with browser headers.
pub async fn scrape_get(
    client: &Client,
    semaphore: &Semaphore,
    url: &str,
) -> Result<String, Error> {
    let _permit = semaphore.acquire().await?;
    let resp = client.get(url).send().await?;
    Ok(resp.text().await?)
}

/// GET with retry and exponential backoff.
pub async fn scrape_get_retry(
    client: &Client,
    semaphore: &Semaphore,
    url: &str,
    max_retries: u32,
) -> Result<String, Error> {
    for attempt in 1..=max_retries {
        let _permit = semaphore.acquire().await?;
        match client.get(url).send().await {
            Ok(resp) => {
                if resp.status().is_server_error() && attempt < max_retries {
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                    continue;
                }
                return Ok(resp.text().await?);
            }
            Err(e) => {
                if (e.is_timeout() || e.is_connect()) && attempt < max_retries {
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
    Err("all retries exhausted".into())
}

/// POST search with pagination support.
/// `counter` is the hdnCounter value from the previous page (0 for first page).
pub async fn search_directory(
    client: &Client,
    semaphore: &Semaphore,
    category: &str,
    letter: char,
    page: u32,
    counter: &str,
) -> Result<String, Error> {
    let _permit = semaphore.acquire().await?;

    let url = format!(
        "https://www.halal.gov.my/index.php?data=ZGlyZWN0b3J5L2luZGV4X2RpcmVjdG9yeTs7Ozs=&negeri=&category={category}&page={page}&cari={letter}"
    );

    let resp = client
        .post(&url)
        .header(REFERER, "https://www.halal.gov.my/index.php")
        .header(ORIGIN, "https://www.halal.gov.my")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[("hdnCounter", counter), ("t", ""), ("a", ""), ("ty", "CO")])
        .send()
        .await?;

    Ok(resp.text().await?)
}
