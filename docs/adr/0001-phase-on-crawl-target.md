# Carry Phase on the Crawl target

A Crawl target is a (category, ty) pair plus its Phase. `config::targets()`
returns one list covering both phases; the phase is an enum (`Phase::Companies`
/ `Phase::Products`) rather than a string literal at the call site.

We chose one list over the previous `company_strategies()` /
`other_strategies()` pair because the same target set was hand-written twice and
the phase that scheduled each target lived as a bare string in `main.rs`. One
list makes "what runs" readable from a single module, and the enum removes the
drift between the phase used for scheduling and the value persisted in
`scrap_log.phase`.

Display names (`"Barang Gunaan"`, `"Produk"`, …) moved out of the target type
into `config::label`; they are presentation, not crawl facts, and no crawl
consumer needs them.

## Considered Options

- **Keep two lists.** Rejected: duplicates the target set and leaves phase as a
  string literal.
- **Keep `category_name` / `sub_name` on the target.** Rejected: widens the
  interface every crawl consumer must learn for fields only progress output
  reads.
- **Rename the persisted `"products"` phase value to `"subcategories"`.**
  Rejected for now: it is a behavior change outside this decision, and
  `scrap_log` already holds `"products"` rows.
