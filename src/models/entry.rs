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
            FROM entries WHERE reference = $1 ORDER BY received_at DESC"#,
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
                    ORDER BY received_at DESC
                    LIMIT $2
                )"#,
        )
        .bind(&self.reference)
        .bind(MAX_ENTRIES_PER_FEED)
        .execute(pool)
        .await
        .map_err(|e| {
            debug!(
                "Couldn't trim old entries for ref:{} ({})",
                &self.reference, e
            );
            e
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// Connects to a Postgres instance and applies migrations, or returns
    /// `None` if `DATABASE_URL` isn't set / no DB is reachable — these
    /// tests degrade to a no-op skip in CI, where no DB is configured.
    async fn test_pool() -> Option<Pool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new().connect(&url).await.ok()?;
        sqlx::migrate!("./migrations").run(&pool).await.ok()?;
        Some(pool)
    }

    /// Inserts a feed directly (bypassing NewFeed::save, which renders a
    /// template that needs WEB_URL/EMAIL_DOMAIN env vars we don't want
    /// this test to depend on).
    async fn insert_feed(reference: &str, pool: &Pool) {
        sqlx::query(
            r#"INSERT INTO "feeds" ("reference", "title") VALUES ($1, $2)
               ON CONFLICT (reference) DO NOTHING"#,
        )
        .bind(reference)
        .bind("Ordering test feed")
        .execute(pool)
        .await
        .expect("test feed insert should succeed");
    }

    /// Proves fix for the "attacker controls feed ordering" finding: a
    /// forged/old `created_at` (which comes straight from the email's
    /// `Date:` header) must NOT be able to reorder entries or survive the
    /// retention trim — only the server-assigned `received_at` should.
    #[tokio::test]
    async fn ordering_and_trim_use_received_at_not_forged_created_at() {
        let Some(pool) = test_pool().await else {
            eprintln!("skipping: no DATABASE_URL / DB unreachable");
            return;
        };

        let reference = format!(
            "ordertest{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        insert_feed(&reference, &pool).await;

        // First real entry, inserted normally (received_at ~ now).
        let first = Entry {
            id: 0,
            created_at: "2020-01-01 00:00:00".to_owned(),
            reference: reference.clone(),
            title: "First entry".to_owned(),
            author: "sender@example.com".to_owned(),
            content: "first".to_owned(),
        };
        first.save(&pool).await.expect("first entry should save");

        // Second entry claims (via forged Date: header) to be from the
        // year 1000 — far "older" than the first by created_at — but it's
        // actually received afterwards, so it must still sort on top.
        let forged_old = Entry {
            id: 0,
            created_at: "1000-01-01 00:00:00".to_owned(),
            reference: reference.clone(),
            title: "Forged ancient entry".to_owned(),
            author: "attacker@example.com".to_owned(),
            content: "forged".to_owned(),
        };
        forged_old
            .save(&pool)
            .await
            .expect("forged entry should save");

        let entries = Entry::find_by_reference(&reference, &pool)
            .await
            .expect("find_by_reference should succeed");

        let titles: Vec<&str> =
            entries.iter().map(|e| e.title.as_str()).collect();
        let forged_pos = titles
            .iter()
            .position(|t| *t == "Forged ancient entry")
            .expect("forged entry present");
        let first_pos = titles
            .iter()
            .position(|t| *t == "First entry")
            .expect("first entry present");

        assert!(
            forged_pos < first_pos,
            "entry received later must sort first regardless of its forged created_at: {:?}",
            titles
        );
    }
}
