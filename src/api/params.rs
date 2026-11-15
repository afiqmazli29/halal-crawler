use serde::Deserialize;

pub const DEFAULT_PER_PAGE: u32 = 20;
pub const MAX_PER_PAGE: u32 = 100;

/// Validated pagination. Constructing it clamps rather than rejects, so a
/// client sending `per_page=10000` gets 100 instead of a 400.
#[derive(Debug, Clone, Copy)]
pub struct PageParams {
    pub page: u32,
    pub per_page: u32,
}

impl PageParams {
    pub fn new(page: u32, per_page: u32) -> Self {
        Self {
            page: page.max(1),
            per_page: per_page.clamp(1, MAX_PER_PAGE),
        }
    }

    pub fn limit(self) -> i64 {
        self.per_page as i64
    }

    pub fn offset(self) -> i64 {
        (self.page as i64 - 1) * self.limit()
    }
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    DEFAULT_PER_PAGE
}

fn default_limit() -> usize {
    20
}

/// `GET /api/v1/companies` query string.
#[derive(Debug, Deserialize)]
pub struct CompanyQuery {
    pub q: Option<String>,
    pub state: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

impl CompanyQuery {
    pub fn pages(&self) -> PageParams {
        PageParams::new(self.page, self.per_page)
    }
}

/// `GET /api/v1/products` query string. `company_id`, `category`, and
/// `subcategory` are independent filters.
#[derive(Debug, Deserialize)]
pub struct ProductQuery {
    pub q: Option<String>,
    pub company_id: Option<i32>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

impl ProductQuery {
    pub fn pages(&self) -> PageParams {
        PageParams::new(self.page, self.per_page)
    }
}

/// `GET /api/v1/scrape-runs` query string.
#[derive(Debug, Deserialize)]
pub struct RunQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Treat a blank query value as absent, so `?q=` doesn't filter on `''`.
pub fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{MAX_PER_PAGE, PageParams};

    #[test]
    fn page_and_per_page_are_clamped() {
        let p = PageParams::new(0, 10_000);
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, MAX_PER_PAGE);
    }

    #[test]
    fn offset_is_zero_on_first_page() {
        let p = PageParams::new(1, 20);
        assert_eq!(p.offset(), 0);
        assert_eq!(p.limit(), 20);
    }

    #[test]
    fn offset_advances_by_page() {
        let p = PageParams::new(3, 10);
        assert_eq!(p.offset(), 20);
    }
}
