CREATE TABLE "feeds" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "reference" TEXT NOT NULL UNIQUE,
    "title" TEXT NOT NULL
);

CREATE TABLE "entries" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "reference" TEXT NOT NULL UNIQUE,
    "feed_id" INTEGER NOT NULL REFERENCES "feeds",
    "title" TEXT NOT NULL,
    "author" TEXT NOT NULL,
    "content" TEXT NOT NULL
);

CREATE INDEX "entriesFeed" ON "entries" ("feed");