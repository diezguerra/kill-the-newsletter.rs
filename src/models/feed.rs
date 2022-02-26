/*

      CREATE TABLE "feeds" (
        "id" INTEGER PRIMARY KEY AUTOINCREMENT,
        "createdAt" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        "updatedAt" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        "reference" TEXT NOT NULL UNIQUE,
        "title" TEXT NOT NULL
      );

*/
use dotenv_codegen::dotenv;
use rand::distributions::{Alphanumeric, DistString};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

const WEB_URL: &str = dotenv!("WEB_URL");
const EMAIL_DOMAIN: &str = dotenv!("EMAIL_DOMAIN");

fn new_reference() -> String {
    Alphanumeric
        .sample_string(&mut rand::thread_rng(), 16)
        .to_lowercase()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewFeed {
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Feed {
    pub id: u64,
    pub created_at: String,
    pub updated_at: String,
    pub reference: String,
    pub title: String,
}

impl NewFeed {
    pub fn save(&self, conn: &mut Connection) -> String {
        let reference: String = new_reference();

        conn.execute(
            concat!(
                r#"INSERT INTO "feeds" ("reference", "title") "#,
                r#"VALUES (?1, ?2);"#
            ),
            params![reference, self.title],
        )
        .expect("Couldn't insert feed!");

        let title = format!("{} inbox created!", self.title);
        let content = format!(
            r#"
        <p>
          Sign up for the newsletter with<br />
          <code class="copyable">{reference}@${email_domain}</code>
        </p>
        <p>
          Subscribe to the Atom feed at<br />
          <code class="copyable">{web_url}/feeds/{reference}.xml</code>
        </p>
        <p>
          <strong>Don’t share these addresses.</strong><br />
          They contain an identifier that other people could use
          to send you spam and to control your newsletter subscriptions.
        </p>
        <p><strong>Enjoy your readings!</strong></p>
        <p>
          <a href="{web_url}/"><strong>Create another inbox</strong></a>
        </p>
      "#,
            web_url = WEB_URL,
            reference = reference,
            email_domain = EMAIL_DOMAIN
        );

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
