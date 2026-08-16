use reqwest::Client;
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

use crate::constants::{DATA_PARAM, MAX_CONCURRENT};
use crate::types::Error;

pub const DEFAULT_BASE_URL: &str = "https://www.halal.gov.my";

/// The Halal Portal as a seam: owns the base URL, the PHP session,
/// the browser-shaped client, and every request the crawler makes.
/// Tests substitute a httpmock server by constructing a Portal with
/// the mock's base URL. Cheap to clone — pass copies into tasks.
#[derive(Clone)]
pub struct Portal {
    client: Client,
    semaphore: Arc<Semaphore>,
    base: String,
}

impl Portal {
    /// Build a portal rooted at `base` (use DEFAULT_BASE_URL in production).
    pub fn new(base: impl Into<String>) -> Result<Self, Error> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (X11; Linux x86_64; rv:153.0) Gecko/20100101 Firefox/153.0",
            ),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

        let client = Client::builder()
            .cookie_store(true)
            .default_headers(headers)
            .timeout(Duration::from_secs(90))
            .build()?;

        Ok(Self {
            client,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT)),
            base: base.into().trim_end_matches('/').to_string(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// Seed the PHP session by visiting the homepage.
    pub async fn init_session(&self) -> Result<(), Error> {
        self.client
            .get(format!("{}/index.php", self.base))
            .send()
            .await?;
        Ok(())
    }

    /// POST the directory search: a (category, ty) pair, letter filter,
    /// page number, and the hdnCounter returned by the previous response.
    pub async fn search(
        &self,
        category: &str,
        ty: &str,
        letter: char,
        page: u32,
        counter: &str,
    ) -> Result<String, Error> {
        let _permit = self.semaphore.acquire().await?;

        let url = format!(
            "{}/index.php?data={DATA_PARAM}&negeri=&category={category}&page={page}&cari={letter}",
            self.base
        );

        let resp = self
            .client
            .post(&url)
            .header(REFERER, format!("{}/index.php", self.base))
            .header(ORIGIN, HeaderValue::from_str(&self.base)?)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .form(&[("hdnCounter", counter), ("t", ""), ("a", ""), ("ty", ty)])
            .send()
            .await?;

        Ok(resp.text().await?)
    }

    /// Semaphore-guarded GET.
    pub async fn get(&self, url: &str) -> Result<String, Error> {
        let _permit = self.semaphore.acquire().await?;
        let resp = self.client.get(url).send().await?;
        Ok(resp.text().await?)
    }

    /// GET with retry and exponential backoff.
    pub async fn get_retry(&self, url: &str, max_retries: u32) -> Result<String, Error> {
        for attempt in 1..=max_retries {
            let _permit = self.semaphore.acquire().await?;
            match self.client.get(url).send().await {
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
}
