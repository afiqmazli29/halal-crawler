use crate::types::SubStrategy;

pub fn company_strategies() -> Vec<SubStrategy> {
    vec![
        SubStrategy {
            category_code: "BG",
            category_name: "Barang Gunaan",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
        SubStrategy {
            category_code: "FM",
            category_name: "Farmaseutikal",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
        SubStrategy {
            category_code: "KO",
            category_name: "Kosmetik & Dandanan",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
        SubStrategy {
            category_code: "MD",
            category_name: "Peranti Perubatan",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
        SubStrategy {
            category_code: "OEM",
            category_name: "OEM",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
        SubStrategy {
            category_code: "PE",
            category_name: "Premis Makanan",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
        SubStrategy {
            category_code: "PL",
            category_name: "Logistik",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
        SubStrategy {
            category_code: "PR",
            category_name: "Produk Makanan/Minuman",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
        SubStrategy {
            category_code: "PS",
            category_name: "Rumah Sembelihan",
            sub_code: "CO",
            sub_name: "Syarikat",
        },
    ]
}

pub fn other_strategies() -> Vec<SubStrategy> {
    vec![
        SubStrategy {
            category_code: "BG",
            category_name: "Barang Gunaan",
            sub_code: "BG",
            sub_name: "Barang Gunaan",
        },
        SubStrategy {
            category_code: "FM",
            category_name: "Farmaseutikal",
            sub_code: "FM",
            sub_name: "Farmaseutikal",
        },
        SubStrategy {
            category_code: "KO",
            category_name: "Kosmetik & Dandanan",
            sub_code: "KO",
            sub_name: "Kosmetik",
        },
        SubStrategy {
            category_code: "MD",
            category_name: "Peranti Perubatan",
            sub_code: "MD",
            sub_name: "Peranti Perubatan",
        },
        SubStrategy {
            category_code: "OEM",
            category_name: "OEM",
            sub_code: "OEM",
            sub_name: "OEM",
        },
        SubStrategy {
            category_code: "PR",
            category_name: "Produk Makanan/Minuman",
            sub_code: "PR",
            sub_name: "Produk",
        },
        SubStrategy {
            category_code: "PE",
            category_name: "Premis Makanan",
            sub_code: "HO",
            sub_name: "Hotel & Resort",
        },
        SubStrategy {
            category_code: "PE",
            category_name: "Premis Makanan",
            sub_code: "PE",
            sub_name: "Premis Makanan",
        },
        SubStrategy {
            category_code: "PS",
            category_name: "Rumah Sembelihan",
            sub_code: "RS",
            sub_name: "Rumah Sembelih",
        },
    ]
}
