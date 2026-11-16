//! Resource limits that protect the application from archive bombs (FR-068).

use freespace_core::{Error, Result};

use super::ArchiveEntry;

/// Hard caps applied while listing an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    /// Maximum number of entries.
    pub max_entries: usize,
    /// Maximum total uncompressed size in bytes.
    pub max_total_size: u64,
    /// Maximum size of a single entry in bytes.
    pub max_entry_size: u64,
    /// Maximum uncompressed-to-compressed ratio before an archive is treated as
    /// a bomb.
    pub max_compression_ratio: u64,
    /// Ratio checks only apply once the uncompressed size reaches this size.
    pub ratio_threshold: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: 1_000_000,
            max_total_size: 1024 * 1024 * 1024 * 1024, // 1 TiB
            max_entry_size: 1024 * 1024 * 1024 * 1024,
            max_compression_ratio: 10_000,
            ratio_threshold: 1024 * 1024 * 1024, // 1 GiB
        }
    }
}

impl ArchiveLimits {
    /// Rejects an archive whose entry count exceeds the cap.
    ///
    /// # Errors
    /// Returns [`Error::Unsupported`] with an explanatory message.
    pub fn check_count(&self, count: usize) -> Result<()> {
        if count > self.max_entries {
            return Err(Error::Unsupported(format!(
                "the archive has more than {} entries; listing was stopped to protect resources",
                self.max_entries
            )));
        }

        Ok(())
    }

    /// Validates one entry against the per-entry cap.
    ///
    /// # Errors
    /// Returns [`Error::Unsupported`] when the entry is too large.
    pub fn check_entry(&self, entry: &ArchiveEntry) -> Result<()> {
        if entry.size > self.max_entry_size {
            return Err(Error::Unsupported(format!(
                "archive entry `{}` is larger than the {} byte limit",
                entry.internal_path, self.max_entry_size
            )));
        }

        Ok(())
    }

    /// Validates the totals of a completed listing.
    ///
    /// # Errors
    /// Returns [`Error::Unsupported`] when the archive exceeds the total-size
    /// cap or looks like a compression bomb.
    pub fn check_totals(&self, total_size: u64, archive_size: u64) -> Result<()> {
        if total_size > self.max_total_size {
            return Err(Error::Unsupported(format!(
                "the archive expands to more than {} bytes; listing was stopped to protect resources",
                self.max_total_size
            )));
        }

        if archive_size > 0
            && total_size >= self.ratio_threshold
            && total_size / archive_size > self.max_compression_ratio
        {
            return Err(Error::Unsupported(format!(
                "the archive expands {total_size} to {archive_size} bytes, which looks like an \
                 archive bomb"
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ArchiveEntry, ArchiveLimits};

    fn entry(size: u64) -> ArchiveEntry {
        ArchiveEntry {
            internal_path: "payload.bin".to_owned(),
            size,
            compressed_size: Some(size / 2),
            is_directory: false,
            encrypted: false,
            share: 0.0,
        }
    }

    #[test]
    fn accepts_normal_archives() {
        let limits = ArchiveLimits::default();

        limits.check_count(10).expect("count");
        limits.check_entry(&entry(1024)).expect("entry");
        limits.check_totals(4096, 2048).expect("totals");
    }

    #[test]
    fn rejects_resource_exhaustion() {
        let limits = ArchiveLimits {
            max_entries: 2,
            max_entry_size: 10,
            max_total_size: 100,
            max_compression_ratio: 2,
            ratio_threshold: 1,
        };

        assert!(limits.check_count(3).is_err());
        assert!(limits.check_entry(&entry(11)).is_err());
        assert!(limits.check_totals(101, 100).is_err());
        assert!(limits.check_totals(300, 100).is_err());
    }

    #[test]
    fn ignores_the_ratio_for_small_archives() {
        let limits = ArchiveLimits {
            max_compression_ratio: 2,
            ratio_threshold: 1024,
            ..ArchiveLimits::default()
        };

        limits
            .check_totals(100, 1)
            .expect("small archives are not bombs");
    }
}
