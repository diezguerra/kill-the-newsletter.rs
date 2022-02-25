/*
    CREATE TABLE "entries" (
        "id" INTEGER PRIMARY KEY AUTOINCREMENT,
        "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        "reference" TEXT NOT NULL UNIQUE,
        "feed" INTEGER NOT NULL REFERENCES "feeds",
        "title" TEXT NOT NULL,
        "author" TEXT NOT NULL,
        "content" TEXT NOT NULL
    );
*/

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub created_at: String,
    pub reference: String,
    pub feed: u64,
    pub title: String,
    pub author: String,
    pub content: String,
}
