//!  Thread-safe SQLite connection pool instatiation
use sqlx::{postgres::PgPoolOptions, Pool as SqlxPool};
use thiserror::Error;

use crate::vars::DATABASE_URL;

pub type Pool = SqlxPool<sqlx::Postgres>;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Couldn't INSERT row")]
    CouldNotInsert,
}

pub async fn get_db_pool() -> Result<Pool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(DATABASE_URL)
        .await
        .expect("Couldn't connect to the DB");

    Ok(pool)
}
