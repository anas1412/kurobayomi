-- Kurobayomi native step 139 -> 140: indexes on the category links (T-04a-036).
-- The runner sets user_version = 140 after this step. IF NOT EXISTS keeps it idempotent: a fresh
-- file gets the same indexes from schema.sql before the migration list runs.
CREATE INDEX IF NOT EXISTS animes_categories_anime_id_index ON animes_categories(anime_id);
CREATE INDEX IF NOT EXISTS animes_categories_category_id_index ON animes_categories(category_id);
