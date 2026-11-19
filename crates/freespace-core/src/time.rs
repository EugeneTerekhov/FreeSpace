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
        let (date, time) = self.format_parts();

        format!("{date} {time} UTC")
    }

    /// Formats the timestamp as an ISO 8601 instant (`YYYY-MM-DDTHH:MM:SSZ`).
    #[must_use]
    pub fn format_iso8601(self) -> String {
        let (date, time) = self.format_parts();

        format!("{date}T{time}Z")
    }

    /// Parses an ISO 8601 UTC instant written by [`Self::format_iso8601`].
    ///
    /// Fractional seconds and a numeric offset are accepted; an offset is
    /// normalised to UTC. Returns `None` for malformed values.
    #[must_use]
    pub fn parse_iso8601(value: &str) -> Option<Self> {
        let value = value.trim();
        let (date, rest) = value.split_once('T')?;
        let (time, offset_seconds) = split_offset(rest)?;

        let mut date_parts = date.split('-');
        let year: i64 = date_parts.next()?.parse().ok()?;
        let month: u32 = date_parts.next()?.parse().ok()?;
        let day: u32 = date_parts.next()?.parse().ok()?;
        if date_parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }

        let mut time_parts = time.split(':');
        let hour: i64 = time_parts.next()?.parse().ok()?;
        let minute: i64 = time_parts.next()?.parse().ok()?;
        let seconds_text = time_parts.next()?;
        if time_parts.next().is_some() {
            return None;
        }
        let (second_text, millis) = match seconds_text.split_once('.') {
            Some((seconds, fraction)) => {
                let digits: String = fraction.chars().take(3).collect();
                let millis = format!("{digits:0<3}").parse::<i64>().ok()?;
                (seconds, millis)
            }
            None => (seconds_text, 0),
        };
        let second: i64 = second_text.parse().ok()?;
        if hour > 23 || minute > 59 || second > 60 {
            return None;
        }

        let days = days_from_civil(year, month, day);
        let millis_of_day = ((hour * 60 + minute) * 60 + second) * 1_000 + millis;
        let total = days
            .saturating_mul(MILLIS_PER_DAY)
            .saturating_add(millis_of_day)
            .saturating_sub(offset_seconds.saturating_mul(1_000));

        Some(Self(total))
    }

    fn format_parts(self) -> (String, String) {
        let days = self.0.div_euclid(MILLIS_PER_DAY);
        let millis_of_day = self.0.rem_euclid(MILLIS_PER_DAY);
        let (year, month, day) = civil_from_days(days);

        let seconds = millis_of_day / 1_000;
        let hour = seconds / 3_600;
        let minute = (seconds % 3_600) / 60;
        let second = seconds % 60;

        (
            format!("{year:04}-{month:02}-{day:02}"),
            format!("{hour:02}:{minute:02}:{second:02}"),
        )
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

/// Splits the time part of an ISO 8601 instant into the clock and the offset.
fn split_offset(value: &str) -> Option<(&str, i64)> {
    if let Some(time) = value.strip_suffix('Z') {
        return Some((time, 0));
    }

    let index = value.rfind(['+', '-'])?;
    let (time, offset) = value.split_at(index);
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let offset = offset.get(1..)?;
    let (hours, minutes) = offset.split_once(':')?;
    let hours: i64 = hours.parse().ok()?;
    let minutes: i64 = minutes.parse().ok()?;

    Some((time, sign * (hours * 3_600 + minutes * 60)))
}

/// Converts a proleptic Gregorian date into days since 1970-01-01.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146_097 + day_of_era - 719_468
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
            Timestamp::from_millis(951_868_800_000).format_iso8601(),
            "2000-03-01T00:00:00Z"
        );
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

    #[test]
    fn parses_iso8601_instants() {
        let timestamp = Timestamp::from_millis(1_763_164_800_000);

        assert_eq!(
            Timestamp::parse_iso8601(&timestamp.format_iso8601()),
            Some(timestamp)
        );
        assert_eq!(
            Timestamp::parse_iso8601("2000-03-01T00:00:00Z").map(Timestamp::as_millis),
            Some(951_868_800_000)
        );
        assert_eq!(
            Timestamp::parse_iso8601("2000-03-01T03:00:00+03:00").map(Timestamp::as_millis),
            Some(951_868_800_000)
        );
        assert_eq!(
            Timestamp::parse_iso8601("1969-12-31T23:59:59.999Z").map(Timestamp::as_millis),
            Some(-1)
        );
        assert_eq!(Timestamp::parse_iso8601("not a date"), None);
        assert_eq!(Timestamp::parse_iso8601("2000-13-01T00:00:00Z"), None);
    }
}
