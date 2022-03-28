//!  Thread-safe SQLite connection pool instatiation
use sqlx::{Pool as SqlxPool, SqlitePool};
use thiserror::Error;

use crate::vars::DB_FILE;

pub type Pool = SqlxPool<sqlx::Sqlite>;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Couldn't INSERT row")]
    CouldNotInsert,
}

pub async fn get_db_pool() -> Result<Pool, sqlx::Error> {
    let pool = SqlitePool::connect(DB_FILE)
        .await
        .expect("Couldn't connect to the DB");

    Ok(pool)
}
