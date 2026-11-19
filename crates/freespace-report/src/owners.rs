//! Per-owner report section (FR-064).
//!
//! The report keeps the application's numbers unchanged: totals are computed
//! from the same [`OwnerSummary`](freespace_analysis::OwnerSummary) rows the UI
//! displays, so CSV, XML, HTML and XLSX exports cannot disagree with the tree.

use freespace_analysis::OwnerSummary;

/// Report-ready owner table.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OwnerReport {
    summaries: Vec<OwnerSummary>,
    total_bytes: u64,
    file_count: u64,
}

impl OwnerReport {
    /// Builds a report from the analysis output.
    ///
    /// Rows keep the order produced by the analysis (largest owner first).
    #[must_use]
    pub fn from_summaries(summaries: Vec<OwnerSummary>) -> Self {
        let total_bytes = summaries
            .iter()
            .map(|summary| summary.total_bytes)
            .fold(0, u64::saturating_add);
        let file_count = summaries
            .iter()
            .map(|summary| summary.file_count)
            .fold(0, u64::saturating_add);

        Self {
            summaries,
            total_bytes,
            file_count,
        }
    }

    /// Report rows.
    #[must_use]
    pub fn summaries(&self) -> &[OwnerSummary] {
        &self.summaries
    }

    /// Number of report rows.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.summaries.len()
    }

    /// Total logical size across every owner.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Total number of files across every owner.
    #[must_use]
    pub const fn file_count(&self) -> u64 {
        self.file_count
    }

    /// Returns `true` when the report has no owners.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.summaries.is_empty()
    }

    /// Share of `bytes` in the report total.
    #[must_use]
    pub fn share(&self, bytes: u64) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            bytes as f64 / self.total_bytes as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use freespace_analysis::{OwnerSummary, UNKNOWN_OWNER};

    use super::OwnerReport;

    #[test]
    fn totals_match_the_rows() {
        let report = OwnerReport::from_summaries(vec![
            OwnerSummary {
                owner_id: 0,
                sid: "S-1-5-21-1".to_owned(),
                display_name: r"PC\Alice".to_owned(),
                total_bytes: 300,
                file_count: 2,
            },
            OwnerSummary {
                owner_id: u32::MAX,
                sid: String::new(),
                display_name: UNKNOWN_OWNER.to_owned(),
                total_bytes: 100,
                file_count: 1,
            },
        ]);

        assert_eq!(report.row_count(), 2);
        assert_eq!(report.total_bytes(), 400);
        assert_eq!(report.file_count(), 3);
        assert!((report.share(300) - 0.75).abs() < f64::EPSILON);
        assert!(!report.is_empty());
    }

    #[test]
    fn empty_reports_have_zero_totals() {
        let report = OwnerReport::default();

        assert!(report.is_empty());
        assert_eq!(report.total_bytes(), 0);
        assert_eq!(report.share(10), 0.0);
    }
}
