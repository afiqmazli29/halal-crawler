use scraper::{Html, Selector};
use serde_json::json;

use crate::constants::STATES;

/// Parse the search results page — extracts company-name and company-address spans.
pub fn parse_table(html: &str, _page: u32) -> Vec<serde_json::Value> {
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

        records.push(json!({
            "name": name,
            "address": address,
            "postcode": postcode,
            "state": state,
        }));
    }

    records
}

/// Extract the hdnCounter value from the response HTML for pagination.
pub fn extract_counter(html: &str) -> u32 {
    // Look for <input type="hidden" name="hdnCounter" value="41">
    for line in html.lines() {
        if let Some(pos) = line.find("hdnCounter") {
            if let Some(v_start) = line[pos..].find("value=\"") {
                let after = &line[pos + v_start + 7..];
                if let Some(v_end) = after.find('"') {
                    let val = &after[..v_end];
                    if let Ok(n) = val.parse::<u32>() {
                        return n;
                    }
                }
            }
        }
    }
    0
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

pub fn extract_onclick_urls(html: &str, _base: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let domain = "https://www.halal.gov.my";

    for line in html.lines() {
        if let Some(url) = extract_first_arg(line, "openModal(") {
            let full = resolve_url(&url, domain);
            if !urls.contains(&full) {
                urls.push(full);
            }
            continue;
        }
        for prefix in [
            "do_detail(",
            "do_lihat(",
            "window.location='",
            "location.href='",
        ] {
            if let Some(pos) = line.find(prefix) {
                let after = &line[pos + prefix.len()..];
                if let Some(end) = after.find('\'') {
                    let path = &after[..end];
                    let full = resolve_url(path, domain);
                    if !urls.contains(&full) {
                        urls.push(full);
                    }
                }
            }
        }
    }
    urls
}

pub fn extract_first_arg(line: &str, fn_name: &str) -> Option<String> {
    let pos = line.find(fn_name)?;
    let after = &line[pos + fn_name.len()..];
    let start = after.find('\'')?;
    let end = after[start + 1..].find('\'')?;
    Some(after[start + 1..start + 1 + end].replace("&amp;", "&"))
}

pub fn resolve_url(path: &str, domain: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else if path.starts_with('/') {
        format!("https://www.halal.gov.my{path}")
    } else if path.starts_with('?') {
        format!("{domain}/index.php{path}")
    } else {
        format!("{domain}/{path}")
    }
}

pub fn extract_table_data(html: &str) -> serde_json::Map<String, serde_json::Value> {
    parse_table_rows(html)
}

pub fn parse_table_rows(html: &str) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::{Map, Value};

    let mut data = Map::new();
    let mut in_table = false;
    let mut depth = 0u32;
    let mut in_row = false;
    let mut key = String::new();
    let mut value_html = String::new();
    let mut col = 0u32;
    let table_start = html.find("<table").unwrap_or(0);

    for line in html[table_start..].lines() {
        let t = line.trim().to_lowercase();

        if t.contains("<table") {
            if in_table {
                value_html.push_str(line);
                value_html.push('\n');
            }
            depth += 1;
            in_table = true;
            continue;
        }
        if t.contains("</table") {
            depth -= 1;
            if depth == 0 {
                break;
            }
            if depth == 1 && !value_html.is_empty() {
                value_html.push_str(line);
                value_html.push('\n');
            }
            continue;
        }
        if !in_table {
            continue;
        }

        if depth == 1 {
            if t.contains("<tr") {
                in_row = true;
                key.clear();
                value_html.clear();
                col = 0;
                continue;
            }
            if t.contains("</tr") {
                in_row = false;
                if !key.is_empty() {
                    let v = if value_html.contains("<table") {
                        Value::Array(
                            parse_table_array(&value_html)
                                .into_iter()
                                .map(|m| Value::Object(m))
                                .collect(),
                        )
                    } else {
                        Value::String(strip_tags(&value_html).trim().to_string())
                    };
                    data.insert(slugify(&key), v);
                }
                continue;
            }
            if in_row && (t.contains("<td") || t.contains("<th")) {
                let content = strip_tags(line).trim().to_string();
                if col == 0 {
                    key = content;
                } else {
                    if !value_html.is_empty() {
                        value_html.push(' ');
                    }
                    value_html.push_str(&content);
                }
                col += 1;
            }
        }

        if depth > 1 && !value_html.is_empty() {
            value_html.push_str(line);
            value_html.push('\n');
        }
    }

    data
}

pub fn parse_table_array(html: &str) -> Vec<serde_json::Map<String, serde_json::Value>> {
    use serde_json::{Map, Value};

    let mut rows: Vec<Map<String, Value>> = Vec::new();
    let mut in_row = false;
    let mut cells: Vec<String> = Vec::new();

    for line in html.lines() {
        let t = line.trim().to_lowercase();
        if t.contains("<tr") {
            in_row = true;
            cells.clear();
            continue;
        }
        if t.contains("</tr") {
            in_row = false;
            if cells.len() >= 2 {
                let mut map = Map::new();
                let k = slugify(&cells[0]);
                let v = cells[1..].join(" ");
                map.insert(k, Value::String(v.trim().to_string()));
                rows.push(map);
            } else if cells.len() == 1 {
                let mut map = Map::new();
                map.insert("value".into(), Value::String(cells[0].trim().to_string()));
                rows.push(map);
            }
            continue;
        }
        if in_row && (t.contains("<td") || t.contains("<th")) {
            cells.push(strip_tags(line));
        }
    }

    rows
}

pub fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }
    out
}

pub fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
