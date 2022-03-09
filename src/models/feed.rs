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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewFeed {
    pub title: String,
    pub reference: Option<String>,
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

#[derive(Template, Copy, Clone)]
#[template(path = "sentinel_entry.html", ext = "html")]
pub struct SentinelTemplate<'a> {
    pub email_domain: &'a str,
    pub reference: &'a str,
    pub title: &'a str,
    pub web_url: &'a str,
}

#[derive(Template, Copy, Clone)]
#[template(path = "created.html", ext = "html", escape = "none")]
pub struct FeedCreatedTemplate<'a> {
    pub email_domain: &'a str,
    pub reference: &'a str,
    pub title: &'a str,
    pub web_url: &'a str,
    pub entry: SentinelTemplate<'a>,
}

impl NewFeed {
    fn new_reference() -> String {
        Alphanumeric
            .sample_string(&mut rand::thread_rng(), 16)
            .to_lowercase()
    }

    pub fn save(&mut self, conn: &mut Connection) -> String {
        let reference: String = NewFeed::new_reference();
        self.reference.replace(reference.to_owned());

        conn.execute(
            concat!(
                r#"INSERT INTO "feeds" ("reference", "title") "#,
                r#"VALUES (?1, ?2);"#
            ),
            params![self.reference.as_ref().unwrap(), self.title],
        )
        .expect("Couldn't insert feed!");

        let content = SentinelTemplate {
            email_domain: EMAIL_DOMAIN,
            reference: &self.reference.as_ref().unwrap(),
            title: &self.title,
            web_url: WEB_URL,
        };
        let content = content.render().unwrap();

        let entry_title = format!("{} inbox created!", self.title);
        conn.execute(
            concat!(
                r#"INSERT INTO "entries" "#,
                r#"("reference", "title", "author", "content") "#,
                r#"VALUES (?1, ?2, ?3, ?4);"#
            ),
            params![reference, entry_title, "Kill The Newsletter", content],
        )
        .expect("Couldn't insert initial entry!");
        reference
    }

    //pub fn created_template<'a>(reference: Option<&'a str>, title: &'a str) -> FeedCreatedTemplate<'a> {
    pub fn created_template(&self) -> FeedCreatedTemplate {
        let entry = SentinelTemplate {
            email_domain: EMAIL_DOMAIN,
            reference: &self.reference.as_ref().unwrap(),
            title: &self.title,
            web_url: WEB_URL,
        };

        FeedCreatedTemplate {
            email_domain: EMAIL_DOMAIN,
            reference: &self.reference.as_ref().unwrap(),
            title: &self.title,
            web_url: WEB_URL,
            entry: entry,
        }
    }
}
