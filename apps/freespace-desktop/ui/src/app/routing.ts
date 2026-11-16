import type { ScreenId } from '../api/dto';

export type ScreenBatch = 'core' | 'analysis' | 'filesystem' | 'inspection' | 'settings';

export type ScreenCapability =
  | 'scan'
  | 'browse'
  | 'treemap'
  | 'filter'
  | 'delete'
  | 'journal'
  | 'settings'
  | 'export';

export interface ScreenMeta {
  id: ScreenId;
  batch: ScreenBatch;
  titleKey: string;
  requiresVolume: boolean;
  requiresScan: boolean;
  capabilities: ScreenCapability[];
  cliAnalog: string | null;
}

/** The 24 screens in delivery order. */
export const SCREENS: readonly ScreenMeta[] = [
  {
    id: 'dashboard',
    batch: 'core',
    titleKey: 'nav.dashboard',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['scan'],
    cliAnalog: 'drives',
  },
  {
    id: 'scan',
    batch: 'core',
    titleKey: 'nav.scan',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['scan'],
    cliAnalog: 'scan',
  },
  {
    id: 'tree',
    batch: 'core',
    titleKey: 'nav.tree',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['browse', 'filter', 'delete'],
    cliAnalog: 'tree',
  },
  {
    id: 'treemap',
    batch: 'core',
    titleKey: 'nav.treemap',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['treemap', 'browse'],
    cliAnalog: 'tree',
  },
  {
    id: 'top_files',
    batch: 'analysis',
    titleKey: 'nav.top',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete'],
    cliAnalog: 'top',
  },
  {
    id: 'types',
    batch: 'analysis',
    titleKey: 'nav.types',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete'],
    cliAnalog: 'types',
  },
  {
    id: 'duplicates',
    batch: 'analysis',
    titleKey: 'nav.duplicates',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete'],
    cliAnalog: 'duplicates',
  },
  {
    id: 'cleanup',
    batch: 'analysis',
    titleKey: 'nav.cleanup',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'delete'],
    cliAnalog: 'cleanup',
  },
  {
    id: 'snapshots',
    batch: 'analysis',
    titleKey: 'nav.snapshots',
    requiresVolume: true,
    requiresScan: false,
    capabilities: ['export'],
    cliAnalog: 'snapshot',
  },
  {
    id: 'journal',
    batch: 'filesystem',
    titleKey: 'nav.journal',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['journal', 'delete'],
    cliAnalog: 'journal',
  },
  {
    id: 'size_details',
    batch: 'filesystem',
    titleKey: 'nav.size_details',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['browse'],
    cliAnalog: 'tree',
  },
  {
    id: 'long_paths',
    batch: 'filesystem',
    titleKey: 'nav.long_paths',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['browse', 'delete'],
    cliAnalog: 'tree',
  },
  {
    id: 'rename',
    batch: 'filesystem',
    titleKey: 'nav.rename',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter'],
    cliAnalog: 'rename',
  },
  {
    id: 'permissions',
    batch: 'filesystem',
    titleKey: 'nav.permissions',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter'],
    cliAnalog: 'attrs',
  },
  {
    id: 'metadata',
    batch: 'inspection',
    titleKey: 'nav.metadata',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'export'],
    cliAnalog: 'filter',
  },
  {
    id: 'owners',
    batch: 'inspection',
    titleKey: 'nav.owners',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'export'],
    cliAnalog: 'owners',
  },
  {
    id: 'archives',
    batch: 'inspection',
    titleKey: 'nav.archives',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['filter', 'export'],
    cliAnalog: 'archives',
  },
  {
    id: 'export',
    batch: 'inspection',
    titleKey: 'nav.export',
    requiresVolume: true,
    requiresScan: true,
    capabilities: ['export'],
    cliAnalog: 'export',
  },
  {
    id: 'unlock',
    batch: 'inspection',
    titleKey: 'nav.unlock',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['filter'],
    cliAnalog: 'unlock',
  },
  {
    id: 'shred',
    batch: 'settings',
    titleKey: 'nav.shred',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['delete'],
    cliAnalog: 'shred',
  },
  {
    id: 'monitor',
    batch: 'settings',
    titleKey: 'nav.monitor',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['scan'],
    cliAnalog: 'monitor',
  },
  {
    id: 'elevation',
    batch: 'settings',
    titleKey: 'nav.elevation',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['settings'],
    cliAnalog: null,
  },
  {
    id: 'settings',
    batch: 'settings',
    titleKey: 'nav.settings',
    requiresVolume: false,
    requiresScan: false,
    capabilities: ['settings'],
    cliAnalog: 'config',
  },
  {
    id: 'about',
    batch: 'settings',
    titleKey: 'nav.about',
    requiresVolume: false,
    requiresScan: false,
    capabilities: [],
    cliAnalog: null,
  },
];

export const SCREEN_BY_ID: Readonly<Record<ScreenId, ScreenMeta>> = Object.fromEntries(
  SCREENS.map((screen) => [screen.id, screen]),
) as Record<ScreenId, ScreenMeta>;

/** Returns true when a persisted value is a known screen. */
export function isScreenId(value: string): value is ScreenId {
  return SCREENS.some((screen) => screen.id === value);
}

/** Restores the last screen only when it is still part of the registry. */
export function resolveInitialScreen(
  last: ScreenId | null | undefined,
  ready: boolean,
): ScreenId {
  if (ready && last !== null && last !== undefined && isScreenId(last)) {
    return last;
  }

  return 'dashboard';
}

/** Filters screens by delivery batch. */
export function screensInBatch(batch: ScreenBatch): ScreenMeta[] {
  return SCREENS.filter((screen) => screen.batch === batch);
}
