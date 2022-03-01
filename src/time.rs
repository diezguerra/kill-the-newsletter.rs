use chrono::{DateTime, NaiveDateTime, Utc};

/// Trying doctests
/// ```
/// # use ktn::time::sqlite_datetime_to_rfc3339;
/// let date_in = "2021-12-01 12:01:03";
/// let date_out = "2021-12-01T12:01:03+00:00";
///
/// assert_eq!(
///     sqlite_datetime_to_rfc3339(&date_in),
///     date_out,
///     "A valid date wasn't parsed properly"
/// );
/// ```
pub fn sqlite_datetime_to_rfc3339(date: &str) -> String {
    let dt: DateTime<Utc> = DateTime::from_utc(
        NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S").unwrap(),
        Utc,
    );

    dt.to_rfc3339()
}

pub mod filters {
    use crate::time::sqlite_datetime_to_rfc3339;

    pub fn rfc3339(s: &str) -> ::askama::Result<String> {
        Ok(sqlite_datetime_to_rfc3339(&s))
    }
}

mod tests {

    #[test]
    fn sqlite_datetime_to_rfc3339_valid_date() {
        use super::sqlite_datetime_to_rfc3339;
        let date_in = "2021-12-01 12:01:03";
        let date_out = "2021-12-01T12:01:03+00:00";

        assert_eq!(
            sqlite_datetime_to_rfc3339(&date_in),
            date_out,
            "A valid date wasn't parsed properly"
        );
    }

    #[test]
    fn sqlite_datetime_to_rfc3339_valid_date_string() {
        use super::sqlite_datetime_to_rfc3339;
        let date_in: String = String::from("2021-12-01 12:01:03");
        let date_out = "2021-12-01T12:01:03+00:00";

        assert_eq!(
            sqlite_datetime_to_rfc3339(&date_in),
            date_out,
            "A valid date wasn't parsed properly"
        );
    }

    #[test]
    #[should_panic]
    fn sqlite_datetime_to_rfc3339_wrong_date() {
        use super::sqlite_datetime_to_rfc3339;
        let date_in = "2021-13-01 12:01:03";
        let date_out = "2021-13-01T12:01:03+00:00";

        assert_eq!(sqlite_datetime_to_rfc3339(&date_in), date_out);
    }

    #[test]
    #[should_panic]
    fn sqlite_datetime_to_rfc3339_not_sqlite_format() {
        use super::sqlite_datetime_to_rfc3339;
        let date_in = "2021-12-01T12:01:03Z";
        let date_out = "2021-12-01T12:01:03+00:00";

        assert_eq!(sqlite_datetime_to_rfc3339(&date_in), date_out);
    }
}
