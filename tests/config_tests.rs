use halal_crawler::config;
use halal_crawler::types::Phase;

#[test]
fn test_targets_cover_both_phases() {
    let targets = config::targets();
    assert!(targets.iter().any(|t| t.phase == Phase::Companies));
    assert!(targets.iter().any(|t| t.phase == Phase::Products));
}

#[test]
fn test_company_targets_all_have_co_ty() {
    for t in config::targets()
        .iter()
        .filter(|t| t.phase == Phase::Companies)
    {
        assert_eq!(t.ty, "CO", "{} has ty={}", t.category_code, t.ty);
    }
}

#[test]
fn test_product_targets_never_have_co_ty() {
    for t in config::targets()
        .iter()
        .filter(|t| t.phase == Phase::Products)
    {
        assert_ne!(t.ty, "CO", "{} should not be CO", t.category_code);
    }
}

#[test]
fn test_targets_have_codes() {
    for t in &config::targets() {
        assert!(!t.category_code.is_empty());
        assert!(!t.ty.is_empty());
    }
}

#[test]
fn test_product_targets_have_unique_ty_codes() {
    let targets = config::targets();
    let mut codes: Vec<&str> = targets
        .iter()
        .filter(|t| t.phase == Phase::Products)
        .map(|t| t.ty)
        .collect();
    let total = codes.len();
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), total);
}

#[test]
fn test_label_is_non_empty_for_every_target() {
    for t in &config::targets() {
        let label = config::label(t);
        assert!(!label.is_empty(), "empty label for {}", t.category_code);
    }
}

#[test]
fn test_company_label_contains_category_code() {
    for t in config::targets()
        .iter()
        .filter(|t| t.phase == Phase::Companies)
    {
        let label = config::label(t);
        assert!(
            label.contains(t.category_code),
            "label missing code: {label}"
        );
    }
}

#[test]
fn test_phase_as_str_matches_scrap_log_values() {
    assert_eq!(Phase::Companies.as_str(), "companies");
    assert_eq!(Phase::Products.as_str(), "products");
}
