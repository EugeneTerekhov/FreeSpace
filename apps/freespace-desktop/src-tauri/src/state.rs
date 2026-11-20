//! Shared Tauri state: settings, window geometry, tasks and the current snapshot.
//!
//! The interface may run several tasks (a scan, a duplicate search, a monitor) but
//! only one scan per volume. The registry enforces that rule, keeps generations
//! monotonic and owns the immutable tree snapshot.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use freespace_app::{
    about::about_info,
    dto::{
        AppBootstrap, AppCapabilities, ScanSessionSummary, SettingsPatch, SettingsView, Task,
        TaskKind, TaskProgress, TaskState, UiSettings, UiSettingsPatch, WindowState,
    },
    duplicates::{self, DuplicateSearch},
    error::{UiError, UiErrorCode},
    monitor::{MonitorDefaults, MonitorService},
    operations::OperationService,
    settings::{SettingsSection, SettingsStore, settings_path},
    snapshots::SnapshotService,
    tree::TreeSnapshot,
    window_state::{MonitorBounds, load_window_state, save_window_state, window_state_path},
};
use freespace_core::{
    model::{ScanSession, Volume},
    time::Timestamp,
    tree::FsTree,
};
use freespace_ops::store::Store;

use crate::{boot::BootState, security};

/// Window geometry and settings ready for the first paint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupState {
    /// Migrated interface settings.
    pub settings: UiSettings,
    /// Sanitised window geometry.
    pub window_state: WindowState,
    /// Theme/language/motion injected into the webview.
    pub boot: BootState,
}

/// One registered task with its cancellation flag.
#[derive(Debug)]
struct TaskRecord {
    task: Task,
    cancel: Arc<AtomicBool>,
}

/// Mutable part protected by one mutex.
#[derive(Debug)]
struct Inner {
    data_dir: PathBuf,
    settings: SettingsStore,
    window_state: WindowState,
    tasks: HashMap<String, TaskRecord>,
    active_scans: HashMap<String, String>,
    active_duplicates: Option<String>,
    sessions: Vec<ScanSessionSummary>,
    duplicates: HashMap<String, Arc<DuplicateSearch>>,
    generation: u64,
    snapshot: Option<Arc<TreeSnapshot>>,
}

/// Application state shared by every command.
#[derive(Debug)]
pub struct AppState {
    inner: Mutex<Inner>,
    event_sequence: AtomicU64,
    store: Mutex<Store>,
    snapshots: SnapshotService,
    operations: OperationService,
    /// Monitor sessions (US6). The registry owns policy and history; the worker
    /// thread lives in `commands::monitor` because it needs the Tauri handle.
    monitors: MonitorService,
}

impl AppState {
    /// Loads settings and window geometry from `data_dir`.
    ///
    /// # Errors
    /// Propagates settings migration and window-state read failures.
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self, UiError> {
        let data_dir = data_dir.into();
        let settings = SettingsStore::load(settings_path(&data_dir))?;
        let window_state = load_window_state(&window_state_path(&data_dir))?;
        let store = Store::open(&data_dir.join("freespace.db"))?;
        let snapshots = SnapshotService::new(SnapshotService::default_dir(&data_dir));

        Ok(Self {
            inner: Mutex::new(Inner {
                data_dir,
                settings,
                window_state,
                tasks: HashMap::new(),
                active_scans: HashMap::new(),
                active_duplicates: None,
                sessions: Vec::new(),
                duplicates: HashMap::new(),
                generation: 0,
                snapshot: None,
            }),
            event_sequence: AtomicU64::new(0),
            store: Mutex::new(store),
            snapshots,
            operations: OperationService::new(),
            monitors: MonitorService::new(),
        })
    }

    /// Returns the next sequence for events that are not bound to a task.
    pub fn next_event_sequence(&self) -> u64 {
        self.event_sequence.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Application-owned data directory.
    #[must_use]
    pub fn data_dir(&self) -> PathBuf {
        self.lock().data_dir.clone()
    }

    /// Current interface settings.
    #[must_use]
    pub fn ui_settings(&self) -> UiSettings {
        self.lock().settings.ui().clone()
    }

    /// Settings payload for the first paint.
    #[must_use]
    pub fn startup_state(&self) -> StartupState {
        let inner = self.lock();
        StartupState {
            settings: inner.settings.ui().clone(),
            window_state: inner.window_state.clone(),
            boot: BootState::from(inner.settings.ui()),
        }
    }

    /// Applies a settings patch and persists it.
    ///
    /// # Errors
    /// Propagates validation and write failures from [`SettingsStore::update`].
    pub fn update_settings(&self, patch: UiSettingsPatch) -> Result<UiSettings, UiError> {
        self.lock().settings.update(patch)
    }

    /// Full settings payload used by the settings screen.
    #[must_use]
    pub fn settings_view(&self) -> SettingsView {
        self.lock().settings.view()
    }

    /// Applies a full settings patch and persists it.
    ///
    /// # Errors
    /// Propagates validation and write failures from
    /// [`SettingsStore::update_view`].
    pub fn update_settings_view(&self, patch: SettingsPatch) -> Result<SettingsView, UiError> {
        self.lock().settings.update_view(patch)
    }

    /// Resets one section of the interface settings.
    ///
    /// # Errors
    /// Propagates validation and write failures from
    /// [`SettingsStore::reset_section`].
    pub fn reset_settings(&self, section: SettingsSection) -> Result<UiSettings, UiError> {
        self.lock().settings.reset_section(section)
    }

    /// Resets one section and returns the full settings payload.
    ///
    /// # Errors
    /// Propagates validation and write failures from
    /// [`SettingsStore::reset_section`].
    pub fn reset_settings_view(&self, section: SettingsSection) -> Result<SettingsView, UiError> {
        let mut inner = self.lock();
        inner.settings.reset_section(section)?;

        Ok(inner.settings.view())
    }

    /// Persisted window geometry.
    #[must_use]
    pub fn window_state(&self) -> WindowState {
        self.lock().window_state.clone()
    }

    /// Stores and persists window geometry.
    ///
    /// # Errors
    /// Returns `E_VALIDATION` for invalid geometry or `E_IO` for write failures.
    pub fn set_window_state(
        &self,
        state: WindowState,
        monitors: &[MonitorBounds],
    ) -> Result<(), UiError> {
        let mut inner = self.lock();
        let sanitized = state.sanitized(monitors);
        save_window_state(&window_state_path(&inner.data_dir), &sanitized)?;
        inner.window_state = sanitized;
        Ok(())
    }

    /// Builds the bootstrap payload.
    #[must_use]
    pub fn bootstrap(
        &self,
        app_version: impl Into<String>,
        os: impl Into<String>,
        webview_version: Option<String>,
        monitors: &[MonitorBounds],
    ) -> AppBootstrap {
        let inner = self.lock();
        let app_version = app_version.into();
        let settings = inner.settings.ui().clone();
        let window_state = inner.window_state.sanitized(monitors);
        let about = about_info(&inner.data_dir, &app_version, webview_version.clone());
        AppBootstrap {
            ipc_version: freespace_app::dto::IPC_VERSION,
            app_version,
            os: os.into(),
            webview_version,
            settings,
            window_state,
            capabilities: security::capabilities(),
            about,
        }
    }

    /// Capabilities advertised to the client.
    #[must_use]
    pub fn capabilities(&self) -> AppCapabilities {
        security::capabilities()
    }

    /// Reserves the next generation.
    ///
    /// # Errors
    /// Returns `E_INTERNAL` when the counter overflows.
    pub fn next_generation(&self) -> Result<u64, UiError> {
        let mut inner = self.lock();
        inner.generation = inner.generation.checked_add(1).ok_or_else(|| {
            UiError::new(UiErrorCode::Internal).with_details("generation overflow")
        })?;
        Ok(inner.generation)
    }

    /// Installs a finished snapshot and its per-root session summaries.
    pub fn install_snapshot(&self, snapshot: Arc<TreeSnapshot>, sessions: Vec<ScanSessionSummary>) {
        let mut inner = self.lock();
        inner.generation = inner.generation.max(snapshot.generation());
        inner.snapshot = Some(snapshot);
        inner.sessions.extend(sessions);
    }

    /// Merges a finished scan into the current snapshot.
    ///
    /// The previous tree is reused in place when no command holds it; otherwise
    /// it is cloned before the append. The new snapshot keeps every root that
    /// the scan did not re-analyse.
    pub fn merge_snapshot(
        &self,
        generation: u64,
        tree: FsTree,
        volumes: Vec<Volume>,
        sessions: Vec<ScanSession>,
        summaries: Vec<ScanSessionSummary>,
    ) -> Arc<TreeSnapshot> {
        let mut inner = self.lock();
        let merged = match inner.snapshot.take() {
            Some(previous) => match Arc::try_unwrap(previous) {
                Ok(snapshot) => snapshot.merge_owned(generation, tree, volumes, sessions),
                Err(previous) => (*previous)
                    .clone()
                    .merge_owned(generation, tree, volumes, sessions),
            },
            None => TreeSnapshot::new_multi(tree, generation, volumes, sessions),
        };
        let merged = Arc::new(merged);
        inner.generation = inner.generation.max(generation);
        inner.snapshot = Some(Arc::clone(&merged));
        inner.sessions.extend(summaries);

        merged
    }

    /// Returns the current snapshot, or `E_NOT_FOUND` when no scan has finished.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` before the first successful scan.
    pub fn snapshot(&self) -> Result<Arc<TreeSnapshot>, UiError> {
        self.lock()
            .snapshot
            .clone()
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("no scan has finished"))
    }

    /// Returns the snapshot only when its generation matches.
    ///
    /// # Errors
    /// Returns `E_STALE_CURSOR` for an outdated generation and `E_NOT_FOUND` when
    /// no scan has finished.
    pub fn snapshot_for(&self, generation: u64) -> Result<Arc<TreeSnapshot>, UiError> {
        let inner = self.lock();
        let snapshot = inner.snapshot.clone().ok_or_else(|| {
            UiError::new(UiErrorCode::NotFound).with_details("no scan has finished")
        })?;

        if snapshot.generation() != generation {
            return Err(UiError::new(UiErrorCode::StaleCursor)
                .with_details("the requested generation is no longer current"));
        }

        Ok(snapshot)
    }

    /// Snapshot workspace rooted in `data/snapshots`.
    #[must_use]
    pub const fn snapshots(&self) -> &SnapshotService {
        &self.snapshots
    }

    /// Preview and operation registry.
    #[must_use]
    pub const fn operations(&self) -> &OperationService {
        &self.operations
    }

    /// Write-monitor registry (US6).
    #[must_use]
    pub const fn monitors(&self) -> &MonitorService {
        &self.monitors
    }

    /// Persisted defaults for a new monitor session.
    #[must_use]
    pub fn monitor_defaults(&self) -> MonitorDefaults {
        MonitorDefaults::from_settings(self.lock().settings.core())
    }

    /// Runs a closure with the journal store locked.
    ///
    /// # Errors
    /// Propagates the closure error.
    pub fn with_store<T>(
        &self,
        action: impl FnOnce(&Store) -> Result<T, UiError>,
    ) -> Result<T, UiError> {
        let store = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        action(&store)
    }

    /// Registers a monitor task for the shell status bar.
    ///
    /// The monitor registry enforces one running session; this method only makes
    /// the task visible through `task-queue-changed` and `scan_status`-style
    /// lookups. There is no volume reservation because monitoring reads counters
    /// and ETW only and never blocks a destructive operation.
    pub fn register_monitor_task(&self, task_id: &str, cancel: Arc<AtomicBool>) {
        self.lock().tasks.insert(
            task_id.to_owned(),
            TaskRecord {
                task: Task {
                    task_id: task_id.to_owned(),
                    kind: TaskKind::Monitor,
                    state: TaskState::Running,
                    volume_id: None,
                    volume_ids: Vec::new(),
                    progress: None,
                    started_at: Timestamp::now().as_millis(),
                    finished_at: None,
                    cancelable: true,
                    error: None,
                },
                cancel,
            },
        );
    }

    /// Registers a duplicate search task, refusing a concurrent search.
    ///
    /// # Errors
    /// Returns `E_BUSY` when another duplicate search is running.
    pub fn reserve_duplicates(
        &self,
        task_id: &str,
        volume_id: Option<String>,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), UiError> {
        let mut inner = self.lock();
        if let Some(active) = &inner.active_duplicates {
            return Err(UiError::new(UiErrorCode::Busy)
                .with_details(format!("duplicate search `{active}` is already running")));
        }
        inner.active_duplicates = Some(task_id.to_owned());
        inner.tasks.insert(
            task_id.to_owned(),
            TaskRecord {
                task: Task {
                    task_id: task_id.to_owned(),
                    kind: TaskKind::Duplicates,
                    state: TaskState::Running,
                    volume_id: volume_id.clone(),
                    volume_ids: volume_id.into_iter().collect(),
                    progress: None,
                    started_at: Timestamp::now().as_millis(),
                    finished_at: None,
                    cancelable: true,
                    error: None,
                },
                cancel,
            },
        );

        Ok(())
    }

    /// Releases the duplicate search slot.
    pub fn release_duplicates(&self) {
        self.lock().active_duplicates = None;
    }

    /// Stores a finished duplicate search.
    pub fn install_duplicates(&self, task_id: &str, search: Arc<DuplicateSearch>) {
        self.lock().duplicates.insert(task_id.to_owned(), search);
    }

    /// Returns a finished duplicate search.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown or unfinished task.
    pub fn duplicates(&self, task_id: &str) -> Result<Arc<DuplicateSearch>, UiError> {
        self.lock().duplicates.get(task_id).cloned().ok_or_else(|| {
            UiError::new(UiErrorCode::NotFound).with_details("unknown duplicate task")
        })
    }

    /// Validates a duplicate selection and returns the actual paths.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task and `E_VALIDATION` when a copy
    /// that must stay is selected.
    pub fn duplicate_targets(
        &self,
        task_id: &str,
        selected: &[String],
    ) -> Result<Vec<PathBuf>, UiError> {
        let search = self.duplicates(task_id)?;
        duplicates::deletion_targets(&search, selected)
    }

    /// Registers a scan task and reserves its volumes.
    ///
    /// Every volume is reserved atomically: if any of them already has a
    /// running scan, nothing is reserved and `E_BUSY` is returned.
    ///
    /// # Errors
    /// Returns `E_BUSY` when a volume already has a running scan and
    /// `E_VALIDATION` for an empty volume list.
    pub fn reserve_scans(
        &self,
        task_id: &str,
        volume_ids: &[String],
        cancel: Arc<AtomicBool>,
    ) -> Result<(), UiError> {
        if volume_ids.is_empty() {
            return Err(UiError::validation("a scan needs at least one volume"));
        }

        let mut inner = self.lock();
        for volume_id in volume_ids {
            if let Some(active) = inner.active_scans.get(volume_id) {
                return Err(UiError::new(UiErrorCode::Busy)
                    .with_details(format!("volume already has the scan task `{active}`")));
            }
        }

        for volume_id in volume_ids {
            inner
                .active_scans
                .insert(volume_id.clone(), task_id.to_owned());
        }
        inner.tasks.insert(
            task_id.to_owned(),
            TaskRecord {
                task: Task {
                    task_id: task_id.to_owned(),
                    kind: TaskKind::Scan,
                    state: TaskState::Running,
                    volume_id: volume_ids.first().cloned(),
                    volume_ids: volume_ids.to_vec(),
                    progress: None,
                    started_at: Timestamp::now().as_millis(),
                    finished_at: None,
                    cancelable: true,
                    error: None,
                },
                cancel,
            },
        );

        Ok(())
    }

    /// Registers a scan task and reserves its volume.
    ///
    /// # Errors
    /// Returns `E_BUSY` when the volume already has a running scan.
    pub fn reserve_scan(
        &self,
        task_id: &str,
        volume_id: &str,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), UiError> {
        self.reserve_scans(task_id, &[volume_id.to_owned()], cancel)
    }

    /// Releases the volume reservations of a finished scan.
    pub fn release_scans(&self, volume_ids: &[String]) {
        let mut inner = self.lock();
        for volume_id in volume_ids {
            inner.active_scans.remove(volume_id);
        }
    }

    /// Releases the volume reservation of a finished scan.
    pub fn release_scan(&self, volume_id: &str) {
        self.release_scans(&[volume_id.to_owned()]);
    }

    /// Updates progress of a task.
    pub fn update_task_progress(&self, task_id: &str, progress: TaskProgress) {
        let mut inner = self.lock();
        if let Some(record) = inner.tasks.get_mut(task_id) {
            record.task.progress = Some(progress);
        }
    }

    /// Finishes a task with a terminal state.
    pub fn finish_task(&self, task_id: &str, state: TaskState, error: Option<UiError>) {
        let mut inner = self.lock();
        if let Some(record) = inner.tasks.get_mut(task_id) {
            record.task.state = state;
            record.task.finished_at = Some(Timestamp::now().as_millis());
            record.task.cancelable = false;
            record.task.error = error;
        }
    }

    /// Returns all tasks, oldest first.
    #[must_use]
    pub fn tasks(&self) -> Vec<Task> {
        let inner = self.lock();
        let mut tasks: Vec<Task> = inner
            .tasks
            .values()
            .map(|record| record.task.clone())
            .collect();
        tasks.sort_by_key(|task| task.started_at);
        tasks
    }

    /// Returns one task.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task.
    pub fn task(&self, task_id: &str) -> Result<Task, UiError> {
        self.lock()
            .tasks
            .get(task_id)
            .map(|record| record.task.clone())
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("unknown task"))
    }

    /// Requests cancellation and marks the task as cancelling.
    ///
    /// # Errors
    /// Returns `E_NOT_FOUND` for an unknown task.
    pub fn cancel_task(&self, task_id: &str) -> Result<(), UiError> {
        let mut inner = self.lock();
        let record = inner
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| UiError::new(UiErrorCode::NotFound).with_details("unknown task"))?;

        if record.task.state.is_finished() {
            return Ok(());
        }

        record.cancel.store(true, Ordering::Relaxed);
        record.task.state = TaskState::Cancelling;
        record.task.cancelable = false;

        Ok(())
    }

    /// Returns finished scan sessions, newest first.
    #[must_use]
    pub fn scan_sessions(&self) -> Vec<ScanSessionSummary> {
        let mut sessions = self.lock().sessions.clone();
        sessions.reverse();
        sessions
    }

    /// Adds a finished scan session without installing a snapshot (tests).
    pub fn push_session(&self, session: ScanSessionSummary) {
        self.lock().sessions.push(session);
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Resolves the data directory beside the executable.
///
/// # Errors
/// Returns `E_INTERNAL` when the executable location cannot be resolved.
pub fn default_data_dir() -> Result<PathBuf, UiError> {
    Ok(freespace_core::settings::data_root()?)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use freespace_app::{
        dto::{
            FilterSet, Language, Metric, ScreenId, SortSpec, TaskState, ThemePreference,
            UiSettingsPatch, WindowState,
        },
        tree::TreeSnapshot,
        window_state::MonitorBounds,
    };
    use freespace_core::{
        model::{FileSystem, ScanMethod, ScanSession, Volume, VolumeStatus},
        tree::{FsTree, NodeKind, NodeSpec},
    };

    use super::AppState;

    fn state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temp dir");
        let state = AppState::new(directory.path()).expect("state");
        (directory, state)
    }

    fn snapshot() -> Arc<TreeSnapshot> {
        let volume = Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"C:\")),
            mount_point: r"C:\".into(),
            label: "Test".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 1_000,
            free_bytes: 100,
            cluster_size: None,
            scan_method: None,
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };
        Arc::new(TreeSnapshot::new(FsTree::new(), 3, volume, None))
    }

    #[test]
    fn boots_with_defaults_and_creates_no_system_files() {
        let (directory, state) = state();
        let startup = state.startup_state();

        assert_eq!(startup.settings.language, Language::Ru);
        assert_eq!(startup.window_state, WindowState::default());
        assert_eq!(startup.boot.language, Language::Ru);
        assert!(
            state
                .capabilities()
                .commands
                .contains(&"app_bootstrap".to_owned())
        );
        assert!(
            state
                .capabilities()
                .commands
                .contains(&"volumes_list".to_owned())
        );
        assert!(
            directory.path().join("settings.toml").exists()
                || state.ui_settings().schema_version == 1
        );
    }

    #[test]
    fn reserves_one_scan_per_volume() {
        let (_directory, state) = state();
        let cancel = Arc::new(AtomicBool::new(false));
        state
            .reserve_scan("task-1", "volume-1", Arc::clone(&cancel))
            .expect("first");

        let error = state
            .reserve_scan("task-2", "volume-1", Arc::new(AtomicBool::new(false)))
            .expect_err("busy");
        assert_eq!(error.code, freespace_app::error::UiErrorCode::Busy);

        state.cancel_task("task-1").expect("cancel");
        assert!(cancel.load(Ordering::Relaxed));
        assert_eq!(
            state.task("task-1").expect("task").state,
            TaskState::Cancelling
        );

        state.finish_task("task-1", TaskState::Cancelled, None);
        state.release_scan("volume-1");
        state
            .reserve_scan("task-2", "volume-1", Arc::new(AtomicBool::new(false)))
            .expect("second after release");
    }

    #[test]
    fn reserves_all_volumes_atomically() {
        let (_directory, state) = state();
        let cancel = Arc::new(AtomicBool::new(false));
        state
            .reserve_scans(
                "task-1",
                &["volume-1".to_owned(), "volume-2".to_owned()],
                Arc::clone(&cancel),
            )
            .expect("first");

        let task = state.task("task-1").expect("task");
        assert_eq!(task.volume_ids, ["volume-1", "volume-2"]);
        assert_eq!(task.volume_id.as_deref(), Some("volume-1"));

        let error = state
            .reserve_scans(
                "task-2",
                &["volume-2".to_owned(), "volume-3".to_owned()],
                Arc::new(AtomicBool::new(false)),
            )
            .expect_err("busy");
        assert_eq!(error.code, freespace_app::error::UiErrorCode::Busy);
        assert!(state.task("task-2").is_err(), "no partial reservation");
        assert_eq!(
            state.task("task-1").expect("task").state,
            TaskState::Running
        );

        state.release_scans(&["volume-1".to_owned(), "volume-2".to_owned()]);
        state
            .reserve_scans(
                "task-3",
                &["volume-2".to_owned()],
                Arc::new(AtomicBool::new(false)),
            )
            .expect("released");
    }

    #[test]
    fn generations_are_monotonic_and_snapshots_are_bound_to_them() {
        let (_directory, state) = state();
        assert_eq!(state.next_generation().expect("generation"), 1);
        assert_eq!(state.next_generation().expect("generation"), 2);
        assert!(state.snapshot().is_err());

        state.install_snapshot(snapshot(), Vec::new());
        assert_eq!(state.snapshot_for(3).expect("snapshot").generation(), 3);
        assert_eq!(
            state.snapshot_for(2).expect_err("stale").code,
            freespace_app::error::UiErrorCode::StaleCursor
        );
    }

    #[test]
    fn merges_a_finished_scan_into_the_current_snapshot() {
        let (_directory, state) = state();
        state.install_snapshot(snapshot(), Vec::new());
        assert_eq!(state.snapshot().expect("snapshot").volumes().len(), 1);

        let volume = Volume {
            id: Volume::id_for_mount_point(std::path::Path::new(r"D:\")),
            mount_point: r"D:\".into(),
            label: "Data".to_owned(),
            file_system: FileSystem::Ntfs,
            total_bytes: 2_000,
            free_bytes: 500,
            cluster_size: None,
            scan_method: Some(ScanMethod::DirectoryWalk),
            is_elevated: false,
            is_removable: false,
            status: VolumeStatus::Available,
        };
        let mut tree = FsTree::new();
        tree.add_node(
            None,
            NodeSpec {
                name: r"D:\".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root");
        tree.recompute_totals();
        let session = ScanSession::new(
            volume.id,
            vec![r"D:\".into()],
            ScanMethod::DirectoryWalk,
            Vec::new(),
        );

        state.merge_snapshot(4, tree, vec![volume], vec![session], Vec::new());

        let merged = state.snapshot_for(4).expect("merged");
        assert_eq!(merged.generation(), 4);
        assert_eq!(merged.volumes().len(), 2);
        assert_eq!(merged.tree().roots().count(), 1);
    }

    #[test]
    fn persists_settings_and_window_geometry() {
        let (directory, state) = state();
        let settings = state
            .update_settings(UiSettingsPatch {
                theme: Some(ThemePreference::Dark),
                language: Some(Language::En),
                last_screen: Some(ScreenId::Tree),
                saved_filters: Some(
                    [(
                        ScreenId::Tree,
                        vec![FilterSet {
                            text: "pdf".to_owned(),
                            ..FilterSet::default()
                        }],
                    )]
                    .into_iter()
                    .collect(),
                ),
                ..UiSettingsPatch::default()
            })
            .expect("update");
        assert_eq!(settings.theme, ThemePreference::Dark);
        assert!(directory.path().join("settings.toml").exists());

        state
            .reset_settings(freespace_app::settings::SettingsSection::Theme)
            .expect("reset");

        let window = WindowState {
            width: 1600,
            height: 900,
            ..WindowState::default()
        };
        let monitors = [MonitorBounds {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            name: None,
        }];
        state
            .set_window_state(window.clone(), &monitors)
            .expect("window");
        assert_eq!(state.window_state().width, 1600);
        assert!(directory.path().join("ui-state.toml").exists());

        let reloaded = AppState::new(directory.path()).expect("reload");
        assert_eq!(reloaded.ui_settings().language, Language::En);
        assert_eq!(reloaded.window_state().width, 1600);
    }

    #[test]
    fn bootstrap_contains_capabilities_and_about() {
        let (_directory, state) = state();
        let bootstrap = state.bootstrap("1.1.0", "windows", None, &[]);

        assert_eq!(bootstrap.ipc_version, 2);
        assert_eq!(bootstrap.app_version, "1.1.0");
        assert!(!bootstrap.capabilities.commands.is_empty());
        assert!(
            bootstrap
                .capabilities
                .commands
                .contains(&"snapshot_volumes".to_owned())
        );
        assert!(
            bootstrap
                .capabilities
                .events
                .contains(&"scan-progress".to_owned())
        );
        assert!(bootstrap.about.data_dir.contains("data") || !bootstrap.about.data_dir.is_empty());
        assert_eq!(
            bootstrap.capabilities.platform_capabilities,
            freespace_platform::Capabilities::current()
        );
        assert!(bootstrap.capabilities.platform_capabilities.trash);
        assert_eq!(bootstrap.settings.language, Language::Ru);
        let _ = (Metric::OnDisk, SortSpec::default());
    }
}
