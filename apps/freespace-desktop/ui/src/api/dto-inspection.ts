/**
 * Inspection and report payloads (US5): metadata, owners, archives, export and
 * unlock.
 *
 * These types mirror `crates/freespace-app/src/dto/inspection.rs` and section 8
 * of `specs/002-native-ui-redesign/contracts/ipc.md`. Every size, total and
 * share is produced by Rust; the interface only formats the values.
 */

import type { FilterSet, Metric, SortSpec, TaskState, TreePage, UiError } from './dto';

/** One metadata tag with the reader that produced it. */
export interface MetadataTag {
  key: string;
  value: string;
  source: string;
}

/** Extracted metadata of one file (`metadata_get`). */
export interface MetadataView {
  node_id: number;
  path: string;
  /** Metadata family: `exif`, `audio`, `video` or `pe`. */
  kind: string;
  /** Localisable source key of the reader, for example `metadata.source.exif`. */
  source_key: string;
  /** Extraction status; `ok` for a successful read. */
  status: string;
  /** True when the record can be displayed. */
  available: boolean;
  tags: MetadataTag[];
  /** True when tags or values were truncated to the documented limits. */
  truncated: boolean;
  size_bytes: number;
  modified_at: number | null;
}

/** UI projection of the extraction outcome. */
export type MetadataStatus = 'supported' | 'unsupported' | 'corrupt';

/** Metadata family used by the kind filter. */
export type MetadataKind = 'exif' | 'audio' | 'video' | 'pe';

/** One owner row of `owners_page`. */
export interface OwnerSummary {
  /** Dense owner index; `null` for the documented unknown category. */
  owner_id: number | null;
  sid: string;
  display_name: string;
  total_bytes: number;
  file_count: number;
  share: number;
  accessible: boolean;
  label_key: string;
}

/** One page of owner summaries. */
export interface OwnerSummaryPage {
  generation: number;
  cursor: string | null;
  next_cursor: string | null;
  rows: OwnerSummary[];
  total_estimate: number;
  total_bytes: number;
  unknown_bytes: number;
  error_count: number;
  warnings: string[];
  stale_cursor: boolean;
}

/** One entry of an archive listing. */
export interface ArchiveEntry {
  /** Path inside the archive, using `/` as the separator. */
  internal_path: string;
  /** Uncompressed size; zero for directories. */
  size: number;
  /** Compressed size when the container reports it. */
  compressed_size: number | null;
  is_directory: boolean;
  encrypted: boolean;
  /** Share of the archive's uncompressed content in `0.0..=1.0`. */
  share: number;
  /** Nesting depth derived from the internal path. */
  depth: number;
  /** Lower-cased extension without the dot, when the entry has one. */
  extension: string | null;
}

/** One page of archive entries (`archives_entries_page`). */
export interface ArchiveEntryPage {
  path: string;
  format: string;
  cursor: string | null;
  next_cursor: string | null;
  entries: ArchiveEntry[];
  total_entries: number;
  total_files: number;
  total_size: number;
  encrypted_count: number;
  warnings: string[];
  stale_cursor: boolean;
}

/** Output format of an exported report or archive list. */
export type ExportFormat = 'csv' | 'xml' | 'html' | 'xlsx' | 'json';

/** Formats accepted by the report writer. */
export type ReportFormat = 'csv' | 'xml' | 'html' | 'xlsx';

/** Formats accepted by the archive contents list export. */
export type ArchiveExportFormat = 'csv' | 'xml' | 'html' | 'json';

/** One section of an exported report. */
export type ExportSection = 'tree' | 'top' | 'types' | 'duplicates' | 'owners';

/** Scope of an export preview. */
export interface ExportScope {
  generation: number;
  out: string;
}

/** Which sections an export contains and how it treats an existing file. */
export interface ExportComposition {
  sections: ExportSection[];
  max_tree_rows?: number | null;
  overwrite?: boolean;
}

/** Input of `export_preview`. */
export interface ExportPreviewRequest {
  scope: ExportScope;
  format: ReportFormat;
  composition?: ExportComposition;
}

/** One-time preview of an export. */
export interface ExportPreview {
  preview_id: string;
  confirmation_token: string;
  format: ExportFormat;
  out: string;
  sections: ExportSection[];
  /** Rough row count; `null` when a section cannot be estimated cheaply. */
  row_estimate: number | null;
  total_bytes: number;
  on_disk_bytes: number;
  physical_bytes: number;
  overwrite: boolean;
  warnings: string[];
  expires_at: number;
}

/** Live state of one export task (`export_status`). */
export interface ExportStatus {
  task_id: string;
  state: TaskState;
  /** Completed stages. */
  processed: number;
  /** Total stages. */
  total: number;
  phase_key: string;
  out: string | null;
  /** Bytes written so far; set on completion. */
  bytes_written: number | null;
  rows_written: number;
  cancelable: boolean;
  started_at: number;
  finished_at: number | null;
  error: UiError | null;
}

/** Identifier of a started export. */
export interface ExportStarted {
  task_id: string;
}

/** Result of a synchronous export such as `archives_export_list`. */
export interface ExportResult {
  path: string;
  format: ExportFormat;
  bytes_written: number;
  rows_written: number;
  sections: ExportSection[];
  warnings: string[];
}

/** One process that holds a file (`unlock_locks`). */
export interface FileLock {
  process_id: number;
  process_name: string;
  service_name: string;
  application_type: string;
  access_type: string;
  access_type_key: string;
  can_soft_close: boolean;
  requires_reboot: boolean;
  critical: boolean;
  handle_count: number | null;
}

/** How a lock is released. */
export type UnlockMode = 'soft' | 'force';

/** Result of `unlock_release` and `unlock_schedule_reboot`. */
export interface UnlockAccepted {
  operation_id: string;
}

/** Arguments accepted by `owners_page`. */
export interface OwnersPageQuery {
  filter?: FilterSet | null;
  cursor?: string | null;
  page_size?: number;
}

/** Arguments accepted by `owners_files_page`. */
export interface OwnerFilesQuery {
  filter?: FilterSet | null;
  metric?: Metric;
  sort?: SortSpec;
  cursor?: string | null;
  page_size?: number;
}

/** Inputs accepted by `archives_entries_page`. */
export interface ArchiveEntriesQuery {
  task_id?: string | null | undefined;
  path: string;
  filter?: FilterSet | null | undefined;
  cursor?: string | null | undefined;
  page_size?: number | undefined;
}

/** Inputs accepted by `archives_export_list`. */
export interface ArchiveExportRequest {
  path: string;
  format: ArchiveExportFormat;
  out: string;
}

/** Payload of `owners_files_page`. */
export type OwnerFilesPage = TreePage;

/** Payload of `export_cancel`; the command returns an empty object. */
export type ExportCancelled = Record<string, never>;
