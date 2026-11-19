import {
  ChevronRight,
  File,
  FileArchive,
  FileText,
  Film,
  Folder,
  FolderOpen,
  HardDrive,
  Image as ImageIcon,
  type LucideIcon,
} from 'lucide-react';

import type { NodeRow } from '../api/dto';
import { t } from '../i18n';
import { Badge } from './Choice';
import styles from './TreeRow.module.css';

/**
 * File-type icon of a tree row (plan.md, «UX-перекомпоновка»; contract §3,
 * `TreeRow`). The set is intentionally small: a folder, its open state, the five
 * media families, the volume and the generic file.
 */
export type TreeIconKind =
  | 'folder'
  | 'folder-open'
  | 'text'
  | 'image'
  | 'video'
  | 'archive'
  | 'volume'
  | 'file';

const TEXT_EXTENSIONS = new Set([
  'txt',
  'text',
  'md',
  'markdown',
  'log',
  'ini',
  'cfg',
  'conf',
  'json',
  'jsonc',
  'xml',
  'yml',
  'yaml',
  'toml',
  'csv',
  'tsv',
  'pdf',
  'doc',
  'docx',
  'odt',
  'rtf',
  'xls',
  'xlsx',
  'ppt',
  'pptx',
  'ods',
  'html',
  'htm',
  'css',
  'js',
  'jsx',
  'ts',
  'tsx',
  'rs',
  'sql',
  'sh',
  'ps1',
  'bat',
]);

const IMAGE_EXTENSIONS = new Set([
  'jpg',
  'jpeg',
  'png',
  'gif',
  'bmp',
  'webp',
  'svg',
  'ico',
  'tif',
  'tiff',
  'heic',
  'heif',
  'avif',
  'raw',
  'cr2',
  'nef',
  'dng',
  'psd',
]);

const VIDEO_EXTENSIONS = new Set([
  'mp4',
  'm4v',
  'mkv',
  'avi',
  'mov',
  'wmv',
  'webm',
  'mpg',
  'mpeg',
  'flv',
  '3gp',
]);

const ARCHIVE_EXTENSIONS = new Set([
  'zip',
  '7z',
  'rar',
  'tar',
  'gz',
  'tgz',
  'bz2',
  'xz',
  'zst',
  'cab',
  'lz',
  'lzma',
]);

/** Disk images report the volume icon: they stand for a whole filesystem. */
const VOLUME_EXTENSIONS = new Set([
  'vhd',
  'vhdx',
  'vdi',
  'vmdk',
  'iso',
  'img',
  'qcow',
  'qcow2',
  'dmg',
]);

/** Drive roots look like `C:` or `C:\`; they are not files inside a volume. */
const VOLUME_PATH = /^[a-z]:\\?$/i;

/** A drive root row represents a whole volume, not an ordinary folder. */
export function isVolumeRow(row: NodeRow): boolean {
  return row.kind === 'directory' && VOLUME_PATH.test(row.path);
}

const ICONS: Record<TreeIconKind, LucideIcon> = {
  folder: Folder,
  'folder-open': FolderOpen,
  text: FileText,
  image: ImageIcon,
  video: Film,
  archive: FileArchive,
  volume: HardDrive,
  file: File,
};

/** Extension without the dot, lower case; empty when the name has none. */
export function fileExtension(name: string): string {
  const dot = name.lastIndexOf('.');

  if (dot <= 0 || dot === name.length - 1) return '';

  return name.slice(dot + 1).toLowerCase();
}

/** Icon for one row; folders flip to the open variant when expanded. */
export function treeIconKind(row: NodeRow, expanded = false): TreeIconKind {
  if (row.kind === 'directory') {
    if (isVolumeRow(row)) return 'volume';

    return expanded ? 'folder-open' : 'folder';
  }

  if (row.kind !== 'file') return 'file';

  const extension = fileExtension(row.name);

  if (VOLUME_EXTENSIONS.has(extension)) return 'volume';
  if (ARCHIVE_EXTENSIONS.has(extension)) return 'archive';
  if (IMAGE_EXTENSIONS.has(extension)) return 'image';
  if (VIDEO_EXTENSIONS.has(extension)) return 'video';
  if (TEXT_EXTENSIONS.has(extension)) return 'text';

  return 'file';
}

/**
 * Share of the parent, in whole percent.
 *
 * A node of unknown size or a parent without bytes has no share instead of a
 * fabricated one; the result never leaves 0…100 so the bar cannot overflow.
 */
export function sharePercent(size: number, parentSize: number): number {
  if (!Number.isFinite(size) || !Number.isFinite(parentSize)) return 0;
  if (size <= 0 || parentSize <= 0) return 0;

  return Math.min(100, Math.max(0, Math.round((size / parentSize) * 100)));
}

/** Decorative type icon; the name always carries the meaning for a reader. */
export function TreeRowIcon({ row, expanded }: { row: NodeRow; expanded: boolean }) {
  const kind = treeIconKind(row, expanded);
  const Icon = ICONS[kind];

  return (
    <span className={styles.icon} data-tree-icon={kind} aria-hidden="true">
      <Icon size={15} strokeWidth={1.7} />
    </span>
  );
}

export interface TreeRowShareBarProps {
  /** Depth in the flattened tree, 1 for the root. */
  level: number;
  /** Whole percent of the parent size. */
  share: number;
}

/**
 * Inline progress bar of a row.
 *
 * The track starts right after the chevron so the indent and the toggle stay
 * clear; the fill spans the rest of the row and sits behind the cells. Rows keep
 * a 1 px inset at the top and the bottom, which leaves a 2 px gap between
 * neighbouring bars.
 */
export function TreeRowShareBar({ level, share }: TreeRowShareBarProps) {
  if (share <= 0) return null;

  const indent = `calc(var(--fs-space-3) + ${Math.max(0, level - 1) * 16}px)`;

  return (
    <span
      className={styles.shareTrack}
      style={{ left: `calc(${indent} + 24px)` }}
      data-tree-fill=""
      data-share={String(share)}
      aria-hidden="true"
    >
      <span className={styles.shareFill} style={{ width: `${share}%` }} />
    </span>
  );
}

export interface TreeRowLabelProps {
  row: NodeRow;
  expanded: boolean;
  onToggle: (row: NodeRow) => void;
}

/**
 * Content of the first cell: chevron, type icon, name and the access warning.
 *
 * The chevron is a real button so a mouse and a screen reader can expand the
 * branch; keyboard users also get `←`/`→` on the row itself (DataTable).
 */
export function TreeRowLabel({ row, expanded, onToggle }: TreeRowLabelProps) {
  const expandable = row.kind === 'directory' && row.child_count > 0;

  return (
    <span className={styles.label}>
      {expandable ? (
        <button
          type="button"
          className={styles.toggle}
          aria-label={`${expanded ? t('tree.collapse') : t('tree.expand')}: ${row.name}`}
          aria-expanded={expanded}
          onClick={(event) => {
            event.stopPropagation();
            onToggle(row);
          }}
          onDoubleClick={(event) => {
            event.stopPropagation();
          }}
          onKeyDown={(event) => {
            // Enter and Space belong to the button; the row must not also
            // activate or select the directory.
            if (event.key === 'Enter' || event.key === ' ') event.stopPropagation();
          }}
        >
          <span
            className={styles.chevron}
            data-open={expanded ? '' : undefined}
            aria-hidden="true"
          >
            <ChevronRight size={14} strokeWidth={1.8} />
          </span>
        </button>
      ) : (
        <span className={styles.toggleSpacer} aria-hidden="true" />
      )}
      <TreeRowIcon row={row} expanded={expanded} />
      <span className={styles.name}>{row.name}</span>
      {row.accessible ? null : <Badge tone="warning">{t('tree.access_denied')}</Badge>}
    </span>
  );
}
