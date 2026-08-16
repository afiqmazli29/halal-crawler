# AGENTS.md

## Setup

```bash
cp .env.example .env   # edit DATABASE_URL if needed
cargo build            # or cargo run
```

Requires a running PostgreSQL instance (default: `postgres://postgres:postgres@localhost/halal`, also `compose.yaml` for podman/docker). This crate uses **Rust edition 2024** (`Cargo.toml:4`), Rust 1.85+.

## Verification

- `cargo check` — compile check (pre-commit hook runs this + `cargo fmt --check`)
- `cargo test` — integration tests in `tests/`. Crawl tests use `httpmock` (no DB needed); DB insert tests need a live PostgreSQL.

## Domain model

`CONTEXT.md` holds the domain glossary (Portal, Listing, Category, Subcategory, Company, Product, Record, Crawl). Use those terms.

## Architecture

- `main.rs` — entrypoint. Seeds the portal session, crawls company categories and subcategory listings, inserts, prints sample DB rows.
- `portal.rs` — the Portal seam: base URL, session, semaphore, POST search, GET/retry. Tests substitute an httpmock server via `Portal::new(base_url)`.
- `listing.rs` — the listing fetcher: `fetch_companies` (page-param pagination, name dedup) and `fetch_subcategory` (same crawl, product rows). Hides the crawl protocols.
- `parser.rs` — HTML extractors: `parse_table` (company spans), `parse_product_table` (product rows), `extract_total_pages`.
- `records.rs` — typed `Company`/`Product` with `from_value` adapters owning the portal's Malay+English key variants.
- `db.rs` — PostgreSQL init + upserts + `sample_companies`. Schema is created/migrated via `CREATE TABLE IF NOT EXISTS` and `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`.
- `config.rs` — category strategy lists (`company_strategies`, `other_strategies`).
- `constants.rs` — `MAX_CONCURRENT`, `DATA_PARAM` (base64 directory param), `STATES`.

## Portal protocol notes

The live portal is reachable without auth: GET `index.php` to seed a PHP session, then POST `index.php?data=DATA_PARAM&negeri=&category=C&page=N&cari=L` with form `hdnCounter`, `t`, `a`, `ty` (the subcategory code). Pagination is driven by the `page` parameter alone — the `hdnCounter` echo is only a record-count display, and page one announces the total page count in a `Total Record : … From N` line. Listing rows are spans (`company-name`, `company-brand`, `company-address`) plus an expiry cell; product brands are prefixed `JENAMA:`.

## Adding new JSON fields

When the scraped page adds new keys, add them to the key lists in the `from_value` adapters in `records.rs` (Malay first, then English) — the corresponding DB columns live in `db.rs`.
