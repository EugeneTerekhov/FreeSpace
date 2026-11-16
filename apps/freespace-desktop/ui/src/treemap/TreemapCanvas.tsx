import { useEffect, useRef, useState } from 'react';

import type { TreemapTile } from '../api/dto';
import {
  adjacentTileIndex,
  initialTileIndex,
  type TileDirection,
} from './keyboard';
import { pickTileInk, treemapToken } from './palette';
import styles from './TreemapCanvas.module.css';

export interface TreemapCanvasProps {
  tiles: TreemapTile[];
  selectedNodeId: number | null;
  label: string;
  onSelect: (tile: TreemapTile) => void;
  onActivate: (tile: TreemapTile) => void;
  /** Navigates one level up; wired to Escape/Backspace. */
  onUp?: () => void;
  /** Localized one-line description announced for the focused tile. */
  describeTile?: (tile: TreemapTile) => string;
}

function token(name: string): string {
  if (typeof window === 'undefined') return '';
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function inkFor(colour: string): string {
  return pickTileInk(
    colour,
    token('--fs-tile-ink'),
    token('--fs-tile-ink-alt'),
  );
}

function directionOf(key: string): TileDirection | null {
  if (key === 'ArrowRight') return 'right';
  if (key === 'ArrowLeft') return 'left';
  if (key === 'ArrowDown') return 'down';
  if (key === 'ArrowUp') return 'up';

  return null;
}

/** Canvas renderer with hit-testing, spatial keyboard navigation and a text summary. */
export function TreemapCanvas({
  tiles,
  selectedNodeId,
  label,
  onSelect,
  onActivate,
  onUp,
  describeTile,
}: TreemapCanvasProps) {
  const wrapper = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [active, setActive] = useState(-1);
  const [themeVersion, setThemeVersion] = useState(0);

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setThemeVersion((version) => version + 1);
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    });

    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    const element = wrapper.current;
    if (element === null) return undefined;

    const observer = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect;
      if (rect !== undefined) {
        setSize({ width: rect.width, height: rect.height });
      }
    });
    observer.observe(element);
    setSize({ width: element.clientWidth, height: element.clientHeight });

    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    const element = canvas.current;
    if (element === null || size.width === 0 || size.height === 0) return;

    const ratio = window.devicePixelRatio || 1;
    element.width = Math.floor(size.width * ratio);
    element.height = Math.floor(size.height * ratio);
    const context = element.getContext('2d');
    if (context === null) return;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, size.width, size.height);

    const accent = token('--fs-accent-strong');
    const selectedTile = tiles.findIndex((tile) => tile.node_id === selectedNodeId);
    const maxX = Math.max(1, ...tiles.map((item) => item.rect.x + item.rect.w));
    const maxY = Math.max(1, ...tiles.map((item) => item.rect.y + item.rect.h));
    const scaleX = size.width / maxX;
    const scaleY = size.height / maxY;

    tiles.forEach((tile, index) => {
      const colour = token(treemapToken(tile.color_key)) || token('--fs-tm-9');
      const x = tile.rect.x * scaleX;
      const y = tile.rect.y * scaleY;
      const width = Math.max(1, tile.rect.w * scaleX);
      const height = Math.max(1, tile.rect.h * scaleY);

      context.fillStyle = colour;
      context.fillRect(x, y, width, height);
      context.strokeStyle = token('--fs-bg');
      context.lineWidth = 1;
      context.strokeRect(x + 0.5, y + 0.5, width - 1, height - 1);

      if (tile.label !== '' && width > 64 && height > 18) {
        context.fillStyle = inkFor(colour);
        context.font = '12px "Segoe UI", system-ui, sans-serif';
        context.textBaseline = 'middle';
        context.save();
        context.beginPath();
        context.rect(x + 4, y + 2, width - 8, height - 4);
        context.clip();
        context.fillText(tile.label, x + 6, y + 14);
        context.restore();
      }

      if (index === selectedTile || index === active) {
        context.strokeStyle = accent;
        context.lineWidth = 3;
        context.strokeRect(x + 1.5, y + 1.5, width - 3, height - 3);
      }
    });
  }, [active, selectedNodeId, size, themeVersion, tiles]);

  const hit = (clientX: number, clientY: number): number => {
    const element = canvas.current;
    if (element === null) return -1;
    const rect = element.getBoundingClientRect();
    const x = ((clientX - rect.left) / Math.max(1, rect.width)) * size.width;
    const y = ((clientY - rect.top) / Math.max(1, rect.height)) * size.height;
    const maxX = Math.max(1, ...tiles.map((tile) => tile.rect.x + tile.rect.w));
    const maxY = Math.max(1, ...tiles.map((tile) => tile.rect.y + tile.rect.h));

    return tiles.findIndex((tile) => {
      const left = (tile.rect.x / maxX) * size.width;
      const top = (tile.rect.y / maxY) * size.height;
      const width = (tile.rect.w / maxX) * size.width;
      const height = (tile.rect.h / maxY) * size.height;

      return x >= left && x <= left + width && y >= top && y <= top + height;
    });
  };

  const selectIndex = (index: number) => {
    if (index < 0) return;
    setActive(index);
    const tile = tiles[index];
    if (tile !== undefined) onSelect(tile);
  };

  const focusedTile = active >= 0 ? tiles[active] : undefined;
  const activeText = focusedTile === undefined || describeTile === undefined
    ? null
    : describeTile(focusedTile);
  const ariaLabel = activeText === null ? label : `${label}: ${activeText}`;

  return (
    <div ref={wrapper} className={styles.wrapper}>
      <p className="fs-visually-hidden" aria-live="polite" aria-atomic="true">
        {activeText ?? ''}
      </p>
      <canvas
        ref={canvas}
        className={styles.canvas}
        role="img"
        aria-label={ariaLabel}
        tabIndex={0}
        onClick={(event) => {
          const index = hit(event.clientX, event.clientY);
          if (index >= 0) selectIndex(index);
        }}
        onDoubleClick={(event) => {
          const index = hit(event.clientX, event.clientY);
          const tile = tiles[index];
          if (tile !== undefined) onActivate(tile);
        }}
        onKeyDown={(event) => {
          const direction = directionOf(event.key);
          if (direction !== null) {
            event.preventDefault();
            const next = adjacentTileIndex(tiles, active, direction);
            if (next >= 0) selectIndex(next);
            return;
          }
          if (event.key === 'Home' || event.key === 'End') {
            event.preventDefault();
            selectIndex(
              event.key === 'Home' ? initialTileIndex(tiles) : tiles.length - 1,
            );
            return;
          }
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            const index = active >= 0 ? active : initialTileIndex(tiles);
            const tile = tiles[index];
            if (tile === undefined) return;
            if (event.key === ' ') {
              selectIndex(index);
            } else {
              onActivate(tile);
            }
            return;
          }
          if ((event.key === 'Escape' || event.key === 'Backspace') && onUp !== undefined) {
            event.preventDefault();
            onUp();
          }
        }}
      />
    </div>
  );
}
