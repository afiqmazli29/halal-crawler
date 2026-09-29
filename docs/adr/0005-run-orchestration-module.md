# `crawl::run` owns the run orchestration

The two crawl phases, the listing-before-enrichment insert ordering, and each
target's `scrap_log` open/close moved out of `main.rs` into `crawl::run`. Its
interface is one call — `run(portal, pool, targets, max_pages) -> RunReport` —
and `main` is now thin: seed the session, call `run`, print the summary from the
report.

Previously this orchestration lived only in the binary, so the highest-risk
logic (the double company insert, the `scrap_log` lifecycle on every branch)
had no test seam. `run_target` now owns the log pairing, and a failing target is
recorded in `RunReport.failures` and skipped rather than aborting the run —
preserving the deliberate per-target isolation while making incompleteness
explicit instead of only printing `✗`.

`max_pages` is a parameter, not read inside `run`, so the run is deterministic
and testable; the debug/env policy stays in `constants`.

## Considered Options

- **Two functions (`run_companies` / `run_products`) called from `main`.**
  Rejected: leaves the phase order and shared report in the caller.
- **A `Crawler` struct holding portal + pool.** Rejected: no state worth
  carrying between calls; a free function is the smaller interface.
- **A closure-taking `db::with_scrap_log`.** Rejected: awkward with async and
  adds no depth over `run_target` owning the pairing.
