//! Scan orchestration for the desktop client.
//!
//! The scanner itself lives in `freespace-scan`; this module validates the
//! interface request, translates the method choice and applies optional ADS
//! enrichment. Progress and cancellation are reported through the existing
//! [`ScanObserver`] trait.

use std::path::PathBuf;

use freespace_core::{
    model::{ScanIssue, ScanSession, Volume},
    tree::FsTree,
};
use freespace_scan::session::{
    MethodChoice, RootScanOutcome, ScanFailure, ScanManyResult, ScanObserver,
    ScanRequest as CoreScanRequest, scan_many,
};

use crate::{
    dto::{ScanMethodChoice, ScanRequest},
    error::UiError,
    validation::validate_scan_request,
};

/// Everything a finished scan produced, in interface terms.
#[derive(Debug)]
pub struct ScanOutcome {
    /// Immutable tree for the new generation.
    pub tree: FsTree,
    /// Outcomes of the successful roots, in request order.
    pub roots: Vec<RootScanOutcome>,
    /// Roots that could not be scanned.
    pub failures: Vec<ScanFailure>,
    /// User-facing warnings, including method fallback and failed roots.
    pub warnings: Vec<String>,
    /// Non-fatal problems.
    pub issues: Vec<ScanIssue>,
    /// True when the user cancelled.
    pub cancelled: bool,
    /// Number of alternate data streams found, when enrichment ran.
    pub ads_streams: u64,
}

impl ScanOutcome {
    /// Splits the result into the parts [`TreeSnapshot`](crate::tree::TreeSnapshot)
    /// needs: one arena, every successfully scanned volume and one session per
    /// volume.
    #[must_use]
    pub fn into_snapshot_parts(self) -> (FsTree, Vec<Volume>, Vec<ScanSession>) {
        let volumes = self.roots.iter().map(|root| root.volume.clone()).collect();
        let sessions = self.roots.iter().map(|root| root.session.clone()).collect();

        (self.tree, volumes, sessions)
    }
}

/// Runs a scan for an IPC request.
///
/// Every path is scanned into one arena; a failing root does not discard the
/// others. When no root succeeds the error is propagated and the previous
/// snapshot stays current.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed request and the mapped scanner error
/// for missing roots, denied access or unsupported methods.
pub fn run_scan(
    request: &ScanRequest,
    observer: &mut dyn ScanObserver,
) -> Result<ScanOutcome, UiError> {
    validate_scan_request(request)?;

    let method = method_choice(request.method);
    let requests: Vec<CoreScanRequest> = request
        .paths
        .iter()
        .map(|path| CoreScanRequest {
            root: PathBuf::from(path),
            method,
            exclusions: request.exclusions.clone(),
            incremental: request.incremental,
        })
        .collect();

    let ScanManyResult {
        mut tree,
        roots,
        failures,
        cancelled,
    } = scan_many(&requests, observer)?;

    let mut warnings: Vec<String> = roots
        .iter()
        .flat_map(|root| root.warnings.iter().cloned())
        .collect();
    for failure in &failures {
        warnings.push(format!(
            "{}: {}",
            freespace_platform::paths::display(&failure.root),
            failure.error
        ));
    }
    let issues: Vec<ScanIssue> = roots
        .iter()
        .flat_map(|root| root.issues.iter().cloned())
        .collect();

    let mut ads_streams = 0u64;
    if request.include_ads {
        let report = freespace_analysis::accounting::enrich_ads(&mut tree);
        ads_streams = u64::try_from(report.files_with_streams).unwrap_or(u64::MAX);
    }

    Ok(ScanOutcome {
        tree,
        roots,
        failures,
        warnings,
        issues,
        cancelled,
        ads_streams,
    })
}

/// Maps the method choice to the domain enum for tests and diagnostics.
#[must_use]
pub const fn method_choice(choice: ScanMethodChoice) -> MethodChoice {
    match choice {
        ScanMethodChoice::Auto => MethodChoice::Auto,
        ScanMethodChoice::Mft => MethodChoice::Mft,
        ScanMethodChoice::Walk => MethodChoice::Walk,
    }
}

#[cfg(test)]
mod tests {
    use freespace_scan::session::NoopObserver;

    use super::{method_choice, run_scan};
    use crate::dto::{Metric, ScanMethodChoice, ScanRequest};

    #[test]
    fn maps_method_choices() {
        assert_eq!(
            method_choice(ScanMethodChoice::Auto),
            freespace_scan::session::MethodChoice::Auto
        );
        assert_eq!(
            method_choice(ScanMethodChoice::Mft),
            freespace_scan::session::MethodChoice::Mft
        );
        assert_eq!(
            method_choice(ScanMethodChoice::Walk),
            freespace_scan::session::MethodChoice::Walk
        );
    }

    #[test]
    fn scans_a_temporary_directory() {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("payload.bin"), [b'a'; 32]).expect("file");
        std::fs::create_dir(directory.path().join("nested")).expect("dir");
        std::fs::write(directory.path().join("nested").join("note.txt"), b"hi").expect("file");

        let request = ScanRequest {
            paths: vec![directory.path().to_string_lossy().into_owned()],
            method: ScanMethodChoice::Walk,
            incremental: false,
            exclusions: Vec::new(),
            include_ads: false,
            metric: Metric::OnDisk,
            merge: false,
        };
        let outcome = run_scan(&request, &mut NoopObserver).expect("scan");

        assert!(!outcome.cancelled);
        assert_eq!(outcome.roots.len(), 1);
        assert_eq!(outcome.roots[0].method.as_str(), "directory-walk");
        assert!(outcome.tree.live_len() >= 4);
        assert_eq!(
            outcome.roots[0].session.nodes_count,
            outcome.tree.live_len() as u64
        );
        assert!(outcome.roots[0].volume.mount_point.exists());
    }

    #[test]
    fn rejects_relative_paths_before_scanning() {
        let request = ScanRequest {
            paths: vec!["relative".to_owned()],
            method: ScanMethodChoice::Auto,
            incremental: false,
            exclusions: Vec::new(),
            include_ads: false,
            metric: Metric::OnDisk,
            merge: false,
        };

        let error = run_scan(&request, &mut NoopObserver).expect_err("must fail");
        assert_eq!(error.code, crate::error::UiErrorCode::Validation);
    }
}
