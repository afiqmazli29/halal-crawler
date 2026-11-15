# Company and product crawls are separate jobs

The crawler previously treated companies and products as two phases of one run,
but the companies phase also scraped the company modal's **Product / Menu List**
and inserted those rows with an empty brand and a `CO` subcategory. The products
phase then inserted the same products from the category listings, with a brand
and a real subcategory. Because product identity is
`(company_id, name, brand)`, the two passes could create duplicate rows, and
every modal product also carried a `CO` membership that is not a real product
category.

The crawler is now two jobs:

- **`companies`** — company listings plus modal enrichment. It writes only
  `companies`; modal product rows are ignored.
- **`products`** — product/premise subcategory listings. It is the only writer
  of `products` and `product_categories`.
- **`all`** — companies first, then products, preserving the holder-resolution
  order.

`config` exposes `company_targets()` and `product_targets()`; `main` selects the
job from its first argument. `parse_modal` returns a `Company`, and
`fetch_company_modals` returns companies only.

## Consequences

- Product data has one source, with a brand and the real subcategory. Empty-brand
  and `CO` duplicates stop being created.
- The product job is now business-critical. It has not been verified end-to-end
  against the live portal (F5), so `products` stays empty until it is.
- Products resolve their holder against `companies`, so `all` runs companies
  first. Running `products` alone against stale companies skips unresolved
  holders; an `unlinked_products` staging table is the follow-up if the job must
  be fully independent (F8).
- `PL` has a company target but no product target, so it contributes companies
  only.
- Legacy phase-1 product rows and `CO` memberships remain in existing databases
  until cleaned up. The `/categories` endpoint hides `CO` and is seeded from
  product targets.

## Considered Options

- **Keep both writes and reconcile duplicates.** Rejected for now: matching
  modal products to listing products needs a merge key that ignores brand, plus
  cleanup of existing rows; the simpler invariant is one writer.
- **Keep phase 1 products and drop phase 2.** Rejected: modal products lack
  brand and real category membership, and coverage depends on modal layouts.
- **Stage unresolved products now.** Deferred: it makes the jobs fully
  independent but adds a table and reconciliation path before the product job
  is even live-verified (F8).
- **Make `products.company_id` nullable.** Rejected: `NULL` company ids break
  the unique identity and the API's company join; a staging table is cleaner.
