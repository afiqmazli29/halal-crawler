use halal_crawler::parser::{extract_total_pages, parse_modal, parse_product_table, parse_table};

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

#[test]
fn test_parse_table_multiline_address_with_br() {
    // The live portal emits multi-line addresses split across <br> tags. These
    // must be joined with ", " instead of being concatenated with no delimiter.
    let html = "<span class=\"company-name\">Syarikat ABC</span>\n\
                <span class=\"company-address\">No 12, Jalan Merdeka<br>50000 Kuala Lumpur<br>Selangor</span>";

    let records = parse_table(html);
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].address,
        "No 12, Jalan Merdeka, 50000 Kuala Lumpur, Selangor"
    );
    assert_eq!(records[0].postcode, "50000");
    assert_eq!(records[0].state, "Selangor");
}

#[test]
fn test_parse_table_multiline_address_in_nested_element() {
    // Line breaks may appear inside nested elements (e.g. <i>), and whitespace
    // around tags should collapse to single spaces.
    let html = "<span class=\"company-name\">Syarikat XYZ</span>\n\
                <span class=\"company-address\"><i>No 12<br>Jalan Merdeka</i><br>\
                47000 Shah Alam<br>Selangor</span>";

    let records = parse_table(html);
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].address,
        "No 12, Jalan Merdeka, 47000 Shah Alam, Selangor"
    );
    assert_eq!(records[0].postcode, "47000");
    assert_eq!(records[0].state, "Selangor");
}

#[test]
fn test_parse_table_single_line_address_preserved() {
    // An already-readable single-line address stays identical.
    let html = "<span class=\"company-name\">Co</span>\n\
                <span class=\"company-address\">123 Jalan, 50000 KL, Selangor</span>";
    let records = parse_table(html);
    assert_eq!(records[0].address, "123 Jalan, 50000 KL, Selangor");
}

#[test]
fn test_parse_table_name_split_across_adjacent_text_nodes() {
    // The portal can split a company name across several adjacent text runs
    // (or nested tags) with no whitespace between them. These must be
    // concatenated as-is — NOT padded with stray spaces mid-word.
    let html = "<span class=\"company-name\">ACE<b> HEALTH</b><i> PRODUCTS</i> SDN BHD</span>\n\
                <span class=\"company-address\">1 Jalan, 50000 KL</span>";
    let records = parse_table(html);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "ACE HEALTH PRODUCTS SDN BHD");
}

#[test]
fn test_parse_table_name_split_per_character() {
    // Extreme case: a name whose characters sit in separate adjacent text nodes.
    let html = "<span class=\"company-name\"><b>A</b><b>C</b><b>E</b></span>\n\
                <span class=\"company-address\">2 Jalan</span>";
    let records = parse_table(html);
    assert_eq!(records[0].name, "ACE");
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

// ── comp_code extraction from the listing rows ──────────────────────

#[test]
fn test_parse_table_extracts_comp_codes() {
    let html = format!(
        "<html><body><table>\
         <tr class=\"cursor-pointer\" onclick=\"openModal('directory/slm_viewdetail.php?comp_code=COMP-20230804-130326&type=C', 'Maklumat Sijil Halal');\">\
         <td><span class=\"company-name\">CPL Aromas (Malaysia) Sdn. Bhd.</span>\
         <span class=\"company-address\">Lot 126, 42920 Pelabuhan Klang, Selangor</span></td></tr>\
         </table></body></html>"
    );
    let records = parse_table(&html);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].comp_code, "COMP-20230804-130326");
}

#[test]
fn test_parse_table_missing_onclick_gives_empty_comp_code() {
    let html = "<html><body><table>\
         <tr class=\"cursor-pointer\">\
         <td><span class=\"company-name\">No Modal Co</span>\
         <span class=\"company-address\">1 Jalan, 50000 KL, Kuala Lumpur</span></td></tr>\
         </table></body></html>";
    let records = parse_table(html);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].comp_code, "");
}

// ── modal detail page parsing ───────────────────────────────────────

/// A minimal modal page in the live portal's shape: label/value rows
/// followed by the Product / Menu List table.
fn modal_html() -> String {
    String::from(
        "<html><body><table>\
         <tr><td width=\"25%\"><b><div align=\"right\">Name :</div></b></td>\
         <td>CPL Aromas (Malaysia) Sdn. Bhd.</td></tr>\
         <tr><td><b><div align=\"right\">Address :</div></b></td>\
         <td>No. Lot 126 Jalan Sungai Pinang <br>Taman Perindustrian <br>42920, Pelabuhan Klang<br>Selangor</td></tr>\
         <tr><td><b><div align=\"right\">State :</div></b></td><td>Selangor</td></tr>\
         <tr><td><b><div align=\"right\">Phone No :</div></b></td><td>0331617111</td></tr>\
         <tr><td><b><div align=\"right\">Fax No. :</div></b></td><td></td></tr>\
         <tr><td><b><div align=\"right\">e-mail :</div></b></td><td>haziq@example.com</td></tr>\
         <tr><td><b><div align=\"right\">Website :</div></b></td><td></td></tr>\
         <tr><td><b><div align=\"right\">Reference No. :</div></b></td><td>JAKIM.700-2/3/6 070-11/2023</td></tr>\
         <tr><td><b><div align=\"right\">Officer :</div></b></td><td>Mohammad Saharin Bin Mat Yusof<br>Amirul Bin Sahadad</td></tr>\
         <tr><td colspan=\"2\"><b>Product / Menu List :</b>\
         <table width=\"100%\" border=\"1\">\
         <tr bgcolor=\"#00CCCC\"><td><b>No.</b></td><td><b>Product</b></td><td><b>Brand</b></td><td><b>Expiry Date</b></td></tr>\
         <tr><td align=\"center\">1.</td>\
         <td class=\"txt\">HK314634 MALICE AF (CONCENTRATED FRAGRANCE)</td>\
         <td class=\"txt\">CPL AROMAS (MALAYSIA) SDN. BHD.</td>\
         <td align=\"center\">15/07/2029</td></tr>\
         <tr><td align=\"center\">2.</td>\
         <td class=\"txt\">SP163825 BLACK OUD AP (CONCENTRATED FRAGRANCE)</td>\
         <td class=\"txt\">CPL AROMAS (MALAYSIA) SDN BHD</td>\
         <td align=\"center\">30/09/2028</td></tr>\
         </table></td></tr>\
         </table></body></html>",
    )
}

#[test]
fn test_parse_modal_company_fields() {
    let (company, _products) = parse_modal(&modal_html());
    assert_eq!(company.name, "CPL Aromas (Malaysia) Sdn. Bhd.");
    assert!(!company.address.is_empty());
    assert_eq!(company.postcode, "42920");
    assert_eq!(company.state, "Selangor");
    assert_eq!(company.phone_no, "0331617111");
    // Empty fax stays empty
    assert_eq!(company.fax_no, "");
    assert_eq!(company.email, "haziq@example.com");
    assert_eq!(company.website, "");
    assert_eq!(company.reference_no, "JAKIM.700-2/3/6 070-11/2023");
    // Officers are <br>-joined with ", "
    assert!(
        company.officer.contains("Mohammad Saharin Bin Mat Yusof")
            && company.officer.contains("Amirul Bin Sahadad"),
        "officer: {:?}",
        company.officer
    );
}

#[test]
fn test_parse_modal_products() {
    let (_, products) = parse_modal(&modal_html());
    assert_eq!(products.len(), 2);

    assert_eq!(
        products[0].name,
        "HK314634 MALICE AF (CONCENTRATED FRAGRANCE)"
    );
    assert_eq!(products[0].holder, "CPL AROMAS (MALAYSIA) SDN. BHD.");
    assert_eq!(products[0].expiry_date, "15/07/2029");

    assert_eq!(products[1].expiry_date, "30/09/2028");
}

#[test]
fn test_parse_modal_empty_html() {
    let (company, products) = parse_modal("<html><body></body></html>");
    assert_eq!(company.name, "");
    assert!(products.is_empty());
}
