use halal_crawler::records::{Company, Product, pick_str};
use serde_json::Value;

fn obj(pairs: &[(&str, &str)]) -> Value {
    let mut map = serde_json::Map::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), Value::String(v.to_string()));
    }
    Value::Object(map)
}

// ── pick_str ───────────────────────────────────────────────────

#[test]
fn test_pick_str_first_key_wins() {
    let val = obj(&[("first", "hello"), ("second", "world")]);
    assert_eq!(pick_str(&val, &["first", "second"]), "hello");
}

#[test]
fn test_pick_str_fallback_to_later_key() {
    let val = obj(&[("second", "world")]);
    assert_eq!(pick_str(&val, &["first", "second"]), "world");
}

#[test]
fn test_pick_str_skips_empty_string() {
    let val = obj(&[("first", ""), ("second", "valid")]);
    assert_eq!(pick_str(&val, &["first", "second"]), "valid");
}

#[test]
fn test_pick_str_skips_whitespace_only() {
    let val = obj(&[("first", "   "), ("second", "valid")]);
    assert_eq!(pick_str(&val, &["first", "second"]), "valid");
}

#[test]
fn test_pick_str_no_key_matches() {
    let val = obj(&[("other", "value")]);
    assert_eq!(pick_str(&val, &["first", "second"]), "");
}

#[test]
fn test_pick_str_empty_keys() {
    let val = obj(&[("a", "b")]);
    assert_eq!(pick_str(&val, &[]), "");
}

#[test]
fn test_pick_str_empty_object() {
    let val = Value::Object(serde_json::Map::new());
    assert_eq!(pick_str(&val, &["any"]), "");
}

#[test]
fn test_pick_str_trims_whitespace() {
    let val = obj(&[("name", "  Syarikat ABC  ")]);
    assert_eq!(pick_str(&val, &["name"]), "Syarikat ABC");
}

#[test]
fn test_pick_str_non_string_value_skipped() {
    let mut map = serde_json::Map::new();
    map.insert("num".to_string(), Value::Number(42.into()));
    map.insert("name".to_string(), Value::String("valid".to_string()));
    let val = Value::Object(map);
    assert_eq!(pick_str(&val, &["num", "name"]), "valid");
}

// ── Company::from_value ────────────────────────────────────────

#[test]
fn test_company_from_value_malay_first() {
    let val = obj(&[("nama_syarikat", "Syarikat ABC"), ("name", "Company ABC")]);
    let company = Company::from_value(&val);
    assert_eq!(company.name, "Syarikat ABC");
}

#[test]
fn test_company_from_value_english_fallback() {
    let val = obj(&[
        ("name", "Company ABC"),
        ("address", "1 Jalan"),
        ("state", "Selangor"),
    ]);
    let company = Company::from_value(&val);
    assert_eq!(company.name, "Company ABC");
    assert_eq!(company.address, "1 Jalan");
    assert_eq!(company.state, "Selangor");
}

#[test]
fn test_company_from_value_missing_fields_empty() {
    let company = Company::from_value(&Value::Object(serde_json::Map::new()));
    assert_eq!(company, Company::default());
}

// ── Product::from_value ────────────────────────────────────────

#[test]
fn test_product_from_value_full() {
    let val = obj(&[
        ("name", "Biskut A"),
        ("jenama", "BrandA"),
        ("company", "Holder Co"),
        ("tarikh_tamat", "2028-01-01"),
    ]);
    let product = Product::from_value(&val);
    assert_eq!(product.name, "Biskut A");
    assert_eq!(product.brand, "BrandA");
    assert_eq!(product.holder, "Holder Co");
    assert_eq!(product.expiry_date, "2028-01-01");
}

#[test]
fn test_product_from_value_missing_fields_empty() {
    let product = Product::from_value(&Value::Object(serde_json::Map::new()));
    assert_eq!(product, Product::default());
}
