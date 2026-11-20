#!/usr/bin/env node
// Generates the golden command fixtures used by `contract_fixtures`.
//
// The shapes mirror `crates/freespace-app/src/dto`; values are illustrative and
// contain no user data. Run from the repository root:
//   node tests/contract/generate_fixtures.mjs

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const dir = join(process.cwd(), 'tests', 'contract', 'fixtures');
mkdirSync(dir, { recursive: true });

// Reads an exported string array from the interface transport registry.
function readInterfaceList(name) {
  const source = readFileSync(
    join(
      process.cwd(),
      'apps',
      'freespace-desktop',
      'ui',
      'src',
      'api',
      'transport.ts',
    ),
    'utf8',
  );
  const match = new RegExp(`export const ${name} = \\[([\\s\\S]*?)\\] as const`).exec(
    source,
  );
  if (match === null) {
    throw new Error(`cannot find ${name} in transport.ts`);
  }

  return [...match[1].matchAll(/'([^']+)'/g)].map((item) => item[1]);
}

const generation = 1;
const sort = { key: 'size', descending: true };
const metric = 'on_disk';

const node = {
  id: 0,
  parent_id: null,
  name: 'C:\\',
  path: 'C:\\',
  kind: 'directory',
  logical_size: 760000000000,
  on_disk_size: 760000000000,
  physical_size: 760000000000,
  ads_size: 0,
  child_count: 6,
  file_count: 180000,
  modified_at: 1763222400000,
  accessed_at: 1763222400000,
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
};

const volume = {
  volume_id: '11111111-1111-1111-1111-111111111111',
  mount_point: 'C:\\',
  label: 'System',
  file_system: 'ntfs',
  total_bytes: 1000000000000,
  free_bytes: 240000000000,
  used_bytes: 760000000000,
  is_removable: false,
  status: 'available',
  scan_method: 'mft',
  is_elevated: true,
};

const settings = {
  theme: 'system',
  language: 'ru',
  density: 'comfortable',
  units: 'binary',
  reduced_motion: 'system',
  last_screen: 'dashboard',
  sidebar_collapsed: false,
  column_layouts: {},
  saved_filters: {},
  schema_version: 1,
};

const windowState = {
  width: 1280,
  height: 800,
  x: null,
  y: null,
  maximized: false,
  monitor_id: 'primary',
};

const capabilities = {
  // The interface registry is the single source of truth; reading it here keeps
  // the golden fixture and `ui/tests/unit/contract.test.ts` in sync.
  commands: readInterfaceList('COMMANDS'),
  events: readInterfaceList('EVENTS'),
  csp_version: 1,
  // Mirrors `freespace_platform::Capabilities::windows()`; the running backend
  // fills this field, the golden fixture documents the Windows contract.
  platform_capabilities: {
    platform: 'windows',
    scan_methods: { mft: true, usn: true, walk: true },
    trash: true,
    permanent: true,
    shred: true,
    acl: true,
    attrs: true,
    ads: true,
    unlock: true,
    context_menu: true,
    monitor: true,
    elevation: true,
    long_paths: true,
    case_sensitive: false,
    max_path: null,
  },
};

const about = {
  version: '2.0.0',
  build: 'golden',
  webview_version: '130.0.2849.68',
  licenses: [{ name: 'Tauri', license: 'MIT / Apache-2.0', url: null }],
  data_dir: 'C:\\FreeSpace\\data',
};

const page = {
  generation,
  parent_id: 0,
  cursor: null,
  next_cursor: null,
  rows: [node],
  total_estimate: 1,
  metric,
  sort,
  stale_cursor: false,
};

const task = {
  task_id: '00000000-0000-4000-8000-000000000001',
  kind: 'scan',
  state: 'running',
  volume_id: volume.volume_id,
  progress: {
    processed: 1000,
    total: 180000,
    current_path: 'C:\\Windows',
    bytes_per_second: 420000000,
    items_per_second: 12000,
    phase_key: 'scan.phase.enumerating',
    updated_at: 1763222400000,
  },
  started_at: 1763222400000,
  finished_at: null,
  cancelable: true,
  error: null,
};

const session = {
  session_id: '00000000-0000-4000-8000-000000000002',
  volume_id: volume.volume_id,
  method: 'directory-walk',
  started_at: 1763222400000,
  finished_at: 1763222402400,
  status: 'completed',
  nodes_count: 180000,
  roots: ['C:\\'],
  issues_count: 0,
};

const envelope = (command, input, result) => ({
  command,
  input,
  result,
  error: null,
});

const fixtures = {
  'app_bootstrap.success.json': envelope(
    'app_bootstrap',
    {},
    {
      ipc_version: 1,
      app_version: '2.0.0',
      os: 'windows',
      webview_version: '130.0.2849.68',
      settings,
      window_state: windowState,
      capabilities,
      about,
    },
  ),
  'app_about_info.success.json': envelope('app_about_info', {}, about),
  'app_capabilities.success.json': envelope('app_capabilities', {}, capabilities),
  'app_elevation_status.success.json': envelope('app_elevation_status', {}, {
    elevated: false,
    can_elevate: true,
    reason_key: 'elevation.reason.mft',
  }),
  'app_navigation_request.success.json': envelope(
    'app_navigation_request',
    { request: { screen: 'tree', node_id: 0, path: 'C:\\' } },
    {},
  ),
  'app_settings_get.success.json': envelope('app_settings_get', {}, settings),
  'app_settings_set.success.json': envelope(
    'app_settings_set',
    { patch: { theme: 'dark' } },
    { ...settings, theme: 'dark' },
  ),
  'app_window_state_get.success.json': envelope(
    'app_window_state_get',
    {},
    windowState,
  ),
  'app_window_state_set.success.json': envelope(
    'app_window_state_set',
    { window_state: windowState },
    {},
  ),
  'scan_cancel.success.json': envelope(
    'scan_cancel',
    { task_id: task.task_id },
    {},
  ),
  'scan_sessions.success.json': envelope('scan_sessions', {}, [session]),
  'scan_status.success.json': envelope(
    'scan_status',
    { task_id: task.task_id },
    task,
  ),
  'tree_children.success.json': envelope(
    'tree_children',
    { generation, parent_id: 0, metric, sort, page_size: 500, cursor: null },
    page,
  ),
  'tree_path.success.json': envelope('tree_path', { generation, node_id: 0 }, []),
  'tree_search.success.json': envelope(
    'tree_search',
    { generation, query: 'Windows', metric, sort, page_size: 500, cursor: null },
    { ...page, parent_id: null },
  ),
  'volumes_current.success.json': envelope('volumes_current', {}, volume),
  'volumes_refresh.success.json': envelope('volumes_refresh', {}, [volume]),
  'volumes_summary.success.json': envelope(
    'volumes_summary',
    { volume_id: volume.volume_id },
    { ...volume, cluster_size: 4096, used_fraction: 0.76 },
  ),
  'app_settings_set.validation.json': {
    command: 'app_settings_set',
    input: { patch: { theme: 'neon' } },
    result: null,
    error: {
      code: 'E_VALIDATION',
      category: 'validation',
      message_key: 'error.validation',
      details: 'unsupported ui schema version 99',
      retryable: false,
      task_id: null,
      journal_entry_id: null,
    },
  },
  'scan_start.busy.json': {
    command: 'scan_start',
    input: { request: { path: 'C:\\' } },
    result: null,
    error: {
      code: 'E_BUSY',
      category: 'operation',
      message_key: 'error.busy',
      details: 'volume already has the scan task 00000000-0000-4000-8000-000000000001',
      retryable: true,
      task_id: null,
      journal_entry_id: null,
    },
  },
  'volumes_summary.not_found.json': {
    command: 'volumes_summary',
    input: { volume_id: 'missing' },
    result: null,
    error: {
      code: 'E_NOT_FOUND',
      category: 'io',
      message_key: 'error.not_found',
      details: 'unknown volume',
      retryable: false,
      task_id: null,
      journal_entry_id: null,
    },
  },
};

for (const [name, value] of Object.entries(fixtures)) {
  writeFileSync(join(dir, name), `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

console.log(`wrote ${Object.keys(fixtures).length} fixtures to ${dir}`);
