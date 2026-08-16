use halal_crawler::parser::{
    extract_counter, extract_total_pages, parse_product_table, parse_table,
};

// ── extract_total_pages ────────────────────────────────────────

#[test]
fn test_extract_total_pages_standard_format() {
    let md = "Total Record : 12345 From 15";
    assert_eq!(extract_total_pages(md), 15);
}

#[test]
fn test_extract_total_pages_portal_format() {
    // Live portal format: "Total Record : 955 - Page 1 From 48"
    let md = "Total Record : 955 - Page 1 From 48";
    assert_eq!(extract_total_pages(md), 48);
}

#[test]
fn test_extract_total_pages_multiline() {
    let md = "Header\nTotal Record 100 From 1\nFooter";
    assert_eq!(extract_total_pages(md), 1);
}

#[test]
fn test_extract_total_pages_no_match() {
    assert_eq!(extract_total_pages("no pages here"), 1);
}

#[test]
fn test_extract_total_pages_empty() {
    assert_eq!(extract_total_pages(""), 1);
}

// ── extract_counter ────────────────────────────────────────────

#[test]
fn test_extract_counter_unquoted() {
    // Live portal renders value=21 without quotes.
    let html = "<input type=\"hidden\" name=\"hdnCounter\" value=21>";
    assert_eq!(extract_counter(html), 21);
}

#[test]
fn test_extract_counter_quoted() {
    let html = "<input type=\"hidden\" name=\"hdnCounter\" value=\"41\">";
    assert_eq!(extract_counter(html), 41);
}

#[test]
fn test_extract_counter_zero() {
    let html = "<input type=\"hidden\" name=\"hdnCounter\" value=0>";
    assert_eq!(extract_counter(html), 0);
}

#[test]
fn test_extract_counter_missing() {
    assert_eq!(extract_counter("<html>no counter</html>"), 0);
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

    let records = parse_table(html);
    assert_eq!(records.len(), 2);

    assert_eq!(records[0].name, "Syarikat ABC");
    assert_eq!(records[0].address, "123 Jalan, 50000 KL, Selangor");
    assert_eq!(records[0].postcode, "50000");
    assert_eq!(records[0].state, "Selangor");

    assert_eq!(records[1].name, "Syarikat XYZ");
    assert_eq!(records[1].postcode, "47000");
    assert_eq!(records[1].state, "Selangor");
}

#[test]
fn test_parse_table_empty() {
    let records = parse_table("<html>no spans</html>");
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

    let records = parse_table(html);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "Valid Co");
}

#[test]
fn test_parse_table_missing_address() {
    let html = "<span class=\"company-name\">Only Name Co</span>";

    let records = parse_table(html);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "Only Name Co");
    assert_eq!(records[0].address, "");
}

#[test]
fn test_parse_table_no_postcode_or_state() {
    let html = "<span class=\"company-name\">N</span>\n<span class=\"company-address\">Somewhere vague</span>";
    let records = parse_table(html);
    assert_eq!(records[0].postcode, "");
    assert_eq!(records[0].state, "");
}

// ── parse_product_table ────────────────────────────────────────

#[test]
fn test_parse_product_table_row() {
    let html = "<html><body><table>\n\
        <tr>\n\
        <td class=\"text-center font-semibold\">1.</td>\n\
        <td>\n\
        <span class=\"company-name\">Biskut Coklat</span>\n\
        <span class=\"company-brand\"><br><b>JENAMA:</b>Orang Kaya<br></span>\n\
        <span class=\"company-address\"><i>Syarikat Contoh Sdn Bhd</i></span>\n\
        </td>\n\
        <td class=\"text-center\">31/05/2028</td>\n\
        </tr>\n\
        </table></body></html>";

    let records = parse_product_table(html);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "Biskut Coklat");
    assert_eq!(records[0].brand, "Orang Kaya");
    assert_eq!(records[0].holder, "Syarikat Contoh Sdn Bhd");
    assert_eq!(records[0].expiry_date, "31/05/2028");
}

#[test]
fn test_parse_product_table_empty_brand() {
    let html = "<table>\n\
        <tr>\n\
        <td class=\"text-center font-semibold\">1.</td>\n\
        <td>\n\
        <span class=\"company-name\">Tanpa Jenama</span>\n\
        <span class=\"company-brand\"></span>\n\
        <span class=\"company-address\"><i>Co</i></span>\n\
        </td>\n\
        <td class=\"text-center\"></td>\n\
        </tr>\n\
        </table>";

    let records = parse_product_table(html);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "Tanpa Jenama");
    assert_eq!(records[0].brand, "");
    assert_eq!(records[0].expiry_date, "");
}

#[test]
fn test_parse_product_table_no_rows() {
    assert_eq!(parse_product_table("<table></table>").len(), 0);
}
