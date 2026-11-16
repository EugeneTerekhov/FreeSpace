/**
 * Typed wrappers for the US6 command family.
 *
 * Every helper goes through `api.transport.invoke`, so the Tauri transport and
 * the in-memory mock share one code path and the union of `CommandName` keeps
 * the command names honest.
 */

import type {
  AboutInfo,
  ElevationResult,
  ElevationStatus,
  LicenseInfo,
  MonitorEventFilter,
  MonitorEventPage,
  MonitorRequest,
  MonitorStarted,
  MonitorStatus,
  OperationPreview,
  OperationProgress,
  PathValidation,
  SettingsPatch,
  SettingsSection,
  SettingsView,
  ShredPreviewRequest,
  ShredStarted,
} from './dto';
import { api } from './runtime';

/** Builds the destructive shredder preview; never starts the operation. */
export function shredPreview(request: ShredPreviewRequest): Promise<OperationPreview> {
  return api.transport.invoke<OperationPreview>('shred_preview', {
    targets: request.targets,
    passes: request.passes,
    pattern: request.pattern,
  });
}

/**
 * Starts a previewed shredding operation.
 *
 * `extra_confirmation` is always `true`: the screen only enables the button
 * after the irreversible-action checkbox is checked.
 */
export function shredStart(
  previewId: string,
  confirmationToken: string,
  extraConfirmation = true,
): Promise<ShredStarted> {
  return api.transport.invoke<ShredStarted>('shred_start', {
    preview_id: previewId,
    confirmation_token: confirmationToken,
    extra_confirmation: extraConfirmation,
  });
}

/** Current progress of a shredding operation. */
export function shredStatus(operationId: string): Promise<OperationProgress> {
  return api.transport.invoke<OperationProgress>('shred_status', {
    operation_id: operationId,
  });
}

/** Requests cancellation; the native layer decides whether it is still safe. */
export function shredCancel(operationId: string): Promise<void> {
  return api.transport.invoke<void>('shred_cancel', {
    operation_id: operationId,
  });
}

/** Starts the write monitor. */
export function monitorStart(request: MonitorRequest): Promise<MonitorStarted> {
  return api.transport.invoke<MonitorStarted>('monitor_start', {
    threshold: request.threshold,
    interval: request.interval,
    exclusions: request.exclusions,
  });
}

/** Stops the write monitor. */
export function monitorStop(taskId: string): Promise<void> {
  return api.transport.invoke<void>('monitor_stop', { task_id: taskId });
}

/** Current monitor task state, source and counters. */
export function monitorStatus(taskId: string): Promise<MonitorStatus> {
  return api.transport.invoke<MonitorStatus>('monitor_status', { task_id: taskId });
}

/** Cursor page of monitor events; filters are applied by the native layer. */
export function monitorEventsPage(
  taskId: string,
  filter: MonitorEventFilter,
  cursor: string | null = null,
  pageSize = 200,
): Promise<MonitorEventPage> {
  return api.transport.invoke<MonitorEventPage>('monitor_events_page', {
    task_id: taskId,
    filter,
    cursor,
    page_size: pageSize,
  });
}

/** Updates the exclusion masks of a running monitor. */
export function monitorSetExclusions(exclusions: string[]): Promise<void> {
  return api.transport.invoke<void>('monitor_set_exclusions', { exclusions });
}

/** Reads the legacy settings and the `[ui]` section. */
export function settingsGet(): Promise<SettingsView> {
  return api.transport.invoke<SettingsView>('settings_get');
}

/** Applies a settings patch and returns the persisted view. */
export function settingsSet(patch: SettingsPatch): Promise<SettingsView> {
  return api.transport.invoke<SettingsView>('settings_set', { patch });
}

/** Validates user-supplied paths without modifying anything. */
export function settingsValidatePaths(paths: string[]): Promise<PathValidation[]> {
  return api.transport.invoke<PathValidation[]>('settings_validate_paths', { paths });
}

/** Restores one `[ui]` section (or all of them) to the documented defaults. */
export function settingsReset(section: SettingsSection): Promise<SettingsView> {
  return api.transport.invoke<SettingsView>('settings_reset', { section });
}

/** About payload: version, build, WebView2, data directory and licences. */
export function aboutInfo(): Promise<AboutInfo> {
  return api.transport.invoke<AboutInfo>('about_info');
}

/** Licence list, shown as text and never opened automatically. */
export function aboutLicenses(): Promise<LicenseInfo[]> {
  return api.transport.invoke<LicenseInfo[]>('about_licenses');
}

/** Current process elevation and whether it can be requested. */
export function elevationStatus(): Promise<ElevationStatus> {
  return api.transport.invoke<ElevationStatus>('app_elevation_status');
}

/** Requests elevation for an explained reason; refusal changes no data. */
export function elevationRequest(reasonKey: string): Promise<ElevationResult> {
  return api.transport.invoke<ElevationResult>('app_elevation_request', {
    reason_key: reasonKey,
  });
}
