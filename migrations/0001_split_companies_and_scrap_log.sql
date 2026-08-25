-- Migration: split companies into company-only data + a scrap_log run log,
-- and move products' category/subcategory columns into a product_categories
-- mapping table.
--
-- companies loses category_code (moves to scrap_log) and scraped_at
-- (replaced by created_at/updated_at); it becomes unique on (name) alone.
-- products loses category_code, subcategory_code, and scraped_at
-- (kept as company_id FK, created_at/updated_at); unique on (company_id, name, brand).
-- product_categories: new mapping table (product_id, category_code, subcategory_code).
--
-- Note: this assumes no rows are foreign-key-referenced in a way that must be
-- preserved. Run once against an existing DB; the app creates the new schema
-- from scratch on a fresh DB.

BEGIN;

-- scrap_log: new table tracking scrape runs per category+phase.
CREATE TABLE IF NOT EXISTS scrap_log (
    id BIGSERIAL PRIMARY KEY,
    category_code TEXT NOT NULL,
    phase TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    inserted_count INT NOT NULL DEFAULT 0,
    updated_count INT NOT NULL DEFAULT 0
);

-- companies: drop old (category_code, name) key, then category + scrape columns
ALTER TABLE companies DROP CONSTRAINT IF EXISTS companies_category_code_name_key;
ALTER TABLE companies DROP COLUMN IF EXISTS category_code;
ALTER TABLE companies DROP COLUMN IF EXISTS scraped_at;
-- Only add the unique(name) constraint if it doesn't already exist (e.g. if
-- the app's init already created it on a fresh DB).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'companies_name_key' AND conrelid = 'companies'::regclass
    ) THEN
        ALTER TABLE companies ADD CONSTRAINT companies_name_key UNIQUE (name);
    END IF;
END
$$;

-- products: extract category/subcategory into product_categories, then drop
-- them from products. Also drop the old scrape timestamp.
-- product_categories must exist before we migrate data into it.
CREATE TABLE IF NOT EXISTS product_categories (
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    category_code TEXT NOT NULL,
    subcategory_code TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (product_id, category_code, subcategory_code)
);

DO $$
DECLARE
    prod_has_cat boolean;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'products' AND column_name = 'category_code'
    ) INTO prod_has_cat;

    IF prod_has_cat THEN
        -- Migrate existing rows: create product_categories entries from
        -- the old (product, category_code, subcategory_code) associations.
        INSERT INTO product_categories (product_id, category_code, subcategory_code)
        SELECT id, category_code, subcategory_code
        FROM products
        WHERE category_code IS NOT NULL AND subcategory_code IS NOT NULL
        ON CONFLICT DO NOTHING;
    END IF;
END
$$;

-- Drop the old product category/subcategory columns and scrape timestamp.
ALTER TABLE products DROP COLUMN IF EXISTS category_code;
ALTER TABLE products DROP COLUMN IF EXISTS subcategory_code;

-- Add the new unique constraint on (company_id, name, brand) if it doesn't exist.
-- This is normally created by the app's CREATE TABLE on a fresh DB; existing
-- tables need it added explicitly.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'products_company_id_name_brand_key'
        AND conrelid = 'products'::regclass
    ) THEN
        ALTER TABLE products ADD CONSTRAINT products_company_id_name_brand_key UNIQUE (company_id, name, brand);
    END IF;
END
$$;

COMMIT;
ALTER TABLE products DROP COLUMN IF EXISTS scraped_at;

COMMIT;

