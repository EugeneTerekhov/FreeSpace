import type {
  AboutInfo,
  DuplicateKeepRule,
  ElevationStatus,
  LicenseInfo,
  MonitorEvent,
  MonitorEventFilter,
  MonitorEventPage,
  MonitorStatus,
  OperationPreview,
  OperationProgress,
  PathValidation,
  SettingsPatch,
  SettingsSection,
  SettingsView,
  ShredPattern,
  UiError,
  UiSettings,
  UiSettingsPatch,
} from '../dto';
import type { MockRegister } from './register';

const MB = 1024 * 1024;
const GB = 1024 * MB;
const EVENT_BASE_TIME = 1_700_000_000_000;
const KEEP_RULES: DuplicateKeepRule[] = ['oldest', 'newest', 'shortest-path', 'original-dir'];
const SETTINGS_SECTIONS: SettingsSection[] = [
  'theme',
  'language',
  'density',
  'units',
  'reduced_motion',
  'last_screen',
  'sidebar',
  'column_layouts',
  'saved_filters',
  'all',
];

function mockError(code: UiError['code'], details: string): UiError {
  const category =
    code === 'E_VALIDATION'
      ? 'validation'
      : code === 'E_ACCESS_DENIED' || code === 'E_ELEVATION_DENIED'
        ? 'access'
        : code === 'E_NOT_FOUND' || code === 'E_IO'
          ? 'io'
          : code === 'E_BUSY'
            ? 'operation'
            : 'internal';

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

function failure(code: UiError['code'], details: string): Error {
  return Object.assign(new Error(details), mockError(code, details));
}

function asString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

function asStringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function defaultUi(): UiSettings {
  return {
    theme: 'system',
    language: 'ru',
    density: 'comfortable',
    units: 'binary',
    reduced_motion: 'system',
    last_screen: 'overview',
    sidebar_collapsed: false,
    column_layouts: {},
    saved_filters: {},
    schema_version: 1,
  };
}

function defaultView(): SettingsView {
  return {
    language: 'ru',
    theme: 'system',
    units: 'binary',
    exclusions: ['**\\node_modules'],
    duplicate_keep_rule: 'oldest',
    outdated_threshold_days: 180,
    monitor_threshold_bytes: GB,
    monitor_interval_secs: 60,
    quarantine_retention_days: 30,
    context_menu_enabled: false,
    confirm_destructive_actions: true,
    ui: defaultUi(),
  };
}

function mirror(view: SettingsView): void {
  view.language = view.ui.language;
  view.theme = view.ui.theme;
  view.units = view.ui.units;
}

function licenses(): LicenseInfo[] {
  return [
    { name: 'Tauri', license: 'MIT / Apache-2.0', url: null },
    { name: 'React', license: 'MIT', url: null },
    { name: 'lucide-react', license: 'ISC', url: null },
    { name: 'Rust standard library', license: 'MIT / Apache-2.0', url: null },
  ];
}

function monitorEvents(threshold: number): MonitorEvent[] {
  const samples: {
    process: string;
    processId: number;
    bytes: number;
    paths: string[];
    offset: number;
  }[] = [
    {
      process: 'explorer.exe',
      processId: 1420,
      bytes: 64 * MB,
      paths: ['C:\\Users\\Public\\Documents\\report.tmp'],
      offset: 0,
    },
    {
      process: 'freespace-gui.exe',
      processId: 4242,
      bytes: 2 * GB,
      paths: ['C:\\FreeSpace\\data\\journal.db', 'C:\\FreeSpace\\data\\cache\\index.bin'],
      offset: 1200,
    },
    {
      process: 'backup-agent.exe',
      processId: 8080,
      bytes: 512 * MB,
      paths: ['D:\\Backups\\nightly\\system-image.vhdx'],
      offset: 2400,
    },
  ];

  return samples.map((sample, index) => ({
    sequence: index + 1,
    timestamp: EVENT_BASE_TIME + sample.offset * 1000,
    process_id: sample.processId,
    process: sample.process,
    bytes: sample.bytes,
    paths: sample.paths,
    threshold_exceeded: sample.bytes >= threshold,
  }));
}

function matchesFilter(event: MonitorEvent, filter: MonitorEventFilter): boolean {
  const process = filter.process?.trim().toLowerCase() ?? '';
  const path = filter.path?.trim().toLowerCase() ?? '';
  if (process !== '' && !event.process.toLowerCase().includes(process)) return false;
  if (path !== '' && !event.paths.some((item) => item.toLowerCase().includes(path))) {
    return false;
  }
  if (filter.min_bytes !== null && filter.min_bytes !== undefined) {
    if (event.bytes < filter.min_bytes) return false;
  }
  if (filter.threshold_only === true && !event.threshold_exceeded) return false;

  return true;
}

function splitTargets(value: unknown): string[] {
  if (Array.isArray(value)) {
    return asStringList(value).map((item) => item.trim()).filter(Boolean);
  }

  return asString(value)
    .split(/\r?\n|;/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function isIntegerBetween(value: number, min: number, max: number): boolean {
  return Number.isInteger(value) && value >= min && value <= max;
}

/**
 * US6 in-memory handlers: shredder, write monitor, elevation, settings and
 * About.
 *
 * All state is created per `MockTransport` instance, so tests and the browser
 * preview never share counters or previews. Event metadata is deterministic to
 * keep screenshots stable.
 */
export function registerSettingsMocks(register: MockRegister): void {
  const view = defaultView();
  let elevated = false;
  let elevationReason: string | null = 'elevation.protected_hint';
  let previewCounter = 0;
  let operationCounter = 0;
  let monitorCounter = 0;

  const previews = new Map<
    string,
    { targets: string[]; passes: number; pattern: ShredPattern; token: string }
  >();
  const operations = new Map<string, OperationProgress>();
  const monitors = new Map<string, { status: MonitorStatus; events: MonitorEvent[] }>();

  const applyPatch = (patch: SettingsPatch): SettingsView => {
    if (patch.ui !== undefined) {
      const uiPatch: UiSettingsPatch = patch.ui;
      view.ui = { ...view.ui, ...uiPatch };
    }
    if (patch.language !== undefined) view.ui.language = patch.language;
    if (patch.theme !== undefined) view.ui.theme = patch.theme;
    if (patch.units !== undefined) view.ui.units = patch.units;

    if (patch.exclusions !== undefined) {
      view.exclusions = asStringList(patch.exclusions)
        .map((item) => item.trim())
        .filter(Boolean);
    }
    if (patch.duplicate_keep_rule !== undefined) {
      if (!KEEP_RULES.includes(patch.duplicate_keep_rule as DuplicateKeepRule)) {
        throw failure('E_VALIDATION', 'unknown duplicate keep rule');
      }
      view.duplicate_keep_rule = patch.duplicate_keep_rule;
    }
    if (patch.outdated_threshold_days !== undefined) {
      if (!isIntegerBetween(patch.outdated_threshold_days, 0, 3650)) {
        throw failure('E_VALIDATION', 'outdated threshold is out of range');
      }
      view.outdated_threshold_days = patch.outdated_threshold_days;
    }
    if (patch.quarantine_retention_days !== undefined) {
      if (!isIntegerBetween(patch.quarantine_retention_days, 0, 3650)) {
        throw failure('E_VALIDATION', 'quarantine retention is out of range');
      }
      view.quarantine_retention_days = patch.quarantine_retention_days;
    }
    if (patch.monitor_threshold_bytes !== undefined) {
      if (!Number.isInteger(patch.monitor_threshold_bytes) || patch.monitor_threshold_bytes < 0) {
        throw failure('E_VALIDATION', 'monitor threshold is out of range');
      }
      view.monitor_threshold_bytes = patch.monitor_threshold_bytes;
    }
    if (patch.monitor_interval_secs !== undefined) {
      if (!isIntegerBetween(patch.monitor_interval_secs, 1, 86_400)) {
        throw failure('E_VALIDATION', 'monitor interval is out of range');
      }
      view.monitor_interval_secs = patch.monitor_interval_secs;
    }
    if (patch.confirm_destructive_actions !== undefined) {
      view.confirm_destructive_actions = patch.confirm_destructive_actions;
    }
    if (patch.context_menu_enabled !== undefined) {
      view.context_menu_enabled = patch.context_menu_enabled;
    }

    mirror(view);

    return clone(view);
  };

  const reset = (section: SettingsSection): SettingsView => {
    if (!SETTINGS_SECTIONS.includes(section)) {
      throw failure('E_VALIDATION', 'unknown settings section');
    }
    const defaults = defaultUi();

    switch (section) {
      case 'theme':
        view.ui.theme = defaults.theme;
        break;
      case 'language':
        view.ui.language = defaults.language;
        break;
      case 'density':
        view.ui.density = defaults.density;
        break;
      case 'units':
        view.ui.units = defaults.units;
        break;
      case 'reduced_motion':
        view.ui.reduced_motion = defaults.reduced_motion;
        break;
      case 'last_screen':
        view.ui.last_screen = defaults.last_screen;
        break;
      case 'sidebar':
        view.ui.sidebar_collapsed = defaults.sidebar_collapsed;
        break;
      case 'column_layouts':
        view.ui.column_layouts = defaults.column_layouts;
        break;
      case 'saved_filters':
        view.ui.saved_filters = defaults.saved_filters;
        break;
      case 'all':
        view.ui = defaults;
        break;
    }

    mirror(view);

    return clone(view);
  };

  register('settings_get', () => clone(view));

  register('settings_set', (args) => {
    const patch = (args.patch ?? {}) as SettingsPatch;
    return applyPatch(patch);
  });

  register('settings_reset', (args) => reset(asString(args.section, 'all') as SettingsSection));

  register('settings_validate_paths', (args) => {
    const paths = asStringList(args.paths);
    const results: PathValidation[] = paths.map((raw) => {
      const path = raw.trim();
      const absolute = /^[a-zA-Z]:\\/.test(path) || /^\\\\[^\\]+\\/.test(path);
      const mask = path.includes('*') || path.includes('?');
      const valid = path !== '' && absolute && !mask;
      const isDirectory = valid && path.endsWith('\\');

      return {
        path,
        valid,
        exists: valid,
        is_directory: isDirectory,
        parent_exists: valid,
        writable: valid,
        error_key: valid ? null : 'settings.path.invalid',
      };
    });

    return results;
  });

  register('shred_preview', (args) => {
    const targets = splitTargets(args.targets);
    const passes = Number(args.passes ?? 1);
    const pattern: ShredPattern = args.pattern === 'random' ? 'random' : 'zeros';

    if (targets.length === 0) {
      throw failure('E_VALIDATION', 'shred.no_files');
    }
    if (!isIntegerBetween(passes, 1, 35)) {
      throw failure('E_VALIDATION', 'shred passes must be between 1 and 35');
    }

    previewCounter += 1;
    const token = `shred-token-${previewCounter}`;
    const preview: OperationPreview = {
      preview_id: `shred-preview-${previewCounter}`,
      kind: 'shred',
      targets: targets.map((path, index) => ({
        path,
        kind: 'file',
        size: (index + 1) * 64 * MB,
        accessible: true,
        is_system: /^[a-zA-Z]:\\Windows(\\|$)/i.test(path),
      })),
      total_size: targets.reduce((sum, _path, index) => sum + (index + 1) * 64 * MB, 0),
      delete_mode: 'permanent',
      reversible: false,
      requires_elevation: false,
      warnings: ['shred.irreversible', 'shred.ssd_warning', 'confirm.warning.permanent'],
      expires_at: EVENT_BASE_TIME + 600_000,
      confirmation_token: token,
    };

    previews.set(preview.preview_id, { targets, passes, pattern, token });

    return preview;
  });

  register('shred_start', (args) => {
    const previewId = asString(args.preview_id);
    const token = asString(args.confirmation_token);
    const prepared = previews.get(previewId);

    if (prepared === undefined) {
      throw failure('E_PREVIEW_EXPIRED', 'unknown or expired shred preview');
    }
    if (token !== prepared.token) {
      throw failure('E_PREVIEW_EXPIRED', 'unknown or expired shred preview');
    }
    if (args.extra_confirmation !== true) {
      throw failure('E_VALIDATION', 'shredding requires the extra confirmation');
    }

    previews.delete(previewId);
    operationCounter += 1;
    const operationId = `shred-operation-${operationCounter}`;
    operations.set(operationId, {
      operation_id: operationId,
      state: 'completed',
      processed: prepared.targets.length,
      total: prepared.targets.length,
      current_target: null,
      bytes_per_second: null,
      errors: [],
      undo_available: false,
      journal_entry_id: `shred-journal-${operationCounter}`,
    });

    return { operation_id: operationId };
  });

  register('shred_status', (args) => {
    const operation = operations.get(asString(args.operation_id));
    if (operation === undefined) {
      throw failure('E_NOT_FOUND', 'unknown shred operation');
    }

    return clone(operation);
  });

  register('shred_cancel', (args) => {
    const operation = operations.get(asString(args.operation_id));
    if (operation === undefined) {
      throw failure('E_NOT_FOUND', 'unknown shred operation');
    }
    if (operation.state === 'completed') {
      throw failure('E_UNSUPPORTED', 'the shredder has passed the point of no return');
    }
    operation.state = 'cancelled';

    return {};
  });

  register('monitor_start', (args) => {
    const threshold = Number(args.threshold ?? view.monitor_threshold_bytes);
    const interval = Number(args.interval ?? view.monitor_interval_secs);
    const exclusions = asStringList(args.exclusions);

    if (!Number.isInteger(threshold) || threshold < 0) {
      throw failure('E_VALIDATION', 'monitor threshold must be a non-negative integer');
    }
    if (!isIntegerBetween(interval, 1, 86_400)) {
      throw failure('E_VALIDATION', 'monitor interval must be between 1 and 86400 seconds');
    }

    monitorCounter += 1;
    const taskId = `monitor-task-${monitorCounter}`;
    const events = monitorEvents(threshold);
    const status: MonitorStatus = {
      task_id: taskId,
      running: true,
      state: 'running',
      source: 'etw',
      source_key: 'monitor.source.etw',
      threshold_bytes: threshold,
      interval_secs: interval,
      exclusions: [...exclusions],
      events_count: events.length,
      bytes_total: events.reduce((sum, event) => sum + event.bytes, 0),
      started_at: EVENT_BASE_TIME,
      finished_at: null,
      last_event_at: events.at(-1)?.timestamp ?? null,
      warnings: [],
      error: null,
    };
    monitors.set(taskId, { status, events });

    return { task_id: taskId };
  });

  register('monitor_stop', (args) => {
    const monitor = monitors.get(asString(args.task_id));
    if (monitor === undefined) {
      throw failure('E_NOT_FOUND', 'unknown monitor task');
    }
    monitor.status = {
      ...monitor.status,
      running: false,
      state: 'completed',
      finished_at: EVENT_BASE_TIME + monitor.events.length * 1000,
    };

    return {};
  });

  register('monitor_status', (args) => {
    const monitor = monitors.get(asString(args.task_id));
    if (monitor === undefined) {
      throw failure('E_NOT_FOUND', 'unknown monitor task');
    }

    return clone(monitor.status);
  });

  register('monitor_events_page', (args) => {
    const taskId = asString(args.task_id);
    const monitor = monitors.get(taskId);
    if (monitor === undefined) {
      throw failure('E_NOT_FOUND', 'unknown monitor task');
    }
    const filter: MonitorEventFilter =
      args.filter === null || args.filter === undefined
        ? { process: null, path: null }
        : (args.filter as MonitorEventFilter);
    const pageSize = Math.max(1, Number(args.page_size ?? 200));
    const offset =
      args.cursor === null || args.cursor === undefined ? 0 : Number(asString(args.cursor));
    const filtered = monitor.events.filter((event) => matchesFilter(event, filter));
    const events = filtered.slice(offset, offset + pageSize);
    const next = offset + pageSize < filtered.length ? String(offset + pageSize) : null;
    const page: MonitorEventPage = {
      task_id: taskId,
      cursor: args.cursor === undefined ? null : asString(args.cursor),
      next_cursor: next,
      events,
      total_estimate: filtered.length,
    };

    return page;
  });

  register('monitor_set_exclusions', (args) => {
    const exclusions = asStringList(args.exclusions);
    for (const [taskId, monitor] of monitors) {
      monitors.set(taskId, {
        ...monitor,
        status: { ...monitor.status, exclusions: [...exclusions] },
      });
    }

    return {};
  });

  register('about_info', (): AboutInfo => {
    return {
      version: '1.1.0',
      build: 'mock-002',
      webview_version: '130.0.2849.68',
      licenses: licenses(),
      data_dir: 'C:\\FreeSpace\\data',
    };
  });

  register('about_licenses', () => licenses());

  register('app_elevation_status', (): ElevationStatus => {
    return {
      elevated,
      can_elevate: !elevated,
      reason_key: elevationReason,
    };
  });

  register('app_elevation_request', (args) => {
    const reason = asString(args.reason_key);
    if (reason === '') {
      throw failure('E_VALIDATION', 'elevation reason is required');
    }
    elevated = true;
    elevationReason = null;

    return { elevated };
  });
}
