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
- Two crawl jobs: companies (company listings + modal enrichment) and
  products (subcategory listings).
- Upserting records into PostgreSQL with `scrap_log` run tracking.
- A read-only HTTP API over the crawled data: companies, products,
  categories, and run status.

**Non-goals**

- Writing anything back to the portal.
- A write API, or a UI over the data.
- Real-time freshness — the intended cadence is periodic (see F2).

## 2. System overview

The crate is a lib plus two binaries: `src/lib.rs` exposes the
`halal_crawler` crate, `src/main.rs` is the crawler, and
`src/bin/halal-api.rs` is the read-only HTTP API. Tests import
`halal_crawler::...`.

`main.rs` is thin: it picks a job from the first argument (`companies`,
`products`, or `all`), seeds the session, calls
`crawl::run(&portal, &pool, &targets, max_pages)`, then prints the summary from
the returned `RunReport`. `crawl::run` runs whichever phases are present in the
target list it is given:

1. **Companies** — for each company target, a letter search (`a`–`z`)
   discovers each row's `comp_code` from its `onclick` modal link. Listing
   records are upserted first, then each company's modal detail page is fetched
   concurrently to enrich fields (phone, fax, e-mail, website, reference no.,
   officer). Enriched companies are upserted. Products are **not** read from
   modals.
2. **Products** — products/premises per subcategory target, upserted with the
   category mapping carried on each record. This is the only writer of
   `products` and `product_categories`.

`all` runs companies first, then products, so product holders resolve against
the company rows. Each target's `scrap_log` row is opened and closed inside
`crawl::run`. A failing target is recorded in `RunReport.failures` and skipped,
never aborting the run.

```
                 ┌────────────┐
                 │   Portal   │  base URL, PHP session, semaphore, retries
                 └─────┬──────┘
          search (POST)│  fetch_modal (GET)
                 ┌─────▼──────┐
                 │   crawl    │  jobs + insert ordering + scrap_log
                 └─────┬──────┘
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
                 │     db     │  schema init + upserts
                 └────────────┘
```

The API binary is a second, independent consumer of the same database. It
builds `api::router(pool)` and reads through `db::connect`, which opens a pool
without creating or migrating anything. It never calls the crawl modules and
never writes.

### Module map

| Module | Responsibility |
|--------|----------------|
| `crawl.rs` | The run orchestration: `run(portal, pool, targets, max_pages) -> RunReport` owns the job order, the insert ordering, and each target's `scrap_log` lifecycle. |
| `portal.rs` | The Portal seam: base URL, PHP session, browser-shaped client, semaphore, POST `search`, `fetch_modal`, and the one shared retry policy. |
| `listing.rs` | `fetch_companies` (name-dedup), `fetch_subcategory` (key-dedup), `fetch_company_modals` (company enrichment), and the shared `crawl`/`letter_crawl`. Hides pagination and concurrency. |
| `parser.rs` | All HTML record extraction: `parse_table`, `parse_product_table`, `parse_modal` (company fields only), `extract_postcode`, `extract_state`. |
| `records.rs` | `Company` / `Product` types. `Product` carries its own (category, subcategory) membership; `Company::fill_from` owns enrichment precedence. |
| `db.rs` | Schema init, upsert inserts, private `resolve_company`, `start_scrap`/`finish_scrap`, `sample_companies`. |
| `config.rs` | `company_targets()`, `product_targets()`, and `targets()` — the crawl jobs — plus `label()`/`category_name()`/`subcategory_name()` for progress output and API labels. |
| `constants.rs` | `MAX_CONCURRENT`, `DEBUG_MAX_PAGES_PER_LETTER`, `DATA_PARAM`, `STATES`, `max_pages_per_letter`. |
| `types.rs` | `Error` alias, `error_chain`, `CrawlTarget`, `Phase`. |
| `api/` | The read-only HTTP surface: `router`, DTOs, pagination params, error mapping, and one handler module per resource. |
| `bin/halal-api.rs` | The API binary: binding, tracing, CORS, graceful shutdown. Connects with `db::connect`. |

## 3. Domain model

See [`CONTEXT.md`](../CONTEXT.md) for the glossary (Portal, Listing, Category,
Subcategory, Company, Product, Record, Crawl, Crawl target, Phase, Run,
hdnCounter) and its "Avoid:" list. The typed shapes every module hands along
are:

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
The products job (`fetch_subcategory`) is written but has not been run end-to-end
against the live portal (F5).

## 5. Module contracts & seams

**`crawl`** — the run orchestration. `run` takes the portal, pool, targets, and
cap, and returns a `RunReport` (counts + `failures`). It owns the job order,
the listing-before-enrichment insert ordering, and each target's `scrap_log`
open/close. A failed target is recorded and skipped.

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
while the modal's non-empty fields win and the listing fills the gaps. It
returns companies only; products are never read from modals. `fetch_subcategory`
is the only function that returns `Product` records, and it stamps each with the
target's membership before returning it.

**`parser`** — all HTML parsing lives here. `element_text` joins
`<br>`-separated lines with `", "` and collapses whitespace, working around
`scraper`'s delimiter-free `text()`; adjacent text nodes are concatenated
exactly as emitted so names split across runs keep their spacing. `parse_modal`
extracts company fields only; `parse_product_table` is the product extractor.

**`db`** — owns schema init and every upsert. `resolve_company` matches a
product holder to a company by exact name, then case-insensitive name,
preferring the exact match. `connect` opens a pool without touching the schema;
`init` is `connect` plus schema creation and is what the crawler and tests use.

**`api`** — read-only. `router(pool)` wires the routes; handlers use
`sqlx::query_as` with `FromRow` DTOs and never the crawler's write paths.
Paginated lists (`/companies`, `/products`, `/companies/{id}/products`) are
offset-paginated (`page`/`per_page`, capped at 100) and return `{data, meta}`;
collections (`/categories`, `/scrape-runs`) and details return `{data}`; errors
return `{error: {status, message}}`. Product memberships are loaded for a whole
page in one `ANY($1)` query, not per row. The router is layer-free so tests drive
it with `tower::ServiceExt::oneshot`; the binary adds tracing, CORS, and
shutdown. The client contract is [`api/openapi.yaml`](api/openapi.yaml), a
hand-written OpenAPI 3.0.3 spec; update it when a route, parameter, or response
shape changes.

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
- **Products have one source.** Only the products job writes `products` and
  `product_categories`; company modals enrich `companies` only. This prevents
  empty-brand duplicates and company-modal `CO` pseudo-memberships.
- **Product identity is `(company_id, name, brand)`.** Category membership is
  tracked in `product_categories`, not on the product row. `listing`'s in-memory
  dedup key is `(name, brand, holder)` — the same identity with `holder` standing
  in for `company_id` — so dedup and persistence agree.
- **`scrap_log` is per category+phase, not per row.** The lifecycle is owned by
  `crawl::run_target`, which opens and closes the row on every branch.

The API reads these tables directly. `db::init` creates the indexes the read
queries rely on — `companies(state)`, `products(name)`,
`product_categories(category_code, subcategory_code)`, and
`scrap_log(started_at DESC)`; `companies(name)` is already indexed by the unique
constraint and `products(company_id)` is covered by the leading column of
`products(company_id, name, brand)`. The API uses `db::connect` and requires the schema to exist.
`/categories` is seeded from product targets and excludes legacy `CO`
memberships, so it reflects product categories only.

## 7. Concurrency & reliability

- Concurrency is gated by one semaphore, owned by the Portal
  (`MAX_CONCURRENT = 5`, `portal.rs`) and acquired by every request. Modal
  fetches need no local cap.
- Every request shares one retry policy (`Portal::retrying`): retry transport
  failures and server errors 3× with exponential backoff, then surface the last
  error. The portal drops connections under concurrent load, so retries are
  load-bearing. It currently treats other non-2xx responses (including 403,
  404, and 429) as successful bodies; F9 covers making non-2xx explicit and
  retrying 408/429.
- Per-letter and per-modal failures are isolated: logged via
  `types::error_chain` (reqwest hides its real cause behind `Display`) and
  skipped. A single bad letter never aborts a category. A failed target lands
  in `RunReport.failures`, but `main` still exits 0; F9 covers a non-zero exit.
- The semaphore bounds in-flight requests, not task count. `letter_crawl`
  spawns one task per page and `fetch_company_modals` one per company; F1
  covers bounding task spawning and batching writes.
- Debug builds cap each letter at one page so a local `cargo run` doesn't chew
  through the portal's thousands of pages.
- The API is a separate process with its own pool and no in-memory state shared
  with a crawl. It sees committed rows only, so a reader may observe a target
  midway through its insert batches.

## 8. Configuration

| Setting | Default | Where |
|---------|---------|-------|
| `DATABASE_URL` | `postgres://postgres:postgres@localhost/halal` | env |
| `MAX_CONCURRENT` | 5 | `constants.rs` |
| Max pages/letter | 1 in debug, unlimited in release | `max_pages_per_letter()` in `constants.rs` |
| `HALAL_MAX_PAGES` | unset | env; `N` caps, `0` = unlimited |
| `API_BIND` | loopback port 8080 | env; API listen address (`ip:port`) |
| `API_CORS_ORIGINS` | `*` | env; comma-separated CORS allowlist |
| `RUST_LOG` | `info,tower_http=debug` | env; API tracing filter |

## 9. Testing strategy

- **Portal seam** — crawl tests substitute an httpmock server via
  `Portal::new(base_url)`, so pagination and dedup are tested without the live
  portal.
- **Shared fixtures** — `tests/common/mod.rs` provides `listing_html`,
  `product_listing_html`, and the `company`/`product` record constructors;
  reuse them when adding crawl tests.
- **Run orchestration** — `crawl::run` is tested end-to-end with an httpmock
  portal and a live PostgreSQL: counts, enrichment, the `scrap_log` lifecycle,
  and the single-product-source rule (a modal product is not persisted; a
  product-listing row is) are asserted through the one interface.
- **API** — `tests/api_test.rs` drives `api::router(pool)` with
  `tower::ServiceExt::oneshot` against a live PostgreSQL: pagination and
  `meta`, filters, detail/404 shape, product memberships, category counts, and
  `scrap_log` ordering.
- **DB-backed suites** — `crawl_test.rs`, `api_test.rs`, and `tests/common` need
  a live PostgreSQL; point elsewhere via `TEST_DATABASE_URL`.
- **Non-DB suites** (fast) — `parser_tests`, `records_tests`, `config_tests`,
  `constants_tests`, plus `listing`'s in-module unit tests for the pagination
  grammar and cap policy and `api::params`'s pagination unit tests.
- **Compile check** — `cargo check`; the pre-commit hook runs
  `cargo fmt -- --check` + `cargo check`.
- **CI (planned)** — there is no workflow yet. Add a PostgreSQL service job
  running `cargo fmt --check`, `cargo clippy`, `cargo test`, and the OpenAPI
  lint; this would have caught the parallel API test collision. Clear the
  existing clippy warnings before making clippy blocking.

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
| D16 | `crawl::run` owns both jobs and the `scrap_log` lifecycle | The binary becomes thin; the run's insert ordering and log open/close are testable through one interface, and failures surface in `RunReport`. | Orchestration in `main.rs` (no test seam); a closure-based `db::with_scrap_log`. |
| D17 | Records are built directly; no JSON adapter | Production parsing builds `Company`/`Product` in `parser.rs`; a `from_value`/`pick_str` adapter only ever served tests and its suite guarded no production path. Dropping it also drops the `serde_json` production dependency. | Keep a test-only adapter; promote it so the parser emits JSON. |
| D18 | axum, in a second `halal-api` binary in the same crate | Tokio-native, tower middleware, and the lib keeps the test seam. The crawler stays a batch binary; the API deploys and restarts independently. | Subcommand on the crawler (couples deploy artifacts); a separate workspace crate (overhead for one API). |
| D19 | The API uses `db::connect`, never `db::init` | The API is read-only and may run with a role that cannot create or alter tables. Schema ownership stays with the crawler; the binary fails fast if the tables are absent. | Calling `init` from the API (needs DDL privileges); a read-only flag inside `init`. |
| D20 | `/api/v1`, `{data, meta}` lists, offset pagination | A version prefix is cheap; one envelope keeps clients uniform. `page`/`per_page` with a 100 cap is simple and predictable. Keyset pagination can be added later without changing the envelope. | Unversioned paths; a bare array for lists; keyset pagination up front (clients don't need it yet). |
| D21 | Load product memberships in one batch per page | One `ANY($1)` query keeps a product page at three fixed round trips (count, page, memberships) instead of N+1 and avoids `json_agg`/`sqlx::Json`. | A join per product; `json_agg` in the list query. |
| D22 | Substring search via `ILIKE`, no trigram index yet | The dataset is small enough for a scan, and `CREATE EXTENSION` often needs privileges the deploy role lacks. Revisit with `pg_trgm` GIN indexes when query latency shows it. | Requiring `pg_trgm` at init; full-text search (different matching semantics). |
| D23 | Separate company and product jobs; only the products job writes products | Company modals enrich `companies` only. One product source removes empty-brand/`CO` duplicates, makes brand and category membership authoritative, and lets the jobs be scheduled independently. `all` keeps companies-first ordering for holder resolution. | Keeping modal products as a second source (duplicate/ambiguous rows); a staging table for unresolved holders (deferred until the product job must run fully independently). |

## 11. Forward roadmap

Each item below is a job to be checked against this document. Decisions are
proposed with alternatives noted; resolve the open ones before implementing.

### F1 — Product job scale

PR alone announces ~7,350 pages for letter "b". A full product job is
enormous.

- **Recommended:** make the product job incremental/checkpointed — record
  progress per (category, subcategory, letter, page) so a run resumes rather
  than restarts, and re-crawl only what's stale. `scrap_log` is the natural
  place to grow this.
- **Alternatives:** a blunt global page cap (simple, but silently incomplete);
  a daily-incremental strategy keyed off expiry dates.
- **Interaction:** `HALAL_MAX_PAGES` already caps per letter for smoke runs;
  don't conflate that with production completion.
- **Also:** bound page/modal task spawning (the semaphore caps requests, not
  tasks) and batch product writes — preload a holder → company map and use
  multi-row upserts instead of one lookup and category insert per row.

### F2 — Scheduling

The chosen production shape is one weekly `all` job (companies, then products).
Upserts make re-runs safe. If the product job exceeds a week, checkpoint it (F1)
or split companies weekly and products on a longer cadence.

- **Recommended:** one systemd timer running `halal-crawler all` in a container,
  with `flock`, `Persistent=true`, `TimeoutStartSec=infinity`, and journald.
- **Alternatives:** host crontab (weaker logging); separate per-job timers if
  the product cadence diverges.
- **Requirement:** journald or a durable log file, since per-letter progress
  prints to stdout.

### F3 — `company_id` resolution robustness

`resolve_company` matches holder → company by exact/case-insensitive name.
Real data will produce near-misses (punctuation, suffixes, whitespace). Because
the product job is now the only product source, an unresolved holder is a
completeness gap, not just a skipped row.

- **Recommended:** normalize both sides (trim, collapse whitespace, case-fold,
  strip common suffixes) and keep the exact-match preference; log unresolved
  holders for review.
- **Alternatives:** link via `comp_code` where the holder is itself a company;
  fuzzy matching (risk of false positives).

### F4 — Company modal coverage

Company-detail modal parsing is verified for a subset of categories.

- **Recommended:** detect an unrecognized modal layout and log it with the
  category/comp_code rather than silently returning empty fields, then add
  per-category handling as evidence arrives. Also dedup modal fetches across
  company targets: a company in `BG` and `PR` is currently fetched twice.
- **Alternatives:** one generic parser for all layouts (fragile).

### F5 — Live verification

- Verify the company job end-to-end (`halal-crawler companies`).
- Verify the product job: the `ty=PR` POST flow, page-param pagination, and
  brand/holder/expiry extraction against the live portal. It is now the only
  product source, so no products exist until this works.
- Run `cargo test` against a live PostgreSQL.

### F6 — Data completeness

- Populate the remaining NULL company columns via the modal detail pages
  (already wired; verify against real data).
- Refine state matching if real data shows misses (e.g. "W.P. Kuala Lumpur",
  "K.L.") — `STATES` + `extract_state`.
- Fix the address heuristics: `extract_state` checks the full address before
  the last comma parts (contradicting its comment), and `extract_postcode`
  takes the first five-digit part, so a street name or house number can win.
  Prefer the final components and add regression fixtures.

### F7 — API hardening

The first API cut is public, read-only, offset-paginated, and uses `ILIKE`.

- Add `pg_trgm` GIN indexes if substring search becomes slow on real data.
- Decide whether an auth layer is needed; if so, it slots in as a layer in
  `bin/halal-api.rs` without changing handlers.
- Add caching headers/ETags once consumers exist.
- Add OpenAPI generation if external clients need a formal contract.

### F8 — Product job independence

The product job resolves each `holder` against `companies`; unresolved holders
are skipped. With a single product source, those rows are lost until a later
product re-run.

- **Recommended:** keep the companies-first contract for now. If the product
  job must run fully independently, add an `unlinked_products` staging table
  and reconcile it after company refreshes.
- **Alternatives:** nullable `products.company_id` (unique-key and `NULL`
  semantics get awkward); re-crawl the whole product job after a company
  refresh (correct but expensive at product-job scale).

### F9 — Crawl-job reliability

Failures are logged but not always visible, and some error responses look like
empty data.

- **Exit status:** return non-zero when `RunReport.failures` is non-empty, so a
  scheduled job cannot look successful after a failed target.
- **HTTP status:** treat non-2xx responses as errors; retry 408/429/5xx and
  surface other 4xx instead of parsing the body as a listing.
- **Counts:** company reporting sums the listing insert and the enrichment
  upsert, counting one company as both inserted and updated. Track unique names
  or count only the final pass.
- **Observability:** persist per-target failure counts (and capped-letter
  counts) instead of relying on stderr alone.
- **CI:** add a PostgreSQL service job for `fmt`, `clippy`, tests, and the
  OpenAPI lint.
- **Housekeeping:** `.env.example` sets `HALAL_MAX_PAGES=1`; comment it out or
  label it as a smoke-test override. `migrations/0001` has a duplicate `COMMIT`.

### F10 — Production container deployment

See [DEPLOYMENT.md](DEPLOYMENT.md) for the full plan and acceptance criteria.

- **P0:** F9's failure visibility, a `migrate` command with separate crawler/API
  database roles, multi-stage `api`/`crawler` images, production compose/systemd
  with a Postgres volume, TLS, and backups.
- **Blocking risk:** a weekly `all` run may exceed seven days until the product
  job is checkpointed (F1). If it does, checkpoint or split the cadence before
  relying on weekly completion.
- **Open decisions:** managed vs container Postgres; rootless vs rootful
  Podman; existing proxy vs bundled Caddy.

## 12. Open questions & risks

- **Holder → company ambiguity.** Two companies can share a name across
  categories; `name`-unique collapses them, which may merge distinct entities.
- **Products have one source.** The company job no longer masks product-crawl
  failures. Until F5 verifies the product job, `products` stays empty; legacy
  company-modal products and `CO` memberships remain until cleaned up.
- **Product job depends on companies.** Running `products` without a recent
  `companies` job skips unresolved holders; F8's staging table is the path to
  full independence.
- **State extraction.** Substring matching over `STATES` can mis-hit; the
  current order checks the full address before the last comma parts, and
  `extract_postcode` takes the first five-digit part. F6 covers both.
- **Portal stability.** Connection drops under concurrency are expected; if
  they worsen, lower `MAX_CONCURRENT`.
- **HTTP error pages look like empty data.** `Portal::retrying` returns
  non-5xx error bodies, so a 403/404/429 page parses as zero records. F9 covers
  status handling.
- **Silent partial crawls.** A capped or failed letter logs but does not abort
  the run; `RunReport.failures` records failed targets, but `main` exits 0 and a
  capped letter is only visible in stdout. F9 covers the exit status and
  persisted failure counts; F1's checkpointing makes incompleteness visible in
  the report.
- **Company counts overstate.** The listing insert and the enrichment upsert
  both contribute to the company counts, so one company can be counted twice.
  F9 covers unique-name accounting.
- **Task explosion.** The semaphore caps in-flight requests, not task count;
  page- and company-level `JoinSet`s can hold thousands of waiting tasks. F1
  covers bounding them.
- **Company modal layout drift.** The portal can change markup per category
  without notice; F4's detection is the mitigation.
- **API is unauthenticated.** It exposes only already-public directory data,
  but anyone who can reach the port can enumerate it. Keep the bind on loopback
  unless a reverse proxy/allowlist is in front.
- **Deep offset pagination and `ILIKE` counts.** Both scan; fine at current
  scale, worth revisiting with keyset pagination and trigram indexes as data
  grows.
- **API needs the crawler's schema.** `db::connect` does not create tables; the
  binary fails fast with an actionable message, but deployment order matters.

## 13. Production deployment

Status: planned. The full plan, file layout, and acceptance criteria live in
[`DEPLOYMENT.md`](DEPLOYMENT.md).

The target is a single Linux host running two images — `halal-api` (long-lived)
and `halal-crawler` (one-shot) — with PostgreSQL on a persistent volume, a TLS
reverse proxy in front of the API, and a weekly systemd timer running the
crawler container. A one-shot `migrate` job creates/updates the schema; the
crawler and API use separate database roles.

Production is blocked on the F9 failure-visibility fixes, a `migrate` command,
checkpointing (or a longer cadence) for the product job, and the container work
in DEPLOYMENT.md. Open decisions are managed vs containerized PostgreSQL,
rootless vs rootful Podman, and existing vs bundled TLS proxy. The weekly job
runs `all`; if it exceeds a week, the fallback is checkpointing (F1) or a split
cadence.
