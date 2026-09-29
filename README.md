# Malaysia Halal Directory Scraper

Scrapes the [Malaysia Halal Portal](https://www.halal.gov.my/) public directory — extracting company listings and product/premise listings across all categories into a PostgreSQL database.

## Quick start

```bash
cp .env.example .env   # edit DATABASE_URL if needed
cargo run
```

Requires Rust 1.85+ (edition 2024) and a running PostgreSQL instance.

## How it works

1. Seed a PHP session on the portal.
2. For each category, search each letter `a`–`z`: company listings (`ty=CO`) and subcategory listings (products, premises, …) both paginate on the `page` parameter, using the total-page count announced on page one.
3. Each company row in the listing carries an `onclick="openModal('directory/slm_viewdetail.php?comp_code=…', …)"` link. The `comp_code` is extracted, and each company's modal detail page (`/directory/slm_viewdetail.php?comp_code=…&type=C`) is fetched concurrently to enrich the company record (phone, fax, e-mail, website, reference no., officer) and scrape its **Product / Menu List**.
4. Insert the records into the `companies`, `products`, and `product_categories` tables, then print a few sample rows.

The modal parsing currently targets the detail-page layout used by Barang
Gunaan, Farmaseutikal, Kosmetik & Dandanan Diri, Peranti Perubatan, and Produk
Makanan/Minuman — the company targets in `config::targets()`. Other categories'
modal layouts may differ and need separate handling.

## Database

Uses **PostgreSQL**. The default `DATABASE_URL` (set in `main.rs`) is:

```
postgres://postgres:postgres@localhost/halal
```

Override it via the `DATABASE_URL` environment variable.

### Schema

**`companies`** — company data only (no category or scrape timing; those live in `scrap_log`).

| Column | Type | Description |
|--------|------|-------------|
| `id` | `SERIAL PK` | Auto-increment ID |
| `name` | `TEXT` | Company name |
| `address` | `TEXT` | Full address |
| `postcode` | `TEXT` | Postcode parsed from the address |
| `state` | `TEXT` | State parsed from the address |
| `phone_no` | `TEXT` | Phone number (from the modal detail page) |
| `fax_no` | `TEXT` | Fax number (from the modal detail page) |
| `email` | `TEXT` | Email (from the modal detail page) |
| `website` | `TEXT` | Website URL (from the modal detail page) |
| `reference_no` | `TEXT` | Halal reference number |
| `officer` | `TEXT` | Responsible officer(s) (`<br>`-joined with `, `) |
| `comp_code` | `TEXT` | Portal company code from the listing's modal link |
| `created_at` | `TIMESTAMPTZ` | When the row was first inserted |
| `updated_at` | `TIMESTAMPTZ` | When the row was last updated |

Unique on `(name)` — a company appearing in multiple categories collapses into
one row. On upsert, empty values never clobber existing non-empty ones.

**`products`**

| Column | Type | Description |
|--------|------|-------------|
| `id` | `SERIAL PK` | Auto-increment ID |
| `name` | `TEXT` | Product/premise name |
| `brand` | `TEXT` | Brand name (from `JENAMA:`) |
| `holder` | `TEXT` | Certificate holder company (text, not FK) |
| `company_id` | `INTEGER NOT NULL` | FK → `companies.id` |
| `expiry_date` | `TEXT` | Halal expiry date |
| `created_at` | `TIMESTAMPTZ` | When the row was first inserted |
| `updated_at` | `TIMESTAMPTZ` | When the row was last updated |

Unique on `(company_id, name, brand)`. Category/subcategory membership is
tracked separately in `product_categories`.

**`product_categories`** — mapping table linking products to their
(category, subcategory). A product can appear in multiple categories; a
category can contain many products.

| Column | Type | Description |
|--------|------|-------------|
| `product_id` | `INTEGER PK,FK` | → `products.id` (on delete cascade) |
| `category_code` | `TEXT` | Parent category code |
| `subcategory_code` | `TEXT` | Subcategory code |
| `created_at` | `TIMESTAMPTZ` | When the link was first recorded |

Composite PK on `(product_id, category_code, subcategory_code)`.

**`scrap_log`** — records each scrape run generally (not per company/product
row).

| Column | Type | Description |
|--------|------|-------------|
| `id` | `SERIAL PK` | Auto-increment ID |
| `category_code` | `TEXT` | Category crawled (e.g. `BG`, `PR`) |
| `phase` | `TEXT` | `companies` or `products` |
| `started_at` | `TIMESTAMPTZ` | When the crawl started |
| `finished_at` | `TIMESTAMPTZ` | When the crawl finished (NULL until done) |
| `inserted_count` | `INT` | Rows inserted |
| `updated_count` | `INT` | Rows updated |


## Categories scraped

| Code | Name | Companies (`ty=CO`) | Subcategories |
|------|------|---------------------|---------------|
| BG | Barang Gunaan | ✓ | Barang Gunaan (`BG`) |
| FM | Farmaseutikal | ✓ | Farmaseutikal (`FM`) |
| KO | Kosmetik & Dandanan | ✓ | Kosmetik (`KO`) |
| MD | Peranti Perubatan | ✓ | Peranti Perubatan (`MD`) |
| OEM | OEM | ✓ | OEM (`OEM`) |
| PE | Premis Makanan | ✓ | Hotel & Resort (`HO`), Premis Makanan (`PE`) |
| PL | Logistik | ✓ | — |
| PR | Produk Makanan/Minuman | ✓ | Produk (`PR`) |
| PS | Rumah Sembelihan | ✓ | Rumah Sembelih (`RS`) |

## Configuration

| Setting | Default | Where |
|---------|---------|-------|
| Concurrency | 5 parallel requests | `MAX_CONCURRENT` in `constants.rs` |
| DB URL | `postgres://postgres:postgres@localhost/halal` | `DATABASE_URL` env var |
| Max pages/letter | 1 in debug builds, unlimited in release | `max_pages_per_letter()` in `constants.rs` |

Debug builds (`cargo run`, no `--release`) automatically cap each letter's crawl
at one page, so a local smoke run against the live portal won't chew through its
thousands of pages. `HALAL_MAX_PAGES=N` overrides the cap for any build;
`HALAL_MAX_PAGES=0` disables it (full crawl even in debug). See `.env.example`.

## Architecture

| File | Purpose |
|------|---------|
| `main.rs` | Entrypoint. Seeds the portal session, crawls companies then subcategory listings, inserts, prints sample rows. |
| `portal.rs` | The Portal seam: base URL, PHP session, semaphore, one shared retry policy, POST `search`, and `fetch_modal`. Tests substitute an httpmock server via `Portal::new(base_url)`. |
| `listing.rs` | The listing fetcher: `fetch_companies` (name dedup), `fetch_subcategory` (key dedup), and `fetch_company_modals` — all sharing `crawl`/`letter_crawl` (page-param pagination). |
| `parser.rs` | HTML extractors: `parse_table` (company spans + `comp_code`), `parse_product_table` (product rows), `parse_modal` (detail page), `extract_total_pages`, `extract_postcode`, `extract_state`. |
| `records.rs` | Typed `Company`/`Product`. `Product` carries its own (category, subcategory) membership. `from_value`/`pick_str` are test-only helpers; production parsing builds the structs directly in `parser.rs`. |
| `db.rs` | PostgreSQL schema init + upsert inserts + `sample_companies`. |
| `config.rs` | The crawl target list (`targets()`) and progress labels (`label()`). |
| `constants.rs` | `MAX_CONCURRENT`, `DEBUG_MAX_PAGES_PER_LETTER`, `DATA_PARAM`, `STATES`, `max_pages_per_letter`. |

The domain glossary lives in `CONTEXT.md`. The technical design — module
contracts, data-model invariants, decision log, and forward roadmap — lives in
[`docs/DESIGN.md`](docs/DESIGN.md).
