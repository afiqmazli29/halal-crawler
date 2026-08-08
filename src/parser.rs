use scraper::{Html, Selector};
use serde_json::json;

pub fn parse_table(html: &str, page: u32) -> Vec<serde_json::Value> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("table tr").unwrap();
    let cell_selector = Selector::parse("td").unwrap();

    let mut records = Vec::new();

    for row in document.select(&row_selector) {
        let cells: Vec<String> = row
            .select(&cell_selector)
            .map(|td| td.inner_html())
            .collect();

        if cells.len() < 3 {
            continue;
        }

        let bil: i64 = cells[0].trim().parse().unwrap_or(0);
        if bil == 0 {
            continue;
        }

        let (name, address) = split_name_address(&cells[1]);
        let expiry = clean_html(&cells[2]);

        records.push(json!({
            "bil": bil,
            "name": name.trim().to_string(),
            "address": address.trim().to_string(),
            "expiry_date": expiry.trim().to_string(),
            "page": page,
        }));
    }

    records
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

fn split_name_address(cell: &str) -> (String, String) {
    let parts: Vec<&str> = cell
        .split("<br>")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return (String::new(), String::new());
    }
    (parts[0].to_string(), parts[1..].join(", "))
}

fn clean_html(s: &str) -> String {
    s.replace("<br>", ", ")
        .replace(['<', '>'], "")
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn extract_onclick_urls(html: &str, _base: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let domain = "https://myehalal.halal.gov.my/portal-halal/v1";

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

fn extract_first_arg(line: &str, fn_name: &str) -> Option<String> {
    let pos = line.find(fn_name)?;
    let after = &line[pos + fn_name.len()..];
    let start = after.find('\'')?;
    let end = after[start + 1..].find('\'')?;
    Some(after[start + 1..start + 1 + end].replace("&amp;", "&"))
}

fn resolve_url(path: &str, domain: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else if path.starts_with('/') {
        format!("https://myehalal.halal.gov.my{path}")
    } else if path.starts_with('?') {
        format!("{domain}/index.php{path}")
    } else {
        format!("{domain}/{path}")
    }
}

pub fn extract_table_data(html: &str) -> serde_json::Map<String, serde_json::Value> {
    parse_table_rows(html)
}

fn parse_table_rows(html: &str) -> serde_json::Map<String, serde_json::Value> {
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

fn parse_table_array(html: &str) -> Vec<serde_json::Map<String, serde_json::Value>> {
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

fn strip_tags(s: &str) -> String {
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

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
