use scraper::{Html, Selector};

use crate::constants::STATES;
use crate::records::{Company, Product};

/// Parse a directory search results page — extracts company-name and
/// company-address spans into company records with postcode and state
/// derived from the address.
pub fn parse_table(html: &str) -> Vec<Company> {
    let document = Html::parse_document(html);

    let name_sel = Selector::parse("span.company-name").unwrap();
    let addr_sel = Selector::parse("span.company-address").unwrap();

    let names: Vec<String> = document
        .select(&name_sel)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect();

    let addresses: Vec<String> = document
        .select(&addr_sel)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect();

    let len = names.len().max(addresses.len());
    let mut records = Vec::with_capacity(len);

    for i in 0..len {
        let name = names.get(i).cloned().unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let address = addresses.get(i).cloned().unwrap_or_default();
        let postcode = extract_postcode(&address);
        let state = extract_state(&address);

        records.push(Company {
            name,
            address,
            postcode,
            state,
        });
    }

    records
}

/// Parse a subcategory (product/premise) results page: each table row
/// holds a name span, an optional JENAMA brand span, the certificate
/// holder, and an expiry date cell.
pub fn parse_product_table(html: &str) -> Vec<Product> {
    let document = Html::parse_document(html);

    let row_sel = Selector::parse("tr").unwrap();
    let name_sel = Selector::parse("span.company-name").unwrap();
    let brand_sel = Selector::parse("span.company-brand").unwrap();
    let addr_sel = Selector::parse("span.company-address").unwrap();
    let expiry_sel = Selector::parse("td.text-center:not(.font-semibold)").unwrap();

    let mut records = Vec::new();

    for row in document.select(&row_sel) {
        let Some(name_el) = row.select(&name_sel).next() else {
            continue;
        };
        let name = name_el.text().collect::<String>().trim().to_string();
        if name.is_empty() {
            continue;
        }

        let brand = row
            .select(&brand_sel)
            .next()
            .map(|el| el.text().collect::<String>())
            .map(|s| s.replace("JENAMA:", "").trim().to_string())
            .unwrap_or_default();
        let holder = row
            .select(&addr_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let expiry_date = row
            .select(&expiry_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        records.push(Product {
            name,
            brand,
            holder,
            expiry_date,
        });
    }

    records
}

/// Extract the hdnCounter value from the response HTML for pagination.
pub fn extract_counter(html: &str) -> u32 {
    // The portal emits both value=21 and value="41", so accept both.
    for line in html.lines() {
        if let Some(pos) = line.find("hdnCounter") {
            let after = &line[pos..];
            let Some(v_pos) = after.find("value=") else {
                continue;
            };
            let v = &after[v_pos + "value=".len()..];
            let v = v.trim_start().trim_start_matches('"');
            let digits: String = v.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u32>() {
                return n;
            }
        }
    }
    0
}

/// Extract the total-page count from a "Total Record … From N" line.
pub fn extract_total_pages(md: &str) -> u32 {
    md.lines()
        .find_map(|line| {
            if line.contains("Total Record") {
                line.split("From")
                    .nth(1)
                    .map(|s| s.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
            } else {
                None
            }
        })
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

/// Extract exactly 5-digit postcode from address, or empty string.
fn extract_postcode(addr: &str) -> String {
    for part in addr.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let digits: String = part.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() == 5 && part.chars().filter(|c| c.is_ascii_digit()).count() == 5 {
            return digits;
        }
    }
    String::new()
}

/// Match state name from address, checking last parts first against STATES.
fn extract_state(addr: &str) -> String {
    let parts: Vec<&str> = addr
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let lower = addr.to_lowercase();

    // Try each state against the full address (handles multi-word states)
    for &state in STATES {
        if lower.contains(&state.to_lowercase()) {
            return state.to_string();
        }
    }

    // Fallback: try matching last 2 comma-separated parts
    if let Some(last) = parts.last() {
        for &state in STATES {
            if last.to_lowercase().contains(&state.to_lowercase()) {
                return state.to_string();
            }
        }
    }
    if parts.len() >= 2 {
        let second_last = parts[parts.len() - 2];
        for &state in STATES {
            if second_last.to_lowercase().contains(&state.to_lowercase()) {
                return state.to_string();
            }
        }
    }

    String::new()
}
