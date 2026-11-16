import { api } from '../api/runtime';
import type { ElevationStatus, OperationPreview, UiError } from '../api/dto';

/** Returns the process elevation status. */
export function elevationStatus(): Promise<ElevationStatus> {
  return api.appElevationStatus();
}

/** True when the preview can only run elevated. */
export function needsElevation(preview: OperationPreview): boolean {
  return preview.requires_elevation;
}

/** Explains why an operation was refused for missing rights. */
export function elevationError(preview: OperationPreview): UiError | null {
  if (!preview.requires_elevation) return null;

  return {
    code: 'E_ELEVATION_REQUIRED',
    category: 'access',
    message_key: 'error.elevation_required',
    details: null,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };
}
