# A read-only HTTP API beside the crawler

The project now exposes the crawled directory over a read-only JSON API. It is
a second binary, `src/bin/halal-api.rs`, in the same crate: the router and
handlers live in `src/api/`, the binary adds tracing, CORS, and graceful
shutdown. The crawler binary is unchanged.

The API is deliberately not part of the crawl modules. Handlers read the same
tables with `sqlx::query_as` and DTOs, and never call the write paths. It is
built on axum because the project already runs on Tokio and axum's tower
middleware covers CORS, tracing, and future auth without a second runtime.
Responses use `/api/v1`; paginated lists return `{data, meta}` with
`page`/`per_page` offset pagination capped at 100, while collections and details
return `{data}`. Product memberships are loaded for a whole page in one
`ANY($1)` query rather than per row.

The API connects with a new `db::connect`, which opens a pool without creating
or migrating anything; `db::init` remains `connect` plus schema creation and is
what the crawler and DB-backed tests use. This lets the API run with a role that
cannot alter tables, and makes schema ownership explicit. The binary checks for
`companies` at startup and fails with an actionable message if the schema is
missing. `db::init` also creates the indexes the read queries need
(`companies(state)`, `products(name)`,
`product_categories(category_code, subcategory_code)`, `scrap_log(started_at
DESC)`); existing databases pick them up on the next crawler run.

## Consequences

- The crawler stays a batch process; the API can deploy, restart, and scale
  independently, with its own pool.
- The API is public and unauthenticated: no write endpoints, no credentials,
  and CORS defaults to any origin. Keep the bind on loopback unless a proxy is
  in front.
- Search is `ILIKE` for now; deep offsets and substring counts scan. Keyset
  pagination and `pg_trgm` indexes are follow-ups, not prerequisites.
- Nullable legacy columns are read through `COALESCE(col, '')` so DTOs keep a
  stable shape even when older rows contain `NULL`.

## Considered Options

- **actix-web.** Rejected: mature, but a different runtime model and more
  integration work for no benefit on an existing Tokio codebase.
- **A `serve` subcommand on the crawler binary.** Rejected: one executable, but
  it couples the batch job and the service deploy artifacts and adds CLI parsing
  to a binary that has none.
- **A separate workspace crate.** Rejected: strongest dependency isolation, but
  a lot of structure for a project this size; the lib already is the shared seam.
- **Serializing `records::Company`/`Product` directly.** Rejected: they are the
  crawler's shapes (no ids, no timestamps, no memberships); API DTOs keep the
  wire contract independent of the crawl internals.
