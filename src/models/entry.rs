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

use tracing::debug;

use crate::database::Pool;

#[derive(Debug, sqlx::FromRow)]
pub struct Entry {
    pub id: i32,
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
        sqlx::query_as::<_, Entry>(
            r#"SELECT id, created_at, reference, title, author, content
            FROM entries WHERE reference = $1 ORDER BY created_at DESC"#,
        )
        .bind(reference)
        .fetch_all(pool)
        .await
    }

    /// Saves the [`Entry`] to the database.
    /// Returns an error if the feed reference doesn't exist (FK violation).
    /// Also trims the oldest entries beyond `MAX_ENTRIES_PER_FEED` for this feed.
    pub async fn save(&self, pool: &Pool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO "entries"
                ("reference", "title", "author", "content", "created_at")
                VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(&self.reference)
        .bind(&self.title)
        // We don't need the address for display within the feed
        .bind(self.author.split('<').next().unwrap_or("").trim())
        .bind(&self.content)
        .bind(&self.created_at)
        .execute(pool)
        .await
        .map_err(|e| {
            debug!(
                "Couldn't INSERT entry:{} for ref:{} ({})",
                &self, &self.reference, e
            );
            e
        })?;

        // Keep at most MAX_ENTRIES_PER_FEED entries per feed, deleting the oldest.
        const MAX_ENTRIES_PER_FEED: i64 = 100;
        sqlx::query(
            r#"DELETE FROM "entries"
                WHERE reference = $1
                AND id NOT IN (
                    SELECT id FROM "entries"
                    WHERE reference = $1
                    ORDER BY created_at DESC
                    LIMIT $2
                )"#,
        )
        .bind(&self.reference)
        .bind(MAX_ENTRIES_PER_FEED)
        .execute(pool)
        .await
        .map_err(|e| {
            debug!("Couldn't trim old entries for ref:{} ({})", &self.reference, e);
            e
        })?;

        Ok(())
    }
}
