use mailparse::{dateparse, parse_mail, MailHeaderMap};
use tracing::debug;

use crate::time::Epoch;

pub struct ParsedEmail {
    pub to: String,
    pub subject: String,
    pub date: String,
    pub body: String,
}

impl ParsedEmail {
    pub fn to_string(&self) -> String {
        format!(
            "ParsedEmail {{ to: {}, subject: {}, date: {}, body[..50]: {} }}",
            &self.to,
            &self.subject,
            &self.date,
            if &self.body.len() > &50 {
                &self.body[..50]
            } else {
                &self.body
            }
        )
    }
}

impl std::fmt::Display for ParsedEmail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

pub fn parse(email: &[u8]) -> ParsedEmail {
    let parsed = parse_mail(email).unwrap();

    let subject = parsed
        .headers
        .get_first_value("Subject")
        .unwrap_or("No subject".to_owned());

    let to = parsed
        .headers
        .get_first_value("To")
        .unwrap_or("unknown@recipient.mail".to_owned());

    let mut body = String::new();

    // Get the HTML version or the first one if that one isn't found
    if parsed.subparts.len() > 0 {
        for part in 0..parsed.subparts.len() {
            if parsed.subparts[part]
                .ctype
                .mimetype
                .starts_with("text/html")
            {
                body.push_str(&parsed.subparts[part].get_body().unwrap());
            }
        }
        if body.len() == 0 {
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
                .unwrap_or("".to_owned())
                .as_str(),
        )
        .unwrap_or(0),
    )
    .to_string();

    debug!("Date: {:#?}", date);

    ParsedEmail {
        to,
        subject,
        date,
        body,
    }
}
