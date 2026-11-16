/**
 * Typed helpers for the inspection commands of section 8 of the IPC contract.
 *
 * The shared `ApiClient` is owned by the core batch and does not know US5
 * payloads, so the screens go through this small typed facade instead of
 * calling the transport with untyped argument bags.
 */

import type { FilterSet, Metric, SortSpec } from './dto';
import type {
  ArchiveEntriesQuery,
  ArchiveEntryPage,
  ArchiveExportFormat,
  ArchiveExportRequest,
  ExportCancelled,
  ExportPreview,
  ExportPreviewRequest,
  ExportResult,
  ExportStarted,
  ExportStatus,
  FileLock,
  MetadataView,
  OwnerFilesPage,
  OwnerSummaryPage,
  OwnersPageQuery,
  ReportFormat,
  UnlockAccepted,
  UnlockMode,
} from './dto-inspection';
import { api } from './runtime';

/** Reads extracted metadata for one node of the current generation. */
export function fetchMetadata(
  generation: number,
  nodeId: number,
): Promise<MetadataView> {
  return api.transport.invoke<MetadataView>('metadata_get', {
    generation,
    node_id: nodeId,
  });
}

/** Reads one cursor page of owner summaries. */
export function fetchOwnersPage(
  generation: number,
  query: OwnersPageQuery = {},
): Promise<OwnerSummaryPage> {
  return api.transport.invoke<OwnerSummaryPage>('owners_page', {
    generation,
    filter: query.filter ?? null,
    cursor: query.cursor ?? null,
    page_size: query.page_size,
  });
}

/** Reads one cursor page of files owned by the selected account. */
export function fetchOwnerFiles(
  generation: number,
  owner: string,
  options: {
    filter?: FilterSet | null;
    metric?: Metric;
    sort?: SortSpec;
    cursor?: string | null;
    pageSize?: number;
  } = {},
): Promise<OwnerFilesPage> {
  return api.transport.invoke<OwnerFilesPage>('owners_files_page', {
    generation,
    owner,
    filter: options.filter ?? null,
    metric: options.metric,
    sort: options.sort,
    cursor: options.cursor ?? null,
    page_size: options.pageSize,
  });
}

/** Reads one cursor page of archive contents without unpacking them. */
export function fetchArchiveEntries(
  request: {
    taskId?: string | null;
    path: string;
    filter?: FilterSet | null;
    cursor?: string | null;
    pageSize?: number;
  },
): Promise<ArchiveEntryPage> {
  const args: ArchiveEntriesQuery = {
    task_id: request.taskId ?? null,
    path: request.path,
    filter: request.filter ?? null,
    cursor: request.cursor ?? null,
    page_size: request.pageSize,
  };

  return api.transport.invoke<ArchiveEntryPage>('archives_entries_page', {
    ...args,
  });
}

/** Exports the inspected contents list; no archive is unpacked. */
export function exportArchiveList(
  request: ArchiveExportRequest,
): Promise<ExportResult> {
  return api.transport.invoke<ExportResult>('archives_export_list', {
    path: request.path,
    format: request.format,
    out: request.out,
  });
}

/** Validates an export composition and returns its preview summary. */
export function previewExport(
  request: ExportPreviewRequest,
): Promise<ExportPreview> {
  return api.transport.invoke<ExportPreview>('export_preview', {
    scope: request.scope,
    format: request.format,
    composition: request.composition,
  });
}

/** Starts a validated export; progress is read through `fetchExportStatus`. */
export function startExport(
  previewId: string,
  confirmationToken?: string,
): Promise<ExportStarted> {
  return api.transport.invoke<ExportStarted>('export_start', {
    preview_id: previewId,
    confirmation_token: confirmationToken,
  });
}

/** Reads the live state of an export task. */
export function fetchExportStatus(taskId: string): Promise<ExportStatus> {
  return api.transport.invoke<ExportStatus>('export_status', {
    task_id: taskId,
  });
}

/** Requests cancellation of a running export task. */
export function cancelExport(taskId: string): Promise<ExportCancelled> {
  return api.transport.invoke<ExportCancelled>('export_cancel', {
    task_id: taskId,
  });
}

/** Lists processes holding the file; this is read-only. */
export function fetchLocks(path: string): Promise<FileLock[]> {
  return api.transport.invoke<FileLock[]>('unlock_locks', { path });
}

/** Asks applications to close the file or terminates the selected processes. */
export function releaseLocks(
  path: string,
  mode: UnlockMode,
  processIds: number[],
  confirmed = true,
): Promise<UnlockAccepted> {
  return api.transport.invoke<UnlockAccepted>('unlock_release', {
    path,
    mode,
    process_ids: processIds,
    confirmed,
  });
}

/** Registers deletion before the next sign-in; journaled by Rust. */
export function scheduleUnlockReboot(
  path: string,
  confirmed = true,
): Promise<UnlockAccepted> {
  return api.transport.invoke<UnlockAccepted>('unlock_schedule_reboot', {
    path,
    confirmed,
  });
}

/** Formats accepted by the archive list export, in display order. */
export const ARCHIVE_EXPORT_FORMATS: readonly ArchiveExportFormat[] = [
  'csv',
  'json',
  'xml',
  'html',
];

/** Report formats accepted by the export command, in display order. */
export const EXPORT_FORMATS: readonly ReportFormat[] = [
  'csv',
  'xml',
  'html',
  'xlsx',
];
