# TODO

## Crawler

- [ ] Verify live crawl end-to-end after the retry fix (`cargo run`) — watch for `│ ✗` per-letter lines and the final counts
- [ ] Products crawl (phase 2): `fetch_subcategory` is written but never run against the live portal — verify the ty=PR POST flow, page-param pagination, and brand/holder/expiry extraction, and fix whatever breaks
- [ ] Phase 2 scale: PR alone announces ~7,350 pages for letter "b" — consider a page cap or a daily-incremental strategy before running phase 2 in full
- [ ] If the portal keeps dropping connections, lower `MAX_CONCURRENT` in `src/constants.rs` (currently 5)

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
