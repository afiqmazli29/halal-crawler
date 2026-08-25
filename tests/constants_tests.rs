use halal_crawler::constants::{DEBUG_MAX_PAGES_PER_LETTER, max_pages_per_letter};

#[test]
fn max_pages_per_letter_resolves() {
    // All assertions live in one test (with env restored at the end) so the
    // shared process env is never left pointing at a debug value.
    unsafe { std::env::remove_var("HALAL_MAX_PAGES") };

    // Release build, no env → full crawl.
    assert_eq!(max_pages_per_letter(false), None);

    // Debug build, no env → capped at the debug constant (1 page/letter).
    assert_eq!(max_pages_per_letter(true), Some(DEBUG_MAX_PAGES_PER_LETTER));

    // HALAL_MAX_PAGES overrides the debug default.
    unsafe { std::env::set_var("HALAL_MAX_PAGES", "3") };
    assert_eq!(max_pages_per_letter(true), Some(3));
    assert_eq!(max_pages_per_letter(false), Some(3));

    // 0 = unlimited, even in a debug build.
    unsafe { std::env::set_var("HALAL_MAX_PAGES", "0") };
    assert_eq!(max_pages_per_letter(true), None);

    // Restore env.
    unsafe { std::env::remove_var("HALAL_MAX_PAGES") };
}
