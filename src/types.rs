pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A crawl target: a (category, ty) pair on the portal plus the
/// display names used in progress output.
pub struct SubStrategy {
    pub category_code: &'static str,
    pub category_name: &'static str,
    pub sub_code: &'static str,
    pub sub_name: &'static str,
}
