//! Domain model for volumes and scan sessions.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result, time::Timestamp};

/// Namespace used to derive stable volume identifiers from mount points.
const VOLUME_NAMESPACE: Uuid = Uuid::from_u128(0x6d1f_3e8a_5c47_4b9e_9f02_51a3_6db7_2c10);

/// File system of a volume.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSystem {
    /// NTFS, the primary file system supported by FreeSpace.
    #[default]
    Ntfs,
    /// FAT32.
    Fat32,
    /// exFAT.
    ExFat,
    /// ReFS.
    Refs,
    /// Anything else; directory-walk scanning is used.
    Other,
}

impl FileSystem {
    /// Maps a `GetVolumeInformationW` file-system name to the domain value.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_uppercase().as_str() {
            "NTFS" => Self::Ntfs,
            "FAT32" => Self::Fat32,
            "EXFAT" => Self::ExFat,
            "REFS" => Self::Refs,
            _ => Self::Other,
        }
    }

    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ntfs => "ntfs",
            Self::Fat32 => "fat32",
            Self::ExFat => "exfat",
            Self::Refs => "refs",
            Self::Other => "other",
        }
    }

    /// Returns `true` when MFT/USN scanning is available.
    #[must_use]
    pub const fn supports_mft(self) -> bool {
        matches!(self, Self::Ntfs)
    }
}

/// Runtime state of a volume.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeStatus {
    /// The volume is ready for analysis.
    #[default]
    Available,
    /// The volume is not mounted.
    Offline,
    /// The volume exists but cannot be read by the current user.
    Inaccessible,
}

impl VolumeStatus {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Offline => "offline",
            Self::Inaccessible => "inaccessible",
        }
    }
}

/// How the file-system tree was obtained.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanMethod {
    /// Direct MFT enumeration.
    #[default]
    Mft,
    /// Incremental update from the USN change journal.
    Usn,
    /// Parallel directory walk (non-NTFS or unprivileged fallback).
    DirectoryWalk,
}

impl ScanMethod {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mft => "mft",
            Self::Usn => "usn",
            Self::DirectoryWalk => "directory-walk",
        }
    }

    /// Parses a user-supplied or persisted method name.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "mft" => Some(Self::Mft),
            "usn" => Some(Self::Usn),
            "walk" | "directory-walk" => Some(Self::DirectoryWalk),
            _ => None,
        }
    }

    /// Returns `true` when the method can be updated incrementally.
    #[must_use]
    pub const fn supports_incremental(self) -> bool {
        matches!(self, Self::Usn)
    }
}

/// A disk or partition available for analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    /// Stable identifier derived from the mount point.
    pub id: Uuid,
    /// Drive letter root (`C:\`) or mount point.
    pub mount_point: PathBuf,
    /// Volume label.
    pub label: String,
    /// File system.
    pub file_system: FileSystem,
    /// Total volume size in bytes.
    pub total_bytes: u64,
    /// Free space in bytes.
    pub free_bytes: u64,
    /// Cluster size when known.
    pub cluster_size: Option<u32>,
    /// Method used by the most recent scan, when one ran.
    pub scan_method: Option<ScanMethod>,
    /// Whether the last analysis ran elevated.
    pub is_elevated: bool,
    /// Whether the volume is removable.
    pub is_removable: bool,
    /// Current availability.
    pub status: VolumeStatus,
}

impl Volume {
    /// Derives the stable identifier of a volume from its mount point.
    #[must_use]
    pub fn id_for_mount_point(mount_point: &Path) -> Uuid {
        let mut normalized = mount_point.to_string_lossy().replace('/', "\\");
        while normalized.len() > 3 && normalized.ends_with('\\') {
            normalized.pop();
        }
        let normalized = normalized.to_ascii_uppercase();

        Uuid::new_v5(&VOLUME_NAMESPACE, normalized.as_bytes())
    }

    /// Returns `true` when the volume can be analysed.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self.status, VolumeStatus::Available)
    }

    /// Space currently occupied on the volume.
    #[must_use]
    pub const fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.free_bytes)
    }

    /// Validates the volume invariants.
    ///
    /// # Errors
    /// Returns an error when free space exceeds the total size.
    pub fn validate(&self) -> Result<()> {
        if self.free_bytes > self.total_bytes {
            return Err(Error::Operation(format!(
                "volume `{}` reports {} free bytes out of {}",
                self.mount_point.display(),
                self.free_bytes,
                self.total_bytes
            )));
        }

        Ok(())
    }
}

/// Kind of problem recorded while scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanIssueKind {
    /// The scanner was not allowed to read an object.
    AccessDenied,
    /// An I/O error occurred.
    Io,
    /// Anything else.
    Other,
}

impl ScanIssueKind {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AccessDenied => "access-denied",
            Self::Io => "io",
            Self::Other => "other",
        }
    }
}

/// A non-fatal problem encountered during a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanIssue {
    /// Object that triggered the issue.
    pub path: PathBuf,
    /// Kind of problem.
    pub kind: ScanIssueKind,
    /// Human-readable explanation.
    pub message: String,
}

/// USN journal position used for incremental updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalState {
    /// Identifier of the USN journal.
    pub journal_id: u64,
    /// Next record to read.
    pub next_usn: i64,
}

/// Lifecycle state of a scan session.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanStatus {
    /// The scan is in progress.
    #[default]
    Running,
    /// The scan finished successfully.
    Completed,
    /// The user cancelled the scan.
    Cancelled,
    /// The scan stopped because of an error.
    Failed,
}

impl ScanStatus {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Returns `true` when the session has finished.
    #[must_use]
    pub const fn is_finished(self) -> bool {
        !matches!(self, Self::Running)
    }
}

/// A single analysis run over one volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanSession {
    /// Session identifier.
    pub id: Uuid,
    /// Volume being analysed.
    pub volume_id: Uuid,
    /// Analysed roots.
    pub roots: Vec<PathBuf>,
    /// Method used for the scan.
    pub method: ScanMethod,
    /// Start time.
    pub started_at: Timestamp,
    /// Finish time, when the session ended.
    pub finished_at: Option<Timestamp>,
    /// Lifecycle state.
    pub status: ScanStatus,
    /// Progress in whole percents (0–100).
    pub progress: u8,
    /// Number of nodes produced so far.
    pub nodes_count: u64,
    /// Non-fatal problems.
    pub issues: Vec<ScanIssue>,
    /// Journal position for incremental updates.
    pub journal_state: Option<JournalState>,
    /// Paths excluded from the analysis.
    pub exclusions: Vec<String>,
    /// Snapshot created from this session.
    pub snapshot_id: Option<Uuid>,
}

impl ScanSession {
    /// Starts a new running session.
    #[must_use]
    pub fn new(
        volume_id: Uuid,
        roots: Vec<PathBuf>,
        method: ScanMethod,
        exclusions: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            volume_id,
            roots,
            method,
            started_at: Timestamp::now(),
            finished_at: None,
            status: ScanStatus::Running,
            progress: 0,
            nodes_count: 0,
            issues: Vec::new(),
            journal_state: None,
            exclusions,
            snapshot_id: None,
        }
    }

    /// Returns `true` while the session has not finished.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.status, ScanStatus::Running)
    }

    /// Updates progress, clamping values above 100.
    pub fn set_progress(&mut self, percent: u8) {
        self.progress = percent.min(100);
    }

    /// Records a non-fatal issue.
    pub fn record_issue(&mut self, issue: ScanIssue) {
        self.issues.push(issue);
    }

    /// Completes the session.
    ///
    /// # Errors
    /// Returns an error when the session is not running.
    pub fn complete(&mut self) -> Result<()> {
        self.finish(ScanStatus::Completed)
    }

    /// Cancels the session.
    ///
    /// # Errors
    /// Returns an error when the session is not running.
    pub fn cancel(&mut self) -> Result<()> {
        self.finish(ScanStatus::Cancelled)
    }

    /// Marks the session as failed.
    ///
    /// # Errors
    /// Returns an error when the session is not running.
    pub fn fail(&mut self) -> Result<()> {
        self.finish(ScanStatus::Failed)
    }

    fn finish(&mut self, status: ScanStatus) -> Result<()> {
        if !self.is_running() {
            return Err(Error::Operation(format!(
                "scan session `{}` already finished as `{}`",
                self.id,
                self.status.as_str()
            )));
        }
        if status == ScanStatus::Running {
            return Err(Error::InvalidArgument(
                "a scan session cannot be finished as running".to_owned(),
            ));
        }

        self.status = status;
        self.finished_at = Some(Timestamp::now());
        if status == ScanStatus::Completed {
            self.progress = 100;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        FileSystem, JournalState, ScanIssue, ScanIssueKind, ScanMethod, ScanSession, ScanStatus,
        Volume, VolumeStatus,
    };
    use crate::time::Timestamp;

    fn volume() -> Volume {
        Volume {
            id: Volume::id_for_mount_point(PathBuf::from(r"C:\").as_path()),
            mount_point: PathBuf::from(r"C:\"),
            label: "System".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000_000,
            free_bytes: 250_000,
            cluster_size: Some(4_096),
            scan_method: None,
            is_elevated: true,
            is_removable: false,
            status: VolumeStatus::Available,
        }
    }

    #[test]
    fn derives_stable_volume_ids() {
        let a = Volume::id_for_mount_point(PathBuf::from(r"C:\").as_path());
        let b = Volume::id_for_mount_point(PathBuf::from("c:/").as_path());
        let d = Volume::id_for_mount_point(PathBuf::from(r"D:\").as_path());

        assert_eq!(a, b);
        assert_ne!(a, d);
    }

    #[test]
    fn validates_volumes() {
        let valid = volume();
        assert!(valid.validate().is_ok());
        assert!(valid.is_available());
        assert_eq!(valid.used_bytes(), 750_000);

        let mut invalid = volume();
        invalid.free_bytes = invalid.total_bytes + 1;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn parses_file_systems_and_methods() {
        assert_eq!(FileSystem::from_name("ntfs"), FileSystem::Ntfs);
        assert_eq!(FileSystem::from_name("ReFS"), FileSystem::Refs);
        assert_eq!(FileSystem::from_name("cdfs"), FileSystem::Other);
        assert!(FileSystem::Ntfs.supports_mft());
        assert!(!FileSystem::ExFat.supports_mft());

        assert_eq!(ScanMethod::parse_value("MFT"), Some(ScanMethod::Mft));
        assert_eq!(
            ScanMethod::parse_value("directory-walk"),
            Some(ScanMethod::DirectoryWalk)
        );
        assert_eq!(ScanMethod::parse_value("magic"), None);
        assert!(ScanMethod::Usn.supports_incremental());
        assert!(!ScanMethod::Mft.supports_incremental());
    }

    #[test]
    fn runs_a_scan_session_lifecycle() {
        let mut session = ScanSession::new(
            volume().id,
            vec![PathBuf::from(r"C:\")],
            ScanMethod::Mft,
            vec!["**/node_modules".to_owned()],
        );

        assert!(session.is_running());
        session.set_progress(45);
        assert_eq!(session.progress, 45);
        session.set_progress(250);
        assert_eq!(session.progress, 100);

        session.record_issue(ScanIssue {
            path: PathBuf::from(r"C:\System Volume Information"),
            kind: ScanIssueKind::AccessDenied,
            message: "access denied".to_owned(),
        });
        session.journal_state = Some(JournalState {
            journal_id: 7,
            next_usn: 42,
        });
        session.complete().expect("complete session");

        assert_eq!(session.status, ScanStatus::Completed);
        assert_eq!(session.progress, 100);
        assert!(session.finished_at.is_some());
        assert_eq!(session.issues.len(), 1);
        assert!(session.status.is_finished());
    }

    #[test]
    fn refuses_to_finish_twice() {
        let mut session = ScanSession::new(
            volume().id,
            vec![PathBuf::from(r"C:\")],
            ScanMethod::DirectoryWalk,
            Vec::new(),
        );

        session.cancel().expect("cancel session");
        assert_eq!(session.status, ScanStatus::Cancelled);
        assert!(session.complete().is_err());
        assert!(session.fail().is_err());
    }

    #[test]
    fn records_journal_state_for_serde() {
        let state = JournalState {
            journal_id: u64::MAX,
            next_usn: i64::MAX,
        };
        let json = serde_json::to_string(&state).expect("serialise journal state");

        assert_eq!(
            serde_json::from_str::<JournalState>(&json).expect("deserialise journal state"),
            state
        );
        assert_eq!(
            Timestamp::UNIX_EPOCH.format_utc(),
            "1970-01-01 00:00:00 UTC"
        );
    }
}
