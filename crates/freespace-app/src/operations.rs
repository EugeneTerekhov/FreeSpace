//! Preview tokens, per-volume locking and execution of destructive operations.
//!
//! Nothing is deleted without a live preview and its one-time confirmation
//! token. The service is deliberately small: it validates the request, builds
//! `freespace-ops` previews, enforces the TTL and records progress; the actual
//! file-system work stays in `freespace-ops`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use freespace_core::Timestamp;
use freespace_ops::{
    confirm::{PreviewItemKind, is_protected_path, preview_delete, preview_operation},
    delete::{DeleteOptions, delete_paths_cancellable},
    operation as ops,
    quarantine::Quarantine,
    store::Store,
    undo::undo_operation,
};
use uuid::Uuid;

use crate::{
    dto::{
        DeleteMode, JournalEntryView, JournalPage, JournalResult, NodeKind, OperationKind,
        OperationOutcome, OperationPreview, OperationPreviewRequest, OperationProgress,
        OperationState, OperationTarget,
    },
    error::{UiError, UiErrorCode},
};

/// Lifetime of a preview in milliseconds (ten minutes).
pub const PREVIEW_TTL_MS: i64 = 10 * 60 * 1_000;

/// Size above which the interface shows an extra warning.
const LARGE_OPERATION_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// A preview that has not been confirmed yet.
#[derive(Debug, Clone)]
pub struct PreparedOperation {
    /// One-time preview identifier.
    pub preview_id: String,
    /// Operation identifier assigned at confirmation time.
    pub operation_id: Uuid,
    /// Operation kind.
    pub kind: OperationKind,
    /// Targets in canonical form.
    pub targets: Vec<PathBuf>,
    /// Selected deletion mode.
    pub delete_mode: DeleteMode,
    /// Free-form reason recorded in the journal.
    pub reason: String,
    /// Whether elevation is required.
    pub requires_elevation: bool,
    /// Whether the caller explicitly accepted the extra confirmation.
    pub allow_protected: bool,
    /// One-time confirmation token.
    pub token: String,
    /// Expiry in Unix milliseconds.
    pub expires_at: i64,
}

impl PreparedOperation {
    /// True when the operation is irreversible.
    #[must_use]
    pub const fn irreversible(&self) -> bool {
        matches!(self.delete_mode, DeleteMode::Permanent)
            || matches!(self.kind, OperationKind::Shred)
    }
}

/// Live progress of one operation.
#[derive(Debug, Clone)]
struct OperationRuntime {
    cancel: Arc<AtomicBool>,
    progress: OperationProgress,
}

#[derive(Debug, Default)]
struct ServiceInner {
    previews: HashMap<String, PreparedOperation>,
    operations: HashMap<String, OperationRuntime>,
    busy_volumes: HashMap<String, String>,
}

/// Preview, confirmation and progress registry.
#[derive(Debug)]
pub struct OperationService {
    inner: Mutex<ServiceInner>,
    ttl_ms: i64,
}

impl Default for OperationService {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationService {
    /// Creates a service with the documented ten-minute TTL.
    #[must_use]
    pub fn new() -> Self {
        Self::with_ttl(PREVIEW_TTL_MS)
    }

    /// Creates a service with an explicit TTL; used by tests.
    #[must_use]
    pub fn with_ttl(ttl_ms: i64) -> Self {
        Self {
            inner: Mutex::new(ServiceInner::default()),
            ttl_ms,
        }
    }

    /// Builds a preview and stores its one-time token.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for an empty or malformed request.
    #[allow(clippy::too_many_arguments)]
    pub fn preview(
        &self,
        request: &OperationPreviewRequest,
        free_space: Option<u64>,
        now_ms: i64,
    ) -> Result<OperationPreview, UiError> {
        if request.targets.is_empty() {
            return Err(UiError::validation(
                "an operation needs at least one target",
            ));
        }
        if request.targets.len() > 10_000 {
            return Err(UiError::validation(
                "an operation affects at most 10000 targets",
            ));
        }

        let targets: Vec<PathBuf> = request
            .targets
            .iter()
            .map(|path| crate::validation::validate_path(path))
            .collect::<Result<_, _>>()?;
        let (preview, requires_elevation, dto_targets) =
            self.build_preview(request.kind, &targets, free_space)?;

        let mode = match (request.kind, request.delete_mode) {
            (OperationKind::Shred, Some(DeleteMode::Permanent) | None) => DeleteMode::Permanent,
            (OperationKind::Delete, Some(mode)) => mode,
            (OperationKind::Delete, None) => DeleteMode::RecycleBin,
            (_, Some(mode)) => mode,
            _ => DeleteMode::Permanent,
        };
        let mut warnings = Vec::new();
        if requires_elevation {
            warnings.push("confirm.warning.protected".to_owned());
            warnings.push("confirm.warning.elevation".to_owned());
        }
        if preview
            .items
            .iter()
            .any(|item| matches!(item.kind, PreviewItemKind::Missing))
        {
            warnings.push("confirm.warning.missing".to_owned());
        }
        if preview.total_bytes >= LARGE_OPERATION_BYTES {
            warnings.push("confirm.warning.large".to_owned());
        }
        if matches!(mode, DeleteMode::Permanent) || matches!(request.kind, OperationKind::Shred) {
            warnings.push("confirm.warning.permanent".to_owned());
        }

        let preview_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let reversible =
            !matches!(mode, DeleteMode::Permanent) && !matches!(request.kind, OperationKind::Shred);
        let expires_at = now_ms.saturating_add(self.ttl_ms);
        let dto_preview = OperationPreview {
            preview_id: preview_id.clone(),
            kind: request.kind,
            targets: dto_targets,
            total_size: preview.total_bytes,
            delete_mode: Some(mode),
            reversible,
            requires_elevation,
            warnings,
            expires_at,
            confirmation_token: token.clone(),
        };

        self.lock().previews.insert(
            preview_id.clone(),
            PreparedOperation {
                preview_id,
                operation_id: Uuid::new_v4(),
                kind: request.kind,
                targets,
                delete_mode: mode,
                reason: request
                    .reason
                    .clone()
                    .unwrap_or_else(|| "cleanup".to_owned()),
                requires_elevation,
                allow_protected: false,
                token,
                expires_at,
            },
        );

        Ok(dto_preview)
    }

    /// Consumes a preview after verifying its token, TTL and confirmation.
    ///
    /// # Errors
    /// Returns `E_PREVIEW_EXPIRED` for an unknown, used or expired preview,
    /// `E_VALIDATION` without the extra confirmation for irreversible work and
    /// `E_ELEVATION_REQUIRED` when the process is not elevated.
    pub fn take(
        &self,
        preview_id: &str,
        confirmation_token: &str,
        delete_mode: Option<DeleteMode>,
        extra_confirmation: bool,
        elevated: bool,
        now_ms: i64,
    ) -> Result<PreparedOperation, UiError> {
        let mut inner = self.lock();
        let Some(mut prepared) = inner.previews.get(preview_id).cloned() else {
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        };
        if prepared.token != confirmation_token || now_ms > prepared.expires_at {
            inner.previews.remove(preview_id);
            return Err(UiError::new(UiErrorCode::PreviewExpired));
        }
        if let Some(mode) = delete_mode {
            if prepared.kind == OperationKind::Shred && mode != DeleteMode::Permanent {
                return Err(UiError::validation("shredding is always permanent"));
            }
            prepared.delete_mode = mode;
        }
        if prepared.requires_elevation && !elevated {
            return Err(UiError::new(UiErrorCode::ElevationRequired));
        }
        if prepared.irreversible() && !extra_confirmation {
            return Err(UiError::validation(
                "irreversible operations need a separate confirmation",
            ));
        }

        prepared.allow_protected = prepared.requires_elevation && extra_confirmation;
        inner.previews.remove(preview_id);

        Ok(prepared)
    }

    /// Registers a started operation and reserves its volume.
    ///
    /// # Errors
    /// Returns `E_BUSY` when another destructive operation already runs on the
    /// volume.
    pub fn begin(&self, operation_id: &str, volume_id: &str, total: u64) -> Result<(), UiError> {
        let mut inner = self.lock();
        if let Some(active) = inner.busy_volumes.iter().find_map(|(id, volume)| {
            (volume == volume_id && id.as_str() != operation_id).then_some(id)
        }) {
            return Err(UiError::new(UiErrorCode::Busy)
                .with_details(format!("volume `{volume_id}` is busy with `{active}`")));
        }
        inner
            .busy_volumes
            .insert(operation_id.to_owned(), volume_id.to_owned());
        inner.operations.insert(
            operation_id.to_owned(),
            OperationRuntime {
                cancel: Arc::new(AtomicBool::new(false)),
                progress: OperationProgress {
                    operation_id: operation_id.to_owned(),
                    state: OperationState::Running,
                    processed: 0,
                    total,
                    current_target: None,
                    bytes_per_second: None,
                    errors: Vec::new(),
                    undo_available: false,
                    journal_entry_id: None,
                },
            },
        );

        Ok(())
    }

    /// Updates progress after one processed target.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown operation.
    pub fn progress(
        &self,
        operation_id: &str,
        processed: u64,
        current_target: Option<String>,
    ) -> Result<OperationProgress, UiError> {
        let mut inner = self.lock();
        let runtime = inner
            .operations
            .get_mut(operation_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("unknown operation"))?;
        runtime.progress.processed = processed;
        runtime.progress.current_target = current_target;

        Ok(runtime.progress.clone())
    }

    /// Returns the current state of an operation.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown operation.
    pub fn status(&self, operation_id: &str) -> Result<OperationProgress, UiError> {
        self.lock()
            .operations
            .get(operation_id)
            .map(|runtime| runtime.progress.clone())
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("unknown operation"))
    }

    /// Requests cooperative cancellation.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown operation and `E_UNSUPPORTED` when
    /// the operation already reached a terminal state.
    pub fn cancel(&self, operation_id: &str) -> Result<(), UiError> {
        let mut inner = self.lock();
        let runtime = inner
            .operations
            .get_mut(operation_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("unknown operation"))?;
        if runtime.progress.state.is_finished() {
            return Err(UiError::new(UiErrorCode::Unsupported)
                .with_details("the operation has already finished"));
        }
        runtime.cancel.store(true, Ordering::Relaxed);
        runtime.progress.state = OperationState::Cancelling;

        Ok(())
    }

    /// Returns the cancellation flag of an operation.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown operation.
    pub fn cancel_flag(&self, operation_id: &str) -> Result<Arc<AtomicBool>, UiError> {
        self.lock()
            .operations
            .get(operation_id)
            .map(|runtime| Arc::clone(&runtime.cancel))
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("unknown operation"))
    }

    /// Marks an operation terminal and releases its volume.
    pub fn finish(
        &self,
        operation_id: &str,
        state: OperationState,
        errors: Vec<UiError>,
        journal_entry_id: Option<String>,
        undo_available: bool,
    ) {
        let mut inner = self.lock();
        if let Some(mut runtime) = inner.operations.remove(operation_id) {
            runtime.progress.state = state;
            runtime.progress.errors = errors;
            runtime.progress.journal_entry_id = journal_entry_id;
            runtime.progress.undo_available = undo_available;
            runtime.progress.processed = runtime.progress.total;
            inner.operations.insert(operation_id.to_owned(), runtime);
        }
        inner.busy_volumes.remove(operation_id);
    }

    /// Number of live previews; used by tests.
    #[must_use]
    pub fn preview_count(&self) -> usize {
        self.lock().previews.len()
    }

    fn build_preview(
        &self,
        kind: OperationKind,
        targets: &[PathBuf],
        free_space: Option<u64>,
    ) -> Result<(freespace_ops::confirm::Preview, bool, Vec<OperationTarget>), UiError> {
        let preview = match kind {
            OperationKind::Delete | OperationKind::Shred => preview_delete(targets, free_space)?,
            OperationKind::Move | OperationKind::Rename | OperationKind::Permissions => {
                let operation_targets: Vec<ops::OperationTarget> = targets
                    .iter()
                    .map(|path| {
                        let size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
                        ops::OperationTarget::new(path.clone(), size)
                    })
                    .collect();
                let operation = ops::Operation::new(
                    match kind {
                        OperationKind::Move => ops::OperationKind::Move,
                        OperationKind::Rename => ops::OperationKind::Rename,
                        _ => ops::OperationKind::Permissions,
                    },
                    operation_targets,
                );
                preview_operation(&operation, free_space)
            }
            OperationKind::Restore => preview_delete(targets, free_space)?,
        };

        let requires_elevation = preview.requires_elevation;
        let dto_targets = preview
            .items
            .iter()
            .map(|item| {
                let path = item.path.to_string_lossy().into_owned();
                OperationTarget {
                    path: path.clone(),
                    kind: match item.kind {
                        PreviewItemKind::File => NodeKind::File,
                        PreviewItemKind::Directory => NodeKind::Directory,
                        PreviewItemKind::Link => NodeKind::Link,
                        PreviewItemKind::Missing | PreviewItemKind::Unknown => NodeKind::Other,
                    },
                    size: item.size,
                    accessible: !matches!(
                        item.kind,
                        PreviewItemKind::Missing | PreviewItemKind::Unknown
                    ),
                    is_system: is_protected_path(Path::new(&path)),
                }
            })
            .collect();

        Ok((preview, requires_elevation, dto_targets))
    }

    fn lock(&self) -> MutexGuard<'_, ServiceInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Executes a confirmed delete or shred operation.
///
/// Progress is reported through `on_progress`; cancellation is checked between
/// targets. A partial result is returned as an outcome, not as an error.
///
/// # Errors
/// Returns the mapped `freespace-ops` error when the operation cannot run at
/// all.
#[allow(clippy::too_many_arguments)]
pub fn execute_delete(
    service: &OperationService,
    store: &Store,
    quarantine: &Quarantine,
    prepared: &PreparedOperation,
    volume_id: &str,
    available_bytes: Option<u64>,
    on_progress: &mut dyn FnMut(&OperationProgress),
) -> Result<OperationOutcome, UiError> {
    let operation_id = prepared.operation_id.to_string();
    service.begin(&operation_id, volume_id, prepared.targets.len() as u64)?;
    let cancel = service.cancel_flag(&operation_id)?;
    let options = DeleteOptions {
        mode: match prepared.delete_mode {
            DeleteMode::RecycleBin => ops::DeleteMode::RecycleBin,
            DeleteMode::Quarantine => ops::DeleteMode::Quarantine,
            DeleteMode::Permanent => ops::DeleteMode::Permanent,
        },
        reason: prepared.reason.clone(),
        retention_days: 30,
        available_bytes,
        allow_protected: prepared.allow_protected,
    };

    let mut on_target = |processed: usize, _total: usize, path: &Path| {
        let current = path.to_string_lossy().into_owned();
        if let Ok(progress) = service.progress(&operation_id, processed as u64, Some(current)) {
            on_progress(&progress);
        }
    };
    let outcome = delete_paths_cancellable(
        store,
        Some(quarantine),
        &prepared.targets,
        &options,
        true,
        prepared.operation_id,
        Some(cancel.as_ref()),
        &mut on_target,
    );

    match outcome {
        Ok(outcome) => {
            let errors = outcome
                .items
                .iter()
                .filter_map(|item| {
                    item.error.as_ref().map(|error| {
                        UiError::new(UiErrorCode::Io)
                            .with_details(format!("{}: {error}", item.path.to_string_lossy()))
                    })
                })
                .collect::<Vec<_>>();
            let quarantine_ids = outcome
                .items
                .iter()
                .filter_map(|item| item.quarantine_id)
                .map(|id| id.to_string())
                .collect();
            let state = operation_state(outcome.status);
            let journal_entry_id = outcome.journal_id.map(|id| id.to_string());
            service.finish(
                &operation_id,
                state,
                errors.clone(),
                journal_entry_id.clone(),
                outcome.undoable,
            );
            let processed = outcome.summary.succeeded + outcome.summary.failed;
            Ok(OperationOutcome {
                operation_id,
                state,
                journal_entry_id,
                undo_available: outcome.undoable,
                processed: u64::from(processed),
                total: prepared.targets.len() as u64,
                errors,
                quarantine_ids,
            })
        }
        Err(error) => {
            let mapped: UiError = error.into();
            service.finish(
                &operation_id,
                OperationState::Failed,
                vec![mapped.clone()],
                None,
                false,
            );
            Err(mapped)
        }
    }
}

const fn operation_state(status: ops::OperationStatus) -> OperationState {
    match status {
        ops::OperationStatus::Completed => OperationState::Completed,
        ops::OperationStatus::PartiallyCompleted => OperationState::Partial,
        ops::OperationStatus::Cancelled => OperationState::Cancelled,
        ops::OperationStatus::Failed
        | ops::OperationStatus::RolledBack
        | ops::OperationStatus::Planned
        | ops::OperationStatus::Confirmed
        | ops::OperationStatus::Running => OperationState::Failed,
    }
}

/// Reverses a journalled operation through the quarantine.
///
/// # Errors
/// Returns the mapped undo error and `E_UNSUPPORTED` when the operation cannot
/// be undone.
pub fn undo(store: &Store, quarantine: &Quarantine, operation_id: &str) -> Result<u32, UiError> {
    let parsed = Uuid::parse_str(operation_id)
        .map_err(|error| UiError::validation(format!("malformed operation id: {error}")))?;
    let outcome = undo_operation(store, quarantine, parsed)?;

    Ok(outcome.restored)
}

/// Reads one page of the journal, newest first.
///
/// # Errors
/// Returns the mapped store error and `E_VALIDATION` for a malformed page size.
pub fn journal_page(
    store: &Store,
    filter: &crate::dto::JournalFilter,
    page_size: u16,
    cursor: Option<&str>,
) -> Result<JournalPage, UiError> {
    crate::validation::validate_page_size(page_size)?;
    let mut entries: Vec<JournalEntryView> = store
        .journal_list(10_000)?
        .iter()
        .map(journal_view)
        .filter(|entry| journal_matches(entry, filter))
        .collect();
    entries.sort_by_key(|left| std::cmp::Reverse(left.occurred_at));
    let offset = cursor
        .map(|cursor| cursor.parse::<usize>())
        .transpose()
        .map_err(|_| UiError::new(UiErrorCode::StaleCursor))?
        .unwrap_or(0);
    let end = offset
        .saturating_add(usize::from(page_size))
        .min(entries.len());
    let rows = entries[offset.min(entries.len())..end].to_vec();
    let next_cursor = (end < entries.len()).then(|| end.to_string());

    Ok(JournalPage {
        cursor: cursor.map(str::to_owned),
        next_cursor,
        rows,
        total_estimate: entries.len() as u64,
    })
}

fn journal_matches(entry: &JournalEntryView, filter: &crate::dto::JournalFilter) -> bool {
    if !filter.kinds.is_empty() && !filter.kinds.contains(&entry.kind) {
        return false;
    }
    if !filter.results.is_empty() && !filter.results.contains(&entry.result) {
        return false;
    }
    let text = filter.text.trim().to_lowercase();
    if text.is_empty() {
        return true;
    }

    entry.user.to_lowercase().contains(&text)
        || entry.operation_id.to_lowercase().contains(&text)
        || entry
            .quarantine_ids
            .iter()
            .any(|id| id.to_lowercase().contains(&text))
}

/// Returns one journal row by identifier.
///
/// # Errors
/// Returns `E_NOT_FOUND` when the row is absent.
pub fn journal_entry(store: &Store, entry_id: &str) -> Result<JournalEntryView, UiError> {
    let wanted: u64 = entry_id
        .parse()
        .map_err(|_| UiError::new(UiErrorCode::NotFound))?;
    store
        .journal_list(10_000)?
        .iter()
        .find(|entry| entry.id == wanted)
        .map(journal_view)
        .ok_or_else(|| UiError::new(UiErrorCode::NotFound))
}

fn journal_view(entry: &ops::JournalEntry) -> JournalEntryView {
    let details = &entry.details;
    let kind = details
        .get("kind")
        .and_then(|value| value.as_str())
        .and_then(parse_operation_kind)
        .unwrap_or(OperationKind::Delete);
    let status = details.get("status").and_then(|value| value.as_str());
    let result = match status {
        Some("cancelled") => JournalResult::Cancelled,
        _ => match entry.result {
            ops::JournalResult::Success => JournalResult::Success,
            ops::JournalResult::Partial => JournalResult::Partial,
            ops::JournalResult::Error => JournalResult::Failed,
        },
    };
    let quarantine_ids = details
        .get("quarantine_ids")
        .and_then(|value| value.as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let error = (result == JournalResult::Failed).then(|| {
        UiError::new(UiErrorCode::Io)
            .with_details(entry.summary.clone())
            .with_journal(entry.id.to_string())
    });

    JournalEntryView {
        id: entry.id.to_string(),
        operation_id: entry.operation_id.to_string(),
        occurred_at: entry.timestamp.as_millis(),
        user: entry.user.clone(),
        kind,
        targets_count: entry.affected_count,
        total_size: entry.affected_bytes,
        result,
        reversible: entry.undo_available,
        quarantine_ids,
        error,
    }
}

fn parse_operation_kind(value: &str) -> Option<OperationKind> {
    match value {
        "delete" => Some(OperationKind::Delete),
        "move" => Some(OperationKind::Move),
        "rename" => Some(OperationKind::Rename),
        "permissions" => Some(OperationKind::Permissions),
        "shred" => Some(OperationKind::Shred),
        "restore" => Some(OperationKind::Restore),
        _ => None,
    }
}

/// Returns the current local user for the journal.
#[must_use]
pub fn current_user() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_owned())
}

/// Formats a timestamp for test fixtures.
#[must_use]
pub fn now_millis() -> i64 {
    Timestamp::now().as_millis()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        OperationService, PREVIEW_TTL_MS, delete_paths_cancellable, execute_delete, journal_page,
        undo,
    };
    use crate::dto::{DeleteMode, OperationKind, OperationPreviewRequest, OperationState};
    use crate::error::UiErrorCode;
    use freespace_ops::{DeleteOptions, Quarantine, Store};

    fn request(path: &std::path::Path) -> OperationPreviewRequest {
        OperationPreviewRequest {
            kind: OperationKind::Delete,
            targets: vec![path.to_string_lossy().into_owned()],
            delete_mode: Some(DeleteMode::Quarantine),
            reason: Some("test".to_owned()),
            passes: None,
            task_id: None,
        }
    }

    #[test]
    fn preview_is_one_time_and_expires() {
        let directory = tempfile::tempdir().expect("temp dir");
        let file = directory.path().join("a.txt");
        fs::write(&file, b"payload").expect("file");
        let service = OperationService::with_ttl(PREVIEW_TTL_MS);
        let preview = service
            .preview(&request(&file), Some(1_000), 0)
            .expect("preview");

        assert_eq!(preview.total_size, 7);
        assert!(preview.reversible);
        assert_eq!(preview.delete_mode, Some(DeleteMode::Quarantine));
        assert_eq!(service.preview_count(), 1);

        service
            .take(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                false,
                1,
            )
            .expect("take");
        assert_eq!(service.preview_count(), 0);
        let error = service
            .take(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                false,
                2,
            )
            .expect_err("reused");
        assert_eq!(error.code, UiErrorCode::PreviewExpired);
    }

    #[test]
    fn irreversible_work_needs_extra_confirmation_and_elevation() {
        let directory = tempfile::tempdir().expect("temp dir");
        let file = directory.path().join("a.txt");
        fs::write(&file, b"payload").expect("file");

        let protected = std::path::PathBuf::from(
            std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()),
        )
        .join("System32")
        .join("kernel32.dll");
        let mut protected_request = request(&protected);
        protected_request.delete_mode = Some(DeleteMode::Permanent);
        let service = OperationService::with_ttl(PREVIEW_TTL_MS);
        let preview = service
            .preview(&protected_request, None, 0)
            .expect("preview");
        assert!(preview.requires_elevation);
        assert!(!preview.reversible);

        let error = service
            .take(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                true,
                false,
                1,
            )
            .expect_err("elevation");
        assert_eq!(error.code, UiErrorCode::ElevationRequired);

        let preview = service
            .preview(&protected_request, None, 0)
            .expect("preview");
        let error = service
            .take(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                true,
                1,
            )
            .expect_err("extra confirmation");
        assert_eq!(error.code, UiErrorCode::Validation);
    }

    #[test]
    fn executes_cancellable_quarantine_delete_and_undoes_it() {
        let workspace = tempfile::tempdir().expect("workspace");
        let root = tempfile::tempdir().expect("quarantine");
        let store = Store::open_in_memory().expect("store");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");
        let first = workspace.path().join("first.txt");
        let second = workspace.path().join("second.txt");
        fs::write(&first, b"one").expect("first");
        fs::write(&second, b"two").expect("second");

        let service = OperationService::new();
        let mut request = request(&first);
        request.targets.push(second.to_string_lossy().into_owned());
        let preview = service.preview(&request, None, 0).expect("preview");
        let prepared = service
            .take(
                &preview.preview_id,
                &preview.confirmation_token,
                Some(DeleteMode::Quarantine),
                false,
                false,
                1,
            )
            .expect("take");

        let mut seen = Vec::new();
        let outcome = execute_delete(
            &service,
            &store,
            &quarantine,
            &prepared,
            "volume-1",
            None,
            &mut |progress| seen.push(progress.processed),
        )
        .expect("execute");

        assert_eq!(outcome.state, OperationState::Completed);
        assert_eq!(outcome.processed, 2);
        assert!(!first.exists());
        assert!(!second.exists());
        assert!(outcome.undo_available);
        assert_eq!(seen, [1, 2]);
        assert!(matches!(
            service.status(&outcome.operation_id).expect("status").state,
            OperationState::Completed
        ));

        let restored = undo(&store, &quarantine, &outcome.operation_id).expect("undo");
        assert_eq!(restored, 2);
        assert!(first.exists());
        assert!(second.exists());
    }

    #[test]
    fn cancellation_keeps_the_actual_partial_result() {
        let workspace = tempfile::tempdir().expect("workspace");
        let root = tempfile::tempdir().expect("quarantine");
        let store = Store::open_in_memory().expect("store");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");
        let first = workspace.path().join("first.txt");
        fs::write(&first, b"one").expect("first");

        let service = OperationService::new();
        let preview = service.preview(&request(&first), None, 0).expect("preview");
        let prepared = service
            .take(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                false,
                1,
            )
            .expect("take");
        service
            .begin(&prepared.operation_id.to_string(), "volume-2", 1)
            .expect("begin");
        service
            .cancel(&prepared.operation_id.to_string())
            .expect("cancel");

        // The cancellation flag is checked before the first target.
        let cancel = service
            .cancel_flag(&prepared.operation_id.to_string())
            .expect("flag");
        let outcome = delete_paths_cancellable(
            &store,
            Some(&quarantine),
            &prepared.targets,
            &DeleteOptions::new(freespace_ops::DeleteMode::Quarantine),
            true,
            prepared.operation_id,
            Some(cancel.as_ref()),
            &mut |_, _, _| {},
        )
        .expect("cancelled outcome");

        assert_eq!(outcome.status, freespace_ops::OperationStatus::Cancelled);
        assert!(first.exists(), "nothing is removed when cancellation wins");
        assert_eq!(
            service
                .status(&prepared.operation_id.to_string())
                .expect("status")
                .state,
            OperationState::Cancelling
        );
    }

    #[test]
    fn journals_are_paged_and_carry_the_operation_identifier() {
        let directory = tempfile::tempdir().expect("temp dir");
        let root = tempfile::tempdir().expect("quarantine");
        let store = Store::open_in_memory().expect("store");
        let quarantine = Quarantine::new(root.path().join("store")).expect("quarantine");
        let file = directory.path().join("a.txt");
        fs::write(&file, b"payload").expect("file");

        let service = OperationService::new();
        let preview = service.preview(&request(&file), None, 0).expect("preview");
        let prepared = service
            .take(
                &preview.preview_id,
                &preview.confirmation_token,
                None,
                false,
                false,
                1,
            )
            .expect("take");
        let outcome = execute_delete(
            &service,
            &store,
            &quarantine,
            &prepared,
            "volume-3",
            None,
            &mut |_| {},
        )
        .expect("execute");

        let page =
            journal_page(&store, &crate::dto::JournalFilter::default(), 10, None).expect("page");
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].operation_id, outcome.operation_id);
        assert!(page.rows[0].reversible);
        assert_eq!(page.rows[0].kind, OperationKind::Delete);
        assert_eq!(page.rows[0].result, crate::dto::JournalResult::Success);
    }
}
