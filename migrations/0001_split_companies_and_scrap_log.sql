-- Migration: split companies into company-only data + a scrap_log run log.
--
-- companies loses category_code (moves to scrap_log) and scraped_at
-- (replaced by created_at/updated_at); it becomes unique on (name) alone.
-- products loses scraped_at (kept as created_at/updated_at).
--
-- Note: this assumes no rows are foreign-key-referenced in a way that must be
-- preserved (products.company_id is currently unused). Run once against an
-- existing DB; the app creates the new schema from scratch on a fresh DB.

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

-- products: drop the scrape timestamp (kept as created_at/updated_at)
ALTER TABLE products DROP COLUMN IF EXISTS scraped_at;

COMMIT;

