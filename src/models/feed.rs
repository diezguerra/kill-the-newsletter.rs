/*

      CREATE TABLE "feeds" (
        "id" INTEGER PRIMARY KEY AUTOINCREMENT,
        "createdAt" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        "updatedAt" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        "reference" TEXT NOT NULL UNIQUE,
        "title" TEXT NOT NULL
      );

*/
use askama::Template;
use rand::distributions::{Alphanumeric, DistString};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::vars::{EMAIL_DOMAIN, WEB_URL};

#[derive(Debug, Serialize, Deserialize)]
pub struct NewFeed {
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Feed {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub reference: String,
    pub title: String,
}

impl Feed {
    pub fn get_title_given_reference(
        reference: &String,
        conn: &mut Connection,
    ) -> Result<String, rusqlite::Error> {
        let mut stmt =
            conn.prepare(r#"SELECT title FROM feeds WHERE reference = ?1"#)?;
        let row = stmt.query_row(params![reference], |row| row.get(0))?;

        Ok(row)
    }
}

#[derive(Template)]
#[template(path = "sentinel_entry.html")]
struct SentinelTemplate<'a> {
    web_url: &'a str,
    reference: &'a str,
    email_domain: &'a str,
}

impl NewFeed {
    fn new_reference() -> String {
        Alphanumeric
            .sample_string(&mut rand::thread_rng(), 16)
            .to_lowercase()
    }

    pub fn save(&self, conn: &mut Connection) -> String {
        let reference: String = NewFeed::new_reference();

        conn.execute(
            concat!(
                r#"INSERT INTO "feeds" ("reference", "title") "#,
                r#"VALUES (?1, ?2);"#
            ),
            params![reference, self.title],
        )
        .expect("Couldn't insert feed!");

        let title = format!("{} inbox created!", self.title);
        let content = SentinelTemplate {
            web_url: WEB_URL,
            email_domain: EMAIL_DOMAIN,
            reference: &reference,
        };
        let content = content.render().unwrap();

        conn.execute(
            concat!(
                r#"INSERT INTO "entries" "#,
                r#"("reference", "title", "author", "content") "#,
                r#"VALUES (?1, ?2, ?3, ?4);"#
            ),
            params![reference, title, "Kill The Newsletter", content],
        )
        .expect("Couldn't insert initial entry!");
        reference
    }
}
