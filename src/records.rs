/// A company record. `comp_code` is scraped from the directory listing (the
/// `onclick` link); the remaining fields are fetched from the company's modal
/// detail page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Company {
    pub name: String,
    pub address: String,
    pub postcode: String,
    pub state: String,
    pub phone_no: String,
    pub fax_no: String,
    pub email: String,
    pub website: String,
    pub reference_no: String,
    pub officer: String,
    pub comp_code: String,
}

impl Company {
    /// Fill this record's empty fields from `fallback` (the listing record).
    /// Non-empty values win, so a modal value is kept over the listing's.
    ///
    /// `name` is never taken from the fallback: the listing name is the
    /// company's identity (the `companies` conflict key), and it is always
    /// non-empty for a discovered company. Letting the modal rewrite it would
    /// split one company into two rows when the spellings differ.
    pub fn fill_from(&mut self, fallback: &Company) {
        if self.address.is_empty() {
            self.address = fallback.address.clone();
        }
        if self.postcode.is_empty() {
            self.postcode = fallback.postcode.clone();
        }
        if self.state.is_empty() {
            self.state = fallback.state.clone();
        }
        if self.comp_code.is_empty() {
            self.comp_code = fallback.comp_code.clone();
        }
    }
}

/// A subcategory (product/premise) record: name, brand, the
/// certificate holder (company name), and the halal expiry date.
/// The `holder` is resolved to a `company_id` via the companies table
/// at insert time. `category_code` / `subcategory_code` carry the
/// membership the record was discovered under, so the record is
/// self-describing; the mapping is persisted to `product_categories`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Product {
    pub name: String,
    pub brand: String,
    pub holder: String,
    pub expiry_date: String,
    pub category_code: String,
    pub subcategory_code: String,
}
