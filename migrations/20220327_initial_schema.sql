PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS "feeds" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "reference" TEXT NOT NULL UNIQUE,
    "title" TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS "entries" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "reference" TEXT NOT NULL,
    "title" TEXT NOT NULL,
    "author" TEXT NOT NULL,
    "content" TEXT NOT NULL,
    FOREIGN KEY(reference) REFERENCES feeds(reference)
);

CREATE INDEX IF NOT EXISTS "entriesFeed" ON "entries" ("feed");