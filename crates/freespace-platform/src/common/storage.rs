//! Platform-neutral storage hints.

/// Physical media kind of a volume.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageKind {
    /// The media type could not be determined.
    #[default]
    Unknown,
    /// Solid-state storage: overwriting cannot be guaranteed.
    Ssd,
    /// Rotating media.
    Hdd,
}

impl StorageKind {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Ssd => "ssd",
            Self::Hdd => "hdd",
        }
    }

    /// Returns `true` when overwriting gives no hard guarantee.
    #[must_use]
    pub const fn needs_warning(self) -> bool {
        matches!(self, Self::Ssd)
    }
}
