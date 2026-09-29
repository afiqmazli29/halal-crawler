# Category membership travels on the Product record

A `Product` now carries `category_code` and `subcategory_code` — the membership
it was discovered under — instead of `main` threading `(category, ty)` through
`listing` into `db::insert_products`. The record is self-describing;
`insert_products(records)` needs no extra parameters.

`listing` stamps the membership after parsing, because it is the module that
already holds the `CrawlTarget`. The parser stays a pure HTML extractor with no
target knowledge. Modal products (discovered under a company target) are stamped
with that target's `(category_code, "CO")`.

The in-memory dedup key in `fetch_subcategory` changed from
`(name, brand, holder, expiry_date)` to `(name, brand, holder)`. The persistence
identity is `(company_id, name, brand)`, with `holder` standing in for
`company_id`; expiry is data, not identity. The old key let two rows differing
only in expiry both survive dedup and then collide in the upsert, where the
later overwrote the earlier with no signal.

## Consequences

- `product_categories` is now written from each record's own fields and has a
  direct test asserting the mapping row.
- Which expiry wins for a duplicate `(name, brand, holder)` is still
  last-seen-wins; choosing the freshest certificate is a data-quality job
  (roadmap F6), not part of this decision.
