pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// The full error chain — reqwest wraps its real cause (TLS failure,
/// connection reset, timeout) and Display hides it. Log through this.
pub fn error_chain(e: &Error) -> String {
    let mut out = e.to_string();
    let mut source = e.source();
    while let Some(inner) = source {
        out.push_str(": ");
        out.push_str(&inner.to_string());
        source = inner.source();
    }
    out
}

/// The phase a crawl target belongs to. Phase `Companies` discovers companies
/// (and enriches them from their modal detail pages); phase `Products` sweeps
/// subcategory listings (products, premises, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Companies,
    Products,
}

impl Phase {
    /// The value persisted in `scrap_log.phase`.
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Companies => "companies",
            Phase::Products => "products",
        }
    }
}

/// A crawl target: a (category, ty) pair on the Portal plus the phase that
/// crawls it. Display names live in `config::label`, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrawlTarget {
    pub category_code: &'static str,
    pub ty: &'static str,
    pub phase: Phase,
}
