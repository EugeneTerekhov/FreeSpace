//! UTC timestamps stored as milliseconds since the Unix epoch.
//!
//! FreeSpace keeps every timestamp in UTC and only converts to local time for
//! presentation. A dedicated newtype avoids a heavy date-time dependency for
//! the handful of operations the application performs (journalling, retention
//! windows and deterministic snapshots).

use std::{fmt, time::SystemTime, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Number of milliseconds in one 24-hour day.
pub const MILLIS_PER_DAY: i64 = 86_400_000;

/// A point in time in UTC, expressed as Unix milliseconds.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// 1970-01-01T00:00:00Z.
    pub const UNIX_EPOCH: Self = Self(0);

    /// Wraps a raw Unix-millisecond value.
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// Returns the raw Unix-millisecond value.
    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }

    /// Captures the current system time.
    #[must_use]
    pub fn now() -> Self {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(elapsed) => Self(clamp_millis(elapsed.as_millis())),
            Err(error) => Self(-clamp_millis(error.duration().as_millis())),
        }
    }

    /// Returns a timestamp `days` days in the future, saturating on overflow.
    #[must_use]
    pub fn add_days(self, days: u32) -> Self {
        Self(
            self.0
                .saturating_add(i64::from(days).saturating_mul(MILLIS_PER_DAY)),
        )
    }

    /// Formats the timestamp as `YYYY-MM-DD HH:MM:SS UTC`.
    #[must_use]
    pub fn format_utc(self) -> String {
        let days = self.0.div_euclid(MILLIS_PER_DAY);
        let millis_of_day = self.0.rem_euclid(MILLIS_PER_DAY);
        let (year, month, day) = civil_from_days(days);

        let seconds = millis_of_day / 1_000;
        let hour = seconds / 3_600;
        let minute = (seconds % 3_600) / 60;
        let second = seconds % 60;

        format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.format_utc())
    }
}

impl From<SystemTime> for Timestamp {
    fn from(value: SystemTime) -> Self {
        match value.duration_since(UNIX_EPOCH) {
            Ok(elapsed) => Self(clamp_millis(elapsed.as_millis())),
            Err(error) => Self(-clamp_millis(error.duration().as_millis())),
        }
    }
}

fn clamp_millis(millis: u128) -> i64 {
    i64::try_from(millis).unwrap_or(i64::MAX)
}

/// Converts a count of days since the Unix epoch into a civil date.
///
/// Based on Howard Hinnant's `civil_from_days` algorithm; works for proleptic
/// Gregorian dates in the entire `i64` range that fits the date arithmetic.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };

    (
        if month <= 2 { year + 1 } else { year },
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::Timestamp;

    #[test]
    fn formats_the_unix_epoch() {
        assert_eq!(
            Timestamp::UNIX_EPOCH.format_utc(),
            "1970-01-01 00:00:00 UTC"
        );
    }

    #[test]
    fn formats_known_dates() {
        assert_eq!(
            Timestamp::from_millis(951_868_800_000).format_utc(),
            "2000-03-01 00:00:00 UTC"
        );
        assert_eq!(
            Timestamp::from_millis(951_782_400_000).format_utc(),
            "2000-02-29 00:00:00 UTC"
        );
        assert_eq!(
            Timestamp::from_millis(-1).format_utc(),
            "1969-12-31 23:59:59 UTC"
        );
    }

    #[test]
    fn adds_retention_days_across_month_boundaries() {
        let february_28 = Timestamp::from_millis(951_696_000_000);

        assert_eq!(
            february_28.add_days(1).format_utc(),
            "2000-02-29 00:00:00 UTC"
        );
        assert_eq!(
            february_28.add_days(2).format_utc(),
            "2000-03-01 00:00:00 UTC"
        );
    }

    #[test]
    fn saturates_on_overflow() {
        assert_eq!(
            Timestamp::from_millis(i64::MAX).add_days(1).as_millis(),
            i64::MAX
        );
    }

    #[test]
    fn round_trips_through_serde() {
        let timestamp = Timestamp::from_millis(1_763_164_800_000);
        let json = serde_json::to_string(&timestamp).expect("serialise timestamp");

        assert_eq!(json, "1763164800000");
        assert_eq!(
            serde_json::from_str::<Timestamp>(&json).expect("deserialise timestamp"),
            timestamp
        );
    }
}
