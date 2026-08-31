# AGENTS.md

## Setup

```bash
cp .env.example .env   # edit DATABASE_URL if needed
cargo run
```

Requires Rust 1.85+ (edition 2024) and a running PostgreSQL (default
`postgres://postgres:postgres@localhost/halal`; `compose.yaml` brings one up).
`db::init` creates the schema automatically (`CREATE TABLE IF NOT EXISTS` +
`ALTER TABLE ... ADD COLUMN IF NOT EXISTS`); `migrations/0001_*.sql` is a
one-time migration for pre-split databases only.

## Verification

- `cargo check` — compile check. The pre-commit hook (wired via
  `git config core.hooksPath=.githooks`) runs `cargo fmt -- --check` + `cargo check`.
- `cargo test` — **needs a live PostgreSQL.** The DB tests (`crawl_test.rs`,
  `tests/common/mod.rs`) panic without one; point elsewhere via `TEST_DATABASE_URL`.
- Non-DB suites (fast, no PostgreSQL): `cargo test --test parser_tests --test records_tests --test config_tests --test constants_tests`.
- Single test: `cargo test test_scrape_companies_dedups_across_letters`.

## Architecture

Lib + bin: `src/lib.rs` exposes the `halal_crawler` crate; `src/main.rs` is the
binary. Tests import `halal_crawler::...`.

Two crawl phases in `main.rs`:
1. **Companies** — a letter search discovers each row's `comp_code` (from its
   `onclick` modal link); each company's modal detail page is fetched
   concurrently to enrich fields and scrape its product list.
2. **Subcategory listings** — products/premises per category.

- `portal.rs` — Portal seam: base URL, PHP session, browser-shaped client,
  semaphore, POST `search` (retries 3× with backoff) and `get`. Tests substitute
  an httpmock server via `Portal::new(base_url)`.
- `listing.rs` — `fetch_companies` (name-dedup), `fetch_subcategory`,
  `fetch_company_modals`, and the shared `crawl`/`letter_crawl`. Hides pagination
  and concurrency.
- `parser.rs` — HTML extractors: `parse_table` (company rows + comp_code),
  `parse_product_table`, `parse_modal` (detail page), `extract_total_pages`,
  `extract_postcode`, `extract_state`. **All HTML parsing lives here.**
- `records.rs` — `Company`/`Product`. `from_value`/`pick_str` are test-only
  helpers now; production parsing builds the structs directly in `parser.rs`.
- `db.rs` — four tables: `companies` (unique `name`), `products` (unique
  `company_id, name, brand`), `product_categories` (many-to-many), `scrap_log`
  (per category+phase run). Upserts: empty values never clobber non-empty.
- `types.rs` — `Error` alias, `error_chain`, `SubStrategy`. Log crawl errors via
  `types::error_chain` — reqwest hides its real cause behind `Display`.
- `config.rs` — `company_strategies()` / `other_strategies()` (category/ty pairs).
- `constants.rs` — `MAX_CONCURRENT`, `DATA_PARAM`, `STATES`, `max_pages_per_letter`.

Domain terms live in `CONTEXT.md` (Portal, Listing, Category, …); honor its "Avoid:" list.

## Gotchas

- **Debug `cargo run` crawls only 1 page per letter** (`max_pages_per_letter`).
  Override with `HALAL_MAX_PAGES=N`; `HALAL_MAX_PAGES=0` runs the full crawl.
- Pagination is driven by the `page` parameter alone — the portal ignores
  `hdnCounter` (the `counter` arg is always `"0"`). Page 1 announces the total in
  a `Total Record : … From N` line.
- Company modal URL: `/directory/slm_viewdetail.php?comp_code=<comp_code>&type=C`
  (`listing::modal_url`). The modal layout is only verified for a subset of
  categories (see README); others may need separate handling.
- Crawl tests use shared fixtures in `tests/common/mod.rs` (`listing_html`,
  `product_listing_html`) — reuse them when adding crawl tests.

## Portal protocol notes

The live portal is reachable without auth: GET `index.php` to seed a PHP session,
then POST `index.php?data=DATA_PARAM&negeri=&category=C&page=N&cari=L` with form
`hdnCounter`, `t`, `a`, `ty` (the subcategory code). Listing rows are spans
(`company-name`, `company-brand`, `company-address`) plus an expiry cell; product
brands are prefixed `JENAMA:`.
