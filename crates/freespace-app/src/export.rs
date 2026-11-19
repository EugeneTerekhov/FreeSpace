//! Report export preview, start, status and cancel (US5, T114).
//!
//! An export never starts from a raw request: the interface first asks for a
//! preview, which validates the output path, reports section totals and returns
//! a one-time identifier and confirmation token. Starting consumes the preview,
//! runs the report writers on a worker thread and reports progress through
//! `export_status`; no new event types are introduced.
//!
//! Cancellation is cooperative: the worker checks the flag between sections and
//! before the file is written, so nothing is left half-written by a cancel.
//! Overwriting an existing file requires explicit consent in the preview.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use freespace_analysis::{
    DuplicateOptions, annotate, find_duplicates, group_by_extension, top_files,
};
use freespace_core::{Timestamp, size::SizeMetrics, tree::FsTree};
use freespace_platform::paths;
use freespace_report::{Report, ReportFormat, ReportMeta, ReportVolume, SectionName, write_report};
use uuid::Uuid;

use crate::{
    dto::{
        ExportFormat, ExportPreview, ExportPreviewRequest, ExportSection, ExportStatus,
        PathValidation, TaskStarted, TaskState,
    },
    error::{UiError, UiErrorCode},
    tree::TreeSnapshot,
    validation::validate_path,
};

/// Lifetime of an export preview in milliseconds (ten minutes).
pub const EXPORT_PREVIEW_TTL_MS: i64 = 10 * 60 * 1_000;

/// Number of largest files included by an export.
pub const EXPORT_TOP_LIMIT: usize = 1_000;

/// Hard cap on tree rows in one report.
pub const MAX_EXPORT_TREE_ROWS: usize = 5_000_000;

/// A preview that has not been started yet.
#[derive(Debug, Clone)]
struct PreparedExport {
    format: ExportFormat,
    out: PathBuf,
    sections: Vec<ExportSection>,
    max_tree_rows: Option<usize>,
    overwrite: bool,
    snapshot: Arc<TreeSnapshot>,
    expires_at: i64,
    token: String,
}

/// One running or finished export.
#[derive(Debug)]
struct ExportRuntime {
    status: ExportStatus,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
struct ExportInner {
    previews: HashMap<String, PreparedExport>,
    tasks: HashMap<String, ExportRuntime>,
}

/// Preview registry and export workers.
#[derive(Debug, Clone)]
pub struct ExportService {
    inner: Arc<Mutex<ExportInner>>,
    ttl_ms: i64,
    start_delay: Duration,
}

impl Default for ExportService {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportService {
    /// Creates a service with the documented ten-minute TTL.
    #[must_use]
    pub fn new() -> Self {
        Self::with_options(EXPORT_PREVIEW_TTL_MS, Duration::ZERO)
    }

    /// Creates a service with an explicit preview TTL; used by tests.
    #[must_use]
    pub fn with_ttl(ttl_ms: i64) -> Self {
        Self::with_options(ttl_ms, Duration::ZERO)
    }

    /// Creates a service with an explicit TTL and a startup delay.
    ///
    /// The delay makes cooperative cancellation deterministic in tests without
    /// changing production behaviour (the default is zero).
    #[must_use]
    pub fn with_options(ttl_ms: i64, start_delay: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ExportInner::default())),
            ttl_ms,
            start_delay,
        }
    }

    /// Builds a preview and stores its one-time token.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for an unknown format, an empty section list, an
    /// invalid output path or an existing file without overwrite consent, and
    /// `E_STALE_CURSOR` when the snapshot generation changed.
    pub fn preview(
        &self,
        request: &ExportPreviewRequest,
        snapshot: Arc<TreeSnapshot>,
        now_ms: i64,
    ) -> Result<ExportPreview, UiError> {
        if !request.format.supports_report() {
            return Err(UiError::validation(format!(
                "export.error.format: `{}` is not a report format",
                request.format.as_str()
            )));
        }
        let sections = canonical_sections(&request.composition.sections)?;
        if let Some(rows) = request.composition.max_tree_rows {
            if rows == 0 || rows > MAX_EXPORT_TREE_ROWS {
                return Err(UiError::validation(format!(
                    "export.error.max_tree_rows: expected 1..={MAX_EXPORT_TREE_ROWS}"
                )));
            }
        }
        if request.scope.generation != snapshot.generation() {
            return Err(UiError::new(UiErrorCode::StaleCursor)
                .with_details("export.error.stale_generation"));
        }

        let out = validate_output_path(Path::new(request.scope.out.trim()))?;
        if out.exists() && !request.composition.overwrite {
            return Err(UiError::validation(format!(
                "export.error.exists: {}",
                out.display()
            )));
        }

        let totals = snapshot_totals(&snapshot);
        let row_estimate = row_estimate(&snapshot, &sections);
        let mut warnings = Vec::new();
        if request.composition.overwrite {
            warnings.push("export.warning.overwrite".to_owned());
        }
        if sections.contains(&ExportSection::Duplicates) {
            warnings.push("export.warning.duplicates_slow".to_owned());
        }
        if sections.contains(&ExportSection::Owners) {
            warnings.push("export.warning.owners_slow".to_owned());
        }
        if snapshot.node_count() == 0 {
            warnings.push("export.warning.empty".to_owned());
        }

        let preview_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let expires_at = now_ms.saturating_add(self.ttl_ms);
        let mut inner = self.lock();
        inner
            .previews
            .retain(|_, prepared| now_ms <= prepared.expires_at);
        inner.previews.insert(
            preview_id.clone(),
            PreparedExport {
                format: request.format,
                out: out.clone(),
                sections: sections.clone(),
                max_tree_rows: request.composition.max_tree_rows,
                overwrite: request.composition.overwrite,
                snapshot,
                expires_at,
                token: token.clone(),
            },
        );
        prune_previews(&mut inner);
        prune_tasks(&mut inner, now_ms);

        Ok(ExportPreview {
            preview_id,
            confirmation_token: token,
            format: request.format,
            out: out.to_string_lossy().into_owned(),
            sections,
            row_estimate,
            total_bytes: totals.logical,
            on_disk_bytes: totals.on_disk,
            physical_bytes: totals.physical,
            overwrite: request.composition.overwrite,
            warnings,
            expires_at,
        })
    }

    /// Consumes a preview and starts its worker thread.
    ///
    /// The preview identifier is one-time. When a non-empty confirmation token
    /// is supplied it must match the token returned by [`Self::preview`]; the
    /// command layer may omit it, in which case the one-time identifier is the
    /// confirmation.
    ///
    /// # Errors
    /// Returns `E_PREVIEW_EXPIRED` for an unknown, used, expired or mismatched
    /// preview, `E_VALIDATION` when the output path became unwritable and
    /// `E_INTERNAL` when the worker thread cannot be started.
    pub fn start(
        &self,
        preview_id: &str,
        confirmation_token: &str,
        now_ms: i64,
    ) -> Result<TaskStarted, UiError> {
        let prepared = {
            let mut inner = self.lock();
            let Some(prepared) = inner.previews.get(preview_id).cloned() else {
                return Err(UiError::new(UiErrorCode::PreviewExpired));
            };
            let token_mismatch =
                !confirmation_token.is_empty() && prepared.token != confirmation_token;
            if token_mismatch || now_ms > prepared.expires_at {
                inner.previews.remove(preview_id);
                return Err(UiError::new(UiErrorCode::PreviewExpired));
            }
            inner.previews.remove(preview_id);
            prepared
        };

        validate_output_path(&prepared.out)?;
        if prepared.out.exists() && !prepared.overwrite {
            return Err(UiError::validation(format!(
                "export.error.exists: {}",
                prepared.out.display()
            )));
        }

        let task_id = Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        let total = u64::try_from(prepared.sections.len()).unwrap_or(u64::MAX) + 2;
        self.lock().tasks.insert(
            task_id.clone(),
            ExportRuntime {
                status: ExportStatus {
                    task_id: task_id.clone(),
                    state: TaskState::Running,
                    processed: 0,
                    total,
                    phase_key: "export.phase.preparing".to_owned(),
                    out: Some(prepared.out.to_string_lossy().into_owned()),
                    bytes_written: None,
                    rows_written: 0,
                    cancelable: true,
                    started_at: Timestamp::now().as_millis(),
                    finished_at: None,
                    error: None,
                },
                cancel: Arc::clone(&cancel),
            },
        );

        let service = self.clone();
        let worker_id = task_id.clone();
        let worker_cancel = Arc::clone(&cancel);
        let spawn = thread::Builder::new()
            .name("freespace-export".to_owned())
            .spawn(move || service.run_job(prepared, worker_id, worker_cancel));

        if let Err(error) = spawn {
            self.finish(
                &task_id,
                TaskState::Failed,
                "export.phase.failed",
                Some(
                    UiError::new(UiErrorCode::Internal)
                        .with_details(format!("export.error.worker: {error}")),
                ),
                None,
                0,
                0,
                total,
            );

            return Err(UiError::new(UiErrorCode::Internal)
                .with_details("export.error.worker: cannot start the export thread"));
        }

        Ok(TaskStarted { task_id })
    }

    /// Returns the current state of an export.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task.
    pub fn status(&self, task_id: &str) -> Result<ExportStatus, UiError> {
        self.lock()
            .tasks
            .get(task_id)
            .map(|runtime| runtime.status.clone())
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("unknown export task"))
    }

    /// Requests cooperative cancellation.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task. Cancelling a finished task is
    /// a no-op so a late click never fails the screen.
    pub fn cancel(&self, task_id: &str) -> Result<(), UiError> {
        let mut inner = self.lock();
        let runtime = inner.tasks.get_mut(task_id).ok_or_else(|| {
            UiError::new(UiErrorCode::NotFound).with_details("unknown export task")
        })?;
        if runtime.status.state.is_finished() {
            return Ok(());
        }
        runtime.cancel.store(true, Ordering::Relaxed);
        runtime.status.state = TaskState::Cancelling;
        runtime.status.phase_key = "export.phase.cancelling".to_owned();
        runtime.status.cancelable = false;

        Ok(())
    }

    /// Number of live previews; used by tests.
    #[must_use]
    pub fn preview_count(&self) -> usize {
        self.lock().previews.len()
    }

    fn run_job(&self, job: PreparedExport, task_id: String, cancel: Arc<AtomicBool>) {
        if wait_for_start(self.start_delay, cancel.as_ref()) {
            let total = u64::try_from(job.sections.len()).unwrap_or(u64::MAX) + 2;
            self.finish(
                &task_id,
                TaskState::Cancelled,
                "export.phase.cancelled",
                None,
                None,
                0,
                0,
                total,
            );

            return;
        }

        match self.build_and_write(&job, &task_id, cancel.as_ref()) {
            Ok((rows, bytes)) => {
                let total = u64::try_from(job.sections.len()).unwrap_or(u64::MAX) + 2;
                self.finish(
                    &task_id,
                    TaskState::Completed,
                    "export.phase.done",
                    None,
                    Some(job.out.to_string_lossy().into_owned()),
                    bytes,
                    rows,
                    total,
                );
            }
            Err(error) => {
                let cancelled = error.code == UiErrorCode::Cancelled;
                let total = u64::try_from(job.sections.len()).unwrap_or(u64::MAX) + 2;
                self.finish(
                    &task_id,
                    if cancelled {
                        TaskState::Cancelled
                    } else {
                        TaskState::Failed
                    },
                    if cancelled {
                        "export.phase.cancelled"
                    } else {
                        "export.phase.failed"
                    },
                    (!cancelled).then_some(error),
                    None,
                    0,
                    0,
                    total,
                );
            }
        }
    }

    fn build_and_write(
        &self,
        job: &PreparedExport,
        task_id: &str,
        cancel: &AtomicBool,
    ) -> Result<(u64, u64), UiError> {
        let sections = &job.sections;
        let mut processed = 0u64;
        self.stage(task_id, "export.phase.preparing", processed);
        check_cancel(cancel)?;

        let mut tree_clone: Option<FsTree> = sections
            .contains(&ExportSection::Owners)
            .then(|| job.snapshot.tree().clone());
        let mut owner_scan = None;
        if let Some(tree) = tree_clone.as_mut() {
            processed = processed.saturating_add(1);
            self.stage(task_id, ExportSection::Owners.phase_key(), processed);
            check_cancel(cancel)?;
            owner_scan = Some(annotate(tree)?);
            check_cancel(cancel)?;
        }

        let tree_ref: &FsTree = match tree_clone.as_ref() {
            Some(tree) => tree,
            None => job.snapshot.tree(),
        };

        let mut top = None;
        let mut types = None;
        let mut duplicates = None;
        for section in [
            ExportSection::Tree,
            ExportSection::Top,
            ExportSection::Types,
            ExportSection::Duplicates,
        ] {
            if !sections.contains(&section) {
                continue;
            }
            processed = processed.saturating_add(1);
            self.stage(task_id, section.phase_key(), processed);
            check_cancel(cancel)?;
            match section {
                ExportSection::Tree => {}
                ExportSection::Top => {
                    top = Some(top_files(tree_ref, EXPORT_TOP_LIMIT));
                }
                ExportSection::Types => {
                    types = Some(group_by_extension(tree_ref));
                }
                ExportSection::Duplicates => {
                    duplicates = Some(find_duplicates(
                        tree_ref,
                        DuplicateOptions::default(),
                        None,
                    )?);
                }
                ExportSection::Owners => {}
            }
            check_cancel(cancel)?;
        }

        let totals = snapshot_totals(&job.snapshot);
        let roots: Vec<String> = job
            .snapshot
            .session()
            .map(|session| {
                session
                    .roots
                    .iter()
                    .map(|root| paths::display(root))
                    .collect()
            })
            .unwrap_or_else(|| vec![paths::display(&job.snapshot.volume().mount_point)]);
        let session_id = job
            .snapshot
            .session()
            .map(|session| session.id.to_string())
            .unwrap_or_default();
        let meta = ReportMeta::new(Timestamp::now(), env!("CARGO_PKG_VERSION"))
            .with_volume(ReportVolume::from_volume(job.snapshot.volume()))
            .with_roots(roots)
            .with_session(session_id);

        let mut report = Report::new(meta, totals)
            .with_include(
                sections
                    .iter()
                    .map(|section| report_section(*section))
                    .collect(),
            )
            .with_tree(tree_ref);
        if let Some(limit) = job.max_tree_rows {
            if sections.contains(&ExportSection::Tree) {
                report = report.with_max_tree_rows(limit);
            }
        }
        if let Some(top) = &top {
            report = report.with_top(top);
        }
        if let Some(types) = &types {
            report = report.with_types(types);
        }
        if let Some(search) = &duplicates {
            report = report.with_duplicates(&search.groups);
        }
        if let Some(scan) = &owner_scan {
            report = report.with_owners(&scan.summaries, &scan.table);
        }

        check_cancel(cancel)?;
        processed = processed.saturating_add(1);
        self.stage(task_id, "export.phase.writing", processed);

        let rows = u64::try_from(report.total_rows()).unwrap_or(u64::MAX);
        write_report(&report, report_format(job.format)?, &job.out)?;

        let bytes = std::fs::metadata(&job.out)
            .map(|metadata| metadata.len())
            .unwrap_or_default();

        Ok((rows, bytes))
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        &self,
        task_id: &str,
        state: TaskState,
        phase_key: &str,
        error: Option<UiError>,
        out: Option<String>,
        bytes_written: u64,
        rows_written: u64,
        processed: u64,
    ) {
        let mut inner = self.lock();
        if let Some(runtime) = inner.tasks.get_mut(task_id) {
            runtime.status.state = state;
            runtime.status.phase_key = phase_key.to_owned();
            runtime.status.error = error;
            runtime.status.out = out.or_else(|| runtime.status.out.clone());
            runtime.status.bytes_written = Some(bytes_written);
            runtime.status.rows_written = rows_written;
            runtime.status.processed = processed;
            runtime.status.total = runtime.status.total.max(processed);
            runtime.status.cancelable = false;
            runtime.status.finished_at = Some(Timestamp::now().as_millis());
        }
    }

    fn stage(&self, task_id: &str, phase_key: &str, processed: u64) {
        if let Some(runtime) = self.lock().tasks.get_mut(task_id) {
            runtime.status.phase_key = phase_key.to_owned();
            runtime.status.processed = processed;
        }
    }

    fn lock(&self) -> MutexGuard<'_, ExportInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Global export registry used by the command layer.
static EXPORTS: LazyLock<ExportService> = LazyLock::new(ExportService::new);

/// Builds an export preview using the global registry.
///
/// # Errors
/// See [`ExportService::preview`].
pub fn preview(
    request: &ExportPreviewRequest,
    snapshot: Arc<TreeSnapshot>,
    now_ms: i64,
) -> Result<ExportPreview, UiError> {
    EXPORTS.preview(request, snapshot, now_ms)
}

/// Starts an export using the global registry.
///
/// # Errors
/// See [`ExportService::start`].
pub fn start(
    preview_id: &str,
    confirmation_token: &str,
    now_ms: i64,
) -> Result<TaskStarted, UiError> {
    EXPORTS.start(preview_id, confirmation_token, now_ms)
}

/// Returns the state of an export using the global registry.
///
/// # Errors
/// See [`ExportService::status`].
pub fn status(task_id: &str) -> Result<ExportStatus, UiError> {
    EXPORTS.status(task_id)
}

/// Cancels an export using the global registry.
///
/// # Errors
/// See [`ExportService::cancel`].
pub fn cancel(task_id: &str) -> Result<(), UiError> {
    EXPORTS.cancel(task_id)
}

/// Validates and returns an output path used by a report writer.
///
/// The path must be absolute, must not be an existing directory, must not be a
/// read-only file and must have a parent that exists or can be created.
///
/// # Errors
/// Returns `E_VALIDATION` with a stable key when the path cannot be written.
pub fn validate_output_path(path: &Path) -> Result<PathBuf, UiError> {
    let check = check_output_path(path);
    if check.valid {
        Ok(path.to_path_buf())
    } else {
        Err(UiError::validation(format!(
            "{}: {}",
            check
                .error_key
                .unwrap_or_else(|| "export.error.path".to_owned()),
            check.path
        )))
    }
}

/// Returns the path validation result without failing.
#[must_use]
pub fn check_output_path(path: &Path) -> PathValidation {
    let text = path.to_string_lossy().into_owned();
    let mut result = PathValidation {
        path: text.clone(),
        valid: false,
        exists: false,
        is_directory: false,
        parent_exists: false,
        writable: true,
        error_key: None,
    };

    if path.as_os_str().is_empty() || validate_path(&text).is_err() {
        result.error_key = Some("export.error.path".to_owned());

        return result;
    }

    result.exists = path.exists();
    result.is_directory = path.is_dir();
    if result.is_directory {
        result.error_key = Some("export.error.path_is_directory".to_owned());

        return result;
    }
    if result.exists {
        if let Ok(metadata) = std::fs::metadata(path) {
            result.writable = !metadata.permissions().readonly();
        }
        if !result.writable {
            result.error_key = Some("export.error.read_only".to_owned());

            return result;
        }
    }

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let Some(parent) = parent else {
        result.error_key = Some("export.error.parent_missing".to_owned());

        return result;
    };
    result.parent_exists = parent.is_dir();
    if !result.parent_exists {
        let ancestor = path.ancestors().skip(1).find(|ancestor| ancestor.exists());
        if !ancestor.is_some_and(|ancestor| ancestor.is_dir()) {
            result.error_key = Some("export.error.parent_missing".to_owned());

            return result;
        }
    }

    result.valid = true;

    result
}

fn canonical_sections(requested: &[ExportSection]) -> Result<Vec<ExportSection>, UiError> {
    let sections: Vec<ExportSection> = ExportSection::ALL
        .into_iter()
        .filter(|section| requested.contains(section))
        .collect();
    if sections.is_empty() {
        return Err(UiError::validation(
            "export.error.sections: select at least one section",
        ));
    }

    Ok(sections)
}

fn snapshot_totals(snapshot: &TreeSnapshot) -> SizeMetrics {
    snapshot
        .tree()
        .roots()
        .map(|root| snapshot.tree().node(root).size)
        .sum()
}

fn row_estimate(snapshot: &TreeSnapshot, sections: &[ExportSection]) -> Option<u64> {
    let nodes = snapshot.node_count();
    let mut estimate = 0u64;
    for section in sections {
        match section {
            ExportSection::Tree => estimate = estimate.saturating_add(nodes),
            ExportSection::Top => {
                estimate = estimate
                    .saturating_add(nodes.min(u64::try_from(EXPORT_TOP_LIMIT).unwrap_or(u64::MAX)));
            }
            ExportSection::Types | ExportSection::Duplicates | ExportSection::Owners => {
                return None;
            }
        }
    }

    Some(estimate)
}

const fn report_section(section: ExportSection) -> SectionName {
    match section {
        ExportSection::Tree => SectionName::Tree,
        ExportSection::Top => SectionName::Top,
        ExportSection::Types => SectionName::Types,
        ExportSection::Duplicates => SectionName::Duplicates,
        ExportSection::Owners => SectionName::Owners,
    }
}

fn report_format(format: ExportFormat) -> Result<ReportFormat, UiError> {
    match format {
        ExportFormat::Csv => Ok(ReportFormat::Csv),
        ExportFormat::Xml => Ok(ReportFormat::Xml),
        ExportFormat::Html => Ok(ReportFormat::Html),
        ExportFormat::Xlsx => Ok(ReportFormat::Xlsx),
        ExportFormat::Json => Err(UiError::new(UiErrorCode::Unsupported)
            .with_details("export.error.format: json is only available for archive lists")),
    }
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), UiError> {
    if cancel.load(Ordering::Relaxed) {
        Err(UiError::new(UiErrorCode::Cancelled))
    } else {
        Ok(())
    }
}

fn wait_for_start(delay: Duration, cancel: &AtomicBool) -> bool {
    if delay.is_zero() {
        return cancel.load(Ordering::Relaxed);
    }

    let deadline = Instant::now() + delay;
    while Instant::now() < deadline {
        if cancel.load(Ordering::Relaxed) {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }

    cancel.load(Ordering::Relaxed)
}

/// Keeps at most sixteen live previews; they only live for ten minutes.
fn prune_previews(inner: &mut ExportInner) {
    const MAX_PREVIEWS: usize = 16;
    if inner.previews.len() <= MAX_PREVIEWS {
        return;
    }

    let mut by_expiry: Vec<(String, i64)> = inner
        .previews
        .iter()
        .map(|(id, prepared)| (id.clone(), prepared.expires_at))
        .collect();
    by_expiry.sort_by_key(|(_, expires_at)| *expires_at);
    let excess = inner.previews.len().saturating_sub(MAX_PREVIEWS);
    for (id, _) in by_expiry.into_iter().take(excess) {
        inner.previews.remove(&id);
    }
}

fn prune_tasks(inner: &mut ExportInner, now_ms: i64) {
    inner.tasks.retain(|_, runtime| {
        !runtime.status.state.is_finished()
            || runtime
                .status
                .finished_at
                .is_some_and(|finished| now_ms.saturating_sub(finished) < 3_600_000)
    });

    if inner.tasks.len() > 64 {
        let mut finished: Vec<(String, i64)> = inner
            .tasks
            .iter()
            .filter(|(_, runtime)| runtime.status.state.is_finished())
            .map(|(id, runtime)| (id.clone(), runtime.status.started_at))
            .collect();
        finished.sort_by_key(|(_, started)| *started);
        let excess = inner.tasks.len().saturating_sub(64);
        for (id, _) in finished.into_iter().take(excess) {
            inner.tasks.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };

    use freespace_scan::session::NoopObserver;

    use super::{ExportService, check_output_path};
    use crate::{
        dto::{
            ExportComposition, ExportFormat, ExportPreviewRequest, ExportScope, ExportSection,
            Metric, ScanMethodChoice, ScanRequest, TaskState,
        },
        error::UiErrorCode,
        scan::run_scan,
        tree::TreeSnapshot,
    };

    fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Arc<TreeSnapshot>) {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("alpha.txt"), [1u8; 32]).expect("alpha");
        std::fs::write(directory.path().join("beta.bin"), [2u8; 128]).expect("beta");
        std::fs::create_dir(directory.path().join("nested")).expect("nested");
        std::fs::write(directory.path().join("nested").join("note.txt"), b"hi").expect("note");
        let outcome = run_scan(
            &ScanRequest {
                path: directory.path().to_string_lossy().into_owned(),
                method: ScanMethodChoice::Walk,
                incremental: false,
                exclusions: Vec::new(),
                include_ads: false,
                metric: Metric::OnDisk,
            },
            &mut NoopObserver,
        )
        .expect("scan");
        let snapshot = TreeSnapshot::new(outcome.tree, 21, outcome.volume, Some(outcome.session));
        let output = tempfile::tempdir().expect("output");

        (directory, output, Arc::new(snapshot))
    }

    fn request(out: &std::path::Path, sections: Vec<ExportSection>) -> ExportPreviewRequest {
        ExportPreviewRequest {
            scope: ExportScope {
                generation: 21,
                out: out.to_string_lossy().into_owned(),
            },
            format: ExportFormat::Csv,
            composition: ExportComposition {
                sections,
                max_tree_rows: None,
                overwrite: false,
            },
        }
    }

    fn wait(service: &ExportService, task_id: &str) -> crate::dto::ExportStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let status = service.status(task_id).expect("status");
            if status.state.is_finished() {
                return status;
            }
            assert!(Instant::now() < deadline, "export did not finish");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn previews_and_writes_a_report() {
        let (_directory, output, snapshot) = fixture();
        let out = output.path().join("report.csv");
        let service = ExportService::new();
        let preview = service
            .preview(
                &request(&out, vec![ExportSection::Tree, ExportSection::Top]),
                Arc::clone(&snapshot),
                0,
            )
            .expect("preview");
        assert_eq!(preview.format, ExportFormat::Csv);
        assert_eq!(preview.total_bytes, 162);
        assert!(preview.row_estimate.is_some());
        assert_eq!(service.preview_count(), 1);

        let started = service
            .start(&preview.preview_id, &preview.confirmation_token, 1)
            .expect("start");
        assert_eq!(service.preview_count(), 0);
        let status = wait(&service, &started.task_id);
        assert_eq!(status.state, TaskState::Completed, "{status:?}");
        assert!(status.rows_written >= 5);
        assert!(out.exists());
        let text = std::fs::read_to_string(&out).expect("report");
        assert!(text.contains("alpha.txt"));
    }

    #[test]
    fn preview_expires_and_is_one_time() {
        let (_directory, output, snapshot) = fixture();
        let out = output.path().join("report.csv");
        let service = ExportService::with_ttl(1_000);
        let preview = service
            .preview(&request(&out, vec![ExportSection::Tree]), snapshot, 0)
            .expect("preview");

        let error = service
            .start(&preview.preview_id, &preview.confirmation_token, 1_001)
            .expect_err("expired");
        assert_eq!(error.code, UiErrorCode::PreviewExpired);
        assert_eq!(service.preview_count(), 0);
    }

    #[test]
    fn cancellation_stops_before_the_file_is_written() {
        let (_directory, output, snapshot) = fixture();
        let out = output.path().join("cancelled.csv");
        let service = ExportService::with_options(60_000, Duration::from_millis(200));
        let preview = service
            .preview(&request(&out, vec![ExportSection::Tree]), snapshot, 0)
            .expect("preview");
        let started = service
            .start(&preview.preview_id, &preview.confirmation_token, 1)
            .expect("start");
        thread::sleep(Duration::from_millis(20));
        service.cancel(&started.task_id).expect("cancel");

        let status = wait(&service, &started.task_id);
        assert_eq!(status.state, TaskState::Cancelled, "{status:?}");
        assert!(!out.exists());
    }

    #[test]
    fn the_one_time_preview_identifier_is_enough_for_the_contract() {
        let (_directory, output, snapshot) = fixture();
        let out = output.path().join("contract.csv");
        let service = ExportService::new();
        let preview = service
            .preview(
                &request(&out, vec![ExportSection::Tree]),
                Arc::clone(&snapshot),
                0,
            )
            .expect("preview");

        let started = service
            .start(&preview.preview_id, "", 1)
            .expect("contract call without a token");
        let status = wait(&service, &started.task_id);
        assert_eq!(status.state, TaskState::Completed, "{status:?}");

        let mut wrong = request(&out, vec![ExportSection::Tree]);
        wrong.composition.overwrite = true;
        let preview = service.preview(&wrong, snapshot, 2).expect("preview");
        let error = service
            .start(&preview.preview_id, "wrong-token", 3)
            .expect_err("mismatched token");
        assert_eq!(error.code, UiErrorCode::PreviewExpired);
    }

    #[test]
    fn overwrite_requires_consent() {
        let (_directory, output, snapshot) = fixture();
        let out = output.path().join("report.csv");
        std::fs::write(&out, b"previous").expect("existing");
        let service = ExportService::new();

        let error = service
            .preview(
                &request(&out, vec![ExportSection::Tree]),
                Arc::clone(&snapshot),
                0,
            )
            .expect_err("no overwrite consent");
        assert_eq!(error.code, UiErrorCode::Validation);

        let mut allowed = request(&out, vec![ExportSection::Tree]);
        allowed.composition.overwrite = true;
        let preview = service.preview(&allowed, snapshot, 0).expect("preview");
        assert!(preview.overwrite);
        assert!(
            preview
                .warnings
                .iter()
                .any(|warning| warning == "export.warning.overwrite")
        );
    }

    #[test]
    fn validates_output_paths() {
        let directory = tempfile::tempdir().expect("temp dir");
        let valid = directory.path().join("report.csv");
        assert!(check_output_path(&valid).valid);
        assert!(check_output_path(directory.path()).is_directory);

        let relative = std::path::Path::new("report.csv");
        assert!(!check_output_path(relative).valid);
        assert!(!check_output_path(&std::path::PathBuf::from(r"C:\")).valid);

        let nested = check_output_path(&directory.path().join("new").join("report.xlsx"));
        assert!(nested.valid, "a creatable parent is accepted");
        assert!(!nested.parent_exists);
    }

    #[test]
    fn status_of_an_unknown_task_is_not_found() {
        let service = ExportService::new();
        assert_eq!(
            service.status("nope").expect_err("unknown").code,
            UiErrorCode::NotFound
        );
        assert_eq!(
            service.cancel("nope").expect_err("unknown").code,
            UiErrorCode::NotFound
        );
    }
}
