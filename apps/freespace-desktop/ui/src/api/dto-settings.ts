/**
 * Operation and settings payloads (US6): shredder, write monitor, elevation,
 * settings and About.
 *
 * The shapes mirror `crates/freespace-app/src/dto/settings_ops.rs` and
 * `specs/002-native-ui-redesign/contracts/ipc.md` sections 9 and 10. They live
 * here instead of `dto.ts` so the parallel UI batches never edit the shared
 * wire-type file.
 */

import type {
  AboutInfo,
  ElevationStatus,
  Language,
  LicenseInfo,
  OperationPreview,
  OperationProgress,
  TaskState,
  ThemePreference,
  UiError,
  UiSettings,
  UiSettingsPatch,
  UnitSystem,
} from './dto';

/** Overwrite pattern of the shredder. */
export type ShredPattern = 'zeros' | 'random';

/** `shred_preview` input. `passes` is validated as 1..=35 by the native layer. */
export interface ShredPreviewRequest {
  targets: string[];
  passes: number;
  pattern: ShredPattern;
}

/** `shred_start` mirrors `operation_confirm`: a preview token is mandatory. */
export interface ShredStarted {
  operation_id: string;
}

/** Active source reported by the monitor worker. */
export type MonitorSource = 'etw' | 'counters';

/** `monitor_start` input; the native layer fills omitted values from settings. */
export interface MonitorRequest {
  threshold: number;
  interval: number;
  exclusions: string[];
}

/** `monitor_start` returns the task identifier followed by `monitor-tick`. */
export interface MonitorStarted {
  task_id: string;
}

/** `monitor_status` payload. */
export interface MonitorStatus {
  task_id: string;
  running: boolean;
  state: TaskState;
  source: string;
  source_key: string;
  threshold_bytes: number;
  interval_secs: number;
  exclusions: string[];
  events_count: number;
  bytes_total: number;
  started_at: number;
  finished_at: number | null;
  last_event_at: number | null;
  warnings: string[];
  error: UiError | null;
}

/** One write burst attributed to a process. */
export interface MonitorEvent {
  sequence: number;
  timestamp: number;
  process_id: number;
  process: string;
  bytes: number;
  paths: string[];
  threshold_exceeded: boolean;
}

/** Filter accepted by `monitor_events_page`. */
export interface MonitorEventFilter {
  process: string | null;
  path: string | null;
  min_bytes?: number | null;
  threshold_only?: boolean;
}

/** Cursor page of monitor events. */
export interface MonitorEventPage {
  task_id: string;
  cursor: string | null;
  next_cursor: string | null;
  events: MonitorEvent[];
  total_estimate: number;
}

/** `monitor-tick` payload; the envelope also carries the sequence. */
export interface MonitorTickEvent {
  task_id: string;
  sequence: number;
  events: MonitorEvent[];
}

/** Rule used when the duplicate finder keeps one copy, as a wire string. */
export type DuplicateKeepRule = 'oldest' | 'newest' | 'shortest-path' | 'original-dir';

/**
 * `settings_get` / `settings_set` / `settings_reset` payload.
 *
 * The legacy 001 keys keep their historical place; the `[ui]` section is
 * nested. `language`, `theme` and `units` are mirrored from `[ui]` by the
 * native layer so the CLI sees the same values.
 */
export interface SettingsView {
  language: Language;
  theme: ThemePreference;
  units: UnitSystem;
  exclusions: string[];
  duplicate_keep_rule: string;
  outdated_threshold_days: number;
  monitor_threshold_bytes: number;
  monitor_interval_secs: number;
  quarantine_retention_days: number;
  context_menu_enabled: boolean;
  confirm_destructive_actions: boolean;
  ui: UiSettings;
}

/** Patch accepted by `settings_set`; omitted fields stay untouched. */
export interface SettingsPatch {
  ui?: UiSettingsPatch;
  language?: Language;
  theme?: ThemePreference;
  units?: UnitSystem;
  exclusions?: string[];
  duplicate_keep_rule?: string;
  outdated_threshold_days?: number;
  monitor_threshold_bytes?: number;
  monitor_interval_secs?: number;
  quarantine_retention_days?: number;
  context_menu_enabled?: boolean;
  confirm_destructive_actions?: boolean;
}

/** Sections restored by `settings_reset`; all of them belong to `[ui]`. */
export type SettingsSection =
  | 'theme'
  | 'language'
  | 'density'
  | 'units'
  | 'reduced_motion'
  | 'last_screen'
  | 'sidebar'
  | 'column_layouts'
  | 'saved_filters'
  | 'all';

/** Result of checking one user-supplied path. */
export interface PathValidation {
  path: string;
  valid: boolean;
  exists: boolean;
  is_directory: boolean;
  parent_exists: boolean;
  writable: boolean;
  error_key: string | null;
}

/** `app_elevation_request` result. */
export interface ElevationResult {
  elevated: boolean;
}

/** Re-exported payloads so callers can import everything US6 from one module. */
export type {
  AboutInfo,
  ElevationStatus,
  LicenseInfo,
  OperationPreview,
  OperationProgress,
  TaskState,
  UiError,
  UiSettings,
  UiSettingsPatch,
};
