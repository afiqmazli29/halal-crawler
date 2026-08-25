pub const MAX_CONCURRENT: usize = 5;

/// How far a single letter crawl paginates in debug mode (one page).
pub const DEBUG_MAX_PAGES_PER_LETTER: u32 = 1;

/// Max pages crawled per letter, if the crawl is capped.
///
/// The live portal announces thousands of pages for a single letter, which
/// makes an unchecked local run enormous. Debug builds cap each letter's crawl
/// so local runs stay fast:
///
/// - debug build (`cargo run`, no `--release`) → cap at
///   [`DEBUG_MAX_PAGES_PER_LETTER`] (1 page/letter)
/// - `HALAL_MAX_PAGES=N` → cap at `N` pages/letter (takes precedence, `0` = unlimited)
///
/// With neither, the full crawl runs (unlimited).
pub fn max_pages_per_letter(in_debug_build: bool) -> Option<u32> {
    if let Ok(v) = std::env::var("HALAL_MAX_PAGES") {
        if let Ok(n) = v.trim().parse::<u32>() {
            return if n == 0 { None } else { Some(n) };
        }
    }
    if in_debug_build {
        Some(DEBUG_MAX_PAGES_PER_LETTER)
    } else {
        None
    }
}

/// Base64-encoded directory path parameter used in portal requests.
pub const DATA_PARAM: &str = "ZGlyZWN0b3J5L2luZGV4X2RpcmVjdG9yeTs7Ozs=";

/// Malaysian states — used to match state from address strings.
pub const STATES: &[&str] = &[
    "Johor",
    "Kedah",
    "Kelantan",
    "Melaka",
    "Negeri Sembilan",
    "Pahang",
    "Pulau Pinang",
    "Perak",
    "Perlis",
    "Selangor",
    "Terengganu",
    "Sabah",
    "Sarawak",
    "Kuala Lumpur",
    "Labuan",
    "Putrajaya",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_pages_per_letter_resolves() {
        // All assertions live in one test (with env restored at the end) so
        // the shared process env is never left pointing at a debug value.
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
}
