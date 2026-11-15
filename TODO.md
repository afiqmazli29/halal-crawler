# TODO

## P0 — correctness

- [ ] Exit non-zero when `RunReport.failures` is non-empty. A scheduled job must not report success when a target failed.
- [ ] Treat non-2xx portal responses as errors: retry 408/429/5xx, and surface other 4xx instead of parsing an error page as an empty listing.
- [ ] Fix company count reporting. The company job upserts the listing and then the enriched rows, so one company is counted as both inserted and updated; the summary and `scrap_log` overstate the total. Track unique names or count only the final pass.

## Production containers

Plan: [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)

- [ ] Finish the P0 correctness items above first: exit status, 4xx/429 handling, company counts
- [ ] Add a `halal-crawler migrate` job and separate crawler/API database roles
- [ ] Add a multi-stage Dockerfile with `api` and `crawler` targets, plus `.dockerignore`
- [ ] Add a production compose stack: Postgres volume, healthchecks, restart policies, no host DB port
- [ ] Add the weekly systemd service/timer for `all` with `flock`, `Persistent=true`, and `TimeoutStartSec=infinity`
- [ ] Add production env examples/secrets: `API_BIND`, explicit CORS, `HALAL_MAX_PAGES=0`/unset
- [ ] Add a TLS reverse proxy and API rate limiting
- [ ] Add Postgres backup/restore and test a restore
- [ ] Add CI: Postgres-backed tests, fmt/clippy, OpenAPI lint, image build/scan/push
- [ ] Decide managed vs container Postgres and rootless vs rootful Podman. The chosen cadence is one weekly `all`; if it exceeds a week, split companies weekly and products less often.

## Crawler

- [ ] Verify the company job end-to-end (`halal-crawler companies`) after the retry fix; watch modal enrichment and `│ ✗` lines.
- [ ] Product job: `fetch_subcategory` is written but never run against the live portal — verify the ty=PR POST flow, page-param pagination, and brand/holder/expiry extraction. It is now the only product source.
- [ ] Product job scale: PR alone announces ~7,350 pages for letter "b" — add checkpointing or daily-incremental crawling before a full run.
- [ ] Bound task spawning: `letter_crawl` creates a task per page and `fetch_company_modals` one per company; the semaphore caps requests, not task memory.
- [ ] Batch product writes: `insert_products` resolves the holder and inserts a category link per row. Preload a holder → company map and use multi-row upserts.
- [ ] Dedup company modal fetches across category targets; a company in `BG` and `PR` is fetched twice.
- [ ] Persist per-target failure counts (and capped-letter counts) so incomplete runs are visible in the database, not only on stderr.
- [ ] Clean up legacy phase-1 product rows and `CO` memberships in existing databases. `/categories` hides `CO`; `/products` still reads those rows.
- [ ] Add `unlinked_products` staging and reconciliation if the product job must run independently (F8).
- [ ] If the portal keeps dropping connections, lower `MAX_CONCURRENT` in `src/constants.rs` (currently 5).

## Scheduling

- [ ] One weekly `all` run: companies first, then products. Upserts already handle re-runs.
- [ ] If the product job exceeds a week, split into company weekly + product less often, or add F1 checkpointing.
- [ ] Runner: systemd timer (recommended), host crontab, or podman; logs via journald or a durable file.

## Data completeness

- [ ] Verify company enrichment against live data: phone, fax, email, website, reference no., officer.
- [ ] Holder resolution (F3): normalize names and log unresolved holders. `resolve_company` already links exact/case-insensitive matches.
- [ ] Fix address heuristics: `extract_state` checks the full address before the last comma parts (contradicting its comment), and `extract_postcode` takes the first five-digit part. Prefer the final components and add regression fixtures.
- [ ] Refine state matching for "W.P. Kuala Lumpur", "K.L." and similar.

## Testing & delivery

- [ ] Add CI with a PostgreSQL service running `cargo fmt --check`, `cargo clippy`, and `cargo test`.
- [ ] Lint `docs/api/openapi.yaml` in CI (`npx @redocly/cli lint docs/api/openapi.yaml`) if Node is available.
- [x] Run the full test suite against PostgreSQL; 71 tests pass under default parallelism.

## API

- [ ] Add `pg_trgm` GIN indexes if `ILIKE` search becomes slow on real data.
- [ ] Decide on auth or a reverse proxy/allowlist before exposing the API port.
- [ ] Serve the OpenAPI spec at `/openapi.yaml` or generate it with `utoipa` if clients need a live contract.
- [ ] Revisit keyset pagination for deep pages.
- [ ] Add ETag/Cache-Control once consumers exist.

## Housekeeping

- [ ] `.env.example` sets `HALAL_MAX_PAGES=1`; comment it out or label it as a smoke-test override so a release run isn't capped by accident.
- [ ] Fix the duplicate `COMMIT` in `migrations/0001` (the final `ALTER` runs after commit).
- [ ] Consider checking all four tables in runtime `/health`, not just at startup.
