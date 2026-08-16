use halal_crawler::config;

#[test]
fn test_company_strategies_count() {
    let s = config::company_strategies();
    assert_eq!(s.len(), 9);
}

#[test]
fn test_company_strategies_all_have_co_subcode() {
    for s in &config::company_strategies() {
        assert_eq!(
            s.sub_code, "CO",
            "{} has sub_code={}",
            s.category_code, s.sub_code
        );
    }
}

#[test]
fn test_company_strategies_have_codes_and_names() {
    for s in &config::company_strategies() {
        assert!(!s.category_code.is_empty());
        assert!(!s.category_name.is_empty());
    }
}

#[test]
fn test_other_strategies_count() {
    let s = config::other_strategies();
    assert_eq!(s.len(), 9);
}

#[test]
fn test_other_strategies_no_co_subcode() {
    for s in &config::other_strategies() {
        assert_ne!(s.sub_code, "CO", "{} should not be CO", s.category_code);
    }
}

#[test]
fn test_other_strategies_have_unique_codes() {
    let s = config::other_strategies();
    let mut codes: Vec<&str> = s.iter().map(|x| x.sub_code).collect();
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), 9);
}
