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
const FALLBACK_RFC3339: &str = "1970-01-01T00:00:00+00:00";

pub fn datetime_to_rfc3339(date: &str) -> String {
    date.get(..19)
        .and_then(|d| {
            NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H:%M:%S").ok()
        })
        .map(|dt| dt.and_utc().to_rfc3339())
        .unwrap_or_else(|| FALLBACK_RFC3339.to_owned())
}

#[derive(Debug)]
pub struct Epoch(pub i64);

impl Epoch {
    fn now() -> Epoch {
        Epoch(Utc::now().timestamp())
    }
}

/// Seconds between the epoch and 9999-12-31T23:59:59Z — the largest
/// timestamp `chrono` can format without emitting a signed 5+ digit year
/// that our storage format (and `datetime_to_rfc3339`) can't round-trip.
const MAX_EPOCH_SECONDS: i64 = 253_402_300_799;

impl From<i64> for Epoch {
    fn from(timestamp: i64) -> Epoch {
        match timestamp {
            0 => Epoch::now(),
            t if !(0..=MAX_EPOCH_SECONDS).contains(&t) => Epoch::now(),
            t => Epoch(t),
        }
    }
}

impl std::fmt::Display for Epoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            DateTime::from_timestamp(self.0, 0)
                .unwrap_or_default()
                .naive_utc()
                .format("%Y-%m-%d %H:%M:%S")
        )
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
            datetime_to_rfc3339(date_in),
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
    fn datetime_to_rfc3339_wrong_date_falls_back() {
        use super::{datetime_to_rfc3339, FALLBACK_RFC3339};
        let date_in = "2021-13-01 12:01:03";

        assert_eq!(datetime_to_rfc3339(date_in), FALLBACK_RFC3339);
    }

    #[test]
    fn datetime_to_rfc3339_not_sqlite_format_falls_back() {
        use super::{datetime_to_rfc3339, FALLBACK_RFC3339};
        let date_in = "2021-12-01T12:01:03Z";

        assert_eq!(datetime_to_rfc3339(date_in), FALLBACK_RFC3339);
    }

    #[test]
    fn datetime_to_rfc3339_short_string_does_not_panic() {
        use super::{datetime_to_rfc3339, FALLBACK_RFC3339};

        assert_eq!(datetime_to_rfc3339(""), FALLBACK_RFC3339);
        assert_eq!(datetime_to_rfc3339("2021-12-01"), FALLBACK_RFC3339);
    }

    #[test]
    fn datetime_to_rfc3339_oversized_year_does_not_panic() {
        use super::{datetime_to_rfc3339, FALLBACK_RFC3339};
        // What chrono produces for a >9999 year: 21 chars, doesn't match
        // the "%Y-%m-%d %H:%M:%S" pattern within the first 19 bytes.
        let date_in = "+99999-01-01 00:00:00";

        assert_eq!(datetime_to_rfc3339(date_in), FALLBACK_RFC3339);
    }

    #[test]
    fn epoch_from_clamps_absurd_future_year() {
        use super::Epoch;
        // Timestamp for year 99999, as mailparse::dateparse would produce
        // from a forged `Date:` header.
        let absurd_ts: i64 = 3_093_496_444_800;
        let epoch = Epoch::from(absurd_ts);
        // Clamped to "now" rather than stored as-is.
        assert_ne!(epoch.0, absurd_ts);
    }

    #[test]
    fn epoch_from_clamps_negative_timestamp() {
        use super::Epoch;
        let epoch = Epoch::from(-1);
        assert!(epoch.0 >= 0);
    }

    #[test]
    fn epoch_from_accepts_sane_timestamp() {
        use super::Epoch;
        let epoch = Epoch::from(978_307_200); // 2001-01-01
        assert_eq!(epoch.0, 978_307_200);
    }
}
