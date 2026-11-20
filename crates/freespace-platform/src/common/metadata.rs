//! Platform-neutral file metadata records.

/// One named alternate data stream.
///
/// The concept exists on Windows and is always empty on platforms without
/// alternate streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    /// Stream name without the leading colon or the `:$DATA` suffix.
    pub name: String,
    /// Stream length in bytes.
    pub size: u64,
}

/// Kind of a file-system link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkKind {
    /// Symbolic link.
    Symlink,
    /// NTFS junction (directory mount point).
    Junction,
    /// Another reparse tag or link kind.
    Other,
}

impl LinkKind {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Symlink => "symlink",
            Self::Junction => "junction",
            Self::Other => "other",
        }
    }
}
