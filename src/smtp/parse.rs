//! # Mail Parsing module
//!
//! A bunch of boilerplate to use the `mailparse` crate and extract content,
//! including HTML if multipart email is found.
//!
//! Some fun with Traits, for good measure.

use mailparse::{dateparse, parse_mail, MailHeaderMap};
use tracing::debug;

use crate::time::Epoch;

/// Output struct for the SMTP server, containing all the goodies
pub struct ParsedEmail {
    pub to: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub body: String,
}

impl std::fmt::Display for ParsedEmail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            concat!(
                r#"ParsedEmail {{ to: {}, from: {}, subject: {}, date: {},"#,
                r#"" body[..50]: {} }}"#
            ),
            &self.to,
            &self.from,
            &self.subject,
            &self.date,
            if self.body.len() > 50 {
                &self.body[..50]
            } else {
                &self.body
            }
        )
    }
}

pub fn parse(email: &[u8]) -> ParsedEmail {
    let parsed = parse_mail(email).unwrap();

    let subject = parsed
        .headers
        .get_first_value("Subject")
        .unwrap_or_else(|| "No subject".to_owned());

    let to = parsed
        .headers
        .get_first_value("To")
        .unwrap_or_else(|| "unknown@recipient.mail".to_owned());

    let from = parsed
        .headers
        .get_first_value("From")
        .unwrap_or_else(|| "unknown@sender.mail".to_owned());

    let mut body = String::new();

    // Get the HTML version or the first one if that one isn't found
    if !parsed.subparts.is_empty() {
        for part in 0..parsed.subparts.len() {
            if parsed.subparts[part]
                .ctype
                .mimetype
                .starts_with("text/html")
            {
                body.push_str(&parsed.subparts[part].get_body().unwrap());
            }
        }
        if body.is_empty() {
            body.push_str(&parsed.subparts[0].get_body().unwrap());
        }
    } else {
        body.push_str(&parsed.get_body().unwrap());
    }

    let date = Epoch::from(
        dateparse(
            parsed
                .headers
                .get_first_value("Date")
                .unwrap_or_else(|| "".to_owned())
                .as_str(),
        )
        .unwrap_or(0),
    )
    .to_string();

    debug!("Parsed date: {:#?}", date);

    ParsedEmail {
        to,
        from,
        subject,
        date,
        body,
    }
}
