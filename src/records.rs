use serde_json::Value;

/// A company record. The `from_value` adapter owns the portal's
/// Malay+English key variants so callers never learn them.
/// `comp_code` is scraped from the directory listing (the `onclick` link);
/// the remaining fields are fetched from the company's modal detail page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Company {
    pub name: String,
    pub address: String,
    pub postcode: String,
    pub state: String,
    pub phone_no: String,
    pub fax_no: String,
    pub email: String,
    pub website: String,
    pub reference_no: String,
    pub officer: String,
    pub comp_code: String,
}

impl Company {
    /// Map a listing-page record (just name + address + comp_code).
    /// The remaining fields are filled in from the modal detail page.
    pub fn from_value(v: &Value) -> Self {
        Company {
            name: pick_str(v, &["nama_syarikat", "name", "company_name", "nama"]),
            address: pick_str(v, &["alamat", "address"]),
            postcode: pick_str(v, &["postcode", "poskod"]),
            state: pick_str(v, &["negeri", "state"]),
            comp_code: pick_str(v, &["comp_code"]),
            ..Default::default()
        }
    }
}

/// A subcategory (product/premise) record: name, brand, the
/// certificate holder (company name), and the halal expiry date.
/// The `holder` is resolved to a `company_id` via the companies table
/// at insert time; the category/subcategory that the product was seen in
/// are tracked via the `product_categories` mapping table, not here.
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
