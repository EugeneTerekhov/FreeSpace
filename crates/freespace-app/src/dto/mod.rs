//! Data transfer objects exchanged with the desktop client.
//!
//! The shapes follow `specs/002-native-ui-redesign/data-model.md`; field names
//! are `snake_case` on the wire and enum values use the documented lowercase or
//! kebab-case spellings. The interface never recomputes sizes or totals: every
//! number here comes from Rust.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::UiError;

/// Version of the IPC contract. Additive changes keep the version.
pub const IPC_VERSION: u32 = 1;

/// Page size used when the caller does not specify one.
pub const DEFAULT_PAGE_SIZE: u16 = 500;

/// Upper bound of a page.
pub const MAX_PAGE_SIZE: u16 = 2000;

/// The 24 screens of the product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenId {
    /// Volume dashboard.
    Dashboard,
    /// Scan progress.
    Scan,
    /// Virtualised tree.
    Tree,
    /// Canvas Treemap.
    Treemap,
    /// Largest files.
    TopFiles,
    /// File types.
    Types,
    /// Duplicate groups.
    Duplicates,
    /// Cleanup recommendations.
    Cleanup,
    /// Snapshots and comparison.
    Snapshots,
    /// Operation journal.
    Journal,
    /// Size details.
    SizeDetails,
    /// Long paths.
    LongPaths,
    /// Bulk rename.
    Rename,
    /// Permissions and attributes.
    Permissions,
    /// Metadata.
    Metadata,
    /// Owners.
    Owners,
    /// Archives.
    Archives,
    /// Export.
    Export,
    /// File unlocking.
    Unlock,
    /// Shredder.
    Shred,
    /// Write monitor.
    Monitor,
    /// Elevation.
    Elevation,
    /// Settings.
    Settings,
    /// About.
    About,
}

impl ScreenId {
    /// Every screen, in delivery order.
    pub const ALL: [Self; 24] = [
        Self::Dashboard,
        Self::Scan,
        Self::Tree,
        Self::Treemap,
        Self::TopFiles,
        Self::Types,
        Self::Duplicates,
        Self::Cleanup,
        Self::Snapshots,
        Self::Journal,
        Self::SizeDetails,
        Self::LongPaths,
        Self::Rename,
        Self::Permissions,
        Self::Metadata,
        Self::Owners,
        Self::Archives,
        Self::Export,
        Self::Unlock,
        Self::Shred,
        Self::Monitor,
        Self::Elevation,
        Self::Settings,
        Self::About,
    ];

    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dashboard => "dashboard",
            Self::Scan => "scan",
            Self::Tree => "tree",
            Self::Treemap => "treemap",
            Self::TopFiles => "top_files",
            Self::Types => "types",
            Self::Duplicates => "duplicates",
            Self::Cleanup => "cleanup",
            Self::Snapshots => "snapshots",
            Self::Journal => "journal",
            Self::SizeDetails => "size_details",
            Self::LongPaths => "long_paths",
            Self::Rename => "rename",
            Self::Permissions => "permissions",
            Self::Metadata => "metadata",
            Self::Owners => "owners",
            Self::Archives => "archives",
            Self::Export => "export",
            Self::Unlock => "unlock",
            Self::Shred => "shred",
            Self::Monitor => "monitor",
            Self::Elevation => "elevation",
            Self::Settings => "settings",
            Self::About => "about",
        }
    }

    /// Parses a wire name; unknown values are rejected.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|screen| screen.as_str() == value)
    }

    /// Navigation message key.
    #[must_use]
    pub const fn title_key(self) -> &'static str {
        match self {
            Self::Dashboard => "nav.dashboard",
            Self::Scan => "nav.scan",
            Self::Tree => "nav.tree",
            Self::Treemap => "nav.treemap",
            Self::TopFiles => "nav.top",
            Self::Types => "nav.types",
            Self::Duplicates => "nav.duplicates",
            Self::Cleanup => "nav.cleanup",
            Self::Snapshots => "nav.snapshots",
            Self::Journal => "nav.journal",
            Self::SizeDetails => "nav.size_details",
            Self::LongPaths => "nav.long_paths",
            Self::Rename => "nav.rename",
            Self::Permissions => "nav.permissions",
            Self::Metadata => "nav.metadata",
            Self::Owners => "nav.owners",
            Self::Archives => "nav.archives",
            Self::Export => "nav.export",
            Self::Unlock => "nav.unlock",
            Self::Shred => "nav.shred",
            Self::Monitor => "nav.monitor",
            Self::Elevation => "nav.elevation",
            Self::Settings => "nav.settings",
            Self::About => "nav.about",
        }
    }
}

/// Size metric used for sorting.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Sum of file lengths.
    Logical,
    /// Allocated space.
    #[default]
    OnDisk,
    /// Physical contribution with hard links counted once.
    Physical,
}

impl Metric {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Logical => "logical",
            Self::OnDisk => "on_disk",
            Self::Physical => "physical",
        }
    }

    /// Value of this metric in a size record.
    #[must_use]
    pub const fn of(self, size: freespace_core::size::SizeMetrics) -> u64 {
        match self {
            Self::Logical => size.logical,
            Self::OnDisk => size.on_disk,
            Self::Physical => size.physical,
        }
    }
}

/// Sort key of a tree page.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    /// Name, case-insensitive on Windows.
    Name,
    /// Selected size metric.
    #[default]
    Size,
    /// Modification time.
    Modified,
    /// Descendant item count.
    Items,
}

impl SortKey {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Size => "size",
            Self::Modified => "modified",
            Self::Items => "items",
        }
    }
}

/// One active sort key and its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortSpec {
    /// Active key.
    pub key: SortKey,
    /// Descending order when true.
    pub descending: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            key: SortKey::Size,
            descending: true,
        }
    }
}

/// File-system object kind in a row.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// Regular file.
    #[default]
    File,
    /// Directory.
    Directory,
    /// Symbolic link, junction or hard link.
    Link,
    /// Anything else.
    Other,
}

impl NodeKind {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Link => "link",
            Self::Other => "other",
        }
    }
}

/// Attribute flags shown by the interface.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AttributeFlags {
    /// Read-only flag.
    pub read_only: bool,
    /// Hidden flag.
    pub hidden: bool,
    /// System flag.
    pub system: bool,
    /// Compressed flag.
    pub compressed: bool,
    /// Sparse flag.
    pub sparse: bool,
    /// Reparse-point flag.
    pub reparse: bool,
}

/// One row of a tree or table page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRow {
    /// Node identifier inside the generation.
    pub id: u32,
    /// Parent node, `None` for a scan root.
    pub parent_id: Option<u32>,
    /// Display name.
    pub name: String,
    /// Full path.
    pub path: String,
    /// Object kind.
    pub kind: NodeKind,
    /// Logical size in bytes.
    pub logical_size: u64,
    /// Allocated size in bytes.
    pub on_disk_size: u64,
    /// Physical contribution in bytes.
    pub physical_size: u64,
    /// Alternate-data-stream size in bytes.
    pub ads_size: u64,
    /// Direct child count.
    pub child_count: u32,
    /// Descendant file count.
    pub file_count: u64,
    /// Modification time in Unix milliseconds.
    pub modified_at: Option<i64>,
    /// Last access time in Unix milliseconds.
    pub accessed_at: Option<i64>,
    /// Attribute flags.
    pub attributes: AttributeFlags,
    /// False when access was denied; sizes are then incomplete.
    pub accessible: bool,
    /// Reparse-point target.
    pub link_target: Option<String>,
}

/// One page of tree or table rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreePage {
    /// Generation the page belongs to.
    pub generation: u64,
    /// Parent node, `None` for roots and flat lists.
    pub parent_id: Option<u32>,
    /// Cursor that produced this page.
    pub cursor: Option<String>,
    /// Cursor for the next page; `None` at the end.
    pub next_cursor: Option<String>,
    /// Rows, at most `page_size`.
    pub rows: Vec<NodeRow>,
    /// Rough total row count, never used for totals.
    pub total_estimate: Option<u64>,
    /// Metric used for sizes.
    pub metric: Metric,
    /// Active sorting.
    pub sort: SortSpec,
    /// True when the cursor is stale and the first page must be requested.
    pub stale_cursor: bool,
}

/// Filter shared by every table screen.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterSet {
    /// Free-text filter, at most 256 characters.
    pub text: String,
    /// Normalised extensions without the leading dot.
    pub extensions: Vec<String>,
    /// File categories.
    pub categories: Vec<String>,
    /// Minimum size in bytes.
    pub min_size: Option<u64>,
    /// Maximum size in bytes.
    pub max_size: Option<u64>,
    /// Lower bound of the modification time, Unix milliseconds.
    pub modified_after: Option<i64>,
    /// Upper bound of the modification time, Unix milliseconds.
    pub modified_before: Option<i64>,
    /// Only rows that were readable.
    pub accessible_only: bool,
    /// Paths excluded from the result.
    pub exclude_paths: Vec<String>,
    /// Saved filter name.
    pub name: Option<String>,
}

/// Volume summary shown by the dashboard.
/// Volume summary shown by the dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeSummary {
    /// Stable volume identifier.
    pub volume_id: String,
    /// Mount point with a trailing separator.
    pub mount_point: String,
    /// Volume label.
    pub label: String,
    /// File system name.
    pub file_system: String,
    /// Total size.
    pub total_bytes: u64,
    /// Free space.
    pub free_bytes: u64,
    /// Occupied space.
    pub used_bytes: u64,
    /// Removable media.
    pub is_removable: bool,
    /// Availability.
    pub status: String,
    /// Method used by the latest scan.
    pub scan_method: Option<String>,
    /// Whether the latest scan ran elevated.
    pub is_elevated: bool,
}

/// Volume detail with cluster information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolumeDetail {
    /// Summary fields.
    #[serde(flatten)]
    pub summary: VolumeSummary,
    /// Cluster size when known.
    pub cluster_size: Option<u32>,
    /// Occupied fraction in `0.0..=1.0`.
    pub used_fraction: f32,
}

/// Scanner requested by the interface.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanMethodChoice {
    /// MFT where possible, directory walk otherwise.
    #[default]
    Auto,
    /// Require the MFT scanner.
    Mft,
    /// Always walk directories.
    Walk,
}

/// Input of `scan_start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanRequest {
    /// Volume root or directory.
    pub path: String,
    /// Requested scanner.
    #[serde(default)]
    pub method: ScanMethodChoice,
    /// Ask for an incremental update when possible.
    #[serde(default)]
    pub incremental: bool,
    /// Glob exclusions.
    #[serde(default)]
    pub exclusions: Vec<String>,
    /// Include alternate data streams.
    #[serde(default)]
    pub include_ads: bool,
    /// Metric the interface will display.
    #[serde(default)]
    pub metric: Metric,
}

/// Output of `scan_start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanStarted {
    /// Background task identifier.
    pub task_id: String,
    /// Generation reserved for the finished scan.
    pub generation: u64,
}

/// Kind of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// File-system scan.
    Scan,
    /// Duplicate search.
    Duplicates,
    /// Snapshot comparison.
    SnapshotCompare,
    /// Archive scan.
    ArchiveScan,
    /// Write monitor.
    Monitor,
}

/// Lifecycle state of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    /// Waiting for a worker.
    Queued,
    /// Running.
    Running,
    /// Cancellation requested.
    Cancelling,
    /// Cancelled by the user.
    Cancelled,
    /// Finished successfully.
    Completed,
    /// Finished with recoverable issues.
    Partial,
    /// Failed.
    Failed,
}

impl TaskState {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Cancelling => "cancelling",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }

    /// Returns `true` when no further transitions are expected.
    #[must_use]
    pub const fn is_finished(self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Completed | Self::Partial | Self::Failed
        )
    }
}

/// Progress of a background task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskProgress {
    /// Items processed; never decreases.
    pub processed: u64,
    /// Total items when known.
    pub total: Option<u64>,
    /// Current object.
    pub current_path: Option<String>,
    /// Throughput in bytes per second.
    pub bytes_per_second: Option<u64>,
    /// Throughput in items per second.
    pub items_per_second: Option<u64>,
    /// Localisable phase key.
    pub phase_key: String,
    /// Last update, Unix milliseconds.
    pub updated_at: i64,
}

/// Visible state of a background task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// Task identifier.
    pub task_id: String,
    /// Task kind.
    pub kind: TaskKind,
    /// Lifecycle state.
    pub state: TaskState,
    /// Volume the task works on.
    pub volume_id: Option<String>,
    /// Progress while running.
    pub progress: Option<TaskProgress>,
    /// Start time, Unix milliseconds.
    pub started_at: i64,
    /// Finish time, Unix milliseconds.
    pub finished_at: Option<i64>,
    /// False after the point of no return.
    pub cancelable: bool,
    /// Failure or partial-success envelope.
    pub error: Option<UiError>,
}

/// Summary of a finished scan session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanSessionSummary {
    /// Session identifier.
    pub session_id: String,
    /// Volume identifier.
    pub volume_id: String,
    /// Method that ran.
    pub method: String,
    /// Start time, Unix milliseconds.
    pub started_at: i64,
    /// Finish time, Unix milliseconds.
    pub finished_at: Option<i64>,
    /// Lifecycle state.
    pub status: String,
    /// Number of produced nodes.
    pub nodes_count: u64,
    /// Analysed roots.
    pub roots: Vec<String>,
    /// Number of non-fatal issues.
    pub issues_count: u64,
}

/// Theme preference.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    /// Follow the operating system.
    #[default]
    System,
    /// Always dark.
    Dark,
    /// Always light.
    Light,
}

impl ThemePreference {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// Interface language.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Russian.
    #[default]
    Ru,
    /// English.
    En,
}

impl Language {
    /// Stable wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }
}

/// Row density.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    /// Comfortable rows.
    #[default]
    Comfortable,
    /// Compact rows.
    Compact,
}

/// Unit system of the interface.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitSystem {
    /// KiB, MiB, GiB.
    #[default]
    Binary,
    /// KB, MB, GB.
    Decimal,
}

/// Reduced-motion preference.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReducedMotion {
    /// Follow the operating system.
    #[default]
    System,
    /// Force animations off.
    On,
    /// Allow animations.
    Off,
}

/// Persisted column state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnState {
    /// Column identifier unique per screen.
    pub id: String,
    /// Visibility.
    pub visible: bool,
    /// Width in pixels, 40..=1200.
    pub width_px: u16,
    /// Display order, unique per screen.
    pub order: u16,
    /// Whether the column can be sorted.
    #[serde(default)]
    pub sortable: bool,
}

/// Interface settings stored under `[ui]` in `settings.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// Theme preference.
    pub theme: ThemePreference,
    /// Interface language.
    pub language: Language,
    /// Row density.
    pub density: Density,
    /// Unit system.
    pub units: UnitSystem,
    /// Reduced-motion preference.
    pub reduced_motion: ReducedMotion,
    /// Last open screen.
    pub last_screen: Option<ScreenId>,
    /// Sidebar collapsed state.
    pub sidebar_collapsed: bool,
    /// Column layouts per screen.
    pub column_layouts: BTreeMap<ScreenId, Vec<ColumnState>>,
    /// Saved filters per screen.
    pub saved_filters: BTreeMap<ScreenId, Vec<FilterSet>>,
    /// Schema version of the section.
    pub schema_version: u16,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            language: Language::Ru,
            density: Density::Comfortable,
            units: UnitSystem::Binary,
            reduced_motion: ReducedMotion::System,
            last_screen: None,
            sidebar_collapsed: false,
            column_layouts: BTreeMap::new(),
            saved_filters: BTreeMap::new(),
            schema_version: 1,
        }
    }
}

/// Partial update of [`UiSettings`]; absent fields keep their value.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettingsPatch {
    /// Theme preference.
    pub theme: Option<ThemePreference>,
    /// Interface language.
    pub language: Option<Language>,
    /// Row density.
    pub density: Option<Density>,
    /// Unit system.
    pub units: Option<UnitSystem>,
    /// Reduced-motion preference.
    pub reduced_motion: Option<ReducedMotion>,
    /// Last open screen.
    pub last_screen: Option<ScreenId>,
    /// Sidebar collapsed state.
    pub sidebar_collapsed: Option<bool>,
    /// Column layouts per screen.
    pub column_layouts: Option<BTreeMap<ScreenId, Vec<ColumnState>>>,
    /// Saved filters per screen.
    pub saved_filters: Option<BTreeMap<ScreenId, Vec<FilterSet>>>,
}

/// Persisted window geometry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Left edge; `None` centers the window.
    pub x: Option<i32>,
    /// Top edge; `None` centers the window.
    pub y: Option<i32>,
    /// Maximised state.
    pub maximized: bool,
    /// Monitor identifier.
    pub monitor_id: Option<String>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 800,
            x: None,
            y: None,
            maximized: false,
            monitor_id: None,
        }
    }
}

/// License entry shown by the About screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseInfo {
    /// Component name.
    pub name: String,
    /// License identifier.
    pub license: String,
    /// Project URL is never opened automatically; it is shown as text only.
    pub url: Option<String>,
}

/// About-screen payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AboutInfo {
    /// Application version.
    pub version: String,
    /// Build label.
    pub build: String,
    /// WebView2 version when known.
    pub webview_version: Option<String>,
    /// Third-party licenses.
    pub licenses: Vec<LicenseInfo>,
    /// Absolute path of the `data` directory.
    pub data_dir: String,
}

/// Commands and events exposed to the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppCapabilities {
    /// Command names the client may call.
    pub commands: Vec<String>,
    /// Events the client may listen to.
    pub events: Vec<String>,
    /// Version of the CSP policy.
    pub csp_version: u32,
}

/// Everything the shell needs before the first screen renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppBootstrap {
    /// IPC contract version.
    pub ipc_version: u32,
    /// Application version.
    pub app_version: String,
    /// Operating-system label.
    pub os: String,
    /// WebView2 version when known.
    pub webview_version: Option<String>,
    /// Migrated interface settings.
    pub settings: UiSettings,
    /// Sanitised window geometry.
    pub window_state: WindowState,
    /// Command and event registry.
    pub capabilities: AppCapabilities,
    /// About payload.
    pub about: AboutInfo,
}

/// Navigation request from a deep link or the single-instance plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationRequest {
    /// Target screen.
    pub screen: ScreenId,
    /// Optional node to select.
    pub node_id: Option<u32>,
    /// Optional path to reveal.
    pub path: Option<String>,
}

/// Elevation status of the process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevationStatus {
    /// True when the process already runs elevated.
    pub elevated: bool,
    /// True when elevation can be requested.
    pub can_elevate: bool,
    /// Localisable explanation key.
    pub reason_key: Option<String>,
}

/// Kind of a destructive or mutating operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationKind {
    /// Delete to the Recycle Bin, quarantine or permanently.
    Delete,
    /// Move targets.
    Move,
    /// Rename targets.
    Rename,
    /// Change permissions.
    Permissions,
    /// Irreversible shredding.
    Shred,
    /// Restore from quarantine.
    Restore,
}

/// Deletion mode chosen before confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMode {
    /// Move to the Recycle Bin.
    RecycleBin,
    /// Move into the application quarantine.
    Quarantine,
    /// Irreversible deletion.
    Permanent,
}

/// One preview target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationTarget {
    /// Full path.
    pub path: String,
    /// Object kind.
    pub kind: NodeKind,
    /// Size in bytes.
    pub size: u64,
    /// False when the object cannot be read.
    pub accessible: bool,
    /// True for protected system paths.
    pub is_system: bool,
}

/// Preview of a destructive operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationPreview {
    /// One-time preview identifier.
    pub preview_id: String,
    /// Operation kind.
    pub kind: OperationKind,
    /// Targets, never empty.
    pub targets: Vec<OperationTarget>,
    /// Sum of the target sizes.
    pub total_size: u64,
    /// Deletion mode for delete and shred.
    pub delete_mode: Option<DeleteMode>,
    /// Whether the operation can be undone.
    pub reversible: bool,
    /// Whether elevation is required.
    pub requires_elevation: bool,
    /// Localisable warning keys.
    pub warnings: Vec<String>,
    /// Expiry, Unix milliseconds.
    pub expires_at: i64,
    /// One-time confirmation token bound to the preview.
    pub confirmation_token: String,
}

/// Lifecycle state of a running operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationState {
    /// Building the plan.
    Preparing,
    /// Executing.
    Running,
    /// Cancellation requested.
    Cancelling,
    /// Finished successfully.
    Completed,
    /// Finished with errors.
    Partial,
    /// Failed.
    Failed,
    /// Cancelled by the user.
    Cancelled,
}

impl OperationState {
    /// Returns `true` when no further transitions are expected.
    #[must_use]
    pub const fn is_finished(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Partial | Self::Failed | Self::Cancelled
        )
    }
}

/// Progress of a running operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationProgress {
    /// Operation identifier.
    pub operation_id: String,
    /// Lifecycle state.
    pub state: OperationState,
    /// Targets processed.
    pub processed: u64,
    /// Targets in total.
    pub total: u64,
    /// Current target path.
    pub current_target: Option<String>,
    /// Throughput in bytes per second.
    pub bytes_per_second: Option<u64>,
    /// Errors; the journal keeps the full list.
    pub errors: Vec<UiError>,
    /// True when undo is available.
    pub undo_available: bool,
    /// Journal entry, when written.
    pub journal_entry_id: Option<String>,
}

/// Outcome recorded in the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JournalResult {
    /// Every target was processed.
    Success,
    /// Some targets failed.
    Partial,
    /// Nothing was processed.
    Failed,
    /// The user cancelled.
    Cancelled,
}

/// Journal row shown by the Journal screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntryView {
    /// Journal entry identifier.
    pub id: String,
    /// Operation this entry belongs to; used by undo and restore.
    pub operation_id: String,
    /// Occurrence time, Unix milliseconds.
    pub occurred_at: i64,
    /// Local user name.
    pub user: String,
    /// Operation kind.
    pub kind: OperationKind,
    /// Number of targets.
    pub targets_count: u32,
    /// Total size in bytes.
    pub total_size: u64,
    /// Outcome.
    pub result: JournalResult,
    /// Whether undo or restore is available.
    pub reversible: bool,
    /// Quarantine identifiers for restore.
    pub quarantine_ids: Vec<String>,
    /// Error when the result is not a success.
    pub error: Option<UiError>,
}

/// One page of journal rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalPage {
    /// Cursor that produced this page.
    pub cursor: Option<String>,
    /// Cursor for the next page; `None` at the end.
    pub next_cursor: Option<String>,
    /// Rows, newest first.
    pub rows: Vec<JournalEntryView>,
    /// Rough total row count.
    pub total_estimate: u64,
}

/// Server-side journal filter.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JournalFilter {
    /// Free-text match over the user, operation and quarantine identifiers.
    pub text: String,
    /// Restrict to these operation kinds.
    pub kinds: Vec<OperationKind>,
    /// Restrict to these outcomes.
    pub results: Vec<JournalResult>,
}

/// One row of the per-extension type report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeSummary {
    /// Lower-cased extension without the dot; empty for files without one.
    pub extension: String,
    /// Coarse category key.
    pub category: String,
    /// Number of files with this extension.
    pub count: u64,
    /// Summed logical size.
    pub logical_size: u64,
    /// Summed on-disk size.
    pub on_disk_size: u64,
    /// Summed physical size.
    pub physical_size: u64,
    /// Share of the analysed file bytes in `0.0..=1.0`.
    pub share: f64,
}

/// State of a duplicate search task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateTask {
    /// Background task identifier.
    pub task_id: String,
    /// Lifecycle state.
    pub state: TaskState,
    /// Groups found so far or in total.
    pub groups_count: u64,
    /// Files that shared a size with another file.
    pub candidates: u64,
    /// Files whose complete contents were hashed.
    pub hashed: u64,
    /// Total reclaimable bytes.
    pub reclaimable_bytes: u64,
    /// Non-fatal notes.
    pub warnings: Vec<String>,
    /// Failure or partial-success envelope.
    pub error: Option<UiError>,
}

/// One file of a duplicate group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateMemberView {
    /// Tree node.
    pub node_id: u32,
    /// Full path.
    pub path: String,
    /// File name.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Last modification time, Unix milliseconds.
    pub modified_at: i64,
    /// Whether this copy is proposed for deletion.
    pub selected_for_deletion: bool,
    /// Whether a byte-by-byte comparison confirmed the match.
    pub verified: bool,
    /// Why the copy is kept or selected.
    pub reason: String,
}

/// A group of byte-identical files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateGroupView {
    /// Stable identifier of the group inside the task.
    pub id: String,
    /// Digest algorithm.
    pub algorithm: String,
    /// Full digest in hexadecimal form.
    pub hash: String,
    /// Size of every member in bytes.
    pub size: u64,
    /// Members, at least two.
    pub members: Vec<DuplicateMemberView>,
    /// Recommended copy to keep.
    pub keep_node_id: Option<u32>,
    /// Bytes freed by deleting every selected copy.
    pub reclaimable_bytes: u64,
    /// Whether the files changed while the search was running.
    pub stale: bool,
    /// False when the group cannot be offered for deletion.
    pub deletable: bool,
}

/// One page of duplicate groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateGroupPage {
    /// Cursor that produced this page.
    pub cursor: Option<String>,
    /// Cursor for the next page; `None` at the end.
    pub next_cursor: Option<String>,
    /// Groups, largest reclaimable space first.
    pub groups: Vec<DuplicateGroupView>,
    /// Rough total group count.
    pub total_estimate: u64,
}

/// One cleanup recommendation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupCandidateView {
    /// Tree node.
    pub node_id: u32,
    /// Full path.
    pub path: String,
    /// Display name.
    pub name: String,
    /// Category key: temp, cache, log, backup or outdated.
    pub category: String,
    /// Localisable explanation key.
    pub reason_key: String,
    /// Bytes that removal would free.
    pub reclaimable_bytes: u64,
    /// Risk key: low, medium or high.
    pub risk: String,
    /// Recommended action key: review, delete or keep.
    pub recommended_action: String,
}

/// Summary of a stored `.fssnap` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSummary {
    /// Stable identifier: the file stem.
    pub snapshot_id: String,
    /// Absolute file path.
    pub path: String,
    /// Volume identifier stored inside the snapshot.
    pub volume_id: String,
    /// Volume mount point stored inside the snapshot.
    pub mount_point: String,
    /// Creation instant in ISO 8601.
    pub created_at: String,
    /// Number of stored nodes.
    pub node_count: u64,
    /// Size of the file on disk.
    pub file_size: u64,
}

/// One row of a snapshot comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotDiffRow {
    /// Path relative to the snapshot root.
    pub path: String,
    /// Change key: added, removed or changed.
    pub change: String,
    /// Whether the object is a directory.
    pub is_directory: bool,
    /// Signed change of the physical size.
    pub size_delta: i64,
    /// Signed change of the modification time in seconds.
    pub mtime_delta: i64,
    /// Physical size in the base snapshot.
    pub base_size: u64,
    /// Physical size in the target snapshot.
    pub target_size: u64,
}

/// Growth of one directory between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotGrowth {
    /// Path relative to the snapshot root.
    pub path: String,
    /// Signed growth of the subtree.
    pub delta: i64,
}

/// Sort keys of a snapshot diff page.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotDiffSortKey {
    /// Relative path.
    Path,
    /// Change kind.
    Change,
    /// Absolute size change, largest first by default.
    #[default]
    SizeDelta,
}

/// One requested ordering of a snapshot diff page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotDiffSort {
    /// Active key.
    pub key: SnapshotDiffSortKey,
    /// Descending order when true.
    pub descending: bool,
}

impl Default for SnapshotDiffSort {
    fn default() -> Self {
        Self {
            key: SnapshotDiffSortKey::SizeDelta,
            descending: true,
        }
    }
}

/// One page of a snapshot comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotDiffPage {
    /// Task that produced the comparison.
    pub task_id: String,
    /// Cursor that produced this page.
    pub cursor: Option<String>,
    /// Cursor for the next page; `None` at the end.
    pub next_cursor: Option<String>,
    /// Rows, ordered by the requested key.
    pub rows: Vec<SnapshotDiffRow>,
    /// Rough total row count.
    pub total_estimate: u64,
    /// Number of added objects.
    pub added: u64,
    /// Number of removed objects.
    pub removed: u64,
    /// Number of changed objects.
    pub changed: u64,
    /// Total signed growth of the analysed tree.
    pub total_growth_bytes: i64,
    /// Growth per directory, largest first.
    pub growth_by_directory: Vec<SnapshotGrowth>,
}

/// Input of `snapshots_create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCreateRequest {
    /// Volume root or directory to snapshot; defaults to the current scan.
    pub path: Option<String>,
    /// Output file; `None` uses `data/snapshots/<timestamp>.fssnap`.
    pub out: Option<String>,
}

/// Output of `snapshots_create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCreated {
    /// Task identifier of the creation.
    pub task_id: String,
    /// Identifier of the new snapshot.
    pub snapshot_id: String,
    /// Absolute path of the new snapshot.
    pub path: String,
    /// Number of stored nodes.
    pub node_count: u64,
}

/// Identifier of a started background task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStarted {
    /// Background task identifier.
    pub task_id: String,
}

/// Input of `duplicates_start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateStartRequest {
    /// Current volume root or directory.
    pub path: String,
    /// Ignore files smaller than this many bytes.
    #[serde(default = "default_duplicate_min_size")]
    pub min_size: u64,
    /// Digest algorithm.
    #[serde(default = "default_duplicate_algorithm")]
    pub algorithm: String,
    /// Rule that selects the copy to keep.
    #[serde(default = "default_duplicate_rule")]
    pub keep_rule: String,
    /// Glob exclusions; paths are excluded in addition to the scan.
    #[serde(default)]
    pub exclusions: Vec<String>,
}

const fn default_duplicate_min_size() -> u64 {
    1
}

fn default_duplicate_algorithm() -> String {
    "sha256".to_owned()
}

fn default_duplicate_rule() -> String {
    "oldest".to_owned()
}

/// Request of `operation_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationPreviewRequest {
    /// Operation kind.
    pub kind: OperationKind,
    /// Full paths of the targets.
    pub targets: Vec<String>,
    /// Selected deletion mode; optional for delete and shred.
    #[serde(default)]
    pub delete_mode: Option<DeleteMode>,
    /// Free-form reason recorded in the journal.
    #[serde(default)]
    pub reason: Option<String>,
    /// Shredder passes.
    #[serde(default)]
    pub passes: Option<u32>,
    /// Duplicate-search task whose selection must stay safe.
    #[serde(default)]
    pub task_id: Option<String>,
}

/// Identifier of an accepted operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationAccepted {
    /// Operation identifier.
    pub operation_id: String,
}

/// Outcome of a finished operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationOutcome {
    /// Operation identifier.
    pub operation_id: String,
    /// Terminal state.
    pub state: OperationState,
    /// Journal entry, when one was written.
    pub journal_entry_id: Option<String>,
    /// True when undo or restore is available.
    pub undo_available: bool,
    /// Number of processed targets.
    pub processed: u64,
    /// Number of targets in total.
    pub total: u64,
    /// Per-target errors.
    pub errors: Vec<UiError>,
    /// Quarantine identifiers of restored or quarantined objects.
    pub quarantine_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        AppCapabilities, Language, Metric, ScreenId, SortSpec, ThemePreference, UiSettings,
        UiSettingsPatch, WindowState,
    };

    #[test]
    fn exposes_exactly_twenty_four_screens() {
        assert_eq!(ScreenId::ALL.len(), 24);

        for screen in ScreenId::ALL {
            assert_eq!(ScreenId::parse_value(screen.as_str()), Some(screen));
            assert!(screen.title_key().starts_with("nav."));
        }

        assert_eq!(ScreenId::parse_value("nope"), None);
        assert_eq!(
            serde_json::to_string(&ScreenId::TopFiles).expect("serialise"),
            "\"top_files\""
        );
    }

    #[test]
    fn defaults_of_the_interface_are_safe() {
        let settings = UiSettings::default();

        assert_eq!(settings.theme, ThemePreference::System);
        assert_eq!(settings.language, Language::Ru);
        assert_eq!(settings.schema_version, 1);
        assert!(settings.column_layouts.is_empty());
        assert_eq!(WindowState::default().width, 1280);
        assert_eq!(Metric::default(), Metric::OnDisk);
        assert_eq!(SortSpec::default().key, super::SortKey::Size);
    }

    #[test]
    fn serde_keeps_wire_names_and_defaults() {
        let settings: UiSettings =
            serde_json::from_str(r#"{"theme":"dark","language":"en","last_screen":"tree"}"#)
                .expect("parse partial settings");
        assert_eq!(settings.theme, ThemePreference::Dark);
        assert_eq!(settings.language, Language::En);
        assert_eq!(settings.last_screen, Some(ScreenId::Tree));
        assert_eq!(settings.schema_version, 1);

        let patch: UiSettingsPatch =
            serde_json::from_str(r#"{"sidebar_collapsed":true}"#).expect("patch");
        assert_eq!(patch.sidebar_collapsed, Some(true));
        assert!(patch.theme.is_none());
    }

    #[test]
    fn serialises_capabilities() {
        let capabilities = AppCapabilities {
            commands: vec!["app_bootstrap".to_owned()],
            events: vec!["scan-progress".to_owned()],
            csp_version: 1,
        };
        let value = serde_json::to_value(&capabilities).expect("serialise");
        assert_eq!(value["commands"][0], "app_bootstrap");
        assert_eq!(value["csp_version"], 1);
    }
}

// Delivery batches 4–6 keep their payloads in separate modules so the three
// workstreams never edit the same file. Types are re-exported here, so callers
// keep using `freespace_app::dto::SizeDetails`.
pub mod filesystem;
pub mod inspection;
pub mod settings_ops;

#[allow(unused_imports)]
pub use filesystem::*;
#[allow(unused_imports)]
pub use inspection::*;
#[allow(unused_imports)]
pub use settings_ops::*;
