//! # This model works on top of the generated `feeds` SQL table
//!
//! ```sql
//!     CREATE TABLE "feeds" (
//!       "id" INTEGER PRIMARY KEY AUTOINCREMENT,
//!       "createdAt" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
//!       "updatedAt" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
//!       "reference" TEXT NOT NULL UNIQUE,
//!       "title" TEXT NOT NULL
//!     );
//! ```

use askama::Template;
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use std::error::Error;
use tracing::debug;

use crate::database::{DatabaseError, Pool};
use crate::vars::{EMAIL_DOMAIN, WEB_URL};

/// A helper Struct to pass on to Axum so it can deserialize a form submission
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewFeed {
    pub title: String,
    pub reference: Option<String>,
}

/// Represents an individual feed and its related email address and title.
#[derive(Debug, Serialize, Deserialize)]
pub struct Feed {
    pub id: i32,
    pub created_at: String,
    pub updated_at: String,
    /// The `reference` of a [`Feed`] is a randomly generated alphanumeric
    /// string that is used as the email recipient and the unique ID of each
    /// [`Feed`].
    pub reference: String,
    pub title: String,
}

impl Feed {
    /// Returns a [`Feed`]'s `title` given its `reference`.
    pub async fn get_title_given_reference(
        reference: &str,
        pool: &Pool,
    ) -> Result<String, sqlx::Error> {
        let (title,): (String,) =
            sqlx::query_as("SELECT title FROM feeds WHERE reference = $1")
                .bind(reference)
                .fetch_one(pool)
                .await?;

        Ok(title)
    }

    /// Checks whether a [`Feed`] exists given its `reference`.
    pub async fn feed_exists(
        reference: &str,
        pool: &Pool,
    ) -> Result<bool, sqlx::Error> {
        let feed_count: i64 = sqlx::query_scalar(
            "SELECT count(id) FROM feeds WHERE reference = $1",
        )
        .bind(reference)
        .fetch_one(pool)
        .await?;

        match feed_count {
            0 => Ok(false),
            _ => Ok(true),
        }
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

    pub async fn save(
        &mut self,
        pool: &Pool,
    ) -> Result<String, Box<dyn Error>> {
        let reference = self
            .reference
            .get_or_insert_with(NewFeed::new_reference)
            .to_owned();

        let _inserted: i64 = match sqlx::query_as(
            r#"WITH inserted AS (
                INSERT INTO "feeds" ("reference", "title") VALUES ($1, $2)
            RETURNING 1) SELECT COUNT(*) FROM inserted;"#,
        )
        .bind(self.reference.as_ref().unwrap())
        .bind(&self.title)
        .fetch_one(pool)
        .await
        {
            Ok(tup) => {
                if matches!(tup, (_inserted,)) && tup.0 > 0 {
                    tup.0
                } else {
                    return Err(Box::new(DatabaseError::CouldNotInsert));
                }
            }
            Err(e) => {
                debug!(
                    "Couldn't INSERT feed ref:{:?} title:{} ({})",
                    &self.reference, &self.title, e
                );
                return Err(Box::new(DatabaseError::CouldNotInsert));
            }
        };

        let content = SentinelTemplate {
            email_domain: EMAIL_DOMAIN,
            reference: self.reference.as_ref().unwrap(),
            title: &self.title,
            web_url: WEB_URL,
        };
        let content = content.render().unwrap();

        let entry_title = format!("{} inbox created!", self.title);

        let (n_rows,): (i64,) = sqlx::query_as(concat!(
            r#"WITH inserted AS (
                INSERT INTO "entries"
                ("reference", "title", "author", "content")
                VALUES ($1, $2, $3, $4) RETURNING 1)
                SELECT count(*) FROM inserted;"#
        ))
        .bind(self.reference.as_ref().unwrap())
        .bind(entry_title)
        .bind("Kill The Newsletter")
        .bind(content)
        .fetch_one(pool)
        .await?;

        match n_rows {
            n_rows if n_rows > 0 => Ok(reference),
            _ => {
                debug!(
                    "Couldn't INSERT entry ref:{:?} title:{}",
                    &self.reference, &self.title
                );
                Err(Box::new(DatabaseError::CouldNotInsert))
            }
        }
    }

    pub fn created_template(&self) -> FeedCreatedTemplate {
        let entry = SentinelTemplate {
            email_domain: EMAIL_DOMAIN,
            reference: self.reference.as_ref().unwrap(),
            title: &self.title,
            web_url: WEB_URL,
        };

        FeedCreatedTemplate {
            email_domain: EMAIL_DOMAIN,
            reference: self.reference.as_ref().unwrap(),
            title: &self.title,
            web_url: WEB_URL,
            entry,
        }
    }
}
