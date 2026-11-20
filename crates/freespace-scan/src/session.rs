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
use freespace_platform::{paths, volumes};

#[cfg(windows)]
use freespace_platform::mft;

use crate::walk::{self, Exclusions};
#[cfg(windows)]
use crate::{mft_scan, usn};

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

/// Maximum number of roots in one scan.
pub const MAX_SCAN_ROOTS: usize = 32;

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

/// One root that failed inside a multi-root scan.
#[derive(Debug)]
pub struct ScanFailure {
    /// Root that could not be scanned.
    pub root: PathBuf,
    /// Cause of the failure.
    pub error: Error,
}

/// Everything one root produced inside a multi-root scan.
#[derive(Debug)]
pub struct RootScanOutcome {
    /// Session metadata for this root.
    pub session: ScanSession,
    /// Method that actually ran for this root.
    pub method: ScanMethod,
    /// Volume that contains this root.
    pub volume: Volume,
    /// Number of excluded entries below this root.
    pub excluded: u64,
    /// Non-fatal problems below this root.
    pub issues: Vec<ScanIssue>,
    /// User-facing notes for this root.
    pub warnings: Vec<String>,
    /// Whether this root stopped because of cancellation.
    pub cancelled: bool,
}

impl RootScanOutcome {
    fn into_result(self, tree: FsTree) -> ScanResult {
        ScanResult {
            tree,
            session: self.session,
            method: self.method,
            volume: self.volume,
            excluded: self.excluded,
            issues: self.issues,
            warnings: self.warnings,
            cancelled: self.cancelled,
        }
    }
}

/// Everything a multi-root scan produced.
#[derive(Debug)]
pub struct ScanManyResult {
    /// One arena with every successfully scanned root.
    pub tree: FsTree,
    /// Outcomes of the successful roots, in request order.
    pub roots: Vec<RootScanOutcome>,
    /// Roots that could not be scanned.
    pub failures: Vec<ScanFailure>,
    /// Whether the user cancelled before every root finished.
    pub cancelled: bool,
}

/// Number of records the scanner skipped because they were unreadable.
///
/// The MFT scanner reports corrupt records; both scanners report issues,
/// exclusions and cancellation. Keeping the shape internal lets the session
/// compile on platforms without an MFT scanner.
#[derive(Debug, Default)]
struct Outcome {
    excluded: u64,
    issues: Vec<ScanIssue>,
    cancelled: bool,
    corrupt_records: u64,
}

/// Runs a scan according to `request`.
///
/// # Errors
/// Returns an error when the root is missing, the method is unavailable, or the
/// scanner cannot start.
pub fn scan(request: &ScanRequest, observer: &mut dyn ScanObserver) -> Result<ScanResult> {
    let mut tree = FsTree::new();
    let outcome = scan_into(request, &mut tree, observer)?;
    finish_tree(&mut tree)?;
    let current = outcome.session.roots.first().cloned().unwrap_or_default();

    observer.progress(&ScanProgress {
        phase: ScanPhase::Finished,
        nodes: outcome.session.nodes_count,
        bytes: total_logical(&tree),
        fraction: 1.0,
        current,
    });

    Ok(outcome.into_result(tree))
}

/// Appends one root to an existing arena.
///
/// The root is appended as a new `FsTree` root; earlier roots and their
/// identifiers are untouched. The method choice, exclusions and progress are
/// the same as for [`scan`]. Sizes are aggregated later by the caller, so a
/// multi-root scan pays for `recompute_totals` only once.
///
/// # Errors
/// Returns an error when the root cannot be read or scanned. A partially
/// appended root is rolled back before the error is returned.
pub fn scan_into(
    request: &ScanRequest,
    tree: &mut FsTree,
    observer: &mut dyn ScanObserver,
) -> Result<RootScanOutcome> {
    let mark = tree.len();
    match scan_root(request, tree, observer, mark) {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            tree.truncate(mark);
            Err(error)
        }
    }
}

/// Rejects an empty list, too many roots, duplicates and overlapping roots.
fn validate_roots(requests: &[ScanRequest]) -> Result<()> {
    if requests.is_empty() {
        return Err(Error::InvalidArgument(
            "at least one scan root is required".to_owned(),
        ));
    }
    if requests.len() > MAX_SCAN_ROOTS {
        return Err(Error::InvalidArgument(format!(
            "at most {MAX_SCAN_ROOTS} scan roots are supported"
        )));
    }

    let mut keys = Vec::with_capacity(requests.len());
    for request in requests {
        // A missing root is reported by the scanner so the other roots can
        // still complete; validation only needs a stable comparison key.
        let normalized = paths::normalize(&request.root).unwrap_or_else(|_| request.root.clone());
        let key = PathBuf::from(paths::equivalent_key(&normalized));
        if keys.iter().any(|existing| existing == &key) {
            return Err(Error::InvalidArgument(format!(
                "scan root `{}` is listed more than once",
                paths::display(&normalized)
            )));
        }
        keys.push(key);
    }

    for (index, root) in keys.iter().enumerate() {
        for other in &keys[index + 1..] {
            if root.starts_with(other) || other.starts_with(root) {
                return Err(Error::InvalidArgument(format!(
                    "scan roots `{}` and `{}` overlap",
                    root.display(),
                    other.display()
                )));
            }
        }
    }

    Ok(())
}

/// Scans several roots sequentially into one arena.
///
/// A failing root does not stop the others; the failure is reported in
/// [`ScanManyResult::failures`]. Cancellation stops the current and every
/// remaining root. When no root succeeds, the first error is returned and the
/// caller keeps the previous snapshot.
///
/// # Errors
/// Returns `E_INVALID_ARGUMENT` for an empty request and the first root error
/// when every root failed. Aggregation errors are propagated as well.
pub fn scan_many(
    requests: &[ScanRequest],
    observer: &mut dyn ScanObserver,
) -> Result<ScanManyResult> {
    validate_roots(requests)?;

    let mut tree = FsTree::new();
    let mut roots = Vec::new();
    let mut failures = Vec::new();
    let mut cancelled = false;
    let total = requests.len();

    for (index, request) in requests.iter().enumerate() {
        if observer.is_cancelled() {
            cancelled = true;
            break;
        }

        let mut overall = OverallObserver {
            inner: observer,
            index,
            total,
        };
        match scan_into(request, &mut tree, &mut overall) {
            Ok(outcome) => {
                cancelled = outcome.cancelled;
                roots.push(outcome);
                if cancelled {
                    break;
                }
            }
            Err(error) => failures.push(ScanFailure {
                root: request.root.clone(),
                error,
            }),
        }
    }

    if roots.is_empty() && !failures.is_empty() {
        return Err(failures.remove(0).error);
    }

    finish_tree(&mut tree)?;
    let current = requests
        .last()
        .map_or_else(PathBuf::new, |request| request.root.clone());
    observer.progress(&ScanProgress {
        phase: ScanPhase::Finished,
        nodes: u64::try_from(tree.live_len()).unwrap_or(u64::MAX),
        bytes: total_logical(&tree),
        fraction: 1.0,
        current,
    });

    Ok(ScanManyResult {
        tree,
        roots,
        failures,
        cancelled,
    })
}

fn scan_root(
    request: &ScanRequest,
    tree: &mut FsTree,
    observer: &mut dyn ScanObserver,
    mark: usize,
) -> Result<RootScanOutcome> {
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

    #[cfg_attr(not(windows), allow(unused_mut))]
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

    let mut session = ScanSession::new(
        volume.id,
        vec![root.clone()],
        method,
        exclusions.patterns().to_vec(),
    );

    let outcome = match method {
        #[cfg(windows)]
        ScanMethod::Mft => match mft_scan::scan(&root, tree, &exclusions, observer) {
            Ok(outcome) => Outcome {
                excluded: outcome.excluded,
                issues: outcome.issues,
                cancelled: outcome.cancelled,
                corrupt_records: outcome.corrupt_records,
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
                // The MFT scanner may have appended a partial root; drop it
                // before the walk starts so the arena stays consistent.
                tree.truncate(mark);
                session.method = ScanMethod::DirectoryWalk;
                method = ScanMethod::DirectoryWalk;
                let fallback = walk::scan(&root, tree, &exclusions, observer)?;
                Outcome {
                    excluded: fallback.excluded,
                    issues: fallback.issues,
                    cancelled: fallback.cancelled,
                    corrupt_records: 0,
                }
            }
            Err(error) => return Err(error),
        },
        #[cfg(not(windows))]
        ScanMethod::Mft => {
            return Err(Error::Unsupported(
                "MFT scanning is not available on this platform".to_owned(),
            ));
        }
        ScanMethod::DirectoryWalk | ScanMethod::Usn => {
            let outcome = walk::scan(&root, tree, &exclusions, observer)?;
            Outcome {
                excluded: outcome.excluded,
                issues: outcome.issues,
                cancelled: outcome.cancelled,
                corrupt_records: 0,
            }
        }
    };

    observer.progress(&ScanProgress::new(ScanPhase::Aggregating, 0.9));

    for issue in &outcome.issues {
        session.record_issue(issue.clone());
        observer.issue(issue);
    }
    session.nodes_count = u64::try_from(tree.len().saturating_sub(mark)).unwrap_or(u64::MAX);

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

    Ok(RootScanOutcome {
        session,
        method,
        volume,
        excluded: outcome.excluded,
        issues: outcome.issues,
        warnings,
        cancelled: outcome.cancelled,
    })
}

fn finish_tree(tree: &mut FsTree) -> Result<()> {
    tree.recompute_totals();
    tree.validate()?;
    tree.compact();

    Ok(())
}

fn total_logical(tree: &FsTree) -> u64 {
    tree.roots()
        .map(|root| tree.node(root).size.logical)
        .fold(0, u64::saturating_add)
}

/// Maps a single root's local progress into the overall multi-root fraction.
struct OverallObserver<'observer> {
    inner: &'observer mut dyn ScanObserver,
    index: usize,
    total: usize,
}

impl ScanObserver for OverallObserver<'_> {
    fn progress(&mut self, progress: &ScanProgress) {
        let total = self.total.max(1) as f32;
        let fraction = ((self.index as f32) + progress.fraction) / total;

        self.inner.progress(&ScanProgress {
            fraction: fraction.clamp(0.0, 1.0),
            ..progress.clone()
        });
    }

    fn issue(&mut self, issue: &ScanIssue) {
        self.inner.issue(issue);
    }

    fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

/// Applies a journal delta to an existing tree.
///
/// Only available on Windows, where the USN journal exists.
///
/// # Errors
/// Returns an error when the journal cannot be read.
#[cfg(windows)]
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
    let methods = freespace_platform::platform().capabilities.scan_methods;

    match choice {
        MethodChoice::Walk => Ok(ScanMethod::DirectoryWalk),
        MethodChoice::Mft => {
            if !methods.mft {
                return Err(Error::Unsupported(
                    "MFT scanning is not available on this platform".to_owned(),
                ));
            }
            ensure_mft_available(volume, at_volume_root)?;
            #[cfg(windows)]
            mft::mft_record_size(root)?;

            Ok(ScanMethod::Mft)
        }
        MethodChoice::Auto => {
            if !methods.mft || !at_volume_root || !volume.file_system.supports_mft() {
                return Ok(ScanMethod::DirectoryWalk);
            }
            #[cfg(windows)]
            {
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
            #[cfg(not(windows))]
            {
                let _ = (root, warnings);
                Ok(ScanMethod::DirectoryWalk)
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
    let root = paths::normalize(&paths::volume_root(&normalized)?)?;

    Ok(paths::equivalent_key(&normalized) == paths::equivalent_key(&root))
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::is_volume_root;
    use super::{
        MethodChoice, ScanObserver, ScanPhase, ScanProgress, ScanRequest, scan, scan_many,
    };
    use crate::session::NoopObserver;

    /// Records fractions so a test can assert that progress never goes back.
    #[derive(Default)]
    struct RecordingObserver {
        fractions: Vec<f32>,
        cancelled: bool,
    }

    impl ScanObserver for RecordingObserver {
        fn progress(&mut self, progress: &ScanProgress) {
            self.fractions.push(progress.fraction);
        }

        fn is_cancelled(&self) -> bool {
            self.cancelled
        }
    }

    fn walk_request(path: &std::path::Path) -> ScanRequest {
        ScanRequest {
            method: MethodChoice::Walk,
            ..ScanRequest::new(path)
        }
    }

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

    #[cfg(windows)]
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

    #[cfg(not(windows))]
    #[test]
    fn rejects_mft_on_platforms_without_it() {
        let directory = tempfile::tempdir().expect("temp dir");
        let request = ScanRequest {
            method: MethodChoice::Mft,
            ..ScanRequest::new(directory.path())
        };

        let error = scan(&request, &mut NoopObserver).expect_err("must fail");

        assert_eq!(error.code().as_str(), "E_UNSUPPORTED");
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

    #[test]
    fn scans_several_roots_into_one_tree() {
        let first = tempfile::tempdir().expect("first root");
        std::fs::write(first.path().join("a.bin"), [b'a'; 16]).expect("file");
        let second = tempfile::tempdir().expect("second root");
        std::fs::write(second.path().join("b.bin"), [b'b'; 32]).expect("file");

        let requests = [walk_request(first.path()), walk_request(second.path())];
        let result = scan_many(&requests, &mut NoopObserver).expect("multi scan");

        assert_eq!(result.roots.len(), 2);
        assert_eq!(result.tree.roots().count(), 2);
        assert!(result.failures.is_empty());
        assert!(!result.cancelled);
        assert_eq!(
            result
                .roots
                .iter()
                .map(|root| root.session.nodes_count)
                .sum::<u64>(),
            u64::try_from(result.tree.live_len()).expect("node count")
        );
        assert_eq!(
            result.roots[0].session.roots,
            vec![freespace_platform::paths::normalize(first.path()).expect("normalized")]
        );
        assert_eq!(
            result.roots[1].session.roots,
            vec![freespace_platform::paths::normalize(second.path()).expect("normalized")]
        );
        result.tree.validate().expect("one consistent arena");
    }

    #[test]
    fn rejects_duplicate_and_overlapping_roots() {
        let directory = tempfile::tempdir().expect("root");
        std::fs::create_dir(directory.path().join("nested")).expect("nested");

        let duplicate = scan_many(
            &[
                walk_request(directory.path()),
                walk_request(directory.path()),
            ],
            &mut NoopObserver,
        )
        .expect_err("duplicate roots");
        assert_eq!(duplicate.code().as_str(), "E_INVALID_ARGUMENT");

        let nested = directory.path().join("nested");
        let overlap = scan_many(
            &[walk_request(directory.path()), walk_request(&nested)],
            &mut NoopObserver,
        )
        .expect_err("overlapping roots");
        assert_eq!(overlap.code().as_str(), "E_INVALID_ARGUMENT");
    }

    #[test]
    fn keeps_scanning_after_one_root_fails() {
        let good = tempfile::tempdir().expect("root");
        std::fs::write(good.path().join("kept.bin"), [b'k'; 8]).expect("file");
        let other = tempfile::tempdir().expect("other root");
        let missing = other.path().join("gone");

        let requests = [walk_request(good.path()), walk_request(&missing)];
        let result = scan_many(&requests, &mut NoopObserver).expect("partial scan");

        assert_eq!(result.roots.len(), 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].root, missing);
        assert_eq!(result.tree.roots().count(), 1);
    }

    #[test]
    fn returns_the_first_error_when_every_root_fails() {
        let directory = tempfile::tempdir().expect("root");
        let missing = directory.path().join("gone");

        let error =
            scan_many(&[walk_request(&missing)], &mut NoopObserver).expect_err("all roots failed");

        assert_eq!(error.code().as_str(), "E_NOT_FOUND");
    }

    #[test]
    fn overall_progress_is_monotonic_and_finishes_at_one() {
        let first = tempfile::tempdir().expect("first root");
        std::fs::write(first.path().join("a.bin"), [b'a'; 4]).expect("file");
        let second = tempfile::tempdir().expect("second root");
        std::fs::write(second.path().join("b.bin"), [b'b'; 4]).expect("file");

        let requests = [walk_request(first.path()), walk_request(second.path())];
        let mut observer = RecordingObserver::default();
        scan_many(&requests, &mut observer).expect("multi scan");

        assert!(
            observer.fractions.windows(2).all(|pair| pair[0] <= pair[1]),
            "progress went backwards: {:?}",
            observer.fractions
        );
        assert_eq!(observer.fractions.last().copied(), Some(1.0));
    }

    #[test]
    fn cancellation_before_the_first_root_stops_the_run() {
        let directory = tempfile::tempdir().expect("root");
        let mut observer = RecordingObserver {
            cancelled: true,
            ..RecordingObserver::default()
        };

        let result =
            scan_many(&[walk_request(directory.path())], &mut observer).expect("cancelled scan");

        assert!(result.cancelled);
        assert!(result.roots.is_empty());
        assert!(result.failures.is_empty());
    }
}
