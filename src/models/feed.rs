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
use crate::vars::{email_domain, web_url};

/// Form input type — only contains fields submitted by the user.
#[derive(Debug, Deserialize)]
pub struct CreateFeedForm {
    pub title: String,
}

/// Internal domain type used to create and render a new feed.
/// `reference` is `None` before saving and `Some` after.
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
}

#[derive(Template, Clone)]
#[template(path = "sentinel_entry.html", ext = "html")]
pub struct SentinelTemplate {
    pub email_domain: String,
    pub reference: String,
    pub title: String,
    pub web_url: String,
}

#[derive(Template, Clone)]
#[template(path = "created.html", ext = "html", escape = "none")]
#[allow(dead_code)]
pub struct FeedCreatedTemplate {
    pub email_domain: String,
    pub reference: String,
    pub title: String,
    pub web_url: String,
    pub entry: SentinelTemplate,
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

        let content = SentinelTemplate {
            email_domain: email_domain(),
            reference: reference.clone(),
            title: self.title.clone(),
            web_url: web_url(),
        }
        .render()
        .map_err(|e| {
            debug!("Couldn't render SentinelTemplate: {}", e);
            Box::new(DatabaseError::CouldNotInsert) as Box<dyn Error>
        })?;

        let entry_title = format!("{} inbox created!", self.title);

        let mut tx = pool.begin().await?;

        sqlx::query(
            r#"INSERT INTO "feeds" ("reference", "title") VALUES ($1, $2)"#,
        )
        .bind(&reference)
        .bind(&self.title)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            debug!(
                "Couldn't INSERT feed ref:{} title:{} ({})",
                &reference, &self.title, e
            );
            Box::new(DatabaseError::CouldNotInsert) as Box<dyn Error>
        })?;

        sqlx::query(
            r#"INSERT INTO "entries" ("reference", "title", "author", "content") VALUES ($1, $2, $3, $4)"#,
        )
        .bind(&reference)
        .bind(entry_title)
        .bind("Kill The Newsletter")
        .bind(content)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            debug!(
                "Couldn't INSERT sentinel entry ref:{} ({})",
                &reference, e
            );
            Box::new(DatabaseError::CouldNotInsert) as Box<dyn Error>
        })?;

        tx.commit().await?;

        Ok(reference)
    }

    pub fn created_template(&self) -> FeedCreatedTemplate {
        let reference = self.reference.as_ref().unwrap().clone();
        let entry = SentinelTemplate {
            email_domain: email_domain(),
            reference: reference.clone(),
            title: self.title.clone(),
            web_url: web_url(),
        };

        FeedCreatedTemplate {
            email_domain: email_domain(),
            reference,
            title: self.title.clone(),
            web_url: web_url(),
            entry,
        }
    }
}
