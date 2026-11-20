//! Platform-neutral shredder records.
//!
//! The implementation lives in the platform backend: overwriting must go
//! through write-through handles and, on Windows, a TRIM request.

use std::path::PathBuf;

use super::storage::StorageKind;

/// Overwrite pattern used for every pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShredPattern {
    /// Fill the file with zero bytes.
    Zeros,
    /// Fill the file with random bytes.
    Random,
}

impl ShredPattern {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zeros => "zeros",
            Self::Random => "random",
        }
    }
}

/// Parameters of a shred operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShredOptions {
    /// Number of overwrite passes; must be at least one.
    pub passes: u32,
    /// Overwrite pattern.
    pub pattern: ShredPattern,
}

impl Default for ShredOptions {
    fn default() -> Self {
        Self {
            passes: 1,
            pattern: ShredPattern::Zeros,
        }
    }
}

/// Information shown before a shred operation starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShredPreview {
    /// Absolute path that would be destroyed.
    pub path: PathBuf,
    /// Size before the overwrite.
    pub size: u64,
    /// Detected media kind.
    pub storage: StorageKind,
    /// Non-fatal notes, including the SSD warning.
    pub warnings: Vec<String>,
}

/// Result of shredding one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShredOutcome {
    /// Absolute path that was destroyed.
    pub path: PathBuf,
    /// Size before the overwrite.
    pub size: u64,
    /// Applied number of passes.
    pub passes: u32,
    /// Applied pattern.
    pub pattern: ShredPattern,
    /// Detected media kind.
    pub storage: StorageKind,
    /// Whether the file system performed a TRIM request.
    pub trimmed: bool,
    /// Non-fatal notes, including SSD limitations and TRIM failures.
    pub warnings: Vec<String>,
}
