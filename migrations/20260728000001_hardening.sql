-- Security/design hardening pass:
--   * Add server-side received_at so ordering and retention trimming aren't
--     gameable via a forged email Date: header.
--   * Add explicit primary keys (SERIAL alone gives no uniqueness/PK).
--   * Drop the "feedsRef" index, an exact duplicate of "entriesRef" (both
--     defined on entries.reference).
--   * Add a composite index matching the actual hot query shape.

ALTER TABLE "feeds" ADD PRIMARY KEY ("id");
ALTER TABLE "entries" ADD PRIMARY KEY ("id");

ALTER TABLE "entries"
    ADD COLUMN "received_at" TIMESTAMPTZ NOT NULL DEFAULT now();

DROP INDEX IF EXISTS "feedsRef";

CREATE INDEX IF NOT EXISTS "entriesReferenceReceivedAt"
    ON "entries" ("reference", "received_at" DESC);
