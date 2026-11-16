import type {
  AttributeChangeRow,
  AttributeChangeSet,
  AttributeFlags,
  FileAttributeName,
  FileOperationApplied,
  FilterSet,
  NodeRow,
  OperationPreview,
  PermissionsAclAction,
  PermissionsChanges,
  PermissionsPreview,
  PermissionsView,
  RenameApplied,
  RenameCase,
  RenameConflictView,
  RenamePreview,
  RenamePreviewRow,
  SizeDetailsView,
  TreePage,
  UiError,
  UiErrorCode,
} from '../dto';
import type { MockRegister } from './register';

/**
 * US4 in-memory handlers: size details, long paths, rename, permissions and
 * attributes.
 *
 * The journal commands are intentionally left to `MockTransport`: its journal
 * also records operations confirmed from other screens, and overriding it here
 * would hide those entries from the shared journal flow.
 */

const KB = 1024;
const MB = 1024 * KB;
const GB = 1024 * MB;
const PREVIEW_TTL_MS = 10 * 60 * 1000;

const ATTRIBUTE_NAMES: readonly FileAttributeName[] = [
  'read-only',
  'hidden',
  'system',
  'archive',
  'temporary',
  'sparse',
  'compressed',
  'not-content-indexed',
];

const ATTRIBUTE_BITS: Record<FileAttributeName, number> = {
  'read-only': 0x1,
  hidden: 0x2,
  system: 0x4,
  archive: 0x20,
  temporary: 0x100,
  sparse: 0x200,
  compressed: 0x800,
  'not-content-indexed': 0x2000,
};

const LONG_BASE =
  'C:\\Users\\Public\\Documents\\FreeSpace\\Archive\\2026\\Projects\\Quarterly\\';

interface NodeInfo {
  path: string;
  name: string;
  kind: NodeRow['kind'];
  size: number;
}

interface StoredPreview<T> {
  preview: T;
  used: boolean;
}

interface StoredPermissionsPreview extends StoredPreview<PermissionsPreview> {
  path: string;
}

const KNOWN_NODES = new Map<number, NodeInfo>([
  [0, { path: 'C:\\', name: 'C:\\', kind: 'directory', size: 760 * GB }],
  [1, { path: 'C:\\Windows', name: 'Windows', kind: 'directory', size: 180 * GB }],
  [4, { path: 'C:\\pagefile.sys', name: 'pagefile.sys', kind: 'file', size: 16 * GB }],
  [12, { path: 'C:\\Windows\\explorer.exe', name: 'explorer.exe', kind: 'file', size: 4 * MB }],
  [31, { path: 'C:\\Temp\\crash.dmp', name: 'crash.dmp', kind: 'file', size: 2 * GB }],
  [35, {
    path: 'C:\\Users\\Downloads\\archive.zip',
    name: 'archive.zip',
    kind: 'file',
    size: 1 * GB,
  }],
  [37, {
    path: 'C:\\Users\\Downloads\\backup-1.bin',
    name: 'backup-1.bin',
    kind: 'file',
    size: 512 * MB,
  }],
]);

/** Registers every US4 command against the in-memory transport. */
export function registerFilesystemMocks(register: MockRegister): void {
  const longRows = createLongPathRows();
  const renamePreviews = new Map<string, StoredPreview<RenamePreview>>();
  const permissionPreviews = new Map<string, StoredPermissionsPreview>();
  const attributePreviews = new Map<string, StoredPreview<OperationPreview>>();
  let sequence = 0;

  const nextId = (prefix: string): string => {
    sequence += 1;
    return `${prefix}-${sequence}`;
  };

  register('size_details', (args) =>
    sizeDetails(longRows, args),
  );

  register('long_paths_page', (args) =>
    longPathsPage(longRows, args),
  );

  register('long_paths_validate', (args) => validateLongPath(asString(args.path)));

  register('rename_preview', (args) => {
    const preview = buildRenamePreview(nextId, longRows, args);
    renamePreviews.set(
      preview.preview_id,
      { preview, used: false },
    );
    return preview;
  });

  register('rename_apply', (args) => {
    const previewId = asString(args.preview_id);
    const stored = renamePreviews.get(previewId);
    assertUsablePreview(
      stored,
      asString(args.confirmation_token),
      stored?.preview.expires_at ?? 0,
    );
    if (stored === undefined) {
      throw failure(createError('E_PREVIEW_EXPIRED', 'unknown rename preview'));
    }
    stored.used = true;
    const applied: RenameApplied = {
      operation_id: nextId('rename-operation'),
      renamed: stored.preview.change_count,
      journal_entry_id: null,
      undo_available: stored.preview.change_count > 0,
    };

    return applied;
  });

  register('permissions_get', (args) => permissionsView(asString(args.path)));

  register('permissions_preview', (args) => {
    const path = asString(args.path);
    const changes = readChanges(args.changes);
    const preview = buildPermissionsPreview(nextId, path, changes);
    permissionPreviews.set(preview.preview_id, {
      preview,
      used: false,
      path,
    });

    return preview;
  });

  register('permissions_apply', (args) => {
    const previewId = asString(args.preview_id);
    const stored = permissionPreviews.get(previewId);
    assertUsablePreview(
      stored,
      asString(args.confirmation_token),
      stored?.preview.expires_at ?? 0,
    );
    if (stored === undefined) {
      throw failure(createError('E_PREVIEW_EXPIRED', 'unknown permissions preview'));
    }
    if (/\bsystem32\b/i.test(stored.path)) {
      throw failure(createError('E_ELEVATION_REQUIRED', stored.path));
    }
    stored.used = true;
    const applied: FileOperationApplied = {
      operation_id: nextId('permissions-operation'),
      journal_entry_ids: [],
      changed: 1,
      failed: 0,
      errors: [],
    };

    return applied;
  });

  register('attributes_preview', (args) => {
    const targets = asStringArray(args.targets);
    if (targets.length === 0) {
      throw failure(createError('E_VALIDATION', 'attributes_preview needs targets'));
    }
    const preview: OperationPreview = {
      preview_id: nextId('attributes-preview'),
      kind: 'permissions',
      targets: targets.map((path) => ({
        path,
        kind: 'file',
        size: 0,
        accessible: true,
        is_system: false,
      })),
      total_size: 0,
      delete_mode: null,
      reversible: true,
      requires_elevation: false,
      warnings: [],
      expires_at: Date.now() + PREVIEW_TTL_MS,
      confirmation_token: nextId('attributes-token'),
    };
    attributePreviews.set(preview.preview_id, { preview, used: false });

    return preview;
  });

  register('attributes_apply', (args) => {
    const previewId = asString(args.preview_id);
    const stored = attributePreviews.get(previewId);
    assertUsablePreview(
      stored,
      asString(args.confirmation_token),
      stored?.preview.expires_at ?? 0,
    );
    if (stored !== undefined) stored.used = true;
    const applied: FileOperationApplied = {
      operation_id: nextId('attributes-operation'),
      journal_entry_ids: [],
      changed: stored === undefined ? 0 : stored.preview.targets.length,
      failed: 0,
      errors: [],
    };

    return applied;
  });
}

function createLongPathRows(): NodeRow[] {
  return Array.from({ length: 60 }, (_, index) => {
    const ordinal = String(index + 1).padStart(2, '0');
    const folder = `nested-directory-with-a-very-long-descriptive-name-${ordinal}`;
    const isDirectory = index % 10 === 3;
    const name = isDirectory
      ? `long-path-${ordinal}-folder`
      : `long-path-${ordinal}-${'segment-'.repeat(5)}audit-report.pdf`;
    const path = `${LONG_BASE}${'deeply-nested-'.repeat(4)}${folder}\\${name}`;

    return {
      id: 9001 + index,
      parent_id: null,
      name,
      path,
      kind: isDirectory ? 'directory' : 'file',
      logical_size: (index + 1) * 23 * MB,
      on_disk_size: (index + 1) * 23 * MB + 4096,
      physical_size: (index + 1) * 23 * MB + 4096,
      ads_size: 0,
      child_count: isDirectory ? index % 4 : 0,
      file_count: isDirectory ? index % 4 : 1,
      modified_at: Date.UTC(2026, 0, 1) + index * 86_400_000,
      accessed_at: Date.UTC(2026, 1, 1) + index * 86_400_000,
      attributes: makeAttributes({ hidden: index % 7 === 0 }),
      accessible: index % 13 !== 0,
      link_target: null,
    };
  });
}

function makeAttributes(overrides: Partial<AttributeFlags> = {}): AttributeFlags {
  return {
    read_only: false,
    hidden: false,
    system: false,
    compressed: false,
    sparse: false,
    reparse: false,
    ...overrides,
  };
}

function sizeDetails(rows: NodeRow[], args: Record<string, unknown>): SizeDetailsView {
  const nodeId = asNumber(args.node_id, 0);
  const longRow = rows.find((row) => row.id === nodeId);
  const known = KNOWN_NODES.get(nodeId);
  const info: NodeInfo =
    longRow !== undefined
      ? {
          path: longRow.path,
          name: longRow.name,
          kind: longRow.kind,
          size: longRow.logical_size,
        }
      : known ?? {
          path: `C:\\Users\\Public\\data\\node-${nodeId}.bin`,
          name: `node-${nodeId}.bin`,
          kind: 'file',
          size: 8 * MB + nodeId * KB,
        };

  const hardlink = nodeId === 12 ? 4096 : 0;
  const ads = nodeId % 5 === 0 ? 4096 : 0;
  const onDisk = Math.ceil(info.size / 4096) * 4096 + (nodeId % 3 === 0 ? 4096 : 0);
  const compression = /\\windows/i.test(info.path) ? Math.round(info.size * 0.12) : 0;
  const sparse = nodeId % 4 === 0 ? Math.round(info.size * 0.05) : 0;

  return {
    path: info.path,
    name: info.name,
    logical_size: info.size,
    on_disk_size: onDisk,
    physical_size: Math.max(0, onDisk - hardlink),
    allocated_size: onDisk,
    hardlink_contribution: hardlink,
    ads_size: ads,
    compression_savings: compression,
    sparse_savings: sparse,
    link_target: nodeId === 12 ? 'C:\\Windows\\WinSxS\\explorer.exe' : null,
    metric_label: 'details.size',
    attributes:
      longRow?.attributes ??
      makeAttributes({
        system: /\\windows/i.test(info.path),
        compressed: compression > 0,
        sparse: sparse > 0,
      }),
    estimated: false,
  };
}

function longPathsPage(rows: NodeRow[], args: Record<string, unknown>): TreePage {
  const filter = (args.filter as FilterSet | undefined) ?? null;
  const filtered = rows.filter((row) => matchesFilter(row, filter));
  const pageSize = Math.max(1, asNumber(args.page_size, 100));
  const cursor = args.cursor === null || args.cursor === undefined
    ? null
    : asString(args.cursor);
  const offset = cursor === null ? 0 : Math.max(0, Number(cursor));
  const page = filtered.slice(offset, offset + pageSize);
  const next = offset + pageSize < filtered.length ? String(offset + pageSize) : null;

  return {
    generation: asNumber(args.generation, 1),
    parent_id: null,
    cursor,
    next_cursor: next,
    rows: page,
    total_estimate: filtered.length,
    metric: 'logical',
    sort: { key: 'size', descending: true },
    stale_cursor: false,
  };
}

function matchesFilter(row: NodeRow, filter: FilterSet | null): boolean {
  if (filter === null) return true;
  const text = filter.text.trim().toLowerCase();
  if (text !== '' && !row.path.toLowerCase().includes(text)) return false;
  if (
    filter.extensions.length > 0 &&
    !filter.extensions.some((extension) =>
      row.name.toLowerCase().endsWith(`.${extension.replace('.', '').toLowerCase()}`),
    )
  ) {
    return false;
  }
  if (filter.accessible_only && !row.accessible) return false;

  return !(filter.min_size !== null && row.logical_size < filter.min_size);
}

function validateLongPath(path: string): {
  valid: boolean;
  reason_key: string | null;
  normalized: string | null;
  display: string | null;
} {
  const trimmed = path.trim();
  if (trimmed === '') {
    return {
      valid: false,
      reason_key: 'long_paths.path_required',
      normalized: null,
      display: null,
    };
  }
  if (trimmed.length > 32_000) {
    return {
      valid: false,
      reason_key: 'error.validation',
      normalized: null,
      display: null,
    };
  }
  const hasControlCharacter = [...trimmed].some((character) => {
    const code = character.codePointAt(0) ?? 0;
    return code < 32;
  });
  if (/[<>"|?*]/.test(trimmed) || hasControlCharacter) {
    return {
      valid: false,
      reason_key: 'error.validation',
      normalized: null,
      display: null,
    };
  }
  const normalized =
    trimmed.length > 260 && !trimmed.startsWith('\\\\?\\')
      ? `\\\\?\\${trimmed}`
      : trimmed;

  return { valid: true, reason_key: null, normalized, display: trimmed };
}

function buildRenamePreview(
  nextId: (prefix: string) => string,
  rowsForSizes: NodeRow[],
  args: Record<string, unknown>,
): RenamePreview {
  const targets = asStringArray(args.targets);
  const pattern = asString(args.regex);
  const replacement = asString(args.replacement);
  const scope =
    typeof args.scope === 'string' && args.scope.trim() !== ''
      ? args.scope.trim()
      : null;
  const caseValue: RenameCase =
    args.case === 'lower' || args.case === 'upper' ? args.case : 'preserve';

  if (targets.length === 0) {
    throw failure(createError('E_VALIDATION', 'rename_preview needs targets'));
  }
  if (pattern.trim() === '') {
    throw failure(createError('E_VALIDATION', 'rename_preview needs a pattern'));
  }
  if (scope !== null && !/^[a-z]:\\/i.test(scope)) {
    throw failure(createError('E_VALIDATION', 'rename scope must be absolute'));
  }

  let expression: RegExp;
  try {
    expression = new RegExp(pattern, 'g');
  } catch {
    throw failure(createError('E_VALIDATION', 'invalid regular expression'));
  }

  const rows: RenamePreviewRow[] = targets.map((target) => {
    const name = lastSegment(target);
    const newName = applyCase(name.replace(expression, replacement), caseValue);

    return {
      old_path: target,
      new_path: joinPath(directoryOf(target), newName),
      old_name: name,
      new_name: newName,
      size: sizeOfPath(rowsForSizes, target),
      conflict: false,
    };
  });

  const seen = new Map<string, number>();
  for (const row of rows) {
    const key = row.new_path.toLowerCase();
    seen.set(key, (seen.get(key) ?? 0) + 1);
  }

  const conflicts: RenameConflictView[] = [];
  for (const row of rows) {
    const duplicate = (seen.get(row.new_path.toLowerCase()) ?? 0) > 1;
    const invalid =
      row.new_name.trim() === '' || /[\\/:*?"<>|]/.test(row.new_name);
    const outsideScope = scope !== null && !isInsideScope(row.old_path, scope);
    if (duplicate || invalid || outsideScope) {
      row.conflict = true;
      conflicts.push({
        path: row.old_path,
        target: row.new_path,
        reason: duplicate
          ? 'another target produces the same name'
          : invalid
            ? 'the name contains characters Windows rejects'
            : 'the target is outside the selected scope',
        reason_key: 'error.validation',
      });
    }
  }

  const requiresElevation = rows.some((row) =>
    /\\windows|\\program files|\\system32/i.test(row.old_path),
  );

  return {
    preview_id: nextId('rename-preview'),
    confirmation_token: nextId('rename-token'),
    expires_at: Date.now() + PREVIEW_TTL_MS,
    rows,
    conflicts,
    change_count: rows.filter((row) => !row.conflict && row.new_name !== row.old_name)
      .length,
    total_size: rows.reduce((sum, row) => sum + row.size, 0),
    requires_elevation: requiresElevation,
    warnings: requiresElevation ? ['confirm.warning.elevation'] : [],
  };
}

function applyCase(name: string, transform: RenameCase): string {
  if (transform === 'lower') return name.toLowerCase();
  if (transform === 'upper') return name.toUpperCase();

  return name;
}

function sizeOfPath(rows: NodeRow[], path: string): number {
  const fromRows = rows.find((row) => row.path.toLowerCase() === path.toLowerCase());
  if (fromRows !== undefined) return fromRows.logical_size;

  for (const info of KNOWN_NODES.values()) {
    if (info.path.toLowerCase() === path.toLowerCase()) return info.size;
  }

  return 0;
}

function isInsideScope(path: string, scope: string): boolean {
  const normalizedScope = scope.replace(/\\+$/, '').toLowerCase();
  const normalizedPath = path.toLowerCase();

  return (
    normalizedPath === normalizedScope ||
    normalizedPath.startsWith(`${normalizedScope}\\`)
  );
}

function directoryOf(path: string): string {
  const index = path.lastIndexOf('\\');
  if (index < 0) return '';
  if (index === 2 && path[1] === ':') return `${path.slice(0, 2)}\\`;

  return path.slice(0, index);
}

function joinPath(directory: string, name: string): string {
  if (directory === '') return name;

  return directory.endsWith('\\') ? `${directory}${name}` : `${directory}\\${name}`;
}

function lastSegment(path: string): string {
  const parts = path.split('\\');
  return parts.at(-1) ?? path;
}

function permissionsView(path: string): PermissionsView {
  const trimmed = path.trim();
  if (trimmed === '') {
    throw failure(createError('E_VALIDATION', 'permissions_get needs a path'));
  }
  const lower = trimmed.toLowerCase();
  const protectedPath = /\\windows|\\program files|\\system32/i.test(trimmed);
  const isDirectory = !lastSegment(trimmed).includes('.');
  const mask = initialAttributeMask(lower, isDirectory);

  return {
    path: trimmed,
    owner: 'DOMAIN\\alex',
    owner_sid: 'S-1-5-21-1004336348-1177238915-682003330-1001',
    attribute_mask: mask,
    attributes: attributesFromMask(mask),
    is_directory: isDirectory,
    is_reparse_point: lower.includes('\\link'),
    is_protected: protectedPath,
    requires_elevation: protectedPath,
  };
}

function initialAttributeMask(lowerPath: string, isDirectory: boolean): number {
  let mask = isDirectory ? 0 : ATTRIBUTE_BITS.archive;
  if (lowerPath.includes('\\windows')) mask |= ATTRIBUTE_BITS.system;
  if (lowerPath.includes('archive')) mask |= ATTRIBUTE_BITS.compressed;
  if (lowerPath.includes('sparse')) mask |= ATTRIBUTE_BITS.sparse;

  return mask;
}

function attributesFromMask(mask: number): AttributeFlags {
  return makeAttributes({
    read_only: (mask & ATTRIBUTE_BITS['read-only']) !== 0,
    hidden: (mask & ATTRIBUTE_BITS.hidden) !== 0,
    system: (mask & ATTRIBUTE_BITS.system) !== 0,
    compressed: (mask & ATTRIBUTE_BITS.compressed) !== 0,
    sparse: (mask & ATTRIBUTE_BITS.sparse) !== 0,
    reparse: (mask & 0x400) !== 0,
  });
}

function buildPermissionsPreview(
  nextId: (prefix: string) => string,
  path: string,
  changes: PermissionsChanges,
): PermissionsPreview {
  const current = permissionsView(path);
  const trimmed = current.path;
  const owner =
    changes.owner !== undefined && changes.owner.trim() !== ''
      ? changes.owner.trim()
      : null;
  const copyFrom =
    changes.copy_from !== undefined && changes.copy_from.trim() !== ''
      ? changes.copy_from.trim()
      : null;
  const reset = changes.reset === true;
  const aclActions = [owner !== null, reset, copyFrom !== null].filter(Boolean).length;
  if (aclActions > 1) {
    throw failure(
      createError('E_VALIDATION', 'choose one ACL action: owner, reset or copy_from'),
    );
  }

  const attributeRows: AttributeChangeRow[] = [];
  if (changes.attributes !== undefined) {
    for (const name of changes.attributes.set) {
      attributeRows.push({
        attribute: name,
        before: (current.attribute_mask & ATTRIBUTE_BITS[name]) !== 0,
        after: true,
      });
    }
    for (const name of changes.attributes.clear) {
      attributeRows.push({
        attribute: name,
        before: (current.attribute_mask & ATTRIBUTE_BITS[name]) !== 0,
        after: false,
      });
    }
  }
  const attributeChanges = attributeRows.filter((row) => row.before !== row.after);
  const aclAction: PermissionsAclAction | null =
    owner !== null ? 'owner' : reset ? 'reset' : copyFrom !== null ? 'copy' : null;

  if (aclAction === null && attributeChanges.length === 0) {
    throw failure(createError('E_VALIDATION', 'permissions_preview needs changes'));
  }

  const requiresElevation = current.requires_elevation || owner !== null;

  return {
    preview_id: nextId('permissions-preview'),
    confirmation_token: nextId('permissions-token'),
    expires_at: Date.now() + PREVIEW_TTL_MS,
    path: trimmed,
    current,
    owner,
    acl_action: aclAction,
    copy_from: copyFrom,
    attribute_changes: attributeChanges,
    requires_elevation: requiresElevation,
    warnings: requiresElevation ? ['confirm.warning.elevation'] : [],
  };
}

function readChanges(value: unknown): PermissionsChanges {
  const record = isRecord(value) ? value : {};
  const attributesRecord = isRecord(record.attributes) ? record.attributes : null;
  const attributes: AttributeChangeSet | undefined =
    attributesRecord === null
      ? undefined
      : {
          set: readAttributeList(attributesRecord.set),
          clear: readAttributeList(attributesRecord.clear),
        };
  const owner =
    typeof record.owner === 'string' && record.owner.trim() !== ''
      ? record.owner
      : null;
  const copyFrom =
    typeof record.copy_from === 'string' && record.copy_from.trim() !== ''
      ? record.copy_from
      : null;

  return {
    ...(owner === null ? {} : { owner }),
    ...(record.reset === true ? { reset: true } : {}),
    ...(copyFrom === null ? {} : { copy_from: copyFrom }),
    ...(attributes === undefined ? {} : { attributes }),
  };
}

function readAttributeList(value: unknown): FileAttributeName[] {
  const known: readonly string[] = ATTRIBUTE_NAMES;
  return asStringArray(value).filter((item): item is FileAttributeName =>
    known.includes(item),
  );
}

function assertUsablePreview<T>(
  stored: StoredPreview<T> | undefined,
  token: string,
  expiresAt: number,
): void {
  const preview = stored?.preview;
  const valid =
    stored !== undefined &&
    !stored.used &&
    expiryOf(preview) === expiresAt &&
    tokenOf(preview) === token &&
    Date.now() <= expiresAt;

  if (!valid) {
    throw failure(createError('E_PREVIEW_EXPIRED', 'preview token is not usable'));
  }
}

function expiryOf(preview: unknown): number {
  return isRecord(preview) && typeof preview.expires_at === 'number'
    ? preview.expires_at
    : 0;
}

function tokenOf(preview: unknown): string {
  return isRecord(preview) && typeof preview.confirmation_token === 'string'
    ? preview.confirmation_token
    : '';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function asString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

function asNumber(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item: unknown): item is string => typeof item === 'string')
    : [];
}

function createError(code: UiErrorCode, details: string): UiError {
  return {
    code,
    category: categoryFor(code),
    message_key: messageKeyFor(code),
    details,
    retryable: code === 'E_PREVIEW_EXPIRED' || code === 'E_BUSY',
    task_id: null,
    journal_entry_id: null,
  };
}

function categoryFor(code: UiErrorCode): UiError['category'] {
  switch (code) {
    case 'E_VALIDATION':
      return 'validation';
    case 'E_ACCESS_DENIED':
    case 'E_ELEVATION_REQUIRED':
    case 'E_ELEVATION_DENIED':
      return 'access';
    case 'E_STALE_CURSOR':
    case 'E_STALE_NODE':
      return 'stale';
    case 'E_BUSY':
    case 'E_OPERATION_PARTIAL':
      return 'operation';
    case 'E_CANCELLED':
      return 'cancel';
    case 'E_INTERNAL':
      return 'internal';
    default:
      return 'io';
  }
}

function messageKeyFor(code: UiErrorCode): string {
  switch (code) {
    case 'E_ACCESS_DENIED':
      return 'error.access_denied';
    case 'E_NOT_FOUND':
      return 'error.not_found';
    case 'E_STALE_CURSOR':
      return 'error.stale_cursor';
    case 'E_STALE_NODE':
      return 'error.stale_node';
    case 'E_BUSY':
      return 'error.busy';
    case 'E_CANCELLED':
      return 'error.cancelled';
    case 'E_PREVIEW_EXPIRED':
      return 'error.preview_expired';
    case 'E_ELEVATION_REQUIRED':
      return 'error.elevation_required';
    case 'E_ELEVATION_DENIED':
      return 'error.elevation_denied';
    case 'E_OPERATION_PARTIAL':
      return 'error.operation_partial';
    case 'E_UNSUPPORTED':
      return 'error.unsupported';
    case 'E_OFFLINE':
      return 'error.offline';
    case 'E_INTERNAL':
      return 'error.internal';
    case 'E_IO':
      return 'error.io';
    case 'E_VALIDATION':
      return 'error.validation';
  }
}

function failure(error: UiError): Error {
  return Object.assign(new Error(error.details ?? error.message_key), error);
}
