//!  Thread-safe SQLite connection pool instatiation
use r2d2::ManageConnection;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

use crate::vars::DB_FILE;

const MIGRATION: &str = r#"
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

    CREATE INDEX IF NOT EXISTS "entriesFeed" ON "entries" ("feed");"#;

/// Runs the migration script every time a pool is created, but the three
/// CREATE statements in it only run when the tables and index don't yet exist
fn populate_if_needed(mngr: &SqliteConnectionManager) {
    mngr.connect()
        .unwrap()
        .execute_batch(MIGRATION)
        .expect("Couldn't run initial migration");
}

pub fn get_db_pool() -> Arc<r2d2::Pool<SqliteConnectionManager>> {
    let sqlite_connection_manager = SqliteConnectionManager::file(DB_FILE);
    //let sqlite_connection_manager = SqliteConnectionManager::memory();

    populate_if_needed(&sqlite_connection_manager);

    let sqlite_pool = r2d2::Pool::new(sqlite_connection_manager)
        .expect("Failed to create r2d2 SQLite connection pool");
    Arc::new(sqlite_pool)
}
