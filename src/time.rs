use chrono::{DateTime, NaiveDateTime, Utc};

fn sqlite_datetime_to_rfc3339(date: &str) -> String {
    let dt: DateTime<Utc> = DateTime::from_utc(
        NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S").unwrap(),
        Utc
    );

    dt.to_rfc3339()
}

pub mod filters {
    use crate::time::sqlite_datetime_to_rfc3339;

    pub fn rfc3339(s: &str) -> ::askama::Result<String> {
        Ok(sqlite_datetime_to_rfc3339(&s))
    }
}