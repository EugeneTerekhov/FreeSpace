//! Scan orchestration for the desktop client.
//!
//! The scanner itself lives in `freespace-scan`; this module validates the
//! interface request, translates the method choice and applies optional ADS
//! enrichment. Progress and cancellation are reported through the existing
//! [`ScanObserver`] trait.

use std::path::PathBuf;

use freespace_core::{
    model::{ScanIssue, ScanMethod, ScanSession, Volume},
    tree::FsTree,
};
use freespace_scan::session::{
    MethodChoice, ScanObserver, ScanRequest as CoreScanRequest, ScanResult, scan,
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
    /// Session metadata.
    pub session: ScanSession,
    /// Method that actually ran.
    pub method: ScanMethod,
    /// Analysed volume.
    pub volume: Volume,
    /// User-facing warnings, including method fallback.
    pub warnings: Vec<String>,
    /// Non-fatal problems.
    pub issues: Vec<ScanIssue>,
    /// True when the user cancelled.
    pub cancelled: bool,
    /// Number of alternate data streams found, when enrichment ran.
    pub ads_streams: u64,
}

/// Runs a scan for an IPC request.
///
/// # Errors
/// Returns `E_VALIDATION` for a malformed request and the mapped scanner error
/// for missing roots, denied access or unsupported methods.
pub fn run_scan(
    request: &ScanRequest,
    observer: &mut dyn ScanObserver,
) -> Result<ScanOutcome, UiError> {
    validate_scan_request(request)?;

    let method = match request.method {
        ScanMethodChoice::Auto => MethodChoice::Auto,
        ScanMethodChoice::Mft => MethodChoice::Mft,
        ScanMethodChoice::Walk => MethodChoice::Walk,
    };
    let core_request = CoreScanRequest {
        root: PathBuf::from(&request.path),
        method,
        exclusions: request.exclusions.clone(),
        incremental: request.incremental,
    };

    let ScanResult {
        mut tree,
        session,
        method,
        volume,
        issues,
        warnings,
        cancelled,
        ..
    } = scan(&core_request, observer)?;

    let mut ads_streams = 0u64;
    if request.include_ads {
        let report = freespace_analysis::accounting::enrich_ads(&mut tree);
        ads_streams = u64::try_from(report.files_with_streams).unwrap_or(u64::MAX);
    }

    Ok(ScanOutcome {
        tree,
        session,
        method,
        volume,
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
            path: directory.path().to_string_lossy().into_owned(),
            method: ScanMethodChoice::Walk,
            incremental: false,
            exclusions: Vec::new(),
            include_ads: false,
            metric: Metric::OnDisk,
        };
        let outcome = run_scan(&request, &mut NoopObserver).expect("scan");

        assert!(!outcome.cancelled);
        assert_eq!(outcome.method.as_str(), "directory-walk");
        assert!(outcome.tree.live_len() >= 4);
        assert_eq!(outcome.session.nodes_count, outcome.tree.live_len() as u64);
        assert!(outcome.volume.mount_point.exists());
    }

    #[test]
    fn rejects_relative_paths_before_scanning() {
        let request = ScanRequest {
            path: "relative".to_owned(),
            method: ScanMethodChoice::Auto,
            incremental: false,
            exclusions: Vec::new(),
            include_ads: false,
            metric: Metric::OnDisk,
        };

        let error = run_scan(&request, &mut NoopObserver).expect_err("must fail");
        assert_eq!(error.code, crate::error::UiErrorCode::Validation);
    }
}
