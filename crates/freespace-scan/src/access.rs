//! Protected system directories and degraded access (US21, FR-087, FR-088).
//!
//! A limited user cannot open `System Volume Information`, `$Recycle.Bin` and
//! several other NTFS metadata directories. The scanner must keep going, mark
//! those areas as inaccessible and tell the UI that elevation would help instead
//! of failing the whole analysis.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Directory names that Windows protects from a standard user.
pub const PROTECTED_DIRECTORIES: [&str; 7] = [
    "System Volume Information",
    "$Recycle.Bin",
    "Recovery",
    "Config.Msi",
    "$WinREAgent",
    "Documents and Settings",
    "System32",
];

/// Result of probing one object for readability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// The object can be read.
    Available,
    /// The current user is not allowed to read the object.
    Denied,
    /// The object does not exist.
    Missing,
    /// Reading failed for another reason.
    Failed(String),
}

impl Probe {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Denied => "denied",
            Self::Missing => "missing",
            Self::Failed(_) => "failed",
        }
    }

    /// Returns `true` when the object could not be read.
    #[must_use]
    pub const fn is_blocked(&self) -> bool {
        matches!(self, Self::Denied | Self::Failed(_))
    }
}

/// One protected area below a volume root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedArea {
    /// Full path to the directory.
    pub path: PathBuf,
    /// Directory name.
    pub name: String,
    /// Whether the current process can read it.
    pub probe: Probe,
}

/// Returns `true` when any component of `path` is a protected name.
#[must_use]
pub fn is_protected(path: &Path) -> bool {
    path.components().any(|component| {
        let std::path::Component::Normal(name) = component else {
            return false;
        };
        let Some(name) = name.to_str() else {
            return false;
        };

        PROTECTED_DIRECTORIES
            .iter()
            .any(|protected| name.eq_ignore_ascii_case(protected))
    })
}

/// Probes whether a directory can be enumerated.
///
/// `read_dir` is intentional: metadata may succeed for a directory whose entries
/// are still hidden from the caller.
#[must_use]
pub fn probe(path: &Path) -> Probe {
    match fs::read_dir(path) {
        Ok(mut entries) => {
            // Reading one entry detects a directory that is listed but not
            // readable; an empty directory is a valid result.
            let _ = entries.next();
            Probe::Available
        }
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Probe::Denied,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Probe::Missing,
        Err(error) => Probe::Failed(error.to_string()),
    }
}

/// Enumerates the known protected directories below `volume_root`.
#[must_use]
pub fn protected_areas(volume_root: &Path) -> Vec<ProtectedArea> {
    PROTECTED_DIRECTORIES
        .iter()
        .map(|name| {
            let path = volume_root.join(name);

            ProtectedArea {
                path: path.clone(),
                name: (*name).to_owned(),
                probe: probe(&path),
            }
        })
        .collect()
}

/// Access policy of the current process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessPolicy {
    /// Whether the process has an elevated token.
    pub elevated: bool,
}

impl AccessPolicy {
    /// Creates the policy from the current token.
    #[must_use]
    pub fn current() -> Self {
        Self {
            elevated: freespace_platform::elevation::is_elevated(),
        }
    }

    /// Creates a policy for tests and for code that already knows the mode.
    #[must_use]
    pub const fn new(elevated: bool) -> Self {
        Self { elevated }
    }

    /// Returns `true` when the process runs in limited mode.
    #[must_use]
    pub const fn is_limited(self) -> bool {
        !self.elevated
    }

    /// Returns `true` when elevation could unlock `path`.
    #[must_use]
    pub fn requires_elevation(self, path: &Path) -> bool {
        self.is_limited() && is_protected(path)
    }

    /// Explains a blocked probe to the user.
    #[must_use]
    pub fn advice(self, path: &Path, probe: &Probe) -> String {
        match probe {
            Probe::Available => format!("`{}` is readable", path.display()),
            Probe::Missing => format!("`{}` does not exist", path.display()),
            Probe::Denied if self.requires_elevation(path) => format!(
                "`{}` is a protected system directory; restart FreeSpace as administrator \
                 to include it",
                path.display()
            ),
            Probe::Denied => format!(
                "`{}` is not accessible to the current user; sizes are incomplete",
                path.display()
            ),
            Probe::Failed(reason) => format!("`{}` cannot be read: {reason}", path.display()),
        }
    }
}

impl Default for AccessPolicy {
    fn default() -> Self {
        Self::current()
    }
}

/// Message attached to a scan issue when an object is denied.
#[must_use]
pub fn denial_message(path: &Path) -> String {
    if is_protected(path) {
        format!(
            "access denied; `{}` is a protected system directory and requires elevation",
            path.display()
        )
    } else {
        format!(
            "access denied; `{}` is not readable by the current user",
            path.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AccessPolicy, PROTECTED_DIRECTORIES, Probe, denial_message, is_protected, probe,
        protected_areas,
    };

    #[test]
    fn recognises_protected_names_case_insensitively() {
        assert!(is_protected(std::path::Path::new(
            r"C:\System Volume Information"
        )));
        assert!(is_protected(std::path::Path::new(
            r"C:\$Recycle.Bin\S-1-5-21"
        )));
        assert!(is_protected(std::path::Path::new(r"C:\recovery")));
        assert!(!is_protected(std::path::Path::new(r"C:\Users\alice")));
    }

    #[test]
    fn probes_existing_missing_and_denied_objects() {
        let directory = tempfile::tempdir().expect("temp dir");
        let existing = directory.path().join("System Volume Information");
        std::fs::create_dir(&existing).expect("create protected fixture");

        assert_eq!(probe(directory.path()), Probe::Available);
        assert_eq!(probe(&existing), Probe::Available);
        assert_eq!(probe(&directory.path().join("missing")), Probe::Missing);
        assert!(Probe::Denied.is_blocked());
        assert!(!Probe::Available.is_blocked());
        assert_eq!(Probe::Denied.as_str(), "denied");
    }

    #[test]
    fn lists_every_known_protected_area() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(directory.path().join("$Recycle.Bin")).expect("fixture");

        let areas = protected_areas(directory.path());

        assert_eq!(areas.len(), PROTECTED_DIRECTORIES.len());
        let recycle = areas
            .iter()
            .find(|area| area.name == "$Recycle.Bin")
            .expect("recycle bin");
        assert_eq!(recycle.probe, Probe::Available);
        assert_eq!(
            areas
                .iter()
                .find(|area| area.name == "Recovery")
                .expect("recovery")
                .probe,
            Probe::Missing
        );
    }

    #[test]
    fn distinguishes_elevated_and_limited_advice() {
        let directory = tempfile::tempdir().expect("temp dir");
        let protected = directory.path().join("System Volume Information");
        std::fs::create_dir(&protected).expect("fixture");

        let limited = AccessPolicy::new(false);
        let elevated = AccessPolicy::new(true);
        let denied = Probe::Denied;

        assert!(limited.requires_elevation(&protected));
        assert!(!elevated.requires_elevation(&protected));
        assert!(!limited.requires_elevation(directory.path()));
        assert!(
            limited
                .advice(&protected, &denied)
                .contains("administrator")
        );
        assert!(
            elevated
                .advice(&protected, &denied)
                .contains("not accessible")
        );
        assert!(denial_message(&protected).contains("requires elevation"));
    }
}
