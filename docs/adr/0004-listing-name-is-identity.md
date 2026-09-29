# The listing name is the company identity; enrichment never rewrites it

Company enrichment merges a modal detail page onto the listing record. The
listing name is the company's identity — it is the `companies` conflict key —
and enrichment never takes `name` from the modal. Every other field follows
"modal non-empty wins, listing fills gaps", extracted as
`Company::fill_from` in `records.rs`.

Previously the modal name won. Because the portal can spell a name differently
on the modal page (`"ABC Sdn Bhd"` vs `"ABC SDN. BHD."`), the enriched insert
then conflicted on a *different* name and created a second row for the same
company. `companies` is unique on `name` alone, so the duplicate was silent.

The empty-never-clobbers rule now has two explicit halves: `fill_from` handles
precedence *within a run*; the upsert's `CASE` guards handle it *across runs*
(run 2's listing pass has empty modal fields and must not wipe run 1's stored
values). Both are needed; neither replaces the other.

## Consequences

- A company whose modal name is more accurate than its listing name keeps the
  listing spelling. Normalizing or preferring the modal spelling is a
  name-resolution job (roadmap F3), not part of this decision.
