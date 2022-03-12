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

use crate::models::Feed;
use rusqlite::{params, Connection};
use std::error::Error;

#[derive(Debug)]
pub struct Entry {
    pub id: i64,
    pub created_at: String,
    pub reference: String,
    pub title: String,
    pub author: String,
    pub content: String,
}

impl Entry {
    /// Returns all [`Entry`] records for a given [`Feed`] reference
    pub fn find_by_reference(
        reference: &str,
        conn: &mut Connection,
    ) -> Result<Vec<Entry>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            r#"SELECT id, created_at, reference, title, author, content FROM entries
            WHERE reference = ?1"#,
        )?;
        let entries_iter = stmt.query_map(params![reference], |row| {
            Ok(Entry {
                id: row.get(0)?,
                created_at: row.get(1)?,
                reference: row.get(2)?,
                title: row.get(3)?,
                author: row.get(4)?,
                content: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for entry in entries_iter {
            entries.push(entry?);
        }

        Ok(entries)
    }

    /// Saves the [`Entry`] to the database, unless the [`Feed`] doesn't exist.
    pub fn save(&self, conn: &mut Connection) -> Result<(), Box<dyn Error>> {
        if !Feed::feed_exists(&self.reference, conn)? {
            let err: Box<dyn Error> = format!(
                "Tried saving Entry for Feed ref:{} which didn't exist",
                &self.reference
            )
            .into();
            return Err(err);
        }

        conn.execute(
            concat!(
                r#"INSERT INTO "entries" "#,
                r#"("reference", "title", "author", "content", "created_at") "#,
                r#"VALUES (?1, ?2, ?3, ?4, ?5);"#
            ),
            params![
                &self.reference,
                &self.title,
                // We don't need the address for display within the feed
                &self.author.split('<').next().unwrap_or("").trim(),
                &self.content,
                &self.created_at
            ],
        )
        .expect("Couldn't save entry in the DB!");

        Ok(())
    }
}
