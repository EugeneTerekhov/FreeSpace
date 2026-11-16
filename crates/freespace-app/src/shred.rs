//! Shredder preview, confirmation and execution (US6, T125).
//!
//! Shredding is always irreversible. The preview reuses the shared operation
//! token service, so `shred_start` accepts exactly one live preview with its
//! one-time token and expires after the documented ten minutes. The overwrite
//! itself stays in `freespace-ops::shred`; this module only validates the
//! parameters, keeps the plan between preview and start, reports progress and
//! writes the journal entry.

use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, MutexGuard},
};

use crate::{
    dto::{
        DeleteMode, OperationKind, OperationOutcome, OperationPreview, OperationPreviewRequest,
        OperationProgress, OperationState, ShredPatternChoice, ShredPreviewRequest,
    },
    error::{UiError, UiErrorCode},
    operations::{OperationService, PREVIEW_TTL_MS, PreparedOperation, current_user},
};
use freespace_ops::{
    ShredOptions,
    operation::ShredPattern,
    shred::{self as ops_shred, SSD_WARNING},
    store::Store,
};
use freespace_platform::paths;

/// Smallest accepted number of overwrite passes.
pub const MIN_PASSES: u32 = 1;
/// Largest accepted number of overwrite passes.
pub const MAX_PASSES: u32 = 35;
/// Warning key shown for every shred preview.
pub const IRREVERSIBLE_WARNING_KEY: &str = "shred.irreversible";
/// Warning key shown when the target lives on solid-state media.
pub const SSD_WARNING_KEY: &str = "shred.ssd_warning";
/// Warning key shown when the operation affects a protected path.
pub const PROTECTED_WARNING_KEY: &str = "confirm.warning.protected";

/// Parameters that survive between `shred_preview` and `shred_start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShredPlan {
    /// Number of overwrite passes.
    pub passes: u32,
    /// Overwrite pattern.
    pub pattern: ShredPattern,
}

#[derive(Debug, Clone, Copy)]
struct PlanEntry {
    plan: ShredPlan,
    expires_at: i64,
}

#[derive(Debug, Default)]
struct Inner {
    plans: HashMap<String, PlanEntry>,
}

/// Registry that binds a shred plan to its one-time preview.
#[derive(Debug, Default)]
pub struct ShredService {
    inner: Mutex<Inner>,
}

impl ShredService {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a shred preview and remembers its passes and pattern.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for empty targets or invalid parameters and the
    /// operation-service errors for malformed paths.
    pub fn preview(
        &self,
        operations: &OperationService,
        request: &ShredPreviewRequest,
        free_space: Option<u64>,
        now_ms: i64,
    ) -> Result<OperationPreview, UiError> {
        let plan = validate_request(request)?;
        let operation_request = OperationPreviewRequest {
            kind: OperationKind::Shred,
            targets: request.targets.clone(),
            delete_mode: Some(DeleteMode::Permanent),
            reason: Some("shred".to_owned()),
            passes: Some(plan.passes),
            task_id: None,
        };
        let mut preview = operations.preview(&operation_request, free_space, now_ms)?;

        if !preview.reversible {
            preview.warnings.push(IRREVERSIBLE_WARNING_KEY.to_owned());
        }
        if let Some(storage) = request
            .targets
            .first()
            .and_then(|target| freespace_platform::trim::storage_kind(Path::new(target)).ok())
        {
            if storage.needs_warning() {
                preview.warnings.push(SSD_WARNING_KEY.to_owned());
            }
        }

        let mut inner = self.lock();
        inner.plans.retain(|_, entry| entry.expires_at >= now_ms);
        inner.plans.insert(
            preview.preview_id.clone(),
            PlanEntry {
                plan,
                expires_at: now_ms.saturating_add(PREVIEW_TTL_MS),
            },
        );

        Ok(preview)
    }

    /// Consumes a preview and returns the confirmed shred plan.
    ///
    /// Starting a shred is the separate destructive confirmation: the client
    /// only calls it after the dedicated button in the irreversible dialog.
    ///
    /// # Errors
    /// Returns `E_PREVIEW_EXPIRED` for an unknown, used or expired preview,
    /// `E_ELEVATION_REQUIRED` without administrative rights and `E_VALIDATION`
    /// for malformed input.
    pub fn take(
        &self,
        operations: &OperationService,
        preview_id: &str,
        confirmation_token: &str,
        elevated: bool,
        now_ms: i64,
    ) -> Result<(PreparedOperation, ShredPlan), UiError> {
        let prepared = operations.take(
            preview_id,
            confirmation_token,
            Some(DeleteMode::Permanent),
            true,
            elevated,
            now_ms,
        )?;
        let entry = self
            .lock()
            .plans
            .remove(preview_id)
            .ok_or_else(|| UiError::new(UiErrorCode::PreviewExpired))?;

        if entry.expires_at < now_ms || prepared.kind != OperationKind::Shred {
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        }

        Ok((prepared, entry.plan))
    }

    /// Number of remembered plans; used by tests.
    #[must_use]
    pub fn plan_count(&self) -> usize {
        self.lock().plans.len()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Overwrites and deletes every confirmed target.
///
/// The caller must have taken a live preview first. Progress is reported after
/// every target, cancellation is checked between targets, and a partial result
/// is returned as an outcome instead of an error.
///
/// # Errors
/// Returns the mapped `freespace-ops` error when the run cannot start at all.
pub fn execute(
    service: &OperationService,
    store: &Store,
    prepared: &PreparedOperation,
    plan: ShredPlan,
    volume_id: &str,
    on_progress: &mut dyn FnMut(&OperationProgress),
) -> Result<OperationOutcome, UiError> {
    let options = ShredOptions {
        passes: plan.passes,
        pattern: plan.pattern,
    };
    options.validate()?;

    let operation_id = prepared.operation_id.to_string();
    service.begin(&operation_id, volume_id, prepared.targets.len() as u64)?;

    execute_started(service, store, prepared, plan, on_progress)
}

/// Runs a shred operation that already holds its volume lock.
///
/// Kept separate so cancellation can be tested without racing a worker thread.
fn execute_started(
    service: &OperationService,
    store: &Store,
    prepared: &PreparedOperation,
    plan: ShredPlan,
    on_progress: &mut dyn FnMut(&OperationProgress),
) -> Result<OperationOutcome, UiError> {
    let options = ShredOptions {
        passes: plan.passes,
        pattern: plan.pattern,
    };
    options.validate()?;

    let operation_id = prepared.operation_id.to_string();
    let total = prepared.targets.len() as u64;
    let cancel = service.cancel_flag(&operation_id)?;

    let mut outcomes = Vec::new();
    let mut errors = Vec::new();
    let mut processed = 0u64;
    let mut cancelled = false;

    for target in &prepared.targets {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        processed = processed.saturating_add(1);
        let current = paths::display(target);
        if let Ok(progress) = service.progress(&operation_id, processed, Some(current)) {
            on_progress(&progress);
        }
        match ops_shred::shred(target, options, true) {
            Ok(outcome) => outcomes.push(outcome),
            Err(error) => errors.push(target_error(target, &error)),
        }
    }

    let journal_entry_id = if outcomes.is_empty() {
        None
    } else {
        match ops_shred::record(store, &outcomes, &current_user()) {
            Ok(id) => Some(id.to_string()),
            Err(error) => {
                errors.push(
                    UiError::new(UiErrorCode::Io)
                        .with_details(format!("the shred run could not be journalled: {error}")),
                );
                None
            }
        }
    };

    let state = if cancelled {
        OperationState::Cancelled
    } else if errors.is_empty() {
        OperationState::Completed
    } else if outcomes.is_empty() {
        OperationState::Failed
    } else {
        OperationState::Partial
    };

    service.finish(
        &operation_id,
        state,
        errors.clone(),
        journal_entry_id.clone(),
        false,
    );

    Ok(OperationOutcome {
        operation_id,
        state,
        journal_entry_id,
        undo_available: false,
        processed,
        total,
        errors,
        quarantine_ids: Vec::new(),
    })
}

/// Returns the SSD warning text shipped by `freespace-ops`.
#[must_use]
pub fn ssd_warning_text() -> &'static str {
    SSD_WARNING
}

fn validate_request(request: &ShredPreviewRequest) -> Result<ShredPlan, UiError> {
    if !(MIN_PASSES..=MAX_PASSES).contains(&request.passes) {
        return Err(UiError::validation(format!(
            "passes must be between {MIN_PASSES} and {MAX_PASSES}"
        )));
    }

    let pattern = match request.pattern {
        ShredPatternChoice::Zeros => ShredPattern::Zeros,
        ShredPatternChoice::Random => ShredPattern::Random,
    };

    Ok(ShredPlan {
        passes: request.passes,
        pattern,
    })
}

fn target_error(path: &Path, error: &freespace_core::Error) -> UiError {
    UiError::new(UiErrorCode::from_core(error.code()))
        .with_details(format!("{}: {error}", paths::display(path)))
}

/// Converts a wire pattern without failing on an unknown value.
#[must_use]
pub fn pattern_from_wire(value: &str) -> Option<ShredPattern> {
    ShredPatternChoice::parse_value(value).map(|pattern| match pattern {
        ShredPatternChoice::Zeros => ShredPattern::Zeros,
        ShredPatternChoice::Random => ShredPattern::Random,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use freespace_ops::Store;

    use super::{IRREVERSIBLE_WARNING_KEY, MAX_PASSES, ShredService, execute, pattern_from_wire};
    use crate::dto::{OperationState, ShredPatternChoice, ShredPreviewRequest};
    use crate::error::UiErrorCode;
    use crate::operations::OperationService;

    fn request(paths: &[&std::path::Path], passes: u32) -> ShredPreviewRequest {
        ShredPreviewRequest {
            targets: paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            passes,
            pattern: ShredPatternChoice::Random,
        }
    }

    fn take_impl(
        service: &ShredService,
        operations: &OperationService,
        preview: &crate::dto::OperationPreview,
        now: i64,
    ) -> Result<(crate::operations::PreparedOperation, super::ShredPlan), crate::error::UiError>
    {
        service.take(
            operations,
            &preview.preview_id,
            &preview.confirmation_token,
            false,
            now,
        )
    }

    #[test]
    fn validates_the_pass_range_and_pattern() {
        let service = ShredService::new();
        let operations = OperationService::new();
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("a.bin");
        fs::write(&path, b"payload").expect("write");

        for passes in [0, MAX_PASSES + 1] {
            let error = service
                .preview(&operations, &request(&[&path], passes), None, 0)
                .expect_err("passes");
            assert_eq!(error.code, UiErrorCode::Validation);
        }
        assert_eq!(service.plan_count(), 0);
        assert_eq!(
            pattern_from_wire("zeros").map(|p| p.as_str()),
            Some("zeros")
        );
        assert_eq!(pattern_from_wire("magic"), None);
    }

    #[test]
    fn preview_is_irreversible_and_confirmation_is_one_time() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("secret.bin");
        fs::write(&path, b"payload").expect("write");
        let service = ShredService::new();
        let operations = OperationService::with_ttl(1_000);
        let preview = service
            .preview(&operations, &request(&[&path], 3), None, 0)
            .expect("preview");

        assert!(!preview.reversible);
        assert_eq!(preview.kind, crate::dto::OperationKind::Shred);
        assert!(
            preview
                .warnings
                .iter()
                .any(|warning| warning == IRREVERSIBLE_WARNING_KEY)
        );
        assert!(
            preview
                .warnings
                .iter()
                .any(|warning| warning == "confirm.warning.permanent")
        );

        let error = service
            .take(&operations, &preview.preview_id, "wrong-token", false, 1)
            .expect_err("token");
        assert_eq!(error.code, UiErrorCode::PreviewExpired);

        let preview = service
            .preview(&operations, &request(&[&path], 2), None, 0)
            .expect("second preview");
        let (_, plan) = take_impl(&service, &operations, &preview, 1).expect("take");
        assert_eq!(plan.passes, 2);
        assert_eq!(plan.pattern, freespace_ops::operation::ShredPattern::Random);

        let error = service
            .take(
                &operations,
                &preview.preview_id,
                &preview.confirmation_token,
                false,
                2,
            )
            .expect_err("reuse");
        assert_eq!(error.code, UiErrorCode::PreviewExpired);
    }

    #[test]
    fn start_requires_a_live_preview_and_never_touches_the_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("secret.bin");
        fs::write(&path, b"payload").expect("write");
        let service = ShredService::new();
        let operations = OperationService::new();

        let error = service
            .take(&operations, "missing-preview", "missing-token", false, 0)
            .expect_err("no preview");
        assert_eq!(error.code, UiErrorCode::PreviewExpired);
        assert!(path.exists());

        let preview = service
            .preview(&operations, &request(&[&path], 1), None, 0)
            .expect("preview");
        let error = service
            .take(
                &operations,
                &preview.preview_id,
                &preview.confirmation_token,
                false,
                10 * 60 * 1_000 + 1,
            )
            .expect_err("expired");
        assert_eq!(error.code, UiErrorCode::PreviewExpired);
        assert!(path.exists());
    }

    #[test]
    fn destroys_confirmed_files_and_journals_the_run() {
        let directory = tempfile::tempdir().expect("temp dir");
        let first = directory.path().join("first.bin");
        let second = directory.path().join("second.bin");
        fs::write(&first, vec![7u8; 4_096]).expect("first");
        fs::write(&second, vec![9u8; 2_048]).expect("second");
        let store = Store::open_in_memory().expect("store");
        let service = ShredService::new();
        let operations = OperationService::new();
        let preview = service
            .preview(&operations, &request(&[&first, &second], 2), None, 0)
            .expect("preview");
        let (prepared, plan) = take_impl(&service, &operations, &preview, 1).expect("take");

        let mut progress = Vec::new();
        let outcome = execute(
            &operations,
            &store,
            &prepared,
            plan,
            "volume-shred",
            &mut |event| progress.push(event.processed),
        )
        .expect("execute");

        assert_eq!(outcome.state, OperationState::Completed);
        assert_eq!(outcome.processed, 2);
        assert!(!outcome.undo_available);
        assert!(outcome.journal_entry_id.is_some());
        assert!(!first.exists());
        assert!(!second.exists());
        assert_eq!(progress, [1, 2]);

        let entries = store.journal_list(10).expect("journal");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].details["pattern"], "random");
        assert_eq!(entries[0].details["passes"], 2);
    }

    #[test]
    fn a_directory_fails_without_changing_anything() {
        let directory = tempfile::tempdir().expect("temp dir");
        let store = Store::open_in_memory().expect("store");
        let service = ShredService::new();
        let operations = OperationService::new();
        let preview = service
            .preview(&operations, &request(&[directory.path()], 1), None, 0)
            .expect("preview");
        let (prepared, plan) = take_impl(&service, &operations, &preview, 1).expect("take");

        let outcome = execute(
            &operations,
            &store,
            &prepared,
            plan,
            "volume-shred",
            &mut |_| {},
        )
        .expect("outcome");

        assert_eq!(outcome.state, OperationState::Failed);
        assert_eq!(outcome.errors.len(), 1);
        assert!(directory.path().exists());
        assert!(store.journal_list(10).expect("journal").is_empty());
    }

    #[test]
    fn cancellation_before_the_first_target_removes_nothing() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("secret.bin");
        fs::write(&path, b"payload").expect("write");
        let store = Store::open_in_memory().expect("store");
        let service = ShredService::new();
        let operations = OperationService::new();
        let preview = service
            .preview(&operations, &request(&[&path], 1), None, 0)
            .expect("preview");
        let (prepared, plan) = take_impl(&service, &operations, &preview, 1).expect("take");

        operations
            .begin(&prepared.operation_id.to_string(), "volume-shred", 1)
            .expect("begin");
        operations
            .cancel(&prepared.operation_id.to_string())
            .expect("cancel");

        let outcome = super::execute_started(&operations, &store, &prepared, plan, &mut |_| {})
            .expect("cancelled outcome");

        assert_eq!(outcome.state, OperationState::Cancelled);
        assert_eq!(outcome.processed, 0);
        assert!(path.exists(), "nothing is destroyed after cancellation");
        assert!(store.journal_list(10).expect("journal").is_empty());
    }
}
