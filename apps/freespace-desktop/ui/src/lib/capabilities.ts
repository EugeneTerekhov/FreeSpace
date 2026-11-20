/**
 * Platform capability helpers.
 *
 * The Tauri shell advertises `freespace_platform::Capabilities` in the
 * bootstrap payload. The interface never branches on the operating system
 * itself: it asks these helpers whether an action exists on the running
 * backend and hides it otherwise.
 *
 * `null` means the payload is unknown (older backend or a test without
 * bootstrap). In that case the helpers keep the full Windows behaviour so a
 * missing field can never remove an action that actually works.
 */

import type { DeleteMode, PlatformCapabilities } from '../api/dto';

/** Capabilities of the Windows backend, used by previews and tests. */
export const WINDOWS_CAPABILITIES: PlatformCapabilities = {
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
};

/** Capabilities of the Linux backend, used by previews and tests. */
export const LINUX_CAPABILITIES: PlatformCapabilities = {
  platform: 'linux',
  scan_methods: { mft: false, usn: false, walk: true },
  trash: true,
  permanent: true,
  shred: false,
  acl: false,
  attrs: false,
  ads: false,
  unlock: false,
  context_menu: false,
  monitor: true,
  elevation: true,
  long_paths: false,
  case_sensitive: true,
  max_path: 4096,
};

/** Deletion modes the platform provides, in dialog order. */
export function deleteModes(
  caps: PlatformCapabilities | null | undefined,
): readonly DeleteMode[] {
  if (caps === null || caps === undefined) return ['recycle_bin', 'permanent'];

  const modes: DeleteMode[] = [];
  if (caps.trash) modes.push('recycle_bin');
  if (caps.permanent) modes.push('permanent');

  return modes;
}

/** Preferred deletion mode: the system trash when available. */
export function defaultDeleteMode(
  caps: PlatformCapabilities | null | undefined,
): DeleteMode | null {
  const modes = deleteModes(caps);

  return modes[0] ?? null;
}

/** True when the platform offers Explorer context-menu integration. */
export function canUseContextMenu(
  caps: PlatformCapabilities | null | undefined,
): boolean {
  return caps === null || caps === undefined ? true : caps.context_menu;
}

/** True when the platform has an elevation prompt or state detection. */
export function canElevate(
  caps: PlatformCapabilities | null | undefined,
): boolean {
  return caps === null || caps === undefined ? true : caps.elevation;
}

/** True when lock inspection and release are available (Restart Manager). */
export function canUnlock(
  caps: PlatformCapabilities | null | undefined,
): boolean {
  return caps === null || caps === undefined ? true : caps.unlock;
}

/** True when paths beyond the legacy limit are supported. */
export function canUseLongPaths(
  caps: PlatformCapabilities | null | undefined,
): boolean {
  return caps === null || caps === undefined ? true : caps.long_paths;
}

/** True when opening a file manager is implemented for this backend. */
export function canRevealInFileManager(
  caps: PlatformCapabilities | null | undefined,
): boolean {
  return caps === null || caps === undefined ? true : caps.platform === 'windows';
}

/** Scan methods offered by the backend; `auto` is always allowed. */
export function scanMethods(
  caps: PlatformCapabilities | null | undefined,
): readonly ('auto' | 'mft' | 'walk')[] {
  if (caps !== null && caps !== undefined && !caps.scan_methods.mft) return ['auto', 'walk'];

  return ['auto', 'mft', 'walk'];
}
