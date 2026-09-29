# One retry policy, and URL shapes behind the Portal seam

Every Portal request goes through one private `retrying` helper: it retries
transport failures and server errors three times with exponential backoff, then
surfaces the last error. `search` and `fetch_modal` both use it.

Previously only `search` retried; modal fetches used a plain GET, so a dropped
connection silently lost that company's enrichment for the run. A dead
`get_retry` existed but had no call sites. One policy removes the inconsistency
and makes the retry behaviour testable at the seam.

The seam also now owns URL construction: `fetch_modal(comp_code)` builds the
modal URL internally, and `search` sends `hdnCounter="0"` itself. Callers no
longer read `Portal::base()` or pass a `counter` that is always `"0"`. The raw
`get` is private, so the seam's interface is `init_session`, `search`, and
`fetch_modal`.

## Consequences

- A modal fetch that exhausts retries is logged and skipped, exactly as before —
  but a transient failure now recovers instead of losing the company.
- `fetch_company_modals` dropped its redundant local semaphore and
  `max_concurrent` parameter: the Portal's semaphore already bounds concurrency.
