import type {
  AppBootstrap,
  AppCapabilities,
  CleanupCandidateView,
  DuplicateCompletedEvent,
  DuplicateGroupPage,
  DuplicateGroupView,
  DuplicateProgressEvent,
  DuplicateStartRequest,
  DuplicateTask,
  EventEnvelope,
  FilterSet,
  JournalEntryView,
  JournalPage,
  NodeRow,
  OperationAccepted,
  OperationCompletedEvent,
  OperationPreview,
  OperationPreviewRequest,
  OperationProgress,
  PlatformCapabilities,
  ScanProgressEvent,
  SnapshotDiffPage,
  SnapshotDiffRow,
  SnapshotDiffSortKey,
  SnapshotSummary,
  Task,
  TypeSummary,
  UiError,
  UiSettings,
  UiSettingsPatch,
  VolumeDetail,
  VolumeSummary,
} from './dto';
import type { CommandName, EventName, Transport, Unlisten } from './transport';
import { COMMANDS, EVENTS } from './transport';
import { WINDOWS_CAPABILITIES } from '../lib/capabilities';
import { registerFilesystemMocks } from './mock/filesystem';
import { registerInspectionMocks } from './mock/inspection';
import { registerSettingsMocks } from './mock/settings';
import type { MockRegister } from './mock/register';

const MB = 1024 * 1024;
const GB = 1024 * MB;

type Handler = (args: Record<string, unknown>) => unknown;

function emptyFilter(): FilterSet {
  return {
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
}

/**
 * Mirrors the settings the Tauri shell injects before the first paint.
 *
 * The real backend reads `settings.toml`; the browser preview and Playwright
 * tests inject the same object with `addInitScript`, so the mock returns data
 * that matches what `boot.js` already applied to the document.
 */
function bootSettings(): Partial<UiSettings> {
  const boot = (
    globalThis as {
      __FREESPACE_BOOT__?: {
        theme?: unknown;
        language?: unknown;
        reduced_motion?: unknown;
      };
    }
  ).__FREESPACE_BOOT__;

  if (boot === undefined) return {};

  const settings: Partial<UiSettings> = {};
  if (boot.theme === 'dark' || boot.theme === 'light' || boot.theme === 'system') {
    settings.theme = boot.theme;
  }
  if (boot.language === 'ru' || boot.language === 'en') {
    settings.language = boot.language;
  }
  if (
    boot.reduced_motion === 'on' ||
    boot.reduced_motion === 'off' ||
    boot.reduced_motion === 'system'
  ) {
    settings.reduced_motion = boot.reduced_motion;
  }

  return settings;
}

function defaultSettings(): UiSettings {
  return {
    theme: 'dark',
    language: 'ru',
    density: 'comfortable',
    units: 'binary',
    reduced_motion: 'system',
    last_screen: 'overview',
    sidebar_collapsed: false,
    column_layouts: {},
    saved_filters: {},
    schema_version: 1,
    ...bootSettings(),
  };
}

function mockError(code: UiError['code'], details: string): UiError {
  const category =
    code === 'E_VALIDATION'
      ? 'validation'
      : code === 'E_ACCESS_DENIED'
        ? 'access'
        : code === 'E_STALE_CURSOR' || code === 'E_STALE_NODE'
          ? 'stale'
          : code === 'E_BUSY'
            ? 'operation'
            : 'io';

  return {
    code,
    category,
    message_key: `error.${code.toLowerCase().replace('e_', '')}`,
    details,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };
}

function mockFailure(error: UiError): Error {
  return Object.assign(new Error(error.details ?? error.message_key), error);
}

function asString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

function categoryKey(name: string): string {
  const extension = name.includes('.') ? (name.split('.').pop() ?? '').toLowerCase() : '';
  if (['mp4', 'mkv', 'avi', 'mov'].includes(extension)) return 'video';
  if (['jpg', 'png', 'gif', 'webp'].includes(extension)) return 'image';
  if (['pdf', 'docx', 'txt', 'md'].includes(extension)) return 'document';
  if (['zip', '7z', 'rar'].includes(extension)) return 'archive';
  if (['exe', 'dll', 'sys'].includes(extension)) return 'executable';
  return 'other';
}

/**
 * In-memory transport used by Vitest, Playwright and the browser preview.
 *
 * It returns realistic data for the four core screens and simulates a scan so
 * the interface can be exercised without Windows or WebView2.
 */
export class MockTransport implements Transport {
  private readonly handlers = new Map<CommandName, Handler>();
  private readonly listeners = new Map<EventName, Set<(payload: EventEnvelope<unknown>) => void>>();
  private readonly sequences = new Map<string, number>();
  private readonly timers = new Set<ReturnType<typeof setTimeout>>();

  readonly rows: NodeRow[] = [];
  private readonly byId = new Map<number, NodeRow>();
  private readonly children = new Map<number, number[]>();
  settings = defaultSettings();
  volumes: VolumeSummary[] = [];
  tasks: Task[] = [];
  generation = 1;
  /** Volumes of the latest mock scan, used by `snapshot_volumes`. */
  snapshotVolumeIds: string[] = [];
  /**
   * Capabilities the mock backend advertises. Tests and the browser preview can
   * switch this to `LINUX_CAPABILITIES` to exercise capability-driven hiding.
   */
  platformCapabilities: PlatformCapabilities = { ...WINDOWS_CAPABILITIES };

  private duplicateSearches = new Map<
    string,
    { task: DuplicateTask; groups: DuplicateGroupView[] }
  >();
  private snapshots: SnapshotSummary[] = [];
  private comparisons = new Map<string, SnapshotDiffPage>();
  private journal: JournalEntryView[] = [];
  private previews = new Map<
    string,
    { request: OperationPreviewRequest; preview: OperationPreview }
  >();
  private operations = new Map<string, OperationProgress>();

  constructor() {
    this.seedVolumes();
    // The preview starts with a one-volume snapshot; scanning another volume
    // adds its root to the same tree.
    this.snapshotVolumeIds = this.volumes[0] === undefined ? [] : [this.volumes[0].volume_id];
    this.seedTree();
    this.seedHandlers();
  }

  setHandler(command: CommandName, handler: Handler): this {
    this.handlers.set(command, handler);
    return this;
  }

  failCommand(command: CommandName, error: UiError): this {
    this.handlers.set(command, () => {
      throw mockFailure(error);
    });
    return this;
  }

  async invoke<T>(command: CommandName, args: Record<string, unknown> = {}): Promise<T> {
    const handler = this.handlers.get(command);
    if (handler === undefined) {
      throw mockFailure(mockError('E_INTERNAL', `mock has no handler for ${command}`));
    }

    return (await handler(args)) as T;
  }

  listen<T>(
    event: EventName,
    handler: (payload: EventEnvelope<T>) => void,
  ): Promise<Unlisten> {
    const set = this.listeners.get(event) ?? new Set();
    const wrapper = handler as (payload: EventEnvelope<unknown>) => void;
    set.add(wrapper);
    this.listeners.set(event, set);

    return Promise.resolve(() => {
      set.delete(wrapper);
    });
  }

  emit<T>(event: EventName, payload: T, taskId: string | null = null): void {
    const key = taskId ?? event;
    const sequence = (this.sequences.get(key) ?? 0) + 1;
    this.sequences.set(key, sequence);
    const envelope: EventEnvelope<T> = {
      version: 1,
      event,
      sequence,
      task_id: taskId,
      emitted_at: new Date().toISOString(),
      payload,
    };

    for (const listener of this.listeners.get(event) ?? []) {
      listener(envelope);
    }
  }

  dispose(): void {
    for (const timer of this.timers) {
      clearTimeout(timer);
    }
    this.timers.clear();
  }

  /** Rows that belong to volumes of the current mock snapshot. */
  private get snapshotRows(): NodeRow[] {
    if (this.snapshotVolumeIds.length === 0) return this.rows;

    return this.rows.filter((row) => {
      const volume = this.volumes.find((item) =>
        row.path.toLowerCase().startsWith(item.mount_point.toLowerCase()),
      );

      return volume === undefined || this.snapshotVolumeIds.includes(volume.volume_id);
    });
  }

  private addRow(
    id: number,
    parentId: number | null,
    name: string,
    kind: NodeRow['kind'],
    size: number,
    extra: Partial<NodeRow> = {},
  ): void {
    const parentPath = parentId === null ? '' : this.byId.get(parentId)?.path ?? '';
    const path =
      parentId === null
        ? name
        : `${parentPath.endsWith('\\') ? parentPath : `${parentPath}\\`}${name}`;
    const row: NodeRow = {
      id,
      parent_id: parentId,
      name,
      path,
      kind,
      logical_size: size,
      on_disk_size: size,
      physical_size: size,
      ads_size: 0,
      child_count: 0,
      file_count: kind === 'file' ? 1 : 0,
      modified_at: Date.now() - id * 3_600_000,
      accessed_at: Date.now() - id * 1_800_000,
      attributes: {
        read_only: false,
        hidden: false,
        system: false,
        compressed: false,
        sparse: false,
        reparse: false,
      },
      accessible: true,
      link_target: null,
      ...extra,
    };

    this.rows.push(row);
    this.byId.set(id, row);
    if (parentId !== null) {
      this.children.set(parentId, [...(this.children.get(parentId) ?? []), id]);
    }
  }

  private seedVolumes(): void {
    this.volumes = [
      {
        volume_id: '11111111-1111-1111-1111-111111111111',
        mount_point: 'C:\\',
        label: 'System',
        file_system: 'ntfs',
        total_bytes: 1_000 * GB,
        free_bytes: 240 * GB,
        used_bytes: 760 * GB,
        is_removable: false,
        status: 'available',
        scan_method: 'mft',
        is_elevated: true,
      },
      {
        volume_id: '22222222-2222-2222-2222-222222222222',
        mount_point: 'D:\\',
        label: 'Data',
        file_system: 'ntfs',
        total_bytes: 2_000 * GB,
        free_bytes: 1_100 * GB,
        used_bytes: 900 * GB,
        is_removable: false,
        status: 'available',
        scan_method: null,
        is_elevated: false,
      },
    ];
  }

  private seedTree(): void {
    // Every directory total equals the sum of its children, exactly like the
    // tree produced by the scanner, so a parent is never smaller than its
    // contents.
    this.addRow(0, null, 'C:\\', 'directory', 760 * GB, {
      child_count: 9,
      file_count: 180_000,
    });
    this.addRow(1, 0, 'Windows', 'directory', 180 * GB, {
      child_count: 4,
      file_count: 90_000,
    });
    this.addRow(2, 0, 'Program Files', 'directory', 90 * GB, {
      child_count: 4,
      file_count: 30_000,
    });
    this.addRow(3, 0, 'Users', 'directory', 22_548 * MB, {
      child_count: 4,
      file_count: 55_000,
    });
    this.addRow(4, 0, 'pagefile.sys', 'file', 16 * GB, {
      attributes: {
        read_only: false,
        hidden: true,
        system: true,
        compressed: false,
        sparse: false,
        reparse: false,
      },
    });
    this.addRow(5, 0, 'hiberfil.sys', 'file', 12 * GB, {
      attributes: {
        read_only: false,
        hidden: true,
        system: true,
        compressed: false,
        sparse: false,
        reparse: false,
      },
    });
    this.addRow(6, 0, 'Temp', 'directory', 5 * GB, { child_count: 3 });
    this.addRow(7, 0, 'Windows.old', 'directory', 200 * GB);
    this.addRow(8, 0, 'Recovery', 'directory', 60 * GB);
    this.addRow(9, 0, '$Recycle.Bin', 'directory', 175 * GB - 20 * MB);
    this.addRow(10, 1, 'System32', 'directory', 60 * GB, { child_count: 4 });
    this.addRow(11, 1, 'WinSxS', 'directory', 80 * GB, { child_count: 3 });
    this.addRow(12, 1, 'explorer.exe', 'file', 4 * MB);
    this.addRow(13, 1, 'SysWOW64', 'directory', 40 * GB - 4 * MB);
    this.addRow(20, 2, 'FreeSpace', 'directory', 40 * MB, { child_count: 1 });
    this.addRow(21, 2, 'Shared', 'directory', 12 * GB);
    this.addRow(22, 3, 'Documents', 'directory', 8 * MB, { child_count: 1 });
    this.addRow(23, 3, 'Downloads', 'directory', 2 * GB, { child_count: 3 });
    this.addRow(24, 3, 'Pictures', 'directory', 12 * MB, { child_count: 1 });
    this.addRow(25, 3, 'AppData', 'directory', 20 * GB);
    this.addRow(30, 6, 'Freespace-cache', 'directory', 3 * GB);
    this.addRow(31, 6, 'crash.dmp', 'file', 2 * GB);
    this.addRow(32, 10, 'notepad.exe', 'file', 200 * MB);
    this.addRow(33, 20, 'freespace-gui.exe', 'file', 40 * MB);
    this.addRow(34, 22, 'report.pdf', 'file', 8 * MB);
    this.addRow(35, 23, 'archive.zip', 'file', 1 * GB);
    this.addRow(36, 24, 'photo.jpg', 'file', 12 * MB);
    this.addRow(37, 23, 'backup-1.bin', 'file', 512 * MB);
    this.addRow(38, 23, 'backup-2.bin', 'file', 512 * MB);
    this.addRow(39, 6, 'Empty folder', 'directory', 0, { child_count: 0 });
    this.addRow(40, 2, 'Common Files', 'directory', 50 * GB);
    this.addRow(41, 2, 'Microsoft', 'directory', 28 * GB - 40 * MB);
    this.addRow(42, 10, 'kernel32.dll', 'file', 20 * GB);
    this.addRow(43, 10, 'ntoskrnl.exe', 'file', 25 * GB);
    this.addRow(44, 10, 'drivers', 'directory', 15 * GB - 200 * MB);
    this.addRow(45, 11, 'ManifestCache', 'directory', 30 * GB);
    this.addRow(46, 11, 'TempCache', 'directory', 25 * GB);
    this.addRow(47, 11, 'Backup', 'directory', 25 * GB);

    // The second seeded volume makes the multi-volume preview and tests
    // representative: both roots exist in the mock tree from the start.
    this.addRow(100, null, 'D:\\', 'directory', 900 * GB, {
      child_count: 3,
      file_count: 120_000,
    });
    this.addRow(101, 100, 'Media', 'directory', 500 * GB, {
      child_count: 1,
      file_count: 80_000,
    });
    this.addRow(102, 101, 'Movies', 'directory', 500 * GB, { child_count: 1 });
    this.addRow(103, 102, 'film.mkv', 'file', 500 * GB);
    this.addRow(104, 100, 'Projects', 'directory', 300 * GB, { child_count: 1 });
    this.addRow(105, 104, 'project.zip', 'file', 300 * GB);
    this.addRow(106, 100, 'backup.bin', 'file', 100 * GB);
  }

  private seedHandlers(): void {
    this.handlers.set('app_bootstrap', () => this.bootstrap());
    this.handlers.set('app_capabilities', () => this.capabilities());
    this.handlers.set('app_about_info', () => this.bootstrap().about);
    this.handlers.set('app_settings_get', () => this.settings);
    this.handlers.set('app_settings_set', (args) => {
      this.settings = {
        ...this.settings,
        ...(args.patch as UiSettingsPatch),
      };
      // The shell and the settings screen both persist through this command in
      // the real app. The mock emits `settings-changed` only from the settings
      // plug-in so a stale core copy never overwrites a theme chosen there.
      return this.settings;
    });
    this.handlers.set('app_window_state_get', () => ({
      width: 1280,
      height: 800,
      x: null,
      y: null,
      maximized: false,
      monitor_id: null,
    }));
    this.handlers.set('app_window_state_set', () => ({}));
    this.handlers.set('app_navigation_request', () => ({}));
    this.handlers.set('app_elevation_status', () => ({
      elevated: true,
      can_elevate: false,
      reason_key: null,
    }));
    this.handlers.set('volumes_list', () => this.volumes);
    this.handlers.set('volumes_refresh', () => this.volumes);
    this.handlers.set('volumes_current', () => this.volumes[0]);
    this.handlers.set('snapshot_volumes', () => {
      if (this.snapshotVolumeIds.length === 0) return this.volumes;
      return this.volumes.filter((volume) =>
        this.snapshotVolumeIds.includes(volume.volume_id),
      );
    });
    this.handlers.set('volumes_summary', (args) => this.volumeDetail(asString(args.volume_id)));
    this.handlers.set('scan_start', (args) => this.startScan(args));
    this.handlers.set('scan_status', (args) => this.task(asString(args.task_id)));
    this.handlers.set('scan_cancel', (args) => this.cancel(asString(args.task_id)));
    this.handlers.set('scan_sessions', () => []);
    this.handlers.set('tree_root', (args) => {
      const selected = this.snapshotVolumeIds;
      const ids = this.rows
        .filter((row) => row.parent_id === null)
        .filter(
          (row) =>
            selected.length === 0 ||
            this.volumes.some(
              (volume) =>
                selected.includes(volume.volume_id) &&
                row.path.toLowerCase() === volume.mount_point.toLowerCase(),
            ),
        )
        .map((row) => row.id);

      return this.page(ids, args);
    });
    this.handlers.set('tree_children', (args) => {
      const parent = Number(args.parent_id);
      return this.page(this.children.get(parent) ?? [], args);
    });
    this.handlers.set('tree_search', (args) => {
      const query = asString(args.query).toLowerCase();
      const ids = this.snapshotRows
        .filter((row) => row.name.toLowerCase().includes(query))
        .map((row) => row.id);
      return this.page(ids, args);
    });
    this.handlers.set('tree_node', (args) => this.byId.get(Number(args.node_id)));
    this.handlers.set('tree_path', (args) => {
      const ancestors: NodeRow[] = [];
      let parent = this.byId.get(Number(args.node_id))?.parent_id ?? null;
      while (parent !== null) {
        const row = this.byId.get(parent);
        if (row === undefined) break;
        ancestors.unshift(row);
        parent = row.parent_id;
      }
      return ancestors;
    });
    this.handlers.set('top_files_page', (args) => this.topFiles(args));
    this.handlers.set('types_summary', (args) => this.typesSummary(args));
    this.handlers.set('types_files_page', (args) => this.typesFiles(args));
    this.handlers.set('duplicates_start', (args) => this.startDuplicates(args));
    this.handlers.set('duplicates_status', (args) => this.duplicateStatus(asString(args.task_id)));
    this.handlers.set('duplicates_groups_page', (args) => this.duplicateGroups(args));
    this.handlers.set('duplicates_group', (args) => this.duplicateGroup(args));
    this.handlers.set('duplicates_cancel', () => ({}));
    this.handlers.set('cleanup_candidates', (args) => this.cleanupCandidates(args));
    this.handlers.set('cleanup_empty_dirs_page', (args) => this.emptyDirs(args));
    this.handlers.set('snapshots_list', () => this.snapshots);
    this.handlers.set('snapshots_create', (args) => this.createSnapshot(args));
    this.handlers.set('snapshots_compare', (args) => this.compareSnapshots(args));
    this.handlers.set('snapshots_diff_page', (args) => this.snapshotDiffPage(args));
    this.handlers.set('snapshots_delete', (args) => {
      this.snapshots = this.snapshots.filter(
        (snapshot) => snapshot.snapshot_id !== asString(args.snapshot_id),
      );
      return {};
    });
    this.handlers.set('snapshots_status', (args) => ({
      task_id: asString(args.task_id),
      kind: 'snapshot_compare',
      state: 'completed',
      volume_id: null,
      volume_ids: [],
      progress: null,
      started_at: Date.now(),
      finished_at: Date.now(),
      cancelable: false,
      error: null,
    }));
    this.handlers.set('operation_preview', (args) => this.preview(args));
    this.handlers.set('operation_confirm', (args) => this.confirm(args));
    this.handlers.set('operation_status', (args) =>
      this.operationProgress(asString(args.operation_id)),
    );
    this.handlers.set('operation_cancel', (args) => {
      const operation = this.operationProgress(asString(args.operation_id));
      operation.state = 'cancelled';
      return {};
    });
    this.handlers.set('operation_undo', (args) => ({
      operation_id: asString(args.operation_id),
    }));
    this.handlers.set('journal_page', () => this.journalPage());
    this.handlers.set('journal_entry', (args) => {
      const entry = this.journal.find((item) => item.id === asString(args.entry_id));
      if (entry === undefined) throw mockFailure(mockError('E_NOT_FOUND', 'unknown entry'));
      return entry;
    });
    this.handlers.set('journal_undo', (args) => this.undoJournal(asString(args.entry_id)));

    // Delivery batches 4–6 register their own handlers from separate modules.
    const register: MockRegister = (command, handler) => {
      this.handlers.set(command, handler);
    };
    registerFilesystemMocks(register);
    registerInspectionMocks(register);
    registerSettingsMocks(register);
  }

  private topFiles(args: Record<string, unknown>): unknown {
    const limit = Number(args.limit ?? 100);
    const fileIds = this.snapshotRows
      .filter((row) => row.kind === 'file')
      .sort((left, right) => right.logical_size - left.logical_size)
      .slice(0, limit)
      .map((row) => row.id);

    return this.page(fileIds, args);
  }

  private typesSummary(args: Record<string, unknown>): TypeSummary[] {
    const filter = (args.filter as FilterSet | undefined) ?? emptyFilter();
    if (args.group_by === 'category') {
      const groups = new Map<string, { count: number; logical: number; onDisk: number; physical: number }>();
      let total = 0;
      for (const row of this.snapshotRows) {
        if (row.kind !== 'file') continue;
        if (filter.text !== '' && !row.name.toLowerCase().includes(filter.text.toLowerCase())) continue;
        const category = categoryKey(row.name);
        const entry = groups.get(category) ?? { count: 0, logical: 0, onDisk: 0, physical: 0 };
        entry.count += 1;
        entry.logical += row.logical_size;
        entry.onDisk += row.on_disk_size;
        entry.physical += row.physical_size;
        groups.set(category, entry);
        total += row.logical_size;
      }
      // Directories form their own category, but volume roots do not belong
      // to it. Directory sizes are aggregates of their children, so counting
      // them here would double count file bytes.
      const folders = this.snapshotRows.filter(
        (row) => row.kind === 'directory' && row.parent_id !== null,
      ).length;
      if (folders > 0) {
        groups.set('folder', { count: folders, logical: 0, onDisk: 0, physical: 0 });
      }
      return [...groups.entries()]
        .map(([category, entry]) => ({
          extension: '',
          category,
          count: entry.count,
          logical_size: entry.logical,
          on_disk_size: entry.onDisk,
          physical_size: entry.physical,
          share: total === 0 ? 0 : entry.logical / total,
        }))
        .sort((left, right) => right.logical_size - left.logical_size);
    }

    const groups = new Map<string, { count: number; logical: number; onDisk: number; physical: number }>();
    let total = 0;
    for (const row of this.snapshotRows) {
      if (row.kind !== 'file') continue;
      if (filter.text !== '' && !row.name.toLowerCase().includes(filter.text.toLowerCase())) continue;
      const extension = row.name.includes('.')
        ? (row.name.split('.').pop() ?? '').toLowerCase()
        : '';
      const entry = groups.get(extension) ?? { count: 0, logical: 0, onDisk: 0, physical: 0 };
      entry.count += 1;
      entry.logical += row.logical_size;
      entry.onDisk += row.on_disk_size;
      entry.physical += row.physical_size;
      groups.set(extension, entry);
      total += row.logical_size;
    }

    return [...groups.entries()]
      .map(([extension, entry]) => ({
        extension,
        category: categoryKey(`file.${extension}`),
        count: entry.count,
        logical_size: entry.logical,
        on_disk_size: entry.onDisk,
        physical_size: entry.physical,
        share: total === 0 ? 0 : entry.logical / total,
      }))
      .sort((left, right) => right.logical_size - left.logical_size);
  }

  private typesFiles(args: Record<string, unknown>): unknown {
    const type = asString(args.file_type);
    const category = type.startsWith('category:') ? type.slice('category:'.length) : null;
    const extension = category === null ? type.replace('.', '').toLowerCase() : null;
    const ids = this.snapshotRows
      .filter((row) => {
        if (category === 'folder') {
          return row.kind === 'directory' && row.parent_id !== null;
        }
        if (category !== null) return row.kind === 'file' && categoryKey(row.name) === category;
        if (row.kind !== 'file') return false;
        const found = row.name.includes('.')
          ? (row.name.split('.').pop() ?? '').toLowerCase()
          : '';
        return found === extension;
      })
      .map((row) => row.id);

    return this.page(ids, args);
  }

  private startDuplicates(args: Record<string, unknown>): { task_id: string } {
    const request = (args.request as DuplicateStartRequest | undefined) ?? {
      path: 'C:\\',
      min_size: 1,
      algorithm: 'sha256',
      keep_rule: 'oldest',
      exclusions: [],
    };
    const bySize = new Map<number, NodeRow[]>();
    for (const row of this.snapshotRows) {
      if (row.kind !== 'file' || row.logical_size < request.min_size) continue;
      const bucket = bySize.get(row.logical_size) ?? [];
      bucket.push(row);
      bySize.set(row.logical_size, bucket);
    }

    const groups: DuplicateGroupView[] = [...bySize.values()]
      .filter((bucket) => bucket.length >= 2)
      .map((members, index) => {
        const groupMembers = members.map((row, position) => ({
          node_id: row.id,
          path: row.path,
          name: row.name,
          size: row.logical_size,
          modified_at: row.modified_at ?? 0,
          selected_for_deletion: position > 0,
          verified: true,
          reason: position === 0 ? 'keep (oldest)' : 'duplicate of the first copy',
        }));
        const size = members[0]?.logical_size ?? 0;
        return {
          id: `group-${index + 1}`,
          algorithm: request.algorithm,
          hash: `${index + 1}`.padStart(64, 'a'),
          size,
          members: groupMembers,
          keep_node_id: members[0]?.id ?? null,
          reclaimable_bytes: size * (members.length - 1),
          stale: false,
          deletable: true,
        };
      });

    const taskId = `duplicates-${Date.now()}`;
    const task: DuplicateTask = {
      task_id: taskId,
      state: 'completed',
      groups_count: groups.length,
      candidates: groups.reduce((sum, group) => sum + group.members.length, 0),
      hashed: groups.reduce((sum, group) => sum + group.members.length, 0),
      reclaimable_bytes: groups.reduce((sum, group) => sum + group.reclaimable_bytes, 0),
      warnings: [],
      error: null,
    };
    this.duplicateSearches.set(taskId, { task, groups });
    this.emit<DuplicateProgressEvent>('duplicates-progress', {
      progress: { processed: task.hashed, total: task.hashed },
      phase_key: 'duplicates.phase.hashing',
    }, taskId);
    this.emit<DuplicateCompletedEvent>('duplicates-completed', {
      task_id: taskId,
      state: 'completed',
      groups_count: groups.length,
      reclaimable_bytes: task.reclaimable_bytes,
      error: null,
    }, taskId);

    return { task_id: taskId };
  }

  private duplicateStatus(taskId: string): DuplicateTask {
    const search = this.duplicateSearches.get(taskId);
    if (search === undefined) throw mockFailure(mockError('E_NOT_FOUND', 'unknown task'));

    return search.task;
  }

  private duplicateGroups(args: Record<string, unknown>): DuplicateGroupPage {
    const search = this.duplicateSearches.get(asString(args.task_id));
    if (search === undefined) throw mockFailure(mockError('E_NOT_FOUND', 'unknown task'));
    const offset = args.cursor === null || args.cursor === undefined ? 0 : Number(args.cursor);
    const pageSize = Number(args.page_size ?? 500);
    const groups = search.groups.slice(offset, offset + pageSize);

    return {
      cursor: args.cursor === undefined ? null : asString(args.cursor),
      next_cursor: offset + pageSize < search.groups.length ? String(offset + pageSize) : null,
      groups,
      total_estimate: search.groups.length,
    };
  }

  private duplicateGroup(args: Record<string, unknown>): DuplicateGroupView {
    const search = this.duplicateSearches.get(asString(args.task_id));
    const group = search?.groups.find((item) => item.id === asString(args.group_id));
    if (group === undefined) throw mockFailure(mockError('E_NOT_FOUND', 'unknown group'));

    return group;
  }

  private cleanupCandidates(args: Record<string, unknown>): CleanupCandidateView[] {
    const olderThan = Number(args.older_than_days ?? 180);
    const candidates: CleanupCandidateView[] = [];
    for (const row of this.snapshotRows) {
      if (row.kind !== 'file' && row.kind !== 'directory') continue;
      const name = row.name.toLowerCase();
      const category = name.includes('cache')
        ? 'cache'
        : name.endsWith('.dmp') || name.endsWith('.log')
          ? 'log'
          : name.endsWith('.bak')
            ? 'backup'
            : name.includes('temp')
              ? 'temp'
              : null;
      if (category === null) continue;
      const risk = category === 'backup' ? 'high' : category === 'log' ? 'medium' : 'low';
      candidates.push({
        node_id: row.id,
        path: row.path,
        name: row.name,
        category,
        reason_key: `cleanup.reason.${category}`,
        reclaimable_bytes: row.logical_size,
        risk,
        recommended_action: risk === 'low' ? 'delete' : 'review',
      });
    }
    if (olderThan > 0) {
      return candidates;
    }

    return candidates;
  }

  private emptyDirs(args: Record<string, unknown>): unknown {
    const ids = this.snapshotRows
      .filter((row) => row.kind === 'directory' && row.child_count === 0)
      .map((row) => row.id);

    return this.page(ids, args);
  }

  private createSnapshot(args: Record<string, unknown>): unknown {
    const request = (args.request as { path?: string | null; out?: string | null } | undefined) ?? {};
    const id = `snap-${Date.now()}`;
    const snapshot: SnapshotSummary = {
      snapshot_id: id,
      path: request.out ?? `C:\\FreeSpace\\data\\snapshots\\${id}.fssnap`,
      volume_id: this.volumes[0]?.volume_id ?? 'volume',
      mount_point: this.volumes[0]?.mount_point ?? 'C:\\',
      created_at: new Date().toISOString(),
      node_count: this.snapshotRows.length,
      file_size: this.snapshotRows.length * 128,
    };
    this.snapshots = [snapshot, ...this.snapshots];

    return {
      task_id: `snapshot-${Date.now()}`,
      snapshot_id: id,
      path: snapshot.path,
      node_count: snapshot.node_count,
    };
  }

  private compareSnapshots(args: Record<string, unknown>): { task_id: string } {
    const base = this.snapshots.find((item) => item.snapshot_id === asString(args.base_id));
    const target = this.snapshots.find((item) => item.snapshot_id === asString(args.target_id));
    if (base === undefined || target === undefined) {
      throw mockFailure(mockError('E_NOT_FOUND', 'unknown snapshot'));
    }
    if (base.volume_id !== target.volume_id) {
      throw mockFailure(mockError('E_UNSUPPORTED', 'snapshots are from different volumes'));
    }

    const rows: SnapshotDiffRow[] = [
      {
        path: 'Users\\Downloads\\archive.zip',
        change: 'added',
        is_directory: false,
        size_delta: 1_073_741_824,
        mtime_delta: 3600,
        base_size: 0,
        target_size: 1_073_741_824,
      },
      {
        path: 'Temp\\crash.dmp',
        change: 'removed',
        is_directory: false,
        size_delta: -2_147_483_648,
        mtime_delta: 0,
        base_size: 2_147_483_648,
        target_size: 0,
      },
      {
        path: 'Users\\Documents\\report.pdf',
        change: 'changed',
        is_directory: false,
        size_delta: 4_194_304,
        mtime_delta: 60,
        base_size: 4_194_304,
        target_size: 8_388_608,
      },
    ];
    const taskId = `compare-${Date.now()}`;
    this.comparisons.set(taskId, {
      task_id: taskId,
      cursor: null,
      next_cursor: null,
      rows,
      total_estimate: rows.length,
      added: 1,
      removed: 1,
      changed: 1,
      total_growth_bytes: -1_069_547_520,
      growth_by_directory: [
        { path: 'Users\\Downloads', delta: 1_073_741_824 },
        { path: 'Temp', delta: -2_147_483_648 },
      ],
    });

    return { task_id: taskId };
  }

  private snapshotDiffPage(args: Record<string, unknown>): SnapshotDiffPage {
    const page = this.comparisons.get(asString(args.task_id));
    if (page === undefined) throw mockFailure(mockError('E_NOT_FOUND', 'unknown comparison'));
    const sort = args.sort as
      | { key: SnapshotDiffSortKey; descending: boolean }
      | undefined;
    const rows = [...page.rows].sort((left, right) => {
      const key = sort?.key ?? 'size_delta';
      const comparison =
        key === 'path'
          ? left.path.localeCompare(right.path)
          : key === 'change'
            ? left.change.localeCompare(right.change)
            : Math.abs(left.size_delta) - Math.abs(right.size_delta);
      return sort?.descending === false ? comparison : -comparison;
    });
    const offset = args.cursor === null || args.cursor === undefined ? 0 : Number(args.cursor);
    const pageSize = Number(args.page_size ?? 500);

    return {
      ...page,
      rows: rows.slice(offset, offset + pageSize),
      cursor: args.cursor === undefined ? null : asString(args.cursor),
      next_cursor: offset + pageSize < rows.length ? String(offset + pageSize) : null,
    };
  }

  private preview(args: Record<string, unknown>): OperationPreview {
    const request = args.request as OperationPreviewRequest;
    const targets = request.targets.map((path) => {
      const row = this.rows.find((item) => item.path === path);
      return {
        path,
        kind: row?.kind ?? 'file',
        size: row?.logical_size ?? 1024,
        accessible: row?.accessible ?? true,
        is_system: row?.attributes.system ?? false,
      };
    });
    const total = targets.reduce((sum, target) => sum + target.size, 0);
    const mode = request.delete_mode ?? 'recycle_bin';
    const preview: OperationPreview = {
      preview_id: `preview-${Date.now()}`,
      kind: request.kind,
      targets,
      total_size: total,
      delete_mode: mode,
      reversible: mode !== 'permanent',
      requires_elevation: targets.some((target) => target.is_system),
      warnings:
        mode === 'permanent'
          ? ['confirm.warning.permanent']
          : targets.some((target) => target.is_system)
            ? ['confirm.warning.protected', 'confirm.warning.elevation']
            : [],
      expires_at: Date.now() + 600_000,
      confirmation_token: `token-${Date.now()}`,
    };
    this.previews.set(preview.preview_id, { request, preview });

    return preview;
  }

  private confirm(args: Record<string, unknown>): OperationAccepted {
    const preview = this.previews.get(asString(args.preview_id));
    if (preview === undefined) throw mockFailure(mockError('E_PREVIEW_EXPIRED', 'unknown preview'));
    const operationId = `operation-${Date.now()}`;
    const mode = (args.delete_mode as string | null | undefined) ?? preview.preview.delete_mode ?? 'recycle_bin';
    const entry: JournalEntryView = {
      id: String(this.journal.length + 1),
      operation_id: operationId,
      occurred_at: Date.now(),
      user: 'mock-user',
      kind: preview.request.kind,
      targets_count: preview.preview.targets.length,
      total_size: preview.preview.total_size,
      result: 'success',
      reversible: mode !== 'permanent',
      error: null,
    };
    this.journal = [entry, ...this.journal];
    const progress: OperationProgress = {
      operation_id: operationId,
      state: 'completed',
      processed: preview.preview.targets.length,
      total: preview.preview.targets.length,
      current_target: null,
      bytes_per_second: null,
      errors: [],
      undo_available: entry.reversible,
      journal_entry_id: entry.id,
    };
    this.operations.set(operationId, progress);
    this.previews.delete(preview.preview.preview_id);
    this.emit<OperationCompletedEvent>('operation-completed', {
      operation_id: operationId,
      state: 'completed',
      journal_entry_id: entry.id,
      undo_available: entry.reversible,
      error: null,
    });

    return { operation_id: operationId };
  }

  private operationProgress(operationId: string): OperationProgress {
    const progress = this.operations.get(operationId);
    if (progress === undefined) throw mockFailure(mockError('E_NOT_FOUND', 'unknown operation'));

    return progress;
  }

  private journalPage(): JournalPage {
    return {
      cursor: null,
      next_cursor: null,
      rows: this.journal,
      total_estimate: this.journal.length,
    };
  }

  private undoJournal(entryId: string): OperationAccepted {
    const entry = this.journal.find((item) => item.id === entryId);
    if (entry === undefined) throw mockFailure(mockError('E_NOT_FOUND', 'unknown entry'));
    entry.reversible = false;

    return { operation_id: entry.operation_id };
  }

  private capabilities(): AppCapabilities {
    return {
      commands: [...COMMANDS],
      events: [...EVENTS],
      csp_version: 1,
      platform_capabilities: this.platformCapabilities,
    };
  }

  private bootstrap(): AppBootstrap {
    return {
      ipc_version: 1,
      app_version: '1.3.0',
      os: 'windows',
      webview_version: '130.0.2849.68',
      settings: this.settings,
      window_state: {
        width: 1280,
        height: 800,
        x: null,
        y: null,
        maximized: false,
        monitor_id: 'primary',
      },
      capabilities: this.capabilities(),
      about: {
        version: '1.3.0',
        build: 'mock',
        webview_version: '130.0.2849.68',
        licenses: [
          { name: 'Tauri', license: 'MIT / Apache-2.0', url: null },
          { name: 'React', license: 'MIT', url: null },
        ],
        data_dir: 'C:\\FreeSpace\\data',
      },
    };
  }

  private volumeDetail(volumeId: string): VolumeDetail {
    const volume = this.volumes.find((item) => item.volume_id === volumeId);
    if (volume === undefined) {
      throw mockFailure(mockError('E_NOT_FOUND', 'unknown volume'));
    }

    return {
      ...volume,
      cluster_size: 4096,
      used_fraction:
        volume.total_bytes === 0 ? 0 : volume.used_bytes / volume.total_bytes,
    };
  }

  private page(ids: number[], args: Record<string, unknown>): unknown {
    const metric = (args.metric as 'logical' | 'on_disk' | 'physical' | undefined) ?? 'on_disk';
    const sort =
      (args.sort as { key: 'name' | 'path' | 'size' | 'modified' | 'items'; descending: boolean } | undefined) ??
      { key: 'size', descending: true };
    const filter = (args.filter as FilterSet | undefined) ?? emptyFilter();
    const pageSize = Number(args.page_size ?? 500);
    const offset =
      args.cursor === null || args.cursor === undefined
        ? 0
        : Number(asString(args.cursor));

    const sizeOf = (row: NodeRow) =>
      metric === 'logical' ? row.logical_size : metric === 'physical' ? row.physical_size : row.on_disk_size;

    const filtered = ids
      .map((id) => this.byId.get(id))
      .filter((row): row is NodeRow => row !== undefined)
      .filter((row) => {
        if (filter.text !== '' && !row.name.toLowerCase().includes(filter.text.toLowerCase())) {
          return false;
        }
        if (
          filter.extensions.length > 0 &&
          !filter.extensions.some((extension) =>
            row.name.toLowerCase().endsWith(`.${extension.replace('.', '').toLowerCase()}`),
          )
        ) {
          return false;
        }
        if (filter.accessible_only && !row.accessible) return false;
        return !(filter.min_size !== null && sizeOf(row) < filter.min_size);
      });

    filtered.sort((left, right) => {
      const comparison =
        sort.key === 'name'
          ? left.name.localeCompare(right.name)
          : sort.key === 'path'
            ? left.path.localeCompare(right.path)
            : sort.key === 'modified'
              ? (left.modified_at ?? 0) - (right.modified_at ?? 0)
              : sort.key === 'items'
                ? left.child_count - right.child_count
                : sizeOf(left) - sizeOf(right);

      return sort.descending ? -comparison : comparison;
    });

    const rows = filtered.slice(offset, offset + pageSize);
    const next = offset + pageSize < filtered.length ? String(offset + pageSize) : null;

    return {
      generation: this.generation,
      parent_id: args.parent_id === undefined ? null : Number(args.parent_id),
      cursor: args.cursor === undefined ? null : asString(args.cursor),
      next_cursor: next,
      rows,
      total_estimate: filtered.length,
      metric,
      sort,
      stale_cursor: false,
    };
  }

  private startScan(args: Record<string, unknown>): unknown {
    const taskId = `mock-scan-${this.tasks.length + 1}`;
    const targetGeneration = this.generation + 1;
    const request = (args.request ?? args) as {
      paths?: unknown;
      path?: unknown;
      merge?: unknown;
    };
    const paths = Array.isArray(request.paths)
      ? request.paths.filter((path): path is string => typeof path === 'string')
      : [asString(request.path, 'C:\\')];
    const scanned = paths.length === 0 ? ['C:\\'] : paths;
    const scannedVolumeIds: string[] = [];
    for (const path of scanned) {
      const volume = this.volumes.find((item) =>
        path.toLowerCase().startsWith(item.mount_point.toLowerCase()),
      );
      if (volume !== undefined && !scannedVolumeIds.includes(volume.volume_id)) {
        scannedVolumeIds.push(volume.volume_id);
      }
    }
    if (scannedVolumeIds.length === 0 && this.volumes[0] !== undefined) {
      scannedVolumeIds.push(this.volumes[0].volume_id);
    }
    const snapshotVolumeIds =
      request.merge === true ? [...this.snapshotVolumeIds] : [];
    for (const volumeId of scannedVolumeIds) {
      if (!snapshotVolumeIds.includes(volumeId)) snapshotVolumeIds.push(volumeId);
    }
    this.snapshotVolumeIds = snapshotVolumeIds;

    const task: Task = {
      task_id: taskId,
      kind: 'scan',
      state: 'running',
      volume_id: scannedVolumeIds[0] ?? null,
      volume_ids: scannedVolumeIds,
      progress: {
        processed: 0,
        total: 180_000,
        current_path: scanned[0] ?? 'C:\\',
        bytes_per_second: null,
        items_per_second: null,
        phase_key: 'scan.phase.preparing',
        updated_at: Date.now(),
      },
      started_at: Date.now(),
      finished_at: null,
      cancelable: true,
      error: null,
    };
    this.tasks.push(task);
    this.emit('task-queue-changed', { tasks: this.tasks });

    const steps = [0.18, 0.42, 0.68, 0.86, 1] as const;
    steps.forEach((step, index) => {
      const timer = setTimeout(() => {
        this.timers.delete(timer);
        if (task.state === 'cancelled') return;

        const progress: ScanProgressEvent = {
          progress: {
            processed: Math.round(180_000 * step),
            total: 180_000,
          },
          phase_key:
            step >= 1
              ? 'scan.phase.finished'
              : step > 0.6
                ? 'scan.phase.aggregating'
                : 'scan.phase.enumerating',
          current_path: scanned[0] ?? 'C:\\',
          bytes: Math.round(760 * GB * step),
          bytes_per_second: 420 * MB,
        };
        task.progress = {
          processed: progress.progress.processed,
          total: 180_000,
          current_path: progress.current_path,
          bytes_per_second: progress.bytes_per_second,
          items_per_second: 12_000,
          phase_key: progress.phase_key,
          updated_at: Date.now(),
        };
        this.emit('scan-progress', progress, taskId);

        if (step === 1) {
          task.state = 'completed';
          task.finished_at = Date.now();
          task.cancelable = false;
          this.generation = targetGeneration;
          this.emit(
            'scan-completed',
            {
              task_id: taskId,
              state: 'completed',
              generation: targetGeneration,
              summary: {
                nodes_count: 180_000,
                method: 'mft',
                methods: scanned.map(() => 'mft'),
                roots: scanned,
                volumes: this.volumes
                  .filter((volume) => scannedVolumeIds.includes(volume.volume_id))
                  .map((volume) => volume.mount_point),
                warnings: [],
                issues_count: 0,
                elapsed_ms: 2_400,
              },
              error: null,
            },
            taskId,
          );
          this.emit('task-queue-changed', { tasks: this.tasks });
        }
      }, 120 * (index + 1));
      this.timers.add(timer);
    });

    return { task_id: taskId, generation: targetGeneration };
  }

  private task(taskId: string): Task {
    const task = this.tasks.find((item) => item.task_id === taskId);
    if (task === undefined) {
      throw mockFailure(mockError('E_NOT_FOUND', 'unknown task'));
    }

    return task;
  }

  private cancel(taskId: string): unknown {
    const task = this.task(taskId);
    if (task.state !== 'running') return {};

    task.state = 'cancelled';
    task.finished_at = Date.now();
    task.cancelable = false;
    this.emit(
      'scan-completed',
      {
        task_id: taskId,
        state: 'cancelled',
        generation: this.generation,
        summary: null,
        error: null,
      },
      taskId,
    );

    return {};
  }
}

/** Creates the default in-memory transport. */
export function createMockTransport(): MockTransport {
  return new MockTransport();
}
