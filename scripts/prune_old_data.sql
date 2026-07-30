-- One-off data retention cleanup, run manually (e.g. `psql "$DATABASE_URL" -f scripts/prune_old_data.sql`).
--
-- Rules:
--   1. Delete entries older than 1 year, judged by `received_at` (server-assigned
--      on arrival) rather than `created_at` (taken from the email's Date: header
--      and forgeable by the sender — see migrations/20260728000001_hardening.sql).
--   2. Delete feeds created more than 3 months ago that ended up with fewer than
--      3 entries (run after step 1, so feeds only kept alive by now-pruned old
--      entries are swept up too). Entries are deleted before their parent feed
--      since entries.reference -> feeds.reference has no ON DELETE CASCADE.
--
-- Run inside a transaction so the whole thing can be rolled back if the
-- reported row counts look wrong.

BEGIN;

-- Step 1: prune entries older than a year.
DELETE FROM "entries"
WHERE "received_at" < now() - interval '1 year';

-- Step 2: prune feeds (and their remaining entries) that are older than 3
-- months and have fewer than 3 entries left. Computed once into a temp table
-- so the entries-delete and feed-delete below act on the exact same set.
CREATE TEMP TABLE "stale_feeds" ON COMMIT DROP AS
SELECT f."reference"
FROM "feeds" f
LEFT JOIN "entries" e ON e."reference" = f."reference"
WHERE f."created_at"::timestamptz < now() - interval '3 months'
GROUP BY f."reference"
HAVING count(e."id") < 3;

DELETE FROM "entries"
WHERE "reference" IN (SELECT "reference" FROM "stale_feeds");

DELETE FROM "feeds"
WHERE "reference" IN (SELECT "reference" FROM "stale_feeds");

-- Inspect the row counts in the output above before deciding whether to
-- COMMIT or ROLLBACK.
COMMIT;
