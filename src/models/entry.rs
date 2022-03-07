/*
    CREATE TABLE "entries" (
        "id" INTEGER PRIMARY KEY AUTOINCREMENT,
        "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        "reference" TEXT NOT NULL UNIQUE,
        "title" TEXT NOT NULL,
        "author" TEXT NOT NULL,
        "content" TEXT NOT NULL
    );
*/

use rusqlite::{params, Connection};

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
    pub fn find_by_reference(
        reference: &String,
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
}
