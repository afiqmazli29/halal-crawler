use serde_json::Value;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct SubStrategy {
    pub category_code: &'static str,
    pub category_name: &'static str,
    pub sub_code: &'static str,
    pub sub_name: &'static str,
}

pub fn pick_str(val: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(s) = val[k].as_str() {
            let s = s.trim();
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    String::new()
}
