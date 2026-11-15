use crate::types::{CrawlTarget, Phase};

/// The company crawl: each category's `ty=CO` listing. These targets write
/// only `companies`; products come from [`product_targets`].
pub fn company_targets() -> Vec<CrawlTarget> {
    vec![
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
    ]
}

/// The product crawl: subcategory listings (products, premises, …). These
/// targets are the only writers of `products` and `product_categories`.
pub fn product_targets() -> Vec<CrawlTarget> {
    vec![
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

/// Both jobs in run order: companies first (products resolve holders against
/// them), then products. Use [`company_targets`] or [`product_targets`] to run
/// one job on its own.
pub fn targets() -> Vec<CrawlTarget> {
    company_targets()
        .into_iter()
        .chain(product_targets())
        .collect()
}

/// The human-readable name for a category code. Unknown codes are returned
/// unchanged so the API never hides data it doesn't have a label for.
pub fn category_name(code: &str) -> &str {
    match code {
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
    }
}

/// The human-readable name for a subcategory (`ty`) code.
pub fn subcategory_name(code: &str) -> &str {
    match code {
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
    }
}

/// The human-readable label for a target, used in progress output. Lives here
/// beside the codes so the code→name mapping stays in one module.
pub fn label(target: &CrawlTarget) -> String {
    let category = category_name(target.category_code);

    match target.phase {
        Phase::Companies => format!("{category} ({})", target.category_code),
        Phase::Products => format!("{category} — {}", subcategory_name(target.ty)),
    }
}
