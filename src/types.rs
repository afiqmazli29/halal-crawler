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

/// A crawl target: a (category, ty) pair on the portal plus the
/// display names used in progress output.
pub struct SubStrategy {
    pub category_code: &'static str,
    pub category_name: &'static str,
    pub sub_code: &'static str,
    pub sub_name: &'static str,
}
