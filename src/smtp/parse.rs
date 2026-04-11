//! # Mail Parsing module
//!
//! A bunch of boilerplate to use the `mailparse` crate and extract content,
//! including HTML if multipart email is found.
//!
//! Some fun with Traits, for good measure.

use mailparse::{dateparse, parse_mail, MailHeaderMap};
use regex::Regex; // used via LazyLock below
use std::sync::LazyLock;
use tracing::{debug, warn};

use crate::models::Entry;
use crate::smtp::app::Email;
use crate::time::Epoch;
use crate::vars::email_domain;

// Yanked blindly from https://emailregex.com/
static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r#"(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)"#,
        r#"*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-"#,
        r#"\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\"#,
        r#".)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:25[0-5]|2[0-4][0-9]|[0"#,
        r#"1]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?|[a-z"#,
        r#"0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|"#,
        r#"\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])"#
    ))
    .expect("EMAIL_REGEX is a valid regex")
});

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

/// Takes the slice of unsigned bytes that is the email DATA body and returns
/// a parsed struct of type `ParsedEmail`
fn parse_bytes_to_email(email: &[u8]) -> Result<ParsedEmail, String> {
    let parsed = parse_mail(email)
        .map_err(|e| format!("Failed to parse email: {}", e))?;

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
        for part in &parsed.subparts {
            if part.ctype.mimetype.starts_with("text/html") {
                match part.get_body() {
                    Ok(b) => body.push_str(&b),
                    Err(e) => warn!("Failed to decode HTML part: {}", e),
                }
            }
        }
        if body.is_empty() {
            match parsed.subparts[0].get_body() {
                Ok(b) => body.push_str(&b),
                Err(e) => warn!("Failed to decode first part: {}", e),
            }
        }
    } else {
        match parsed.get_body() {
            Ok(b) => body.push_str(&b),
            Err(e) => warn!("Failed to decode email body: {}", e),
        }
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

    Ok(ParsedEmail {
        to,
        from,
        subject,
        date,
        body,
    })
}

impl TryFrom<Email> for Entry {
    type Error = String;
    fn try_from(envelope: Email) -> Result<Self, Self::Error> {
        if envelope.rcpt.is_empty() && envelope.body.is_empty() {
            warn!("Empty envelope received and discarded");
            return Err("Empty envelope discarded".to_owned());
        }

        let recipient = match EMAIL_REGEX.find(&envelope.rcpt) {
            Some(m) => m.as_str(),
            _ => "invalid@email.address",
        };

        debug!("Received email for {}", recipient);

        let parsed: ParsedEmail =
            parse_bytes_to_email(envelope.body.as_bytes())?;

        let parsed_to = match EMAIL_REGEX.find(&parsed.to) {
            Some(m) => m.as_str(),
            _ => "invalid@email.address",
        };

        debug!("Parsed envelope addressed to {}", parsed_to);

        let received = Entry {
            id: 0, // this won't be used
            created_at: parsed.date,
            reference: parsed_to.split('@').next().unwrap_or("").to_owned(),
            title: parsed.subject,
            author: parsed.from,
            content: parsed.body,
        };

        let domain = email_domain();
        if !(recipient.ends_with(&domain) || parsed.to.ends_with(&domain))
        {
            Err(format!(
                "Email for {} received and discarded. Parsed entry: {}",
                recipient, received
            ))
        } else {
            Ok(received)
        }
    }
}
