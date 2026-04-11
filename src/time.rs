//! Time helpers to create, format, and parse datetimes in epoch,
//! PostgreSQL, and RFC3339 (Atom) standards.
use chrono::{DateTime, NaiveDateTime, Utc};

/// Usage
/// ```
/// # use ktn::time::datetime_to_rfc3339;
/// let date_in = "2021-12-01 12:01:03";
/// let date_out = "2021-12-01T12:01:03+00:00";
///
/// assert_eq!(
///     datetime_to_rfc3339(&date_in),
///     date_out,
///     "A valid date wasn't parsed properly"
/// );
/// ```
pub fn datetime_to_rfc3339(date: &str) -> String {
    let dt: DateTime<Utc> =
        NaiveDateTime::parse_from_str(&date[..19], "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc();

    dt.to_rfc3339()
}

#[derive(Debug)]
pub struct Epoch(pub i64);

impl Epoch {
    fn now() -> Epoch {
        Epoch(Utc::now().timestamp())
    }
}

impl From<i64> for Epoch {
    fn from(timestamp: i64) -> Epoch {
        match timestamp {
            0 => Epoch::now(),
            _ => Epoch(timestamp),
        }
    }
}

impl ToString for Epoch {
    fn to_string(&self) -> String {
        DateTime::from_timestamp(self.0, 0)
            .unwrap_or_default()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }
}

pub mod filters {
    use crate::time::datetime_to_rfc3339;

    pub fn rfc3339(s: &str) -> ::askama::Result<String> {
        Ok(datetime_to_rfc3339(s))
    }
}

mod tests {

    #[test]
    fn datetime_to_rfc3339_valid_date() {
        use super::datetime_to_rfc3339;
        let date_in = "2021-12-01 12:01:03";
        let date_out = "2021-12-01T12:01:03+00:00";

        assert_eq!(
            datetime_to_rfc3339(&date_in),
            date_out,
            "A valid date wasn't parsed properly"
        );
    }

    #[test]
    fn datetime_to_rfc3339_valid_date_string() {
        use super::datetime_to_rfc3339;
        let date_in: String = String::from("2021-12-01 12:01:03");
        let date_out = "2021-12-01T12:01:03+00:00";

        assert_eq!(
            datetime_to_rfc3339(&date_in),
            date_out,
            "A valid date wasn't parsed properly"
        );
    }

    #[test]
    #[should_panic]
    fn datetime_to_rfc3339_wrong_date() {
        use super::datetime_to_rfc3339;
        let date_in = "2021-13-01 12:01:03";
        let date_out = "2021-13-01T12:01:03+00:00";

        assert_eq!(datetime_to_rfc3339(&date_in), date_out);
    }

    #[test]
    #[should_panic]
    fn datetime_to_rfc3339_not_sqlite_format() {
        use super::datetime_to_rfc3339;
        let date_in = "2021-12-01T12:01:03Z";
        let date_out = "2021-12-01T12:01:03+00:00";

        assert_eq!(datetime_to_rfc3339(&date_in), date_out);
    }
}
