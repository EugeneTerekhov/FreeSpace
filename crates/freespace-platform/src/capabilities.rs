//! Platform capability descriptor.
//!
//! `Capabilities` is the single source of truth for the features a platform can
//! provide. The GUI uses it to hide or disable actions, the CLI uses it to
//! return `E_UNSUPPORTED` consistently, and tests use it to verify that the
//! advertised behaviour matches the actual backend.

use serde::{Deserialize, Serialize};

/// Operating-system family of the running backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformId {
    /// Windows 10/11.
    Windows,
    /// Linux.
    Linux,
    /// Any other platform (no backend is provided).
    Other,
}

impl PlatformId {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Other => "other",
        }
    }
}

/// Scan methods available on a platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanMethods {
    /// Direct NTFS MFT reading.
    pub mft: bool,
    /// Incremental USN journal updates.
    pub usn: bool,
    /// Directory-tree traversal.
    pub walk: bool,
}

impl ScanMethods {
    /// Windows supports all three methods.
    #[must_use]
    pub const fn windows() -> Self {
        Self {
            mft: true,
            usn: true,
            walk: true,
        }
    }

    /// Linux supports directory traversal only.
    #[must_use]
    pub const fn linux() -> Self {
        Self {
            mft: false,
            usn: false,
            walk: true,
        }
    }
}

/// Features and limits provided by the running platform backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Operating-system family.
    pub platform: PlatformId,
    /// Available scan methods.
    pub scan_methods: ScanMethods,
    /// System trash (Recycle Bin or XDG trash).
    pub trash: bool,
    /// Irreversible deletion.
    pub permanent: bool,
    /// Shredder (Windows CLI only).
    pub shred: bool,
    /// NTFS ACL and ownership changes.
    pub acl: bool,
    /// Windows file attributes.
    pub attrs: bool,
    /// NTFS alternate data streams.
    pub ads: bool,
    /// Restart Manager lock inspection and release.
    pub unlock: bool,
    /// Explorer context-menu integration.
    pub context_menu: bool,
    /// Write monitoring.
    pub monitor: bool,
    /// Elevation state detection.
    pub elevation: bool,
    /// Verbatim long-path support.
    pub long_paths: bool,
    /// File-system name comparison is case-sensitive.
    pub case_sensitive: bool,
    /// Maximum path length in characters, when the platform enforces one.
    pub max_path: Option<usize>,
}

impl Capabilities {
    /// Capabilities of the Windows backend.
    #[must_use]
    pub const fn windows() -> Self {
        Self {
            platform: PlatformId::Windows,
            scan_methods: ScanMethods::windows(),
            trash: true,
            permanent: true,
            shred: true,
            acl: true,
            attrs: true,
            ads: true,
            unlock: true,
            context_menu: cfg!(feature = "context-menu"),
            monitor: true,
            elevation: true,
            long_paths: true,
            case_sensitive: false,
            max_path: None,
        }
    }

    /// Capabilities of the Linux backend.
    #[must_use]
    pub const fn linux() -> Self {
        Self {
            platform: PlatformId::Linux,
            scan_methods: ScanMethods::linux(),
            trash: true,
            permanent: true,
            shred: false,
            acl: false,
            attrs: false,
            ads: false,
            unlock: false,
            context_menu: false,
            monitor: true,
            elevation: true,
            long_paths: false,
            case_sensitive: true,
            max_path: Some(4096),
        }
    }

    /// Capabilities of a platform without a backend.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self {
            platform: PlatformId::Other,
            scan_methods: ScanMethods {
                mft: false,
                usn: false,
                walk: false,
            },
            trash: false,
            permanent: false,
            shred: false,
            acl: false,
            attrs: false,
            ads: false,
            unlock: false,
            context_menu: false,
            monitor: false,
            elevation: false,
            long_paths: false,
            case_sensitive: false,
            max_path: None,
        }
    }

    /// Capabilities of the current build target.
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(windows)]
        {
            Self::windows()
        }

        #[cfg(target_os = "linux")]
        {
            Self::linux()
        }

        #[cfg(not(any(windows, target_os = "linux")))]
        {
            Self::unsupported()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Capabilities, PlatformId};

    #[test]
    fn windows_advertises_native_features() {
        let caps = Capabilities::windows();

        assert_eq!(caps.platform, PlatformId::Windows);
        assert!(caps.scan_methods.mft);
        assert!(caps.scan_methods.usn);
        assert!(caps.trash);
        assert!(caps.shred);
        assert!(caps.acl);
        assert!(caps.attrs);
        assert!(caps.ads);
        assert!(caps.unlock);
        assert!(caps.long_paths);
        assert!(!caps.case_sensitive);
        assert_eq!(caps.max_path, None);
    }

    #[test]
    fn linux_advertises_portable_features_only() {
        let caps = Capabilities::linux();

        assert_eq!(caps.platform, PlatformId::Linux);
        assert!(!caps.scan_methods.mft);
        assert!(!caps.scan_methods.usn);
        assert!(caps.scan_methods.walk);
        assert!(caps.trash);
        assert!(caps.permanent);
        assert!(!caps.shred);
        assert!(!caps.acl);
        assert!(!caps.attrs);
        assert!(!caps.ads);
        assert!(!caps.unlock);
        assert!(!caps.context_menu);
        assert!(caps.case_sensitive);
        assert_eq!(caps.max_path, Some(4096));
    }

    #[test]
    fn current_matches_the_build_target() {
        let caps = Capabilities::current();

        if cfg!(windows) {
            assert_eq!(caps.platform, PlatformId::Windows);
        } else if cfg!(target_os = "linux") {
            assert_eq!(caps.platform, PlatformId::Linux);
        } else {
            assert_eq!(caps.platform, PlatformId::Other);
        }
    }

    #[test]
    fn serialises_platform_as_a_lowercase_name() {
        let value = serde_json::to_value(Capabilities::linux()).expect("serialise");

        assert_eq!(value["platform"], "linux");
        assert_eq!(value["scan_methods"]["mft"], false);
        assert_eq!(value["max_path"], 4096);
    }
}
