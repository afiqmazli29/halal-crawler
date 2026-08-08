use super::*;

fn obj(pairs: &[(&str, &str)]) -> Value {
    let mut map = serde_json::Map::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), Value::String(v.to_string()));
    }
    Value::Object(map)
}

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
fn test_pick_str_malay_first_pattern() {
    let val = obj(&[("nama_syarikat", "Syarikat ABC"), ("name", "Company ABC")]);
    assert_eq!(pick_str(&val, &["nama_syarikat", "name"]), "Syarikat ABC");
}

#[test]
fn test_pick_str_english_fallback() {
    let val = obj(&[("state", "Selangor")]);
    let keys = &["negeri", "state"];
    assert_eq!(pick_str(&val, keys), "Selangor");
}

#[test]
fn test_pick_str_non_string_value_skipped() {
    let mut map = serde_json::Map::new();
    map.insert("num".to_string(), Value::Number(42.into()));
    map.insert("name".to_string(), Value::String("valid".to_string()));
    let val = Value::Object(map);
    assert_eq!(pick_str(&val, &["num", "name"]), "valid");
}
