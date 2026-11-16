/**
 * Wire types of the IPC contract.
 *
 * These interfaces mirror `crates/freespace-app/src/dto` and
 * `specs/002-native-ui-redesign/contracts/ipc.md`. Sizes and totals are always
 * produced by Rust; the interface never recomputes them.
 */

export type ScreenId =
  | 'dashboard'
  | 'scan'
  | 'tree'
  | 'treemap'
  | 'top_files'
  | 'types'
  | 'duplicates'
  | 'cleanup'
  | 'snapshots'
  | 'journal'
  | 'size_details'
  | 'long_paths'
  | 'rename'
  | 'permissions'
  | 'metadata'
  | 'owners'
  | 'archives'
  | 'export'
  | 'unlock'
  | 'shred'
  | 'monitor'
  | 'elevation'
  | 'settings'
  | 'about';

export type UiErrorCode =
  | 'E_VALIDATION'
  | 'E_ACCESS_DENIED'
  | 'E_NOT_FOUND'
  | 'E_STALE_CURSOR'
  | 'E_STALE_NODE'
  | 'E_BUSY'
  | 'E_CANCELLED'
  | 'E_PREVIEW_EXPIRED'
  | 'E_ELEVATION_REQUIRED'
  | 'E_ELEVATION_DENIED'
  | 'E_OPERATION_PARTIAL'
  | 'E_IO'
  | 'E_UNSUPPORTED'
  | 'E_OFFLINE'
  | 'E_INTERNAL';

export type ErrorCategory =
  | 'validation'
  | 'access'
  | 'io'
  | 'stale'
  | 'operation'
  | 'cancel'
  | 'internal';

export interface UiError {
  code: UiErrorCode;
  category: ErrorCategory;
  message_key: string;
  details: string | null;
  retryable: boolean;
  task_id: string | null;
  journal_entry_id: string | null;
}

export type Metric = 'logical' | 'on_disk' | 'physical';
export type SortKey = 'name' | 'size' | 'modified' | 'items';

export interface SortSpec {
  key: SortKey;
  descending: boolean;
}

export type NodeKind = 'file' | 'directory' | 'link' | 'other';

export interface AttributeFlags {
  read_only: boolean;
  hidden: boolean;
  system: boolean;
  compressed: boolean;
  sparse: boolean;
  reparse: boolean;
}

export interface NodeRow {
  id: number;
  parent_id: number | null;
  name: string;
  path: string;
  kind: NodeKind;
  logical_size: number;
  on_disk_size: number;
  physical_size: number;
  ads_size: number;
  child_count: number;
  file_count: number;
  modified_at: number | null;
  accessed_at: number | null;
  attributes: AttributeFlags;
  accessible: boolean;
  link_target: string | null;
}

export interface TreePage {
  generation: number;
  parent_id: number | null;
  cursor: string | null;
  next_cursor: string | null;
  rows: NodeRow[];
  total_estimate: number | null;
  metric: Metric;
  sort: SortSpec;
  stale_cursor: boolean;
}

export interface FilterSet {
  text: string;
  extensions: string[];
  categories: string[];
  min_size: number | null;
  max_size: number | null;
  modified_after: number | null;
  modified_before: number | null;
  accessible_only: boolean;
  exclude_paths: string[];
  name: string | null;
}

export interface TreemapRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface TreemapRequest {
  node_id: number | null;
  width: number;
  height: number;
  metric: Metric;
  max_depth: number;
  min_tile_px: number;
  max_tiles: number;
}

export interface TreemapTile {
  node_id: number | null;
  rect: TreemapRect;
  depth: number;
  size: number;
  fraction: number;
  color_key: string;
  label: string;
  is_aggregate: boolean;
  child_count: number;
}

export interface TreemapResponse {
  generation: number;
  tiles: TreemapTile[];
  truncated: boolean;
  total_size: number;
}

export interface VolumeSummary {
  volume_id: string;
  mount_point: string;
  label: string;
  file_system: string;
  total_bytes: number;
  free_bytes: number;
  used_bytes: number;
  is_removable: boolean;
  status: string;
  scan_method: string | null;
  is_elevated: boolean;
}

export interface VolumeDetail extends VolumeSummary {
  cluster_size: number | null;
  used_fraction: number;
}

export type ScanMethodChoice = 'auto' | 'mft' | 'walk';

export interface ScanRequest {
  path: string;
  method: ScanMethodChoice;
  incremental: boolean;
  exclusions: string[];
  include_ads: boolean;
  metric: Metric;
}

export interface ScanStarted {
  task_id: string;
  generation: number;
}

export type TaskState =
  | 'queued'
  | 'running'
  | 'cancelling'
  | 'cancelled'
  | 'completed'
  | 'partial'
  | 'failed';

export interface TaskProgress {
  processed: number;
  total: number | null;
  current_path: string | null;
  bytes_per_second: number | null;
  items_per_second: number | null;
  phase_key: string;
  updated_at: number;
}

export interface Task {
  task_id: string;
  kind: 'scan' | 'duplicates' | 'snapshot_compare' | 'archive_scan' | 'monitor';
  state: TaskState;
  volume_id: string | null;
  progress: TaskProgress | null;
  started_at: number;
  finished_at: number | null;
  cancelable: boolean;
  error: UiError | null;
}

export interface ScanSessionSummary {
  session_id: string;
  volume_id: string;
  method: string;
  started_at: number;
  finished_at: number | null;
  status: string;
  nodes_count: number;
  roots: string[];
  issues_count: number;
}

export type ThemePreference = 'system' | 'dark' | 'light';
export type Language = 'ru' | 'en';
export type Density = 'comfortable' | 'compact';
export type UnitSystem = 'binary' | 'decimal';
export type ReducedMotion = 'system' | 'on' | 'off';

export interface ColumnState {
  id: string;
  visible: boolean;
  width_px: number;
  order: number;
  sortable: boolean;
}

export interface UiSettings {
  theme: ThemePreference;
  language: Language;
  density: Density;
  units: UnitSystem;
  reduced_motion: ReducedMotion;
  last_screen: ScreenId | null;
  sidebar_collapsed: boolean;
  column_layouts: Partial<Record<ScreenId, ColumnState[]>>;
  saved_filters: Partial<Record<ScreenId, FilterSet[]>>;
  schema_version: number;
}

export type UiSettingsPatch = Partial<Omit<UiSettings, 'schema_version'>>;

export interface WindowState {
  width: number;
  height: number;
  x: number | null;
  y: number | null;
  maximized: boolean;
  monitor_id: string | null;
}

export interface LicenseInfo {
  name: string;
  license: string;
  url: string | null;
}

export interface AboutInfo {
  version: string;
  build: string;
  webview_version: string | null;
  licenses: LicenseInfo[];
  data_dir: string;
}

export interface AppCapabilities {
  commands: string[];
  events: string[];
  csp_version: number;
}

export interface AppBootstrap {
  ipc_version: number;
  app_version: string;
  os: string;
  webview_version: string | null;
  settings: UiSettings;
  window_state: WindowState;
  capabilities: AppCapabilities;
  about: AboutInfo;
}

export interface NavigationRequest {
  screen: ScreenId;
  node_id: number | null;
  path: string | null;
}

export interface ElevationStatus {
  elevated: boolean;
  can_elevate: boolean;
  reason_key: string | null;
}

export interface ScanProgressEvent {
  progress: { processed: number; total: number | null };
  phase_key: string;
  current_path: string | null;
  bytes: number;
  bytes_per_second: number | null;
}

export interface ScanSummary {
  nodes_count: number;
  method: string;
  roots: string[];
  warnings: string[];
  issues_count: number;
  elapsed_ms: number;
}

export interface ScanCompletedEvent {
  task_id: string;
  state: TaskState;
  generation: number;
  summary: ScanSummary | null;
  error: UiError | null;
}

export interface EventEnvelope<T> {
  version: number;
  event: string;
  sequence: number;
  task_id: string | null;
  emitted_at: string;
  payload: T;
}

export type JournalResult = 'success' | 'partial' | 'failed' | 'cancelled';

export interface JournalEntryView {
  id: string;
  operation_id: string;
  occurred_at: number;
  user: string;
  kind: OperationKind;
  targets_count: number;
  total_size: number;
  result: JournalResult;
  reversible: boolean;
  quarantine_ids: string[];
  error: UiError | null;
}

export interface JournalPage {
  cursor: string | null;
  next_cursor: string | null;
  rows: JournalEntryView[];
  total_estimate: number;
}

export interface JournalFilter {
  text: string;
  kinds: OperationKind[];
  results: JournalResult[];
}

export interface TypeSummary {
  extension: string;
  category: string;
  count: number;
  logical_size: number;
  on_disk_size: number;
  physical_size: number;
  share: number;
}

export interface DuplicateTask {
  task_id: string;
  state: TaskState;
  groups_count: number;
  candidates: number;
  hashed: number;
  reclaimable_bytes: number;
  warnings: string[];
  error: UiError | null;
}

export interface DuplicateMemberView {
  node_id: number;
  path: string;
  name: string;
  size: number;
  modified_at: number;
  selected_for_deletion: boolean;
  verified: boolean;
  reason: string;
}

export interface DuplicateGroupView {
  id: string;
  algorithm: string;
  hash: string;
  size: number;
  members: DuplicateMemberView[];
  keep_node_id: number | null;
  reclaimable_bytes: number;
  stale: boolean;
  deletable: boolean;
}

export interface DuplicateGroupPage {
  cursor: string | null;
  next_cursor: string | null;
  groups: DuplicateGroupView[];
  total_estimate: number;
}

export interface CleanupCandidateView {
  node_id: number;
  path: string;
  name: string;
  category: string;
  reason_key: string;
  reclaimable_bytes: number;
  risk: string;
  recommended_action: string;
}

export interface SnapshotSummary {
  snapshot_id: string;
  path: string;
  volume_id: string;
  mount_point: string;
  created_at: string;
  node_count: number;
  file_size: number;
}

export interface SnapshotDiffRow {
  path: string;
  change: string;
  is_directory: boolean;
  size_delta: number;
  mtime_delta: number;
  base_size: number;
  target_size: number;
}

export interface SnapshotGrowth {
  path: string;
  delta: number;
}

export type SnapshotDiffSortKey = 'path' | 'change' | 'size_delta';

export interface SnapshotDiffSort {
  key: SnapshotDiffSortKey;
  descending: boolean;
}

export interface SnapshotDiffPage {
  task_id: string;
  cursor: string | null;
  next_cursor: string | null;
  rows: SnapshotDiffRow[];
  total_estimate: number;
  added: number;
  removed: number;
  changed: number;
  total_growth_bytes: number;
  growth_by_directory: SnapshotGrowth[];
}

export interface SnapshotCreateRequest {
  path: string | null;
  out: string | null;
}

export interface SnapshotCreated {
  task_id: string;
  snapshot_id: string;
  path: string;
  node_count: number;
}

export type OperationKind =
  | 'delete'
  | 'move'
  | 'rename'
  | 'permissions'
  | 'shred'
  | 'restore';

export type DeleteMode = 'recycle_bin' | 'quarantine' | 'permanent';

export interface OperationTarget {
  path: string;
  kind: NodeKind;
  size: number;
  accessible: boolean;
  is_system: boolean;
}

export interface OperationPreview {
  preview_id: string;
  kind: OperationKind;
  targets: OperationTarget[];
  total_size: number;
  delete_mode: DeleteMode | null;
  reversible: boolean;
  requires_elevation: boolean;
  warnings: string[];
  expires_at: number;
  confirmation_token: string;
}

export interface OperationPreviewRequest {
  kind: OperationKind;
  targets: string[];
  delete_mode?: DeleteMode | null;
  reason?: string | null;
  passes?: number | null;
  task_id?: string | null;
}

export type OperationState =
  | 'preparing'
  | 'running'
  | 'cancelling'
  | 'completed'
  | 'partial'
  | 'failed'
  | 'cancelled';

export interface OperationProgress {
  operation_id: string;
  state: OperationState;
  processed: number;
  total: number;
  current_target: string | null;
  bytes_per_second: number | null;
  errors: UiError[];
  undo_available: boolean;
  journal_entry_id: string | null;
}

export interface OperationAccepted {
  operation_id: string;
}

export interface OperationOutcome {
  operation_id: string;
  state: OperationState;
  journal_entry_id: string | null;
  undo_available: boolean;
  processed: number;
  total: number;
  errors: UiError[];
  quarantine_ids: string[];
}

export interface TaskStarted {
  task_id: string;
}

export interface DuplicateStartRequest {
  path: string;
  min_size: number;
  algorithm: string;
  keep_rule: string;
  exclusions: string[];
}

export type OperationProgressEvent = OperationProgress;

export interface OperationCompletedEvent {
  operation_id: string;
  state: OperationState;
  journal_entry_id: string | null;
  undo_available: boolean;
  error: UiError | null;
}

export interface DuplicateProgressEvent {
  progress: { processed: number; total: number | null };
  phase_key: string;
}

export interface DuplicateCompletedEvent {
  task_id: string;
  state: TaskState;
  groups_count: number;
  reclaimable_bytes: number;
  error: UiError | null;
}

// Delivery batches 4–6 own their payloads in separate modules so the three
// workstreams never edit the same file.
export * from './dto-filesystem';
export * from './dto-inspection';
export * from './dto-settings';
