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
2. For each category, search each letter `a`–`z`: company listings (`ty=CO`) paginate via the `hdnCounter` the portal echoes; subcategory listings (products, premises, …) advance on the page parameter using the total-page count from page one.
3. Insert the records into the `companies` and `products` tables, then print a few sample rows.

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
| `phone_no` | `TEXT` | Phone number (not yet scraped) |
| `fax_no` | `TEXT` | Fax number (not yet scraped) |
| `email` | `TEXT` | Email (not yet scraped) |
| `website` | `TEXT` | Website URL (not yet scraped) |
| `reference_no` | `TEXT` | Halal reference number (not yet scraped) |
| `officer` | `TEXT` | Responsible officer (not yet scraped) |
| `created_at` | `TIMESTAMPTZ` | When the row was first inserted |
| `updated_at` | `TIMESTAMPTZ` | When the row was last updated |

Unique on `(name)` — a company appearing in multiple categories collapses into
one row.

**`products`**

| Column | Type | Description |
|--------|------|-------------|
| `id` | `SERIAL PK` | Auto-increment ID |
| `name` | `TEXT` | Product/premise name |
| `brand` | `TEXT` | Brand name (from `JENAMA:`) |
| `holder` | `TEXT` | Certificate holder company |
| `category_code` | `TEXT` | Parent category code |
| `subcategory_code` | `TEXT` | Subcategory code |
| `company_id` | `INTEGER` | FK → `companies.id` |
| `expiry_date` | `TEXT` | Halal expiry date |
| `created_at` | `TIMESTAMPTZ` | When the row was first inserted |
| `updated_at` | `TIMESTAMPTZ` | When the row was last updated |

Unique on `(category_code, subcategory_code, name, brand)`.

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
| `portal.rs` | The Portal seam: base URL, PHP session, semaphore, POST search, GET/retry. Tests substitute an httpmock server via `Portal::new(base_url)`. |
| `listing.rs` | The listing fetcher: `fetch_companies` (hdnCounter pagination, name dedup) and `fetch_subcategory` (page-param pagination). Hides the crawl protocols. |
| `parser.rs` | HTML extractors: `parse_table` (company spans), `parse_product_table` (product rows), `extract_counter`, `extract_total_pages`. |
| `records.rs` | Typed `Company`/`Product` with `from_value` adapters owning the portal's Malay+English key variants. |
| `db.rs` | PostgreSQL schema init + upsert inserts + `sample_companies`. |
| `config.rs` | Category strategy lists (`company_strategies`, `other_strategies`). |
| `constants.rs` | `MAX_CONCURRENT`, `DATA_PARAM`, `STATES`. |

The domain glossary lives in `CONTEXT.md`.
