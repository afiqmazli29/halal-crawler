use halal_crawler::parser::{
    extract_first_arg, extract_onclick_urls, extract_table_data, parse_table, parse_table_array,
    parse_table_rows, resolve_url, slugify, strip_tags,
};
use serde_json::Value;

// ── extract_total_pages ────────────────────────────────────────

#[test]
fn test_extract_total_pages_standard_format() {
    let md = "Total Record : 12345 From 15";
    assert_eq!(halal_crawler::parser::extract_total_pages(md), 15);
}

#[test]
fn test_extract_total_pages_multiline() {
    let md = "Header\nTotal Record 100 From 1\nFooter";
    assert_eq!(halal_crawler::parser::extract_total_pages(md), 1);
}

#[test]
fn test_extract_total_pages_no_match() {
    assert_eq!(
        halal_crawler::parser::extract_total_pages("no pages here"),
        1
    );
}

#[test]
fn test_extract_total_pages_empty() {
    assert_eq!(halal_crawler::parser::extract_total_pages(""), 1);
}

#[test]
fn test_extract_total_pages_digits_only_after_from() {
    let md = "Total Record : 12345 From 25";
    assert_eq!(halal_crawler::parser::extract_total_pages(md), 25);
}

// ── parse_table ────────────────────────────────────────────────

#[test]
fn test_parse_table_basic() {
    let html = "<html><body>\n\
        <span class=\"company-name\">Syarikat ABC</span>\n\
        <span class=\"company-address\">123 Jalan, 50000 KL, Selangor</span>\n\
        <span class=\"company-name\">Syarikat XYZ</span>\n\
        <span class=\"company-address\">456 Jalan, 47000 Shah Alam, Selangor</span>\n\
        </body></html>";

    let records = parse_table(html, 0);
    assert_eq!(records.len(), 2);

    assert_eq!(records[0]["name"], "Syarikat ABC");
    assert_eq!(records[0]["address"], "123 Jalan, 50000 KL, Selangor");
    assert_eq!(records[0]["postcode"], "50000");
    assert_eq!(records[0]["state"], "Selangor");

    assert_eq!(records[1]["name"], "Syarikat XYZ");
    assert_eq!(records[1]["postcode"], "47000");
    assert_eq!(records[1]["state"], "Selangor");
}

#[test]
fn test_parse_table_empty() {
    let records = parse_table("<html>no spans</html>", 0);
    assert_eq!(records.len(), 0);
}

#[test]
fn test_parse_table_skips_empty_name() {
    let html = "<html><body>\n\
        <span class=\"company-name\"></span>\n\
        <span class=\"company-address\">Some addr</span>\n\
        <span class=\"company-name\">Valid Co</span>\n\
        <span class=\"company-address\">456 Jalan, 51000 KL, Kuala Lumpur</span>\n\
        </body></html>";

    let records = parse_table(html, 0);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["name"], "Valid Co");
}

#[test]
fn test_parse_table_missing_address() {
    let html = "<span class=\"company-name\">Only Name Co</span>";

    let records = parse_table(html, 0);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["name"], "Only Name Co");
    assert_eq!(records[0]["address"], "");
}

// ── strip_tags ────────────────────────────────────────────────

#[test]
fn test_strip_tags_simple() {
    assert_eq!(strip_tags("<td>hello</td>"), "hello");
}

#[test]
fn test_strip_tags_nested() {
    assert_eq!(strip_tags("<div><span>text</span></div>"), "text");
}

#[test]
fn test_strip_tags_no_tags() {
    assert_eq!(strip_tags("plain text"), "plain text");
}

#[test]
fn test_strip_tags_empty() {
    assert_eq!(strip_tags(""), "");
}

#[test]
fn test_strip_tags_self_closing() {
    assert_eq!(strip_tags("before<br/>after"), "beforeafter");
}

// ── slugify ──────────────────────────────────────────────────

#[test]
fn test_slugify_lowercases() {
    assert_eq!(slugify("Hello World"), "hello_world");
}

#[test]
fn test_slugify_replaces_special_chars() {
    assert_eq!(slugify("foo:bar/baz"), "foo_bar_baz");
}

#[test]
fn test_slugify_collapses_underscores() {
    assert_eq!(slugify("a::b"), "a_b");
}

#[test]
fn test_slugify_strips_leading_trailing_special() {
    assert_eq!(slugify(":hello:"), "hello");
}

#[test]
fn test_slugify_malay_text() {
    assert_eq!(slugify("No. Telefon"), "no_telefon");
    assert_eq!(slugify("Nama Syarikat"), "nama_syarikat");
}

// ── extract_first_arg ───────────────────────────────────────

#[test]
fn test_extract_first_arg_openmodal() {
    let line = r#"onclick="openModal('path/to/page?param=1&amp;other=2', 'target')""#;
    let result = extract_first_arg(line, "openModal(");
    assert_eq!(result, Some("path/to/page?param=1&other=2".to_string()));
}

#[test]
fn test_extract_first_arg_not_found() {
    let line = "no function here";
    assert_eq!(extract_first_arg(line, "openModal("), None);
}

#[test]
fn test_extract_first_arg_empty_fn_name() {
    let line = "'value'";
    assert_eq!(extract_first_arg(line, ""), Some("value".to_string()));
}

#[test]
fn test_extract_first_arg_no_quote_after_fn() {
    let line = "openModal(no quote)";
    assert_eq!(extract_first_arg(line, "openModal("), None);
}

#[test]
fn test_extract_first_arg_unclosed_quote() {
    let line = "openModal('unclosed";
    assert_eq!(extract_first_arg(line, "openModal("), None);
}

// ── resolve_url ─────────────────────────────────────────────

#[test]
fn test_resolve_url_absolute_http() {
    let path = "https://other.example.com/page";
    assert_eq!(resolve_url(path, "https://domain.com/v1"), path);
}

#[test]
fn test_resolve_url_absolute_root() {
    let path = "/portal-halal/v1/index.php";
    assert_eq!(
        resolve_url(path, "https://domain.com"),
        "https://www.halal.gov.my/portal-halal/v1/index.php"
    );
}

#[test]
fn test_resolve_url_query_only() {
    let path = "?data=xyz&category=BG&page=2";
    assert_eq!(
        resolve_url(path, "https://www.halal.gov.my"),
        "https://www.halal.gov.my/index.php?data=xyz&category=BG&page=2"
    );
}

#[test]
fn test_resolve_url_relative() {
    let path = "detail.php?id=1";
    assert_eq!(
        resolve_url(path, "https://www.halal.gov.my"),
        "https://www.halal.gov.my/detail.php?id=1"
    );
}

// ── extract_onclick_urls ─────────────────────────────────────

#[test]
fn test_extract_onclick_urls_openmodal() {
    let html = r#"<td><a onclick="openModal('dir/view_detail.php?ref_no=ABC&amp;cat=BG', 'modal')">view</a></td>"#;
    let urls = extract_onclick_urls(html, "");
    assert_eq!(urls.len(), 1);
    assert!(urls[0].contains("view_detail.php"), "got: {}", urls[0]);
    assert!(urls[0].contains("ref_no=ABC&cat=BG"), "got: {}", urls[0]);
}

#[test]
fn test_extract_onclick_urls_window_location() {
    let html = r##"<td onclick="window.location='detail.php?id=2'">click</td>"##;
    let urls = extract_onclick_urls(html, "");
    assert_eq!(urls.len(), 1);
    assert!(urls[0].contains("detail.php?id=2"), "got: {}", urls[0]);
}

#[test]
fn test_extract_onclick_urls_location_href() {
    let html = r##"<td onclick="location.href='detail.php?id=3'">click</td>"##;
    let urls = extract_onclick_urls(html, "");
    assert_eq!(urls.len(), 1);
    assert!(urls[0].contains("detail.php?id=3"), "got: {}", urls[0]);
}

#[test]
fn test_extract_onclick_urls_deduplicates() {
    let html = "\
        onclick=\"openModal('same/page', '')\"\n\
        onclick=\"openModal('same/page', '')\"\
    ";
    let urls = extract_onclick_urls(html, "");
    assert_eq!(urls.len(), 1);
}

#[test]
fn test_extract_onclick_urls_empty_html() {
    let urls = extract_onclick_urls("", "");
    assert_eq!(urls.len(), 0);
}

#[test]
fn test_extract_onclick_urls_multiple_do_detail_per_line() {
    let html = "\
        do_detail('a.php')\n\
        location.href='b.php'\
    ";
    let urls = extract_onclick_urls(html, "");
    assert_eq!(urls.len(), 2);
}

// ── parse_table_rows ────────────────────────────────────────

#[test]
fn test_parse_table_rows_simple() {
    let html = "<table>\n\
        <tr>\n\
        <td>Nama Syarikat</td>\n\
        <td>ABC Sdn Bhd</td>\n\
        </tr>\n\
        <tr>\n\
        <td>No. Telefon</td>\n\
        <td>03-12345678</td>\n\
        </tr>\n\
        <tr>\n\
        <td>Emel</td>\n\
        <td>abc@example.com</td>\n\
        </tr>\n\
        </table>";

    let data = parse_table_rows(html);
    assert_eq!(data.len(), 3);
    assert_eq!(data["nama_syarikat"], "ABC Sdn Bhd");
    assert_eq!(data["no_telefon"], "03-12345678");
    assert_eq!(data["emel"], "abc@example.com");
}

#[test]
fn test_parse_table_rows_th_cells() {
    let html = "<table>\n\
        <tr>\n\
        <th>Key</th>\n\
        <th>Value</th>\n\
        </tr>\n\
        <tr>\n\
        <td>Name</td>\n\
        <td>XYZ</td>\n\
        </tr>\n\
        </table>";

    let data = parse_table_rows(html);
    assert_eq!(data["name"], "XYZ");
}

#[test]
fn test_parse_table_rows_multi_value_cells() {
    let html = "<table>\n\
        <tr>\n\
        <td>Alamat</td>\n\
        <td>123 Jalan</td>\n\
        <td>Kuala Lumpur</td>\n\
        <td>50000</td>\n\
        </tr>\n\
        </table>";

    let data = parse_table_rows(html);
    assert_eq!(data["alamat"], "123 Jalan Kuala Lumpur 50000");
}

#[test]
fn test_parse_table_rows_skips_rows_with_no_key() {
    let html = "<table>\n\
        <tr>\n\
        </tr>\n\
        <tr>\n\
        <td>Name</td>\n\
        <td>ABC</td>\n\
        </tr>\n\
        </table>";

    let data = parse_table_rows(html);
    assert!(data.contains_key("name"));
    assert_eq!(data.len(), 1);
}

#[test]
fn test_parse_table_rows_strips_tags_in_cells() {
    let html = "<table>\n\
        <tr>\n\
        <td><b>Name</b></td>\n\
        <td><span>ABC</span></td>\n\
        </tr>\n\
        </table>";

    let data = parse_table_rows(html);
    assert_eq!(data["name"], "ABC");
}

#[test]
fn test_parse_table_rows_nested_table() {
    let html = "<table>\n\
        <tr>\n\
        <td>Info</td>\n\
        <td>\n\
        <table>\n\
        <tr>\n\
        <td>SubKey1</td>\n\
        <td>Val1</td>\n\
        </tr>\n\
        <tr>\n\
        <td>SubKey2</td>\n\
        <td>Val2</td>\n\
        </tr>\n\
        </table>\n\
        </td>\n\
        </tr>\n\
        </table>";

    let data = parse_table_rows(html);
    assert!(data.contains_key("info"));

    if let Value::Array(arr) = &data["info"] {
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["subkey1"], "Val1");
        assert_eq!(arr[1]["subkey2"], "Val2");
    } else {
        panic!("Expected array, got {:?}", data["info"]);
    }
}

#[test]
fn test_parse_table_rows_empty_table() {
    let data = parse_table_rows("<table>\n</table>");
    assert!(data.is_empty());
}

#[test]
fn test_parse_table_rows_no_table() {
    let data = parse_table_rows("no table here");
    assert!(data.is_empty());
}

// ── parse_table_array ────────────────────────────────────────

#[test]
fn test_parse_table_array_two_rows() {
    let html = "<table>\n\
        <tr>\n\
        <td>Bil</td>\n\
        <td>Name</td>\n\
        <td>Expiry</td>\n\
        </tr>\n\
        <tr>\n\
        <td>1</td>\n\
        <td>Item A</td>\n\
        <td>2026-12-31</td>\n\
        </tr>\n\
        </table>";

    let rows = parse_table_array(html);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["bil"], "Name Expiry");
    assert_eq!(rows[1]["1"], "Item A 2026-12-31");
}

#[test]
fn test_parse_table_array_single_cell_row() {
    let html = "<table>\n\
        <tr>\n\
        <td>OnlyOne</td>\n\
        </tr>\n\
        </table>";

    let rows = parse_table_array(html);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["value"], "OnlyOne");
}

#[test]
fn test_parse_table_array_no_rows() {
    let rows = parse_table_array("<table>\n</table>");
    assert_eq!(rows.len(), 0);
}

#[test]
fn test_parse_table_array_skips_rows_without_cells() {
    let html = "<table>\n\
        <tr>\n\
        </tr>\n\
        <tr>\n\
        <td>A</td>\n\
        <td>B</td>\n\
        </tr>\n\
        </table>";

    let rows = parse_table_array(html);
    assert_eq!(rows.len(), 1);
}

// ── extract_table_data ───────────────────────────────────────

#[test]
fn test_extract_table_data_delegates_to_parse_table_rows() {
    let html = "<table>\n\
        <tr>\n\
        <td>Key</td>\n\
        <td>Value</td>\n\
        </tr>\n\
        </table>";

    let data = extract_table_data(html);
    assert_eq!(data["key"], "Value");
}
