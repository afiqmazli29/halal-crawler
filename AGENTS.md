# AGENTS.md

## Setup

```bash
cp .env.example .env   # edit DATABASE_URL if needed
cargo build            # or cargo run
```

Requires a running PostgreSQL instance (default: `postgres://postgres:postgres@localhost/halal`).

## Edition

This crate uses **Rust edition 2024** (`Cargo.toml:4`). Requires Rust 1.85+.

## No tests

There are no tests, no CI, no lint/formatter config. Only `cargo check`/`cargo build`/`cargo run` exist as verification.

## Env vars

- `DATABASE_URL` — optional, defaults to `postgres://postgres:postgres@localhost/halal`.

## Architecture

- `main.rs` — entrypoint. Two-phase scrape (companies → products/others), then prints sample DB rows.
- `types.rs` — `SubStrategy` struct, `MAX_CONCURRENT` (5), semaphore factory, both strategy lists. The `data_param` is a base64-encoded directory path.
- `scraper.rs` — Orchestrates HTTP scraping: detail page fetching for companies, pagination for products.
- `http.rs` — Low-level `reqwest` helper: semaphore-guarded HTML fetch with timeout and retry.
- `paginate.rs` — Paginated table scraping for product/premise subcategories.
- `parser.rs` — Markdown table parsing for paginated product/premise listings.
- `db.rs` — PostgreSQL init + inserts. `pick_str()` tries multiple JSON key variants (Malay + English). Uses `CREATE TABLE IF NOT EXISTS` and upsert (`ON CONFLICT ... DO UPDATE`).

## Adding new JSON fields

When the scraped page adds new keys, add them to the `pick_str()` key list in `db.rs` and the corresponding `INSERT` SQL. Follow the existing pattern of trying Malay first, then English.
