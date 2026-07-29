//!  PostgreSQL connection pool instantiation
use sqlx::{postgres::PgPoolOptions, Pool as SqlxPool};
use thiserror::Error;

use crate::vars::database_url;

pub type Pool = SqlxPool<sqlx::Postgres>;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Couldn't INSERT row")]
    CouldNotInsert,
}

pub async fn get_db_pool() -> Result<Pool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::new(3, 0))
        .max_connections(20)
        .connect(&database_url())
        .await
        .expect("Couldn't connect to the DB");

    Ok(pool)
}
