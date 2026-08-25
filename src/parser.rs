use scraper::{ElementRef, Html, Selector};

use crate::constants::STATES;
use crate::records::{Company, Product};

/// Build readable text from an element's children, joining `<br>`-separated
/// lines with ", " and collapsing runs of whitespace into single spaces.
///
/// This works around scraper's raw `text()`, which concatenates descendant
/// text nodes with no delimiter — so an address split across `<br>` tags (as
/// the live portal emits) would otherwise come out as one unreadable run
/// like `No12,JalanMerdeka50000Kuala Lumpur`.
///
/// Adjacent text nodes are concatenated exactly as they appear in the source,
/// so a name split across multiple text runs (common on the portal) keeps its
/// spacing instead of gaining stray spaces mid-word.
fn element_text(el: &ElementRef) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut buf = String::new();
    element_lines(el, &mut lines, &mut buf);
    flush_line(&mut lines, &mut buf);

    let mut out = String::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(&line);
    }
    out.trim().to_string()
}

/// Recursively collect an element's text, buffering each line's raw text nodes
/// (adjacency preserved). A `<br>` flushes the current line and starts a new
/// one; repeated `<br>`s collapse because a flushed empty buffer produces no line.
fn element_lines(el: &ElementRef, lines: &mut Vec<String>, buf: &mut String) {
    for child in el.children() {
        let node = child.value();
        if let Some(text) = node.as_text() {
            buf.push_str(text);
        } else if let Some(elem) = ElementRef::wrap(child) {
            if elem.value().name() == "br" {
                flush_line(lines, buf);
            } else {
                element_lines(&elem, lines, buf);
            }
        }
    }
}

/// Trim and collapse a buffered line's whitespace into single spaces, then push
/// it if non-empty. Clears the buffer so the next line starts fresh.
fn flush_line(lines: &mut Vec<String>, buf: &mut String) {
    let line = buf.split_whitespace().collect::<Vec<_>>().join(" ");
    buf.clear();
    if !line.is_empty() {
        lines.push(line);
    }
}

/// Extract the `comp_code` from an `onclick="openModal(...)"` attribute.
/// The portal emits: onclick="openModal('directory/slm_viewdetail.php?comp_code=COMP-20230804-130326&type=C', ...)"
/// Returns just the `COMP-20230804-130326` part, or empty string if not found.
fn extract_comp_code(onclick: &str) -> String {
    if let Some(start) = onclick.find("comp_code=") {
        let rest = &onclick[start + "comp_code=".len()..];
        let end = rest
            .find('&')
            .or_else(|| rest.find('\''))
            .unwrap_or(rest.len());
        return rest[..end].to_string();
    }
    String::new()
}

/// Parse a directory search results page — extracts company-name and
/// company-address spans into company records with postcode and state
/// derived from the address. Also extracts the `comp_code` from the row's
/// `onclick` handler so the company's modal detail page can be fetched later.
pub fn parse_table(html: &str) -> Vec<Company> {
    let document = Html::parse_document(html);

    let name_sel = Selector::parse("span.company-name").unwrap();
    let addr_sel = Selector::parse("span.company-address").unwrap();
    let row_sel = Selector::parse("tr.cursor-pointer").unwrap();

    // Preferred path: portal listing rows carry the modal onclick with the
    // company's comp_code. Parse per-row so name/address/comp_code stay paired.
    let rows: Vec<_> = document.select(&row_sel).collect();
    if !rows.is_empty() {
        let mut records = Vec::new();
        for row in &rows {
            let Some(name_el) = row.select(&name_sel).next() else {
                continue;
            };
            let name = element_text(&name_el);
            if name.is_empty() {
                continue;
            }
            let address = row
                .select(&addr_sel)
                .next()
                .map(|el| element_text(&el))
                .unwrap_or_default();
            let comp_code = row
                .value()
                .attr("onclick")
                .map(|oc| extract_comp_code(oc))
                .unwrap_or_default();

            records.push(Company {
                name,
                postcode: extract_postcode(&address),
                state: extract_state(&address),
                address,
                comp_code,
                ..Default::default()
            });
        }
        return records;
    }

    // Fallback: bare name/address spans (older fixture shape, no modals).
    let names: Vec<String> = document
        .select(&name_sel)
        .map(|el| element_text(&el))
        .collect();
    let addresses: Vec<String> = document
        .select(&addr_sel)
        .map(|el| element_text(&el))
        .collect();

    let len = names.len().max(addresses.len());
    let mut records = Vec::with_capacity(len);

    for i in 0..len {
        let name = names.get(i).cloned().unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let address = addresses.get(i).cloned().unwrap_or_default();

        records.push(Company {
            name,
            postcode: extract_postcode(&address),
            state: extract_state(&address),
            address,
            ..Default::default()
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
        let name = element_text(&name_el);
        if name.is_empty() {
            continue;
        }

        let brand = row
            .select(&brand_sel)
            .next()
            .map(|el| element_text(&el))
            .map(|s| s.replace("JENAMA:", "").trim().to_string())
            .unwrap_or_default();
        let holder = row
            .select(&addr_sel)
            .next()
            .map(|el| element_text(&el))
            .unwrap_or_default();
        let expiry_date = row
            .select(&expiry_sel)
            .next()
            .map(|el| element_text(&el))
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

/// Parse a company's modal detail page (`/directory/slm_viewdetail.php`),
/// returning enriched company fields and the product list under
/// "Product / Menu List".
pub fn parse_modal(html: &str) -> (Company, Vec<Product>) {
    let document = Html::parse_document(html);

    let tr_sel = Selector::parse("tr").unwrap();
    let txt_sel = Selector::parse("td.txt").unwrap();
    let center_td_sel = Selector::parse("td[align=center]").unwrap();

    let mut company = Company::default();
    let mut in_product_section = false;
    let mut products = Vec::new();

    for row in document.select(&tr_sel) {
        let tds: Vec<ElementRef> = row.select(&Selector::parse("td").unwrap()).collect();

        // Company detail rows: <tr><td><b><div align="right">Label :</div></b></td><td>value</td></tr>
        if tds.len() >= 2 {
            let label = tds[0]
                .select(&Selector::parse("div[align=right]").unwrap())
                .next()
                .map(|el| el.text().collect::<String>().trim().to_lowercase())
                .unwrap_or_default();

            if !label.is_empty() {
                let value = element_text(&tds[1]);
                match label.as_str() {
                    s if s.contains("name :") => company.name = value,
                    s if s.contains("address :") => {
                        company.address = element_text(&tds[1]);
                        company.postcode = extract_postcode(&company.address);
                        company.state = extract_state(&company.address);
                    }
                    s if s.contains("state :") => {
                        if !value.is_empty() && company.state.is_empty() {
                            company.state = value;
                        }
                    }
                    s if s.contains("phone no :") => company.phone_no = value,
                    s if s.contains("fax no") => company.fax_no = value,
                    s if s.contains("e-mail") => company.email = value,
                    s if s.contains("website") => company.website = value,
                    s if s.contains("reference no") => company.reference_no = value,
                    s if s.contains("officer") => company.officer = element_text(&tds[1]),
                    _ => {}
                }
                continue;
            }
        }

        // Check for product section header
        if tds.len() >= 1 {
            let text = tds[0].text().collect::<String>().to_lowercase();
            if text.contains("product / menu list") {
                in_product_section = true;
                continue;
            }
        }

        // Product rows: <tr> with <td class="txt"> cells
        if in_product_section {
            let txt_cells: Vec<ElementRef> = row.select(&txt_sel).collect();
            if txt_cells.len() >= 2 {
                let product_name = element_text(&txt_cells[0]);
                let holder = element_text(&txt_cells[1]);

                // Expiry lives in a <td align="center"> that is NOT the row
                // number — it contains a dd/mm/yyyy pattern.
                let expiry = row
                    .select(&center_td_sel)
                    .find(|el| {
                        let t = el.text().collect::<String>();
                        t.matches('/').count() == 2 && !t.trim().is_empty()
                    })
                    .map(|el| element_text(&el))
                    .unwrap_or_default();

                if !product_name.is_empty() {
                    products.push(Product {
                        name: product_name,
                        brand: String::new(),
                        holder,
                        expiry_date: expiry,
                    });
                }
            }
        }
    }

    (company, products)
}
