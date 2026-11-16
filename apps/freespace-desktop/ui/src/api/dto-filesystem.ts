/**
 * Filesystem command payloads (US4): size details, long paths, rename,
 * permissions and attributes.
 *
 * These mirror `crates/freespace-app/src/dto/filesystem.rs`. Every number here
 * is produced by Rust; the interface formats those numbers but never recomputes
 * logical, on-disk, physical or allocated sizes.
 */

import type { AttributeFlags, UiError } from './dto';

/** Exact size breakdown of one object (Rust `SizeDetails`). */
export interface SizeDetails {
  path: string;
  name: string;
  logical_size: number;
  on_disk_size: number;
  physical_size: number;
  allocated_size: number;
  hardlink_contribution: number;
  ads_size: number;
  compression_savings: number;
  sparse_savings: number;
  link_target: string | null;
  /** Message key naming the metric that best describes the object. */
  metric_label: string;
  attributes: AttributeFlags;
  /** True when the numbers come from a scan and could not be re-measured. */
  estimated: boolean;
}

/** Local alias used by the size-details screen. */
export type SizeDetailsView = SizeDetails;

/** Result of `long_paths_validate`. */
export interface LongPathValidation {
  valid: boolean;
  /** Message key describing the rejection; `null` when the path is valid. */
  reason_key: string | null;
  /** Canonical verbatim form (`\\?\C:\...`) accepted by the operations layer. */
  normalized: string | null;
  /** Same path without the verbatim prefix, for display. */
  display: string | null;
}

/** Case transformation applied after the rename replacement. */
export type RenameCase = 'preserve' | 'lower' | 'upper';

export interface RenameRequest {
  targets: string[];
  regex: string;
  replacement: string;
  /** Optional directory that bounds every target. */
  scope: string | null;
  case: RenameCase;
}

/** Rust name of the rename preview request. */
export type RenamePreviewRequest = RenameRequest;

export interface RenamePreviewRow {
  old_path: string;
  new_path: string;
  old_name: string;
  new_name: string;
  size: number;
  conflict: boolean;
}

export interface RenameConflictView {
  path: string;
  target: string | null;
  reason: string;
  reason_key: string;
}

export interface RenamePreview {
  preview_id: string;
  confirmation_token: string;
  expires_at: number;
  rows: RenamePreviewRow[];
  conflicts: RenameConflictView[];
  /** Number of objects that would be renamed. */
  change_count: number;
  total_size: number;
  requires_elevation: boolean;
  /** Message keys, for example `confirm.warning.elevation`. */
  warnings: string[];
}

/** Result of `rename_apply`. */
export interface RenameApplied {
  operation_id: string;
  renamed: number;
  journal_entry_id: number | null;
  undo_available: boolean;
}

/** Windows file attribute names, in the `kebab-case` wire spelling. */
export type FileAttributeName =
  | 'read-only'
  | 'hidden'
  | 'system'
  | 'archive'
  | 'temporary'
  | 'sparse'
  | 'compressed'
  | 'not-content-indexed';

export interface AttributeChangeSet {
  set: FileAttributeName[];
  clear: FileAttributeName[];
}

/** One attribute before and after the planned change. */
export interface AttributeChangeRow {
  attribute: FileAttributeName;
  before: boolean;
  after: boolean;
}

/** Requested permission changes for one object. */
export interface PermissionsChanges {
  owner?: string;
  reset?: boolean;
  copy_from?: string;
  attributes?: AttributeChangeSet;
}

export type PermissionsAclAction = 'owner' | 'reset' | 'copy';

/** Current owner and attributes of one object. */
export interface PermissionsView {
  path: string;
  owner: string | null;
  owner_sid: string | null;
  attribute_mask: number;
  attributes: AttributeFlags;
  is_directory: boolean;
  is_reparse_point: boolean;
  is_protected: boolean;
  requires_elevation: boolean;
}

/** Preview of a permission or attribute change. */
export interface PermissionsPreview {
  preview_id: string;
  confirmation_token: string;
  expires_at: number;
  path: string;
  current: PermissionsView;
  owner: string | null;
  acl_action: PermissionsAclAction | null;
  copy_from: string | null;
  attribute_changes: AttributeChangeRow[];
  requires_elevation: boolean;
  /** Message keys, for example `confirm.warning.elevation`. */
  warnings: string[];
}

/** Outcome of a permission or attribute change already applied. */
export interface FileOperationApplied {
  operation_id: string;
  journal_entry_ids: number[];
  changed: number;
  failed: number;
  errors: UiError[];
}
