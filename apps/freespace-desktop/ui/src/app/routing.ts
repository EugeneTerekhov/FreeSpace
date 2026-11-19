import type { ScreenId } from '../api/dto';

/** The six sidebar groups of the 002 recomposition. */
export type ScreenGroup =
  | 'overview'
  | 'analysis'
  | 'cleanup'
  | 'operations'
  | 'tools'
  | 'system';

export type ScreenCapability =
  | 'scan'
  | 'browse'
  | 'filter'
  | 'delete'
  | 'journal'
  | 'settings'
  | 'export';

export interface ScreenMeta {
  id: ScreenId;
  group: ScreenGroup;
  titleKey: string;
  requiresVolume: boolean;
  requiresScan: boolean;
  capabilities: ScreenCapability[];
  cliAnalog: string | null;
}

export interface NavGroup {
  id: ScreenGroup;
  titleKey: string;
  screens: readonly ScreenId[];
}

/** Group order and membership; the sidebar renders exactly this list. */
export const GROUPS: readonly NavGroup[] = [
  { id: 'overview', titleKey: 'nav.group.overview', screens: ['overview'] },
  { id: 'analysis', titleKey: 'nav.group.analysis', screens: ['tree', 'analysis', 'duplicates'] },
  { id: 'cleanup', titleKey: 'nav.group.cleanup', screens: ['cleanup'] },
  { id: 'operations', titleKey: 'nav.group.operations', screens: ['journal', 'shred', 'monitor'] },
  { id: 'tools', titleKey: 'nav.group.tools', screens: ['inspect', 'fileops'] },
  { id: 'system', titleKey: 'nav.group.system', screens: ['settings'] },
];

/**
 * The 11 screens in navigation order.
 *
 * Screens absorbed by the recomposition stay visible as tabs or sections:
 * disks and scan → overview; size details → tree; top files, types and owners →
 * analysis; snapshots → cleanup; metadata, archives, unlock and long paths →
 * inspect; rename and permissions → fileops; elevation and about → settings.
 * Report export is a global action, not a screen.
 */
export const SCREENS: readonly ScreenMeta[] = [
  {
    id: 'overview',
    group: 'overview',
    titleKey: 'nav.overview',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['scan', 'export'],
    cliAnalog: 'drives, scan',
  },
  {
    id: 'tree',
    group: 'analysis',
    titleKey: 'nav.tree',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['browse', 'filter', 'delete', 'export'],
    cliAnalog: 'tree, filter',
  },
  {
    id: 'analysis',
    group: 'analysis',
    titleKey: 'nav.analysis',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete', 'export'],
    cliAnalog: 'top, types, owners',
  },
  {
    id: 'duplicates',
    group: 'analysis',
    titleKey: 'nav.duplicates',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete', 'export'],
    cliAnalog: 'duplicates',
  },
  {
    id: 'cleanup',
    group: 'cleanup',
    titleKey: 'nav.cleanup',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete'],
    cliAnalog: 'cleanup, snapshot',
  },
  {
    id: 'journal',
    group: 'operations',
    titleKey: 'nav.journal',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['journal', 'delete', 'export'],
    cliAnalog: 'journal',
  },
  {
    id: 'shred',
    group: 'operations',
    titleKey: 'nav.shredder',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['delete'],
    cliAnalog: 'shred',
  },
  {
    id: 'monitor',
    group: 'operations',
    titleKey: 'nav.monitor',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['scan'],
    cliAnalog: 'monitor',
  },
  {
    id: 'inspect',
    group: 'tools',
    titleKey: 'nav.inspect',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete', 'export'],
    cliAnalog: 'filter, archives, unlock',
  },
  {
    id: 'fileops',
    group: 'tools',
    titleKey: 'nav.fileops',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete'],
    cliAnalog: 'rename, acl, attrs',
  },
  {
    id: 'settings',
    group: 'system',
    titleKey: 'nav.settings',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['settings'],
    cliAnalog: 'config',
  },
];

export const SCREEN_BY_ID: Readonly<Record<ScreenId, ScreenMeta>> = Object.fromEntries(
  SCREENS.map((screen) => [screen.id, screen]),
) as Record<ScreenId, ScreenMeta>;

/**
 * Screens that no longer exist after the recomposition.
 *
 * Persisted `last_screen` values and `navigation-request` payloads written by an
 * older build still resolve to a live screen instead of dumping the user on the
 * overview.
 */
export const LEGACY_SCREEN_ALIASES: Readonly<Record<string, ScreenId>> = {
  dashboard: 'overview',
  scan: 'overview',
  treemap: 'tree',
  size_details: 'tree',
  top_files: 'analysis',
  types: 'analysis',
  owners: 'analysis',
  snapshots: 'cleanup',
  long_paths: 'inspect',
  metadata: 'inspect',
  archives: 'inspect',
  unlock: 'inspect',
  rename: 'fileops',
  permissions: 'fileops',
  elevation: 'settings',
  about: 'settings',
  export: 'analysis',
};

/** Returns true when a persisted value is a known screen. */
export function isScreenId(value: string): value is ScreenId {
  return SCREENS.some((screen) => screen.id === value);
}

/** Maps a live or legacy screen id to the current registry; `null` if unknown. */
export function normalizeScreenId(value: string): ScreenId | null {
  if (isScreenId(value)) return value;
  return LEGACY_SCREEN_ALIASES[value] ?? null;
}

/** Restores the last screen only when it still maps to the registry. */
export function resolveInitialScreen(
  last: string | null | undefined,
  ready: boolean,
): ScreenId {
  if (ready && last !== null && last !== undefined) {
    const resolved = normalizeScreenId(last);
    if (resolved !== null) return resolved;
  }

  return 'overview';
}

/** Filters screens by sidebar group. */
export function screensInGroup(group: ScreenGroup): ScreenMeta[] {
  return SCREENS.filter((screen) => screen.group === group);
}

/** Navigation entries of one group, ready for the sidebar. */
export function groupEntries(): { group: NavGroup; screens: ScreenMeta[] }[] {
  return GROUPS.map((group) => ({
    group,
    screens: group.screens.map((id) => SCREEN_BY_ID[id]),
  }));
}
