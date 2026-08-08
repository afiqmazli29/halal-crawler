use super::*;

#[test]
fn test_company_strategies_count() {
    let s = company_strategies();
    assert_eq!(s.len(), 9);
}

#[test]
fn test_company_strategies_all_have_co_subcode() {
    for s in &company_strategies() {
        assert_eq!(
            s.sub_code, "CO",
            "{} has sub_code={}",
            s.category_code, s.sub_code
        );
    }
}

#[test]
fn test_company_strategies_all_have_data_param() {
    for s in &company_strategies() {
        assert!(!s.data_param.is_empty());
        assert!(!s.category_code.is_empty());
        assert!(!s.category_name.is_empty());
    }
}

#[test]
fn test_other_strategies_count() {
    let s = other_strategies();
    assert_eq!(s.len(), 9);
}

#[test]
fn test_other_strategies_no_co_subcode() {
    for s in &other_strategies() {
        assert_ne!(s.sub_code, "CO", "{} should not be CO", s.category_code);
    }
}

#[test]
fn test_other_strategies_have_unique_codes() {
    let s = other_strategies();
    let mut codes: Vec<&str> = s.iter().map(|x| x.sub_code).collect();
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), 9);
}

#[test]
fn test_semaphore_permits() {
    let s = semaphore();
    assert_eq!(s.available_permits(), MAX_CONCURRENT);
}

#[test]
fn test_max_concurrent_is_positive() {
    assert!(MAX_CONCURRENT > 0);
}
