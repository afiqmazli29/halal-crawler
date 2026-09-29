use crate::types::{CrawlTarget, Phase};

/// Every crawl target, in run order: phase `Companies` first, then phase
/// `Products`. This single list is the one place "what runs" is readable.
pub fn targets() -> Vec<CrawlTarget> {
    vec![
        // ── Phase Companies: each category's `ty=CO` listing ──────────
        CrawlTarget {
            category_code: "BG",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "FM",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "KO",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "MD",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "OEM",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "PE",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "PL",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "PR",
            ty: "CO",
            phase: Phase::Companies,
        },
        CrawlTarget {
            category_code: "PS",
            ty: "CO",
            phase: Phase::Companies,
        },
        // ── Phase Products: subcategory listings (products, premises, …) ─
        CrawlTarget {
            category_code: "BG",
            ty: "BG",
            phase: Phase::Products,
        },
        CrawlTarget {
            category_code: "FM",
            ty: "FM",
            phase: Phase::Products,
        },
        CrawlTarget {
            category_code: "KO",
            ty: "KO",
            phase: Phase::Products,
        },
        CrawlTarget {
            category_code: "MD",
            ty: "MD",
            phase: Phase::Products,
        },
        CrawlTarget {
            category_code: "OEM",
            ty: "OEM",
            phase: Phase::Products,
        },
        CrawlTarget {
            category_code: "PR",
            ty: "PR",
            phase: Phase::Products,
        },
        CrawlTarget {
            category_code: "PE",
            ty: "HO",
            phase: Phase::Products,
        },
        CrawlTarget {
            category_code: "PE",
            ty: "PE",
            phase: Phase::Products,
        },
        CrawlTarget {
            category_code: "PS",
            ty: "RS",
            phase: Phase::Products,
        },
    ]
}

/// The human-readable label for a target, used in progress output. Lives here
/// beside the codes so the code→name mapping stays in one module.
pub fn label(target: &CrawlTarget) -> String {
    let category = match target.category_code {
        "BG" => "Barang Gunaan",
        "FM" => "Farmaseutikal",
        "KO" => "Kosmetik & Dandanan",
        "MD" => "Peranti Perubatan",
        "OEM" => "OEM",
        "PE" => "Premis Makanan",
        "PL" => "Logistik",
        "PR" => "Produk Makanan/Minuman",
        "PS" => "Rumah Sembelihan",
        other => other,
    };

    match target.phase {
        Phase::Companies => format!("{category} ({})", target.category_code),
        Phase::Products => {
            let sub = match target.ty {
                "BG" => "Barang Gunaan",
                "FM" => "Farmaseutikal",
                "KO" => "Kosmetik",
                "MD" => "Peranti Perubatan",
                "OEM" => "OEM",
                "PR" => "Produk",
                "HO" => "Hotel & Resort",
                "PE" => "Premis Makanan",
                "RS" => "Rumah Sembelih",
                other => other,
            };
            format!("{category} — {sub}")
        }
    }
}
