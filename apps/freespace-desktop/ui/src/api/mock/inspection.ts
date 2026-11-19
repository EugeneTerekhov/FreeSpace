import type { FilterSet, NodeRow, TreePage, UiError } from '../dto';
import type {
  ArchiveEntry,
  ArchiveEntryPage,
  ArchiveExportFormat,
  ArchiveExportRequest,
  ExportComposition,
  ExportPreview,
  ExportPreviewRequest,
  ExportResult,
  ExportSection,
  ExportScope,
  ExportStatus,
  FileLock,
  MetadataKind,
  MetadataTag,
  MetadataView,
  OwnerSummary,
  OwnerSummaryPage,
  ReportFormat,
} from '../dto-inspection';
import type { MockRegister } from './register';

const KB = 1024;
const MB = 1024 * KB;
const GB = 1024 * MB;

const EMPTY_FILTER: FilterSet = {
  text: '',
  extensions: [],
  categories: [],
  min_size: null,
  max_size: null,
  modified_after: null,
  modified_before: null,
  accessible_only: false,
  exclude_paths: [],
  name: null,
};

const TERMINAL_EXPORT = new Set(['completed', 'partial', 'failed', 'cancelled']);
const REPORT_FORMATS: readonly ReportFormat[] = ['csv', 'xml', 'html', 'xlsx'];
const EXPORT_SECTIONS: readonly ExportSection[] = [
  'tree',
  'top',
  'types',
  'duplicates',
  'owners',
];

/** Attaches the stable error envelope to a thrown error. */
function failure(
  code: UiError['code'],
  messageKey: string,
  details: string,
): Error {
  const error: UiError = {
    code,
    category:
      code === 'E_VALIDATION'
        ? 'validation'
        : code === 'E_ACCESS_DENIED' || code === 'E_ELEVATION_REQUIRED'
          ? 'access'
          : code === 'E_STALE_CURSOR' || code === 'E_STALE_NODE'
            ? 'stale'
            : code === 'E_BUSY'
              ? 'operation'
              : code === 'E_NOT_FOUND'
                ? 'stale'
                : 'io',
    message_key: messageKey,
    details,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };

  return Object.assign(new Error(details), error);
}

function text(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

function number(value: unknown, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function offsetOf(cursor: unknown): number {
  if (typeof cursor !== 'string' || cursor === '') return 0;
  const parsed = Number(cursor);

  return Number.isFinite(parsed) && parsed >= 0 ? parsed : 0;
}

function nextCursor(offset: number, pageSize: number, total: number): string | null {
  return offset + pageSize < total ? String(offset + pageSize) : null;
}

// --- Metadata --------------------------------------------------------------

const EXTENSION_KIND: Record<string, MetadataKind> = {
  jpg: 'exif',
  jpeg: 'exif',
  tif: 'exif',
  tiff: 'exif',
  heic: 'exif',
  webp: 'exif',
  mp3: 'audio',
  flac: 'audio',
  wav: 'audio',
  ogg: 'audio',
  m4a: 'audio',
  mp4: 'video',
  m4v: 'video',
  mov: 'video',
  exe: 'pe',
  dll: 'pe',
  sys: 'pe',
};

const SOURCE_KEYS: Record<MetadataKind, string> = {
  exif: 'metadata.source.exif',
  audio: 'metadata.source.audio',
  video: 'metadata.source.video',
  pe: 'metadata.source.pe',
};

const KNOWN_PATHS: Record<number, { path: string; size: number }> = {
  4: { path: 'C:\\pagefile.sys', size: 16 * GB },
  5: { path: 'C:\\hiberfil.sys', size: 12 * GB },
  12: { path: 'C:\\Windows\\explorer.exe', size: 4 * MB },
  31: { path: 'C:\\Temp\\crash.dmp', size: 2 * GB },
  32: { path: 'C:\\Windows\\System32\\notepad.exe', size: 200 * MB },
  33: { path: 'C:\\Program Files\\FreeSpace\\freespace-gui.exe', size: 40 * MB },
  34: { path: 'C:\\Users\\Documents\\report.pdf', size: 8 * MB },
  35: { path: 'C:\\Users\\Downloads\\archive.zip', size: GB },
  36: { path: 'C:\\Users\\Pictures\\photo.jpg', size: 12 * MB },
  37: { path: 'C:\\Users\\Downloads\\backup-1.bin', size: 512 * MB },
  38: { path: 'C:\\Users\\Downloads\\backup-2.bin', size: 512 * MB },
};

function tag(key: string, value: string, source: string): MetadataTag {
  return { key, value, source };
}

function metadataTags(kind: MetadataKind, nodeId: number): MetadataTag[] {
  const source = SOURCE_KEYS[kind];
  if (kind === 'exif') {
    return [
      tag('CameraMake', 'Contoso', source),
      tag('CameraModel', `Camera ${nodeId}`, source),
      tag('Width', '6000', source),
      tag('Height', '4000', source),
      tag('TakenAt', '2026-05-01T12:00:00Z', source),
    ];
  }
  if (kind === 'audio') {
    return [
      tag('DurationSecs', '180', source),
      tag('BitrateKbps', '320', source),
      tag('SampleRate', '44100', source),
      tag('Channels', '2', source),
    ];
  }
  if (kind === 'video') {
    return [
      tag('Width', '1920', source),
      tag('Height', '1080', source),
      tag('DurationSecs', '90', source),
      tag('Codec', 'h264', source),
    ];
  }

  return [
    tag('ProductName', 'FreeSpace', source),
    tag('FileVersion', '1.1.0.0', source),
    tag('CompanyName', 'FreeSpace', source),
  ];
}

function extensionOf(path: string): string {
  if (!path.includes('.')) return '';
  const last = path.split('.').at(-1);

  return (last ?? '').toLowerCase();
}

function metadataFor(nodeId: number): MetadataView {
  const known = KNOWN_PATHS[nodeId];
  const path = known?.path ?? `C:\\Users\\Public\\sample-${nodeId}.bin`;
  const extension = extensionOf(path);
  const kind = EXTENSION_KIND[extension];

  if (kind === undefined) {
    const corrupt = extension === 'dmp' || nodeId % 13 === 0;
    throw failure(
      'E_UNSUPPORTED',
      'error.unsupported',
      corrupt ? 'metadata.corrupt: the container cannot be parsed' : 'metadata.unsupported.extension',
    );
  }

  return {
    node_id: nodeId,
    path,
    kind,
    source_key: SOURCE_KEYS[kind],
    status: 'ok',
    available: true,
    tags: metadataTags(kind, nodeId),
    truncated: false,
    size_bytes: known?.size ?? (nodeId + 1) * 3 * MB,
    modified_at: Date.UTC(2026, 4, 1, 12, 0, 0),
  };
}

// --- Owners ----------------------------------------------------------------

const OWNER_BASE: readonly Omit<OwnerSummary, 'share'>[] = [
  {
    owner_id: 1,
    display_name: 'CORP\\alice',
    sid: 'S-1-5-21-1001',
    total_bytes: 18 * GB,
    file_count: 1240,
    accessible: true,
    label_key: 'owners.label.account',
  },
  {
    owner_id: 2,
    display_name: 'CORP\\bob',
    sid: 'S-1-5-21-1002',
    total_bytes: 12 * GB,
    file_count: 860,
    accessible: true,
    label_key: 'owners.label.account',
  },
  {
    owner_id: 3,
    display_name: 'NT AUTHORITY\\SYSTEM',
    sid: 'S-1-5-18',
    total_bytes: 42 * GB,
    file_count: 9400,
    accessible: false,
    label_key: 'owners.label.account',
  },
  {
    owner_id: 4,
    display_name: 'BUILTIN\\Administrators',
    sid: 'S-1-5-32-544',
    total_bytes: 6 * GB,
    file_count: 320,
    accessible: true,
    label_key: 'owners.label.account',
  },
  {
    owner_id: 5,
    display_name: 'CORP\\carol',
    sid: 'S-1-5-21-1003',
    total_bytes: 3 * GB,
    file_count: 410,
    accessible: true,
    label_key: 'owners.label.account',
  },
  {
    owner_id: 6,
    display_name: 'CORP\\dmitri',
    sid: 'S-1-5-21-1004',
    total_bytes: 2 * GB,
    file_count: 275,
    accessible: true,
    label_key: 'owners.label.account',
  },
  {
    owner_id: null,
    display_name: 'unknown',
    sid: '',
    total_bytes: 0,
    file_count: 12,
    accessible: false,
    label_key: 'owners.label.unknown',
  },
];

function ownerSummaries(filter: FilterSet | null): OwnerSummary[] {
  const total = OWNER_BASE.reduce((sum, owner) => sum + owner.total_bytes, 0);
  const needle = filter?.text.trim().toLowerCase() ?? '';

  return OWNER_BASE.map((owner) => ({
    ...owner,
    share: total === 0 ? 0 : owner.total_bytes / total,
  })).filter(
    (owner) =>
      needle === '' ||
      owner.display_name.toLowerCase().includes(needle) ||
      owner.sid.toLowerCase().includes(needle),
  );
}

function ownerFilesPage(
  ownerKey: string,
  cursor: unknown,
  pageSize: number,
  generation: number,
): TreePage {
  const key = ownerKey.trim().toLowerCase();
  const owner = OWNER_BASE.find(
    (item) =>
      item.display_name.toLowerCase() === key ||
      item.sid.toLowerCase() === key ||
      (key === 'unknown' && item.owner_id === null),
  );
  const account = (owner?.display_name ?? 'unknown').split('\\').at(-1) ?? 'user';
  const count = owner === undefined ? 0 : 9;
  const rows: NodeRow[] = Array.from({ length: count }, (_, index) => ({
    id: 10_000 + index,
    parent_id: null,
    name: `owned-${index + 1}.dat`,
    path: `C:\\Users\\${account}\\Documents\\owned-${index + 1}.dat`,
    kind: 'file',
    logical_size: (count - index) * 64 * MB,
    on_disk_size: (count - index) * 64 * MB,
    physical_size: (count - index) * 64 * MB,
    ads_size: 0,
    child_count: 0,
    file_count: 1,
    modified_at: Date.UTC(2026, 4, index + 1, 9, 0, 0),
    accessed_at: Date.UTC(2026, 4, index + 1, 10, 0, 0),
    attributes: {
      read_only: false,
      hidden: false,
      system: false,
      compressed: false,
      sparse: false,
      reparse: false,
    },
    accessible: owner?.accessible !== false,
    link_target: null,
  }));
  const offset = offsetOf(cursor);
  const size = Math.max(1, pageSize);

  return {
    generation,
    parent_id: null,
    cursor: typeof cursor === 'string' ? cursor : null,
    next_cursor: nextCursor(offset, size, rows.length),
    rows: rows.slice(offset, offset + size),
    total_estimate: rows.length,
    metric: 'logical',
    sort: { key: 'size', descending: true },
    stale_cursor: false,
  };
}

// --- Archives --------------------------------------------------------------

const ARCHIVE_NAMES: readonly { name: string; size: number; packed: number }[] = [
  { name: 'readme.txt', size: 4 * KB, packed: 2 * KB },
  { name: 'LICENSE', size: 12 * KB, packed: 5 * KB },
  { name: 'images/photo-01.jpg', size: 8 * MB, packed: 7 * MB },
  { name: 'images/photo-02.jpg', size: 7 * MB, packed: 6 * MB },
  { name: 'docs/report.pdf', size: 3 * MB, packed: 2 * MB },
  { name: 'docs/manual.html', size: 512 * KB, packed: 160 * KB },
  { name: 'data/backup.bin', size: 64 * MB, packed: 48 * MB },
  { name: 'data/index.json', size: 96 * KB, packed: 32 * KB },
  { name: 'bin/app.exe', size: 2 * MB, packed: 900 * KB },
  { name: 'bin/tool.exe', size: 1 * MB, packed: 480 * KB },
  { name: 'media/clip.mp4', size: 24 * MB, packed: 22 * MB },
  { name: 'media/track.mp3', size: 6 * MB, packed: 5 * MB },
];

function archiveFormat(path: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith('.7z')) return '7z';
  if (lower.endsWith('.tar')) return 'tar';
  if (lower.endsWith('.gz') || lower.endsWith('.tgz')) return 'gz';
  if (lower.endsWith('.iso')) return 'iso';
  if (lower.endsWith('.rar')) return 'rar';
  if (lower.endsWith('.zip')) return 'zip';

  return 'unknown';
}

function archivePage(args: Record<string, unknown>): ArchiveEntryPage {
  const path = text(args.path).trim();
  if (path === '') {
    throw failure('E_VALIDATION', 'error.validation', 'archive path is required');
  }

  const lower = path.toLowerCase();
  if (
    lower.includes('password') ||
    lower.includes('protected') ||
    lower.includes('encrypted')
  ) {
    throw failure(
      'E_UNSUPPORTED',
      'error.unsupported',
      'archive.unsupported.password: the archive is password-protected',
    );
  }
  if (lower.includes('corrupt')) {
    throw failure(
      'E_UNSUPPORTED',
      'error.unsupported',
      'archive.unsupported.corrupt: the archive cannot be parsed',
    );
  }

  const entries: ArchiveEntry[] = [...ARCHIVE_NAMES]
    .sort((left, right) => right.size - left.size)
    .map((entry) => ({
      internal_path: entry.name,
      size: entry.size,
      compressed_size: entry.packed,
      is_directory: false,
      encrypted: false,
      share: entry.size / ARCHIVE_NAMES.reduce((sum, item) => sum + item.size, 0),
      depth: entry.name.split('/').length - 1,
      extension: extensionOf(entry.name) === '' ? null : extensionOf(entry.name),
    }));
  const offset = offsetOf(args.cursor);
  const pageSize = Math.max(1, number(args.page_size, 25));

  return {
    path,
    format: archiveFormat(path),
    cursor: typeof args.cursor === 'string' ? args.cursor : null,
    next_cursor: nextCursor(offset, pageSize, entries.length),
    entries: entries.slice(offset, offset + pageSize),
    total_entries: entries.length,
    total_files: entries.length,
    total_size: entries.reduce((sum, entry) => sum + entry.size, 0),
    encrypted_count: 0,
    warnings: [],
    stale_cursor: false,
  };
}

function archiveExportFormat(value: unknown): ArchiveExportFormat {
  const candidate = text(value, 'csv');

  return candidate === 'json' || candidate === 'xml' || candidate === 'html'
    ? candidate
    : 'csv';
}

function exportArchiveList(args: Record<string, unknown>): ExportResult {
  const request: ArchiveExportRequest = {
    path: text(args.path),
    format: archiveExportFormat(args.format),
    out: text(args.out).trim(),
  };
  if (request.out === '') {
    throw failure('E_VALIDATION', 'error.validation', 'export path is required');
  }

  return {
    path: request.out,
    format: request.format,
    bytes_written: 4096,
    rows_written: ARCHIVE_NAMES.length,
    sections: [],
    warnings: [],
  };
}

// --- Export ----------------------------------------------------------------

function reportFormat(value: unknown): ReportFormat {
  const candidate = text(value, 'csv');

  return REPORT_FORMATS.includes(candidate as ReportFormat)
    ? (candidate as ReportFormat)
    : 'csv';
}

function compositionOf(value: unknown): ExportComposition {
  if (typeof value !== 'object' || value === null) {
    return { sections: [...EXPORT_SECTIONS] };
  }
  const source = value as {
    sections?: unknown;
    max_tree_rows?: unknown;
    overwrite?: unknown;
  };
  const sections = Array.isArray(source.sections)
    ? source.sections.filter(
        (section): section is ExportSection =>
          typeof section === 'string' &&
          EXPORT_SECTIONS.includes(section as ExportSection),
      )
    : [...EXPORT_SECTIONS];

  return {
    sections,
    max_tree_rows:
      typeof source.max_tree_rows === 'number' ? source.max_tree_rows : null,
    overwrite: source.overwrite === true,
  };
}

function previewExport(args: Record<string, unknown>): ExportPreview {
  const request = args as Partial<ExportPreviewRequest>;
  const format = reportFormat(request.format);
  if (text(args.format) === 'json') {
    throw failure(
      'E_VALIDATION',
      'error.validation',
      'export.error.format: json is only available for archive lists',
    );
  }
  const composition = compositionOf(request.composition);
  if (composition.sections.length === 0) {
    throw failure('E_VALIDATION', 'error.validation', 'select at least one section');
  }

  const scope = (request.scope ?? {}) as Partial<ExportScope>;
  const output = typeof scope.out === 'string' ? scope.out.trim() : '';
  if (output === '') {
    throw failure('E_VALIDATION', 'error.validation', 'output path is required');
  }

  const nodes = composition.sections.includes('tree') ? 180_000 : 0;
  const top = composition.sections.includes('top') ? 1000 : 0;
  const rowEstimate =
    composition.sections.includes('types') ||
    composition.sections.includes('duplicates') ||
    composition.sections.includes('owners')
      ? null
      : nodes + top;

  return {
    preview_id: `export-preview-${++exportSequence}`,
    confirmation_token: `export-token-${exportSequence}`,
    format,
    out: output,
    sections: composition.sections,
    row_estimate: rowEstimate,
    total_bytes: 760 * GB,
    on_disk_bytes: 780 * GB,
    physical_bytes: 790 * GB,
    overwrite: composition.overwrite === true,
    warnings: [],
    expires_at: Date.now() + 600_000,
  };
}

let exportSequence = 0;

interface ExportRecord {
  preview: ExportPreview;
  status: ExportStatus;
  polls: number;
}

const exportPreviews = new Map<string, ExportPreview>();
const exportTasks = new Map<string, ExportRecord>();

function startExport(
  previewId: string,
  confirmationToken: string,
): { task_id: string } {
  const preview = exportPreviews.get(previewId);
  if (
    preview === undefined ||
    (confirmationToken !== '' &&
      confirmationToken !== preview.confirmation_token)
  ) {
    throw failure('E_PREVIEW_EXPIRED', 'error.preview_expired', 'unknown preview');
  }

  const taskId = `export-task-${++exportSequence}`;
  exportTasks.set(taskId, {
    preview,
    polls: 0,
    status: {
      task_id: taskId,
      state: 'running',
      processed: 0,
      total: preview.sections.length + 2,
      phase_key: 'export.phase.preparing',
      out: preview.out,
      bytes_written: null,
      rows_written: 0,
      cancelable: true,
      started_at: Date.now(),
      finished_at: null,
      error: null,
    },
  });
  exportPreviews.delete(previewId);

  return { task_id: taskId };
}

function exportStatus(taskId: string): ExportStatus {
  const record = exportTasks.get(taskId);
  if (record === undefined) {
    throw failure('E_NOT_FOUND', 'error.not_found', 'unknown export task');
  }
  if (TERMINAL_EXPORT.has(record.status.state)) {
    return { ...record.status };
  }

  record.polls += 1;
  const ratio = Math.min(1, record.polls / 3);
  record.status = {
    ...record.status,
    processed: Math.round(record.status.total * ratio),
    state: ratio >= 1 ? 'completed' : 'running',
    bytes_written: ratio >= 1 ? 8192 : null,
    rows_written: ratio >= 1 ? (record.preview.row_estimate ?? 42) : 0,
    phase_key: ratio >= 1 ? 'export.phase.done' : 'export.phase.writing',
    cancelable: ratio < 1,
    finished_at: ratio >= 1 ? Date.now() : null,
  };

  return { ...record.status };
}

function cancelExport(taskId: string): Record<string, never> {
  const record = exportTasks.get(taskId);
  if (record === undefined) {
    throw failure('E_NOT_FOUND', 'error.not_found', 'unknown export task');
  }
  record.status = {
    ...record.status,
    state: 'cancelled',
    phase_key: 'export.phase.cancelled',
    cancelable: false,
    finished_at: Date.now(),
    error: null,
  };

  return {};
}

// --- Unlock ----------------------------------------------------------------

const releasedPaths = new Set<string>();

function locksFor(path: string): FileLock[] {
  if (path === '') {
    throw failure('E_VALIDATION', 'error.validation', 'file path is required');
  }
  if (releasedPaths.has(path)) return [];
  if (path.toLowerCase().includes('free')) return [];

  return [
    {
      process_id: 4242,
      process_name: 'explorer.exe',
      service_name: '',
      application_type: 'explorer',
      access_type: 'unknown',
      access_type_key: 'unlock.access.unknown',
      can_soft_close: true,
      requires_reboot: false,
      critical: false,
      handle_count: null,
    },
    {
      process_id: 5120,
      process_name: 'MsMpEng.exe',
      service_name: 'Antivirus',
      application_type: 'service',
      access_type: 'unknown',
      access_type_key: 'unlock.access.unknown',
      can_soft_close: false,
      requires_reboot: false,
      critical: true,
      handle_count: null,
    },
    {
      process_id: 2336,
      process_name: 'SearchIndexer.exe',
      service_name: 'Windows Search',
      application_type: 'service',
      access_type: 'unknown',
      access_type_key: 'unlock.access.unknown',
      can_soft_close: true,
      requires_reboot: false,
      critical: false,
      handle_count: null,
    },
  ];
}

function releaseLocks(args: Record<string, unknown>): { operation_id: string } {
  const path = text(args.path).trim();
  const mode = args.mode === 'force' ? 'force' : 'soft';
  const processes = Array.isArray(args.process_ids)
    ? args.process_ids.filter((value): value is number => typeof value === 'number')
    : [];

  if (path === '') {
    throw failure('E_VALIDATION', 'error.validation', 'file path is required');
  }
  if (args.confirmed === false) {
    throw failure('E_VALIDATION', 'error.validation', 'releasing a lock needs confirmation');
  }
  if (path.toLowerCase().includes('protected')) {
    throw failure(
      'E_ELEVATION_REQUIRED',
      'error.elevation_required',
      'administrator rights are required',
    );
  }
  if (mode === 'force' && processes.length === 0) {
    throw failure('E_VALIDATION', 'error.validation', 'select at least one process');
  }

  releasedPaths.add(path);

  return { operation_id: `unlock-operation-${++exportSequence}` };
}

function scheduleReboot(args: Record<string, unknown>): { operation_id: string } {
  const path = text(args.path).trim();
  if (path === '') {
    throw failure('E_VALIDATION', 'error.validation', 'file path is required');
  }
  if (args.confirmed === false) {
    throw failure('E_VALIDATION', 'error.validation', 'scheduling a reboot needs confirmation');
  }
  if (path.toLowerCase().includes('protected')) {
    throw failure(
      'E_ACCESS_DENIED',
      'error.access_denied',
      'administrator rights are required',
    );
  }

  return { operation_id: `reboot-operation-${++exportSequence}` };
}

/**
 * Deterministic US5 handlers shared by Vitest, Playwright and the preview.
 *
 * No handler touches the real filesystem: metadata, owners, archive entries
 * and export progress are generated in memory so the offline UI can be
 * exercised without Windows or WebView2.
 */
export function registerInspectionMocks(register: MockRegister): void {
  register('metadata_get', (args) => {
    const nodeId = number(args.node_id, -1);
    if (nodeId < 0) {
      throw failure('E_STALE_NODE', 'error.stale_node', 'unknown node');
    }

    return metadataFor(nodeId);
  });

  register('owners_page', (args) => {
    const pageSize = Math.max(1, number(args.page_size, 5));
    const rows = ownerSummaries((args.filter as FilterSet | null | undefined) ?? EMPTY_FILTER);
    const offset = offsetOf(args.cursor);

    return {
      generation: number(args.generation, 1),
      cursor: typeof args.cursor === 'string' ? args.cursor : null,
      next_cursor: nextCursor(offset, pageSize, rows.length),
      rows: rows.slice(offset, offset + pageSize),
      total_estimate: rows.length,
      total_bytes: rows.reduce((sum, owner) => sum + owner.total_bytes, 0),
      unknown_bytes: rows
        .filter((owner) => owner.owner_id === null)
        .reduce((sum, owner) => sum + owner.total_bytes, 0),
      error_count: rows.filter((owner) => !owner.accessible).length,
      warnings: [],
      stale_cursor: false,
    } satisfies OwnerSummaryPage;
  });

  register('owners_files_page', (args) =>
    ownerFilesPage(
      text(args.owner),
      args.cursor,
      number(args.page_size, 5),
      number(args.generation, 1),
    ),
  );

  register('archives_entries_page', (args) => archivePage(args));
  register('archives_export_list', (args) => exportArchiveList(args));

  register('export_preview', (args) => {
    const preview = previewExport(args);
    exportPreviews.set(preview.preview_id, preview);

    return preview;
  });
  register('export_start', (args) =>
    startExport(text(args.preview_id), text(args.confirmation_token)),
  );
  register('export_status', (args) => exportStatus(text(args.task_id)));
  register('export_cancel', (args) => cancelExport(text(args.task_id)));

  register('unlock_locks', (args) => locksFor(text(args.path).trim()));
  register('unlock_release', (args) => releaseLocks(args));
  register('unlock_schedule_reboot', (args) => scheduleReboot(args));
}
