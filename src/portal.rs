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

/// Every portal request retries this many times before giving up.
const MAX_ATTEMPTS: u32 = 3;

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

    /// Seed the PHP session by visiting the homepage.
    pub async fn init_session(&self) -> Result<(), Error> {
        let url = format!("{}/index.php", self.base);
        self.retrying(MAX_ATTEMPTS, || self.client.get(&url))
            .await?;
        Ok(())
    }

    /// POST the directory search for a (category, ty) pair, letter filter,
    /// and page number. Pagination is driven by the page parameter alone —
    /// the portal ignores `hdnCounter`, so it is always sent as `"0"`.
    pub async fn search(
        &self,
        category: &str,
        ty: &str,
        letter: char,
        page: u32,
    ) -> Result<String, Error> {
        let url = format!(
            "{}/index.php?data={DATA_PARAM}&negeri=&category={category}&page={page}&cari={letter}",
            self.base
        );
        let referer = format!("{}/index.php", self.base);
        let origin = HeaderValue::from_str(&self.base)?;

        self.retrying(MAX_ATTEMPTS, || {
            self.client
                .post(&url)
                .header(REFERER, referer.clone())
                .header(ORIGIN, origin.clone())
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .form(&[("hdnCounter", "0"), ("t", ""), ("a", ""), ("ty", ty)])
        })
        .await
    }

    /// Fetch a company's modal detail page by `comp_code`. Owns the modal URL
    /// shape so callers never build portal URLs themselves.
    pub async fn fetch_modal(&self, comp_code: &str) -> Result<String, Error> {
        let url = format!(
            "{}/directory/slm_viewdetail.php?comp_code={}&type=C",
            self.base, comp_code
        );
        self.retrying(MAX_ATTEMPTS, || self.client.get(&url)).await
    }

    /// The one retry policy every request shares: retry transport failures and
    /// server errors with exponential backoff, then surface the last error.
    /// The portal drops connections under concurrent load, so retries are
    /// load-bearing.
    async fn retrying<F>(&self, max_attempts: u32, mut build: F) -> Result<String, Error>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        let mut last_err: Option<Error> = None;

        for attempt in 1..=max_attempts {
            let _permit = self.semaphore.acquire().await?;

            match build().send().await {
                Ok(resp) if resp.status().is_server_error() => {
                    last_err = Some(format!("server error: {}", resp.status()).into());
                }
                Ok(resp) => return Ok(resp.text().await?),
                Err(e) => last_err = Some(e.into()),
            }

            if attempt < max_attempts {
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt - 1))).await;
            }
        }

        Err(last_err.expect("loop runs at least once"))
    }
}
