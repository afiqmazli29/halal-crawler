# Malaysia Halal Directory Scraper

Scrapes the [Malaysia Halal Portal](https://myehalal.halal.gov.my/) public directory — extracting company listings and product/premise listings across all categories into a PostgreSQL database.

## Quick start

```bash
cp .env.example .env   # edit DATABASE_URL if needed
cargo run
```

Requires Rust 1.85+ (edition 2024) and a running PostgreSQL instance.

## How it works

Two-phase scrape:

1. **Phase 1 — Companies:** scrapes 9 category × `CO` (Syarikat) listing pages, then fetches each company detail page. Inserts into the `companies` table.
2. **Phase 2 — Products & others:** scrapes 9 subcategory listings (products, premises, hotels, etc.) with pagination. Inserts into the `products` table.

After both phases, a few random sample rows are printed to confirm the output.

## Database

Uses **PostgreSQL**. The default `DATABASE_URL` (set in `main.rs`) is:

```
postgres://postgres:postgres@localhost/halal
```

Override it via the `DATABASE_URL` environment variable.

### Schema

**`companies`**

| Column | Type | Description |
|--------|------|-------------|
| `id` | `SERIAL PK` | Auto-increment ID |
| `category_code` | `TEXT` | e.g. `BG`, `FM` |
| `name` | `TEXT` | Company name |
| `address` | `TEXT` | Full address |
| `state` | `TEXT` | State (`negeri`) |
| `phone_no` | `TEXT` | Phone number |
| `fax_no` | `TEXT` | Fax number |
| `email` | `TEXT` | Email address |
| `website` | `TEXT` | Website URL |
| `reference_no` | `TEXT` | Halal reference number |
| `officer` | `TEXT` | Responsible officer |
| `scraped_at` | `TIMESTAMPTZ` | Timestamp of scrape |

Unique on `(category_code, name)`.

**`products`**

| Column | Type | Description |
|--------|------|-------------|
| `id` | `SERIAL PK` | Auto-increment ID |
| `name` | `TEXT` | Product/premise name |
| `brand` | `TEXT` | Brand name |
| `category_code` | `TEXT` | Parent category code |
| `subcategory_code` | `TEXT` | Subcategory code |
| `company_id` | `INTEGER` | FK → `companies.id` |
| `expiry_date` | `TEXT` | Halal expiry date |
| `scraped_at` | `TIMESTAMPTZ` | Timestamp of scrape |

Unique on `(category_code, subcategory_code, name, brand)`.

## Categories scraped

| Code | Name | Phase 1 (Companies) | Phase 2 (Others) |
|------|------|---------------------|-------------------|
| BG | Barang Gunaan | Syarikat (`CO`) | Barang Gunaan (`BG`) |
| FM | Farmaseutikal | Syarikat (`CO`) | Farmaseutikal (`FM`) |
| KO | Kosmetik & Dandanan | Syarikat (`CO`) | Kosmetik (`KO`) |
| MD | Peranti Perubatan | Syarikat (`CO`) | Peranti Perubatan (`MD`) |
| OEM | OEM | Syarikat (`CO`) | OEM (`OEM`) |
| PE | Premis Makanan | Syarikat (`CO`) | Hotel & Resort (`HO`), Premis Makanan (`PE`) |
| PL | Logistik | Syarikat (`CO`) | — |
| PR | Produk Makanan/Minuman | Syarikat (`CO`) | Produk (`PR`) |
| PS | Rumah Sembelihan | Syarikat (`CO`) | Rumah Sembelih (`RS`) |

## Configuration

| Setting | Default | Where |
|---------|---------|-------|
| Concurrency | 5 parallel requests | `MAX_CONCURRENT` in `config.rs` |
| DB URL | `postgres://postgres:postgres@localhost/halal` | `DATABASE_URL` env var |

## Architecture

| File | Purpose |
|------|---------|
| `main.rs` | Entrypoint. Runs both phases, prints summary + sample rows. |
| `config.rs` | Strategy lists (`company_strategies`, `other_strategies`), semaphore, `MAX_CONCURRENT`. |
| `scraper.rs` | Orchestrates HTTP scraping: detail page fetching for companies, pagination for products. |
| `http.rs` | Low-level `reqwest` helper: semaphore-guarded HTML fetch with timeout and retry. |
| `paginate.rs` | Paginated table scraping for product/premise subcategories. |
| `parser.rs` | Markdown table parsing for paginated product/premise listings. |
| `db.rs` | PostgreSQL schema init + upsert inserts. `pick_str()` tries multiple JSON key variants (Malay + English). |
| `types.rs` | `SubStrategy` struct, `Error` type alias, `pick_str()` helper. |
