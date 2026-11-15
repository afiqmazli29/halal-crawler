# TODO

## Crawler

- [ ] Verify the company job end-to-end (`halal-crawler companies`) after the retry fix
- [ ] Product job: `fetch_subcategory` is written but never run against the live portal — verify the ty=PR POST flow, page-param pagination, and brand/holder/expiry extraction. It is now the only product source, so this must be verified before relying on `products`
- [ ] Product job scale: PR alone announces ~7,350 pages for letter "b" — add checkpointing or a daily-incremental strategy before running it in full
- [ ] If the portal keeps dropping connections, lower `MAX_CONCURRENT` in `src/constants.rs` (currently 5)
- [ ] Clean up legacy phase-1 product rows and `CO` memberships in existing databases; `/categories` hides `CO`, but `/products` still reads those rows until they are removed
- [ ] If the product job must run independently of the company job, add an `unlinked_products` staging table and reconciliation (F8)

## Scheduling

- [ ] Weekly cron job to run the crawler and insert new entries
  - Upserts already handle re-runs (`ON CONFLICT ... DO UPDATE`), so a weekly run refreshes changed rows and adds new ones without duplicates
  - Decide the runner: host crontab, systemd timer, or podman container
  - Log output somewhere persistent (`>> /var/log/halal-crawler.log 2>&1`) since per-letter progress prints to stdout

## Data completeness

- [ ] Populate the NULL company columns (`phone_no`, `fax_no`, `email`, `website`, `reference_no`, `officer`) by fetching certificate detail pages (`directory/slm_viewdetail.php` links in listing rows)
- [ ] Resolve `products.company_id` — link the certificate `holder` back to a `companies.id`
- [ ] Refine state matching if real data shows misses (e.g. "W.P. Kuala Lumpur", "K.L.") — `STATES` + `extract_state` in `parser.rs`

## Tests

- [ ] Run `cargo test` against a live PostgreSQL after the retry changes

## API

- [ ] Add `pg_trgm` GIN indexes if `ILIKE` search becomes slow on real data
- [ ] Decide on auth or a reverse proxy/allowlist before exposing the API port
- [ ] Add OpenAPI generation if external clients need a formal contract
- [ ] Revisit keyset pagination for deep pages
