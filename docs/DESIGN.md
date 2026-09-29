# Technical Design — Halal Directory Crawler

Status: living document. Update it when a job changes a decision or an
invariant below; new work should be checked against this document before it
lands.

Canonical vocabulary lives in [`CONTEXT.md`](../CONTEXT.md). When this document
and `CONTEXT.md` disagree on a domain term, `CONTEXT.md` wins.

## 1. Purpose & scope

Scrape the Malaysian Halal Portal's public directory — company listings and
product/premise listings — into PostgreSQL, idempotently, so repeated runs
refresh changed rows and add new ones without duplicates.

**In scope**

- Reading the public directory (no authentication).
- Two crawl phases: companies (with modal enrichment) and subcategory listings.
- Upserting records into PostgreSQL with `scrap_log` run tracking.

**Non-goals**

- Writing anything back to the portal.
- A UI or API over the data (consumers read the database directly).
- Real-time freshness — the intended cadence is periodic (see F2).

## 2. System overview

The crate is a lib + bin: `src/lib.rs` exposes the `halal_crawler` crate,
`src/main.rs` is the binary. Tests import `halal_crawler::...`.

`main.rs` runs two phases:

1. **Companies** (`main.rs:33-95`) — for each company category, a letter search
   (`a`–`z`) discovers each row's `comp_code` from its `onclick` modal link.
   Listing records are upserted first, then each company's modal detail page is
   fetched concurrently to enrich fields (phone, fax, e-mail, website,
   reference no., officer) and scrape its product list. Enriched companies and
   their products are upserted.
2. **Subcategory listings** (`main.rs:97-124`) — products/premises per
   (category, ty) pair, upserted with their category mapping.

```
                 ┌────────────┐
                 │   Portal   │  base URL, PHP session, semaphore, retries
                 └─────┬──────┘
          search (POST)│  get (modal GET)
                 ┌─────▼──────┐
                 │  listing   │  pagination + concurrency + dedup
                 └─────┬──────┘
                 ┌─────▼──────┐
                 │   parser   │  all HTML extraction
                 └─────┬──────┘
                 ┌─────▼──────┐
                 │  records   │  typed Company / Product
                 └─────┬──────┘
                 ┌─────▼──────┐
                 │     db     │  schema init + upserts + scrap_log
                 └────────────┘
```

### Module map

| Module | Responsibility |
|--------|----------------|
| `portal.rs` | The Portal seam: base URL, PHP session, browser-shaped client, semaphore, POST `search`, `fetch_modal`, and the one shared retry policy. |
| `listing.rs` | `fetch_companies` (name-dedup), `fetch_subcategory` (key-dedup), `fetch_company_modals`, and the shared `crawl`/`letter_crawl`. Hides pagination and concurrency. |
| `parser.rs` | All HTML record extraction: `parse_table`, `parse_product_table`, `parse_modal`, `extract_postcode`, `extract_state`. |
| `records.rs` | `Company` / `Product` types. `Product` carries its own (category, subcategory) membership. `from_value`/`pick_str` are test-only helpers; production parsing builds the structs directly in `parser.rs`. |
| `db.rs` | Schema init, upsert inserts, private `resolve_company`, `start_scrap`/`finish_scrap`, `sample_companies`. |
| `config.rs` | `targets()` — the single list of (category, ty, phase) crawl targets — plus `label()` for progress output. |
| `constants.rs` | `MAX_CONCURRENT`, `DEBUG_MAX_PAGES_PER_LETTER`, `DATA_PARAM`, `STATES`, `max_pages_per_letter`. |
| `types.rs` | `Error` alias, `error_chain`, `CrawlTarget`, `Phase`. |

## 3. Domain model

See [`CONTEXT.md`](../CONTEXT.md) for the glossary (Portal, Listing, Category,
Subcategory, Company, Product, Record, Crawl, hdnCounter) and its "Avoid:"
list. The typed shapes every module hands along are:

```rust
struct Company {
    name, address, postcode, state,
    phone_no, fax_no, email, website,
    reference_no, officer, comp_code,
}

struct Product {
    name, brand, holder, expiry_date,
    category_code, subcategory_code,
}
```

`Company.comp_code` is scraped from the listing's `onclick` link; the remaining
company fields are filled from the modal detail page. `Product.holder` is a
company name (text, not a FK); it is resolved to a `company_id` at insert time.
`Product.category_code` / `subcategory_code` carry the membership the record was
discovered under, so the record is self-describing and `insert_products` needs
no extra parameters.

## 4. Portal protocol contract

This is the reverse-engineered contract the crawler depends on. Treat it as a
compatibility surface: if the portal changes, these are the assumptions to
re-verify.

**Session** — GET `index.php` to seed a PHP session cookie (`init_session`).

**Search** — POST
`index.php?data=DATA_PARAM&negeri=&category=<C>&page=<N>&cari=<letter>`
with form fields `hdnCounter`, `t`, `a`, `ty=<subcategory code>`.
`DATA_PARAM` is the base64 directory path in `constants.rs`.

**Pagination** — driven by the `page` parameter alone. The portal ignores
`hdnCounter`, so `search` sends it internally as `"0"`. Page 1 announces the
total in a `Total Record : … From N` line, parsed by `listing`'s private
`total_pages`; `pages_to_fetch` then applies the cap.

**Modal detail** — `/directory/slm_viewdetail.php?comp_code=<code>&type=C`,
built inside the seam by `Portal::fetch_modal`. Layout is verified only for
Barang Gunaan, Farmaseutikal, Kosmetik & Dandanan Diri, Peranti Perubatan, and
Produk Makanan/Minuman — the categories in `config::targets()`; other layouts
may need separate handling (F4).

**Verified vs. assumed** — the POST search and page-param pagination are
verified live. The modal layout is verified for the categories above only.
Phase 2 (`fetch_subcategory`) is written but has not been run end-to-end
against the live portal (F5).

## 5. Module contracts & seams

**`Portal`** — the only place that knows the base URL, session, headers, URL
shapes, and retry policy. Its interface is `init_session`, `search`, and
`fetch_modal`; the raw `get` is private. Constructed with `Portal::new(base_url)`;
tests substitute an httpmock server's base URL. Cheap to clone; copies are
passed into tasks.

**`listing`** — a deep module: callers hand over a `CrawlTarget` and get
records; they never learn about `Total Record` lines or the page parameter.
`crawl` spawns one task per letter; `letter_crawl` fetches page 1, reads the
total (`total_pages`), applies the cap (`pages_to_fetch`), then fetches the rest
concurrently. A failing letter is logged and skipped, never aborting the
category. `fetch_company_modals` fetches modal
pages concurrently and merges them onto the listing record via
`Company::fill_from`: the listing name is the identity and is never rewritten,
while the modal's non-empty fields win and the listing fills the gaps. Both
`fetch_subcategory` and `fetch_company_modals` stamp each `Product` with the
target's membership before returning it.

**`parser`** — all HTML parsing lives here. `element_text` joins
`<br>`-separated lines with `", "` and collapses whitespace, working around
`scraper`'s delimiter-free `text()`; adjacent text nodes are concatenated
exactly as emitted so names split across runs keep their spacing.

**`db`** — owns schema init and every upsert. `resolve_company` matches a
product holder to a company by exact name, then case-insensitive name,
preferring the exact match.

## 6. Data model & invariants

Four tables, created by `db::init` (`CREATE TABLE IF NOT EXISTS` +
`ALTER TABLE ... ADD COLUMN IF NOT EXISTS`). `migrations/0001_*.sql` is a
one-time migration for pre-split databases only.

| Table | Key | Notes |
|-------|-----|-------|
| `companies` | unique `name` | Company data only; category/timing live in `scrap_log`. A name in multiple categories collapses to one row. |
| `products` | unique `(company_id, name, brand)` | `company_id` is `NOT NULL` FK → `companies.id`. `holder` kept as text. |
| `product_categories` | PK `(product_id, category_code, subcategory_code)` | Many-to-many; `ON DELETE CASCADE` from `products`. |
| `scrap_log` | `id` | One row per category+phase run: `started_at`, `finished_at`, `inserted_count`, `updated_count`. |

**Invariants future jobs must not break**

- **Empty never clobbers non-empty (companies only).** The rule has two halves.
  *Within a run*, `Company::fill_from` (`records.rs`) lets the modal's
  non-empty fields win and fills the gaps from the listing. *Across runs*, the
  upsert's `CASE` guards (`db.rs:190-199`) keep a stored non-empty value when
  the incoming one is empty — run 2's listing pass must not wipe run 1's modal
  fields. Product upserts do *not* follow this rule: `holder` and `expiry_date`
  are overwritten unconditionally on conflict (`db.rs:287-289`), because a
  product's listing row is always authoritative for those two fields.
- **The listing name is the company's identity.** `fill_from` never takes
  `name` from the modal; the modal may spell a name differently, and rewriting
  it would split one company into two rows under the `name`-unique key.
- **`companies` uniqueness is by `name` alone.** Adding category to the key
  would re-introduce duplicates across categories.
- **`company_id` is mandatory.** A product whose holder doesn't resolve to a
  real company is skipped, never linked to a fabricated company (`db.rs:274`).
- **Product identity is `(company_id, name, brand)`.** Category membership is
  tracked in `product_categories`, not on the product row. `listing`'s in-memory
  dedup key is `(name, brand, holder)` — the same identity with `holder` standing
  in for `company_id` — so dedup and persistence agree.
- **`scrap_log` is per category+phase, not per row.**

## 7. Concurrency & reliability

- Concurrency is gated by one semaphore, owned by the Portal
  (`MAX_CONCURRENT = 5`, `portal.rs`) and acquired by every request. Modal
  fetches need no local cap.
- Every request shares one retry policy (`Portal::retrying`): retry transport
  failures and server errors 3× with exponential backoff, then surface the last
  error. The portal drops connections under concurrent load, so retries are
  load-bearing.
- Per-letter and per-modal failures are isolated: logged via
  `types::error_chain` (reqwest hides its real cause behind `Display`) and
  skipped. A single bad letter never aborts a category.
- Debug builds cap each letter at one page so a local `cargo run` doesn't chew
  through the portal's thousands of pages.

## 8. Configuration

| Setting | Default | Where |
|---------|---------|-------|
| `DATABASE_URL` | `postgres://postgres:postgres@localhost/halal` | env |
| `MAX_CONCURRENT` | 5 | `constants.rs` |
| Max pages/letter | 1 in debug, unlimited in release | `max_pages_per_letter()` in `constants.rs` |
| `HALAL_MAX_PAGES` | unset | env; `N` caps, `0` = unlimited |

## 9. Testing strategy

- **Portal seam** — crawl tests substitute an httpmock server via
  `Portal::new(base_url)`, so pagination and dedup are tested without the live
  portal.
- **Shared fixtures** — `tests/common/mod.rs` provides `listing_html` and
  `product_listing_html`; reuse them when adding crawl tests.
- **DB-backed suites** — `crawl_test.rs` and `tests/common` need a live
  PostgreSQL; point elsewhere via `TEST_DATABASE_URL`.
- **Non-DB suites** (fast) — `parser_tests`, `records_tests`, `config_tests`,
  `constants_tests`.
- **Compile check** — `cargo check`; the pre-commit hook runs
  `cargo fmt -- --check` + `cargo check`.

## 10. Decision log

| # | Decision | Rationale | Rejected |
|---|----------|-----------|----------|
| D1 | Lib + bin split | Tests import the crate; the binary stays a thin orchestrator. | Single binary with `#[cfg(test)]` modules. |
| D2 | Paginate by the `page` parameter, ignore `hdnCounter` | The portal ignores `hdnCounter`; page alone advances the listing. | Echoing `hdnCounter` back as a cursor. |
| D3 | `companies` unique on `name` alone | A company appears in several categories; one row per company is the desired shape. | Unique on `(category, name)` — duplicates across categories. |
| D4 | Move category membership to `product_categories` | Products can belong to multiple categories; many-to-many is the honest model. | Category columns on `products`. |
| D5 | Empty never clobbers non-empty on upsert | Listing and modal passes are partial; a later pass must not erase earlier fields. | Blind `DO UPDATE SET` on every column. |
| D6 | Skip products whose holder doesn't resolve | `company_id` is `NOT NULL`; fabricating a company would corrupt the directory. | Auto-create a placeholder company per unresolved holder. |
| D7 | One task per letter + a shared semaphore | Letters are independent; a failing letter must not abort the category, and concurrency must be bounded. | Sequential crawl; unbounded concurrency. |
| D8 | Debug builds cap pages/letter | A local `cargo run` must not crawl thousands of live pages. | Always full crawl; require `--release` to smoke-test. |
| D9 | One retry policy for every Portal request | `search` and `fetch_modal` share `Portal::retrying`; a dropped modal connection must not silently lose a company's enrichment. | Retry only `search`; a separate unused `get_retry`. |
| D10 | The Portal seam owns URL shapes and the ignored `hdnCounter` | Callers pass a `comp_code`, not a URL; `search` sends `hdnCounter="0"` internally. Keeps protocol decisions inside the seam. | `listing` building modal URLs from `Portal::base()`; a caller-visible `counter` parameter. |
| D11 | Category membership travels on the `Product` record | The record is self-describing; `insert_products` takes records alone. `listing` stamps it (the parser stays target-free). | Threading `(category, ty)` from `main` through `listing` into `db`. |
| D12 | In-memory product dedup key is `(name, brand, holder)` | Matches the persistence identity `(company_id, name, brand)`, with `holder` standing in for `company_id`; expiry is data, not identity. | Deduping by `(name, brand, holder, expiry_date)` — let two rows differing only in expiry collide in the upsert. |
| D13 | `Company::fill_from` owns within-run enrichment precedence | One named, unit-testable operation; the modal's non-empty fields win, the listing fills gaps. The SQL `CASE` keeps the cross-run half. | Inline merge in `listing` (no unit seam); removing the SQL `CASE` (would wipe stored fields on a later listing pass). |
| D14 | The listing name is the company identity; the modal never rewrites it | `companies` is unique on `name`; a differently-spelled modal name would otherwise create a duplicate row. | Modal name wins — silently splits one company into two. |
| D15 | `listing` owns the pagination grammar and cap policy | `total_pages` and `pages_to_fetch` are private to the module that paginates; the parser is for record extraction only. | `extract_total_pages` public in `parser`, tested away from its only caller. |

## 11. Forward roadmap

Each item below is a job to be checked against this document. Decisions are
proposed with alternatives noted; resolve the open ones before implementing.

### F1 — Phase 2 scale

PR alone announces ~7,350 pages for letter "b". A full phase-2 run is
enormous.

- **Recommended:** make phase 2 incremental/checkpointed — record progress per
  (category, subcategory, letter, page) so a run resumes rather than restarts,
  and re-crawl only what's stale. `scrap_log` is the natural place to grow this.
- **Alternatives:** a blunt global page cap (simple, but silently incomplete);
  a daily-incremental strategy keyed off expiry dates.
- **Interaction:** `HALAL_MAX_PAGES` already caps per letter for smoke runs;
  don't conflate that with production completion.

### F2 — Scheduling

Weekly run to refresh changed rows and add new ones; upserts already make
re-runs safe.

- **Recommended:** a systemd timer on the host — native logging via journald,
  no container runtime to maintain, easy to reason about.
- **Alternatives:** host crontab (simplest, weaker logging); podman container
  (portable, more moving parts).
- **Requirement:** persist output somewhere durable (`>> /var/log/halal-crawler.log 2>&1`)
  since per-letter progress prints to stdout.

### F3 — `company_id` resolution robustness

`resolve_company` matches holder → company by exact/case-insensitive name.
Real data will produce near-misses (punctuation, suffixes, whitespace).

- **Recommended:** normalize both sides (trim, collapse whitespace, case-fold,
  strip common suffixes) and keep the exact-match preference; log unresolved
  holders for review.
- **Alternatives:** link via `comp_code` where the holder is itself a company;
  fuzzy matching (risk of false positives).

### F4 — Modal coverage

Modal parsing is verified for a subset of categories.

- **Recommended:** detect an unrecognized modal layout and log it with the
  category/comp_code rather than silently returning empty fields, then add
  per-category handling as evidence arrives.
- **Alternatives:** one generic parser for all layouts (fragile).

### F5 — Live verification

- Verify the end-to-end company crawl after the retry fix (`cargo run`).
- Verify phase 2: the `ty=PR` POST flow, page-param pagination, and
  brand/holder/expiry extraction against the live portal.
- Run `cargo test` against a live PostgreSQL.

### F6 — Data completeness

- Populate the remaining NULL company columns via the modal detail pages
  (already wired; verify against real data).
- Refine state matching if real data shows misses (e.g. "W.P. Kuala Lumpur",
  "K.L.") — `STATES` + `extract_state`.

## 12. Open questions & risks

- **Holder → company ambiguity.** Two companies can share a name across
  categories; `name`-unique collapses them, which may merge distinct entities.
- **State extraction.** Substring matching over `STATES` can mis-hit; the
  fallback ordering needs real-data validation.
- **Portal stability.** Connection drops under concurrency are expected; if
  they worsen, lower `MAX_CONCURRENT`.
- **Silent partial crawls.** A capped or failed letter currently logs but does
  not fail the run; F1's checkpointing should make incompleteness explicit.
- **Modal layout drift.** The portal can change markup per category without
  notice; F4's detection is the mitigation.
