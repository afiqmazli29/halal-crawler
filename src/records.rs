use serde_json::Value;

/// A company listing record. The from_value adapter owns the portal's
/// Malay+English key variants so callers never learn them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Company {
    pub name: String,
    pub address: String,
    pub postcode: String,
    pub state: String,
}

impl Company {
    /// Map a raw JSON record, trying Malay keys first, then English.
    pub fn from_value(v: &Value) -> Self {
        Company {
            name: pick_str(v, &["nama_syarikat", "name", "company_name", "nama"]),
            address: pick_str(v, &["alamat", "address"]),
            postcode: pick_str(v, &["postcode", "poskod"]),
            state: pick_str(v, &["negeri", "state"]),
        }
    }
}

/// A subcategory (product/premise) record: name, brand, the
/// certificate holder, and the halal expiry date.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Product {
    pub name: String,
    pub brand: String,
    pub holder: String,
    pub expiry_date: String,
}

impl Product {
    /// Map a raw JSON record, trying Malay keys first, then English.
    pub fn from_value(v: &Value) -> Self {
        Product {
            name: pick_str(v, &["name", "nama", "product_name"]),
            brand: pick_str(v, &["brand", "jenama"]),
            holder: pick_str(v, &["company", "holder", "company_name", "syarikat"]),
            expiry_date: pick_str(v, &["expiry_date", "tarikh_tamat", "tempoh_sah_laku"]),
        }
    }
}

/// Pick the first non-empty string among key candidates.
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
