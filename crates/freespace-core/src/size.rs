//! Disk-space metrics and unit formatting.
//!
//! A single logical byte count is not enough to answer "where did the space
//! go": NTFS hard links, compression, sparse files and alternate data streams
//! all make the logical and the on-disk contribution diverge. [`SizeMetrics`]
//! keeps every dimension and refuses to hide whether a value is exact.

use std::{
    iter::Sum,
    ops::{Add, AddAssign},
};

use serde::{Deserialize, Serialize};

/// How trustworthy a [`SizeMetrics`] value is.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SizeMethod {
    /// Measured directly from file-system metadata.
    #[default]
    Exact,
    /// Estimated, for example when a directory could not be read completely.
    Estimated,
}

/// Unit system used to format byte counts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    /// Powers of 1024 (KiB, MiB, ...), the default for disk tools.
    #[default]
    Binary,
    /// Powers of 1000 (KB, MB, ...).
    Decimal,
}

impl Units {
    /// Returns the stable configuration value of the unit system.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Decimal => "decimal",
        }
    }

    /// Parses a configuration value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "binary" => Some(Self::Binary),
            "decimal" => Some(Self::Decimal),
            _ => None,
        }
    }
}

/// Size of a file-system object along every dimension the application reports.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SizeMetrics {
    /// Sum of file lengths with every hard link counted.
    pub logical: u64,
    /// Space actually allocated on the volume (compression and sparseness aware).
    pub on_disk: u64,
    /// Physical contribution with hard links counted once.
    pub physical: u64,
    /// Space used by alternate data streams.
    pub ads: u64,
    /// Whether the values are exact or estimated.
    pub method: SizeMethod,
}

impl SizeMetrics {
    /// An all-zero exact value.
    pub const ZERO: Self = Self {
        logical: 0,
        on_disk: 0,
        physical: 0,
        ads: 0,
        method: SizeMethod::Exact,
    };

    /// Builds an exact value from the four size dimensions.
    #[must_use]
    pub const fn exact(logical: u64, on_disk: u64, physical: u64, ads: u64) -> Self {
        Self {
            logical,
            on_disk,
            physical,
            ads,
            method: SizeMethod::Exact,
        }
    }

    /// Builds an exact value for a single file where every dimension is equal.
    #[must_use]
    pub const fn file(bytes: u64) -> Self {
        Self::exact(bytes, bytes, bytes, 0)
    }

    /// Space saved by NTFS compression and sparse regions.
    ///
    /// Derived on demand so that it can never drift from the source values.
    #[must_use]
    pub const fn savings(&self) -> u64 {
        self.logical.saturating_sub(self.on_disk)
    }

    /// Total space occupied on the volume, including alternate data streams.
    #[must_use]
    pub const fn total_on_disk(&self) -> u64 {
        self.on_disk.saturating_add(self.ads)
    }

    /// Marks the value as estimated.
    pub fn mark_estimated(&mut self) {
        self.method = SizeMethod::Estimated;
    }

    /// Returns `true` when the value is only an estimate.
    #[must_use]
    pub const fn is_estimated(&self) -> bool {
        matches!(self.method, SizeMethod::Estimated)
    }
}

impl Add for SizeMetrics {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            logical: self.logical.saturating_add(other.logical),
            on_disk: self.on_disk.saturating_add(other.on_disk),
            physical: self.physical.saturating_add(other.physical),
            ads: self.ads.saturating_add(other.ads),
            method: if self.is_estimated() || other.is_estimated() {
                SizeMethod::Estimated
            } else {
                SizeMethod::Exact
            },
        }
    }
}

impl AddAssign for SizeMetrics {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sum for SizeMetrics {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

/// Formats a byte count using binary or decimal units.
///
/// Values below one kibibyte (or kilobyte) are printed as whole bytes; larger
/// values get one fractional digit with a trailing `.0` removed.
#[must_use]
pub fn format_bytes(bytes: u64, units: Units) -> String {
    let (base, suffixes): (f64, [&str; 6]) = match units {
        Units::Binary => (1024.0, ["B", "KiB", "MiB", "GiB", "TiB", "PiB"]),
        Units::Decimal => (1000.0, ["B", "KB", "MB", "GB", "TB", "PB"]),
    };

    if (bytes as f64) < base {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut index = 0;
    while value >= base && index + 1 < suffixes.len() {
        value /= base;
        index += 1;
    }

    let formatted = format!("{value:.1}");
    let formatted = formatted.strip_suffix(".0").unwrap_or(&formatted);

    format!("{formatted} {}", suffixes[index])
}

#[cfg(test)]
mod tests {
    use super::{SizeMethod, SizeMetrics, Units, format_bytes};

    #[test]
    fn savings_never_underflow() {
        let metrics = SizeMetrics::exact(1_000, 4_096, 1_000, 0);

        assert_eq!(metrics.savings(), 0);
    }

    #[test]
    fn savings_reflect_compression() {
        let metrics = SizeMetrics::exact(10_000, 2_000, 10_000, 512);

        assert_eq!(metrics.savings(), 8_000);
        assert_eq!(metrics.total_on_disk(), 2_512);
    }

    #[test]
    fn addition_propagates_estimates() {
        let exact = SizeMetrics::file(1_000);
        let mut estimated = SizeMetrics::file(250);
        estimated.mark_estimated();

        let total = exact + estimated;

        assert_eq!(total.logical, 1_250);
        assert_eq!(total.method, SizeMethod::Estimated);
        assert!(total.is_estimated());
    }

    #[test]
    fn sums_children() {
        let total: SizeMetrics = [
            SizeMetrics::file(1),
            SizeMetrics::file(2),
            SizeMetrics::file(3),
        ]
        .into_iter()
        .sum();

        assert_eq!(total, SizeMetrics::file(6));
    }

    #[test]
    fn saturates_instead_of_overflowing() {
        let total = SizeMetrics::file(u64::MAX) + SizeMetrics::file(1);

        assert_eq!(total.logical, u64::MAX);
    }

    #[test]
    fn formats_binary_units() {
        assert_eq!(format_bytes(0, Units::Binary), "0 B");
        assert_eq!(format_bytes(1_023, Units::Binary), "1023 B");
        assert_eq!(format_bytes(1_024, Units::Binary), "1 KiB");
        assert_eq!(format_bytes(1_536, Units::Binary), "1.5 KiB");
        assert_eq!(format_bytes(1_073_741_824, Units::Binary), "1 GiB");
    }

    #[test]
    fn formats_decimal_units() {
        assert_eq!(format_bytes(999, Units::Decimal), "999 B");
        assert_eq!(format_bytes(1_000, Units::Decimal), "1 KB");
        assert_eq!(format_bytes(1_500_000, Units::Decimal), "1.5 MB");
    }

    #[test]
    fn parses_unit_names() {
        assert_eq!(Units::parse_value("binary"), Some(Units::Binary));
        assert_eq!(Units::parse_value("DECIMAL"), Some(Units::Decimal));
        assert_eq!(Units::parse_value("bits"), None);
    }
}
