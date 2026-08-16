# Halal Directory Crawler

Scrapes the Malaysian Halal Portal's public directory — company listings and product/premise listings — into PostgreSQL.

## Language

**Portal**:
The Malaysian Halal Portal website the crawler reads — owns the base URL, PHP session, and request protocol (category/ty pairs, letter search, hdnCounter pagination).
_Avoid_: website, site, endpoint

**Listing**:
One page of directory search results — a set of records plus pagination state.
_Avoid_: page, results page

**Category**:
A halal certification category on the portal, e.g. `PR` (Produk Makanan/Minuman), `BG` (Barang Gunaan).
_Avoid_: section, type

**Subcategory**:
A listing type within a category — a (category, ty) pair on the portal, e.g. `PR`+`CO` (companies) vs `PR`+`PR` (products).
_Avoid_: kind, mode

**Company**:
A certified company listing record: name, address, postcode, state.
_Avoid_: business, firm, Syarikat

**Product**:
A certified product or premise listing record: name, brand, certificate holder, expiry date.
_Avoid_: item, listing entry

**Record**:
A typed Company or Product extracted from a listing — the shape every module hands along.
_Avoid_: row, JSON

**Crawl**:
One category's full sweep — all letters, all pages — producing records.
_Avoid_: scrape run, job

**hdnCounter**:
The portal's hidden form field that drives company-listing pagination; subcategory listings instead advance on the page parameter with the total page count from page one.
_Avoid_: cursor, offset
