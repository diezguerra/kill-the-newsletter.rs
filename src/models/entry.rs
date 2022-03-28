/*!
 * # This model works on top of the generated `entries` SQL table
 *
 * ```sql
 *    CREATE TABLE "entries" (
 *        "id" INTEGER PRIMARY KEY AUTOINCREMENT,
 *        "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
 *        "reference" TEXT NOT NULL UNIQUE,
 *        "title" TEXT NOT NULL,
 *        "author" TEXT NOT NULL,
 *        "content" TEXT NOT NULL
 *    );
 * ```
*/

use std::error::Error;
use tracing::debug;

use crate::database::{DatabaseError, Pool};
use crate::models::Feed;

#[derive(Debug)]
pub struct Entry {
    pub id: i64,
    pub created_at: String,
    pub reference: String,
    pub title: String,
    pub author: String,
    pub content: String,
}

impl std::fmt::Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"Entry(from="{}", title="{}", date="{}")"#,
            &self.author, &self.title, &self.created_at
        )
    }
}

impl Entry {
    /// Returns all [`Entry`] records for a given [`Feed`] reference
    pub async fn find_by_reference(
        reference: &str,
        pool: &Pool,
    ) -> Result<Vec<Entry>, sqlx::Error> {
        Ok(sqlx::query_as!(
            Entry,
            r#"SELECT id, created_at, reference, title, author, content
            FROM entries WHERE reference = $1 ORDER BY created_at DESC"#,
            reference
        )
        .fetch_all(pool)
        .await?)
    }

    /// Saves the [`Entry`] to the database, unless the [`Feed`] doesn't exist.
    pub async fn save(&self, pool: &Pool) -> Result<(), Box<dyn Error>> {
        if !Feed::feed_exists(&self.reference, pool).await? {
            let err: Box<dyn Error> = format!(
                "Tried saving Entry for Feed ref:{} which didn't exist",
                &self.reference
            )
            .into();
            return Err(err);
        }

        let (n_rows,): (i32,) = sqlx::query_as(
            r#"INSERT INTO "entries"
                ("reference", "title", "author", "content", "created_at")
                VALUES ($1, $2, $3, $4, $5) RETURNING changes();"#,
        )
        .bind(&self.reference)
        .bind(&self.title)
        // We don't need the address for display within the feed
        .bind(&self.author.split('<').next().unwrap_or("").trim())
        .bind(&self.content)
        .bind(&self.created_at)
        .fetch_one(pool)
        .await?;

        match n_rows {
            n_rows if n_rows > 0 => Ok(()),
            n_rows if n_rows == 0 => Ok(()),
            _ => {
                debug!(
                    "Couldn't INSERT entry:{} for ref:{}",
                    &self, &self.reference
                );
                Err(Box::new(DatabaseError::CouldNotInsert))
            }
        }
    }
}
