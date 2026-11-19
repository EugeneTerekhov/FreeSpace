/**
 * Typed helpers for the filesystem commands of User Story 4.
 *
 * All sizes and previews stay on the Rust side; these wrappers only translate
 * camelCase call sites into the snake_case IPC argument names.
 */

import type {
  FilterSet,
  NodeRow,
  OperationPreview,
  TreePage,
} from './dto';
import type {
  AttributeChangeSet,
  FileOperationApplied,
  LongPathValidation,
  PermissionsChanges,
  PermissionsPreview,
  PermissionsView,
  RenameApplied,
  RenamePreview,
  RenameRequest,
  SizeDetailsView,
} from './dto';
import { api } from './runtime';

export interface LongPathsPageOptions {
  filter: FilterSet;
  cursor?: string | null;
  pageSize?: number;
}

/** Exact size metrics of one node. */
export function fetchSizeDetails(
  generation: number,
  nodeId: number,
): Promise<SizeDetailsView> {
  return api.transport.invoke<SizeDetailsView>('size_details', {
    generation,
    node_id: nodeId,
  });
}

/** One page of objects whose full path exceeds the legacy 260-character limit. */
export function fetchLongPathsPage(
  generation: number,
  options: LongPathsPageOptions,
): Promise<TreePage> {
  return api.transport.invoke<TreePage>('long_paths_page', {
    generation,
    filter: options.filter,
    cursor: options.cursor ?? null,
    page_size: options.pageSize,
  });
}

/** Validates a path without touching the filesystem. */
export function validateLongPath(path: string): Promise<LongPathValidation> {
  return api.transport.invoke<LongPathValidation>('long_paths_validate', { path });
}

/** Builds the rename plan shown to the user before anything is written. */
export function requestRenamePreview(request: RenameRequest): Promise<RenamePreview> {
  return api.transport.invoke<RenamePreview>('rename_preview', {
    targets: request.targets,
    regex: request.regex,
    replacement: request.replacement,
    scope: request.scope,
    case: request.case,
  });
}

/** Applies a rename preview exactly once. */
export function applyRename(
  previewId: string,
  confirmationToken: string,
): Promise<RenameApplied> {
  return api.transport.invoke<RenameApplied>('rename_apply', {
    preview_id: previewId,
    confirmation_token: confirmationToken,
  });
}

/** Reads owner, ACL and attributes of one path. */
export function fetchPermissions(path: string): Promise<PermissionsView> {
  return api.transport.invoke<PermissionsView>('permissions_get', { path });
}

/** Preview of ACL, owner and attribute changes. */
export function requestPermissionsPreview(
  path: string,
  changes: PermissionsChanges,
): Promise<PermissionsPreview> {
  return api.transport.invoke<PermissionsPreview>('permissions_preview', {
    path,
    changes,
  });
}

/** Applies a permissions preview exactly once. */
export function applyPermissions(
  previewId: string,
  confirmationToken: string,
): Promise<FileOperationApplied> {
  return api.transport.invoke<FileOperationApplied>('permissions_apply', {
    preview_id: previewId,
    confirmation_token: confirmationToken,
  });
}

/** Preview of attribute-only changes using the generic operation envelope. */
export function requestAttributesPreview(
  targets: string[],
  changes: AttributeChangeSet,
): Promise<OperationPreview> {
  return api.transport.invoke<OperationPreview>('attributes_preview', {
    targets,
    changes,
  });
}

/** Applies an attribute preview exactly once. */
export function applyAttributes(
  previewId: string,
  confirmationToken: string,
): Promise<FileOperationApplied> {
  return api.transport.invoke<FileOperationApplied>('attributes_apply', {
    preview_id: previewId,
    confirmation_token: confirmationToken,
  });
}

/** Resolves a user-entered path to a node of the current generation. */
export async function findNodeByPath(
  generation: number,
  path: string,
): Promise<NodeRow | null> {
  const normalized = normalizePath(path);
  if (normalized === '') return null;

  if (isVolumeRoot(normalized)) {
    const page = await api.treeRoot(generation, { pageSize: 20 });
    const root = page.rows.find((row) =>
      row.path.toLowerCase().startsWith(`${normalized.toLowerCase()}\\`),
    );

    return root ?? page.rows[0] ?? null;
  }

  const page = await api.treeSearch(generation, lastSegment(normalized), { pageSize: 50 });
  const needle = normalized.toLowerCase();

  return page.rows.find((row) => row.path.toLowerCase() === needle) ?? null;
}

function normalizePath(path: string): string {
  return path.trim().replace(/\//g, '\\').replace(/\\+$/, '');
}

function isVolumeRoot(path: string): boolean {
  return /^[a-z]:$/i.test(path);
}

function lastSegment(path: string): string {
  const parts = path.split('\\');
  return parts.at(-1) ?? path;
}
