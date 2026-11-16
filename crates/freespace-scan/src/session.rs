//! Scan orchestration: method selection, progress, cancellation and session
//! bookkeeping.
//!
//! A scan always produces a [`FsTree`] plus the [`ScanSession`] that records
//! what happened (method, issues, counts). The chosen method is never hidden:
//! when MFT access is denied the fallback to the directory walk is reported as
//! a warning (FR-004).

use std::{fs, path::Path, path::PathBuf};

use freespace_core::{
    Error, Result,
    model::{ScanIssue, ScanMethod, ScanSession, Volume},
    tree::FsTree,
};
use freespace_platform::{mft, paths, volumes};

use crate::{
    mft_scan, usn,
    walk::{self, Exclusions},
};

/// Which scanner the caller asks for.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodChoice {
    /// MFT for whole NTFS volumes when possible, otherwise the directory walk.
    #[default]
    Auto,
    /// Require the MFT scanner; fail when it is unavailable.
    Mft,
    /// Always walk the directory tree.
    Walk,
}

impl MethodChoice {
    /// Parses a command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "mft" | "usn" => Some(Self::Mft),
            "walk" | "directory-walk" => Some(Self::Walk),
            _ => None,
        }
    }
}

/// Phase of a running scan.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanPhase {
    /// Validating the request and querying the volume.
    #[default]
    Preparing,
    /// Reading the MFT or journal.
    Enumerating,
    /// Building the arena.
    Building,
    /// Aggregating directory sizes.
    Aggregating,
    /// The scan finished.
    Finished,
}

impl ScanPhase {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Enumerating => "enumerating",
            Self::Building => "building",
            Self::Aggregating => "aggregating",
            Self::Finished => "finished",
        }
    }
}

/// Progress snapshot sent to a frontend.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanProgress {
    /// Current phase.
    pub phase: ScanPhase,
    /// Nodes discovered so far.
    pub nodes: u64,
    /// Logical bytes discovered so far.
    pub bytes: u64,
    /// Completion fraction in `0.0..=1.0`.
    pub fraction: f32,
    /// Object being processed, for the status line.
    pub current: PathBuf,
}

impl ScanProgress {
    /// Creates a clamped progress snapshot.
    #[must_use]
    pub fn new(phase: ScanPhase, fraction: f32) -> Self {
        Self {
            phase,
            nodes: 0,
            bytes: 0,
            fraction: fraction.clamp(0.0, 1.0),
            current: PathBuf::new(),
        }
    }
}

/// Frontend callbacks used by both scanners.
pub trait ScanObserver {
    /// Reports progress; implementations should be cheap.
    fn progress(&mut self, _progress: &ScanProgress) {}

    /// Reports a non-fatal problem.
    fn issue(&mut self, _issue: &ScanIssue) {}

    /// Returns `true` when the user asked to stop.
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Observer that ignores every event; used by tests and the CLI.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopObserver;

impl ScanObserver for NoopObserver {}

/// Parameters accepted by [`scan`].
#[derive(Debug, Clone)]
pub struct ScanRequest {
    /// Volume root or directory to analyse.
    pub root: PathBuf,
    /// Requested scanner.
    pub method: MethodChoice,
    /// Glob patterns excluded from the analysis.
    pub exclusions: Vec<String>,
    /// Ask for an incremental USN update when a journal state is available.
    pub incremental: bool,
}

impl ScanRequest {
    /// Creates an automatic scan request without exclusions.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            method: MethodChoice::Auto,
            exclusions: Vec::new(),
            incremental: false,
        }
    }
}

/// Everything a completed scan produced.
#[derive(Debug)]
pub struct ScanResult {
    /// The analysed tree with aggregated sizes.
    pub tree: FsTree,
    /// Session metadata.
    pub session: ScanSession,
    /// Method that actually ran.
    pub method: ScanMethod,
    /// Analysed volume.
    pub volume: Volume,
    /// Number of excluded entries.
    pub excluded: u64,
    /// Non-fatal problems.
    pub issues: Vec<ScanIssue>,
    /// User-facing notes, for example a method fallback.
    pub warnings: Vec<String>,
    /// Whether the scan stopped because of cancellation.
    pub cancelled: bool,
}

/// Runs a scan according to `request`.
///
/// # Errors
/// Returns an error when the root is missing, the method is unavailable, or the
/// scanner cannot start.
pub fn scan(request: &ScanRequest, observer: &mut dyn ScanObserver) -> Result<ScanResult> {
    let root = paths::normalize(&request.root)?;
    let metadata = fs::metadata(&root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound(paths::display(&root))
        } else {
            Error::Io(error)
        }
    })?;
    if !metadata.is_dir() {
        return Err(Error::InvalidArgument(format!(
            "`{}` is not a directory",
            paths::display(&root)
        )));
    }

    let mut warnings = Vec::new();
    let volume_info = volumes::describe_volume(&root)?;
    let mut volume = volume_info.into_volume();
    let exclusions = Exclusions::new(&request.exclusions)?;
    let at_volume_root = is_volume_root(&root)?;

    let mut method = resolve_method(
        request.method,
        &volume,
        at_volume_root,
        &root,
        &mut warnings,
    )?;

    if request.incremental {
        warnings.push(
            "incremental updates need a stored journal position; a full scan was performed"
                .to_owned(),
        );
    }

    observer.progress(&ScanProgress::new(ScanPhase::Preparing, 0.02));

    let mut tree = FsTree::new();
    let mut session = ScanSession::new(
        volume.id,
        vec![root.clone()],
        method,
        exclusions.patterns().to_vec(),
    );

    let outcome = match method {
        ScanMethod::Mft => match mft_scan::scan(&root, &mut tree, &exclusions, observer) {
            Ok(outcome) => mft_scan::ScanOutcome {
                excluded: outcome.excluded,
                issues: outcome.issues,
                cancelled: outcome.cancelled,
                corrupt_records: outcome.corrupt_records,
                records: outcome.records,
            },
            Err(error)
                if request.method == MethodChoice::Auto
                    && matches!(
                        error.code(),
                        freespace_core::ErrorCode::AccessDenied
                            | freespace_core::ErrorCode::Unsupported
                    ) =>
            {
                warnings.push(format!(
                        "MFT access failed while reading records ({error}); the directory walk was used instead"
                    ));
                tree = FsTree::new();
                session.method = ScanMethod::DirectoryWalk;
                method = ScanMethod::DirectoryWalk;
                let fallback = walk::scan(&root, &mut tree, &exclusions, observer)?;
                mft_scan::ScanOutcome {
                    excluded: fallback.excluded,
                    issues: fallback.issues,
                    cancelled: fallback.cancelled,
                    corrupt_records: 0,
                    records: 0,
                }
            }
            Err(error) => return Err(error),
        },
        ScanMethod::DirectoryWalk | ScanMethod::Usn => {
            let outcome = walk::scan(&root, &mut tree, &exclusions, observer)?;
            mft_scan::ScanOutcome {
                excluded: outcome.excluded,
                issues: outcome.issues,
                cancelled: outcome.cancelled,
                corrupt_records: 0,
                records: 0,
            }
        }
    };

    observer.progress(&ScanProgress::new(ScanPhase::Aggregating, 0.9));
    tree.recompute_totals();
    tree.validate()?;
    tree.compact();

    for issue in &outcome.issues {
        session.record_issue(issue.clone());
        observer.issue(issue);
    }
    session.nodes_count = u64::try_from(tree.live_len()).unwrap_or(u64::MAX);

    if outcome.cancelled {
        session.cancel()?;
    } else {
        session.complete()?;
    }
    session.set_progress(if outcome.cancelled { 0 } else { 100 });

    if outcome.corrupt_records > 0 {
        warnings.push(format!(
            "{} MFT records were skipped because their update sequence did not match",
            outcome.corrupt_records
        ));
    }
    if outcome.excluded > 0 {
        warnings.push(format!("{} entries were excluded", outcome.excluded));
    }

    volume.scan_method = Some(method);
    volume.is_elevated = method == ScanMethod::Mft;

    observer.progress(&ScanProgress {
        phase: ScanPhase::Finished,
        nodes: session.nodes_count,
        bytes: tree
            .roots()
            .next()
            .map_or(0, |root| tree.node(root).size.logical),
        fraction: 1.0,
        current: root,
    });

    Ok(ScanResult {
        tree,
        session,
        method,
        volume,
        excluded: outcome.excluded,
        issues: outcome.issues,
        warnings,
        cancelled: outcome.cancelled,
    })
}

/// Applies a journal delta to an existing tree.
///
/// # Errors
/// Returns an error when the journal cannot be read.
pub fn update_incremental(
    root: &Path,
    tree: &mut FsTree,
    state: freespace_core::model::JournalState,
) -> Result<usn::UsnUpdateReport> {
    usn::update(root, tree, state)
}

fn resolve_method(
    choice: MethodChoice,
    volume: &Volume,
    at_volume_root: bool,
    root: &Path,
    warnings: &mut Vec<String>,
) -> Result<ScanMethod> {
    match choice {
        MethodChoice::Walk => Ok(ScanMethod::DirectoryWalk),
        MethodChoice::Mft => {
            ensure_mft_available(volume, at_volume_root)?;
            mft::mft_record_size(root)?;

            Ok(ScanMethod::Mft)
        }
        MethodChoice::Auto => {
            if !at_volume_root || !volume.file_system.supports_mft() {
                return Ok(ScanMethod::DirectoryWalk);
            }
            match mft::mft_record_size(root) {
                Ok(_) => Ok(ScanMethod::Mft),
                Err(error) => {
                    warnings.push(format!(
                        "MFT access is unavailable ({error}); the directory walk was used instead"
                    ));
                    Ok(ScanMethod::DirectoryWalk)
                }
            }
        }
    }
}

fn ensure_mft_available(volume: &Volume, at_volume_root: bool) -> Result<()> {
    if !volume.file_system.supports_mft() {
        return Err(Error::Unsupported(format!(
            "MFT scanning needs NTFS, but `{}` is {}",
            volume.mount_point.display(),
            volume.file_system.as_str()
        )));
    }
    if !at_volume_root {
        return Err(Error::InvalidArgument(
            "MFT scanning works on whole volumes; use `--method walk` for a directory".to_owned(),
        ));
    }

    Ok(())
}

/// Returns `true` when `path` is the root of its volume.
///
/// # Errors
/// Returns an error when the volume root cannot be resolved.
pub fn is_volume_root(path: &Path) -> Result<bool> {
    let normalized = paths::normalize(path)?;
    let root = paths::volume_root(&normalized)?;
    let left = paths::display(&normalized)
        .trim_end_matches('\\')
        .to_lowercase();
    let right = paths::display(&root).trim_end_matches('\\').to_lowercase();

    Ok(left == right)
}

#[cfg(test)]
mod tests {
    use super::{MethodChoice, ScanPhase, ScanProgress, ScanRequest, is_volume_root, scan};
    use crate::session::NoopObserver;

    #[test]
    fn parses_method_names() {
        assert_eq!(MethodChoice::parse_value("auto"), Some(MethodChoice::Auto));
        assert_eq!(MethodChoice::parse_value("MFT"), Some(MethodChoice::Mft));
        assert_eq!(MethodChoice::parse_value("walk"), Some(MethodChoice::Walk));
        assert_eq!(MethodChoice::parse_value("magic"), None);
        assert_eq!(ScanPhase::Preparing.as_str(), "preparing");
    }

    #[test]
    fn clamps_progress_and_reports_phases() {
        let progress = ScanProgress::new(ScanPhase::Building, 2.0);

        assert_eq!(progress.fraction, 1.0);
        assert_eq!(progress.phase.as_str(), "building");
    }

    #[test]
    fn scans_a_directory_with_the_walk_method() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("payload.bin"), [b'a'; 64]).expect("file");

        let request = ScanRequest {
            method: MethodChoice::Walk,
            ..ScanRequest::new(directory.path())
        };
        let result = scan(&request, &mut NoopObserver).expect("scan");

        assert_eq!(result.method.as_str(), "directory-walk");
        assert_eq!(result.session.nodes_count, 2);
        assert_eq!(result.tree.roots().count(), 1);
        assert!(!result.cancelled);
        assert_eq!(result.session.status.as_str(), "completed");
        assert_eq!(result.volume.scan_method, Some(result.method));
    }

    #[test]
    fn reports_missing_roots() {
        let directory = tempfile::tempdir().expect("temp dir");
        let missing = directory.path().join("gone");
        let request = ScanRequest::new(missing);

        let error = scan(&request, &mut NoopObserver).expect_err("must fail");

        assert_eq!(error.code().as_str(), "E_NOT_FOUND");
    }

    #[test]
    fn rejects_mft_for_plain_directories() {
        let directory = tempfile::tempdir().expect("temp dir");
        let request = ScanRequest {
            method: MethodChoice::Mft,
            ..ScanRequest::new(directory.path())
        };

        let error = scan(&request, &mut NoopObserver).expect_err("must fail");

        assert_eq!(error.code().as_str(), "E_INVALID_ARGUMENT");
        assert!(!is_volume_root(directory.path()).unwrap_or(true));
    }

    #[test]
    fn auto_falls_back_to_the_walk_without_privileges() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("payload.bin"), [b'a'; 8]).expect("file");
        let request = ScanRequest::new(directory.path());

        let result = scan(&request, &mut NoopObserver).expect("scan");

        // A temporary directory is not a volume root, so auto always walks.
        assert_eq!(result.method.as_str(), "directory-walk");
        assert!(result.tree.live_len() >= 2);
    }
}
