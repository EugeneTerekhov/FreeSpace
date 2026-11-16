import type { TreemapTile } from '../api/dto';

export type TileDirection = 'left' | 'right' | 'up' | 'down';

export interface TilePoint {
  x: number;
  y: number;
}

/** Centre of a tile in layout coordinates. */
export function tileCentre(tile: TreemapTile): TilePoint {
  return {
    x: tile.rect.x + tile.rect.w / 2,
    y: tile.rect.y + tile.rect.h / 2,
  };
}

/** Largest tile, used as the first stop when the map gains focus. */
export function initialTileIndex(tiles: TreemapTile[]): number {
  let best = -1;
  let bestArea = -1;

  tiles.forEach((tile, index) => {
    const area = tile.rect.w * tile.rect.h;
    if (area > bestArea) {
      best = index;
      bestArea = area;
    }
  });

  return best;
}

function isCandidate(
  direction: TileDirection,
  from: TilePoint,
  to: TilePoint,
): boolean {
  const epsilon = 0.5;
  switch (direction) {
    case 'right':
      return to.x > from.x + epsilon;
    case 'left':
      return to.x < from.x - epsilon;
    case 'down':
      return to.y > from.y + epsilon;
    case 'up':
      return to.y < from.y - epsilon;
  }
}

function distance(
  direction: TileDirection,
  from: TilePoint,
  to: TilePoint,
): number {
  const dx = Math.abs(to.x - from.x);
  const dy = Math.abs(to.y - from.y);
  const alongX = direction === 'left' || direction === 'right';

  return alongX ? dx + dy * 2 : dy + dx * 2;
}

/**
 * Nearest tile in a direction, by layout geometry.
 *
 * When nothing lies in that direction (a single-column map, or the edge of the
 * map) the neighbour in DOM order is used so every arrow key moves the focus
 * predictably instead of trapping it on one edge tile.
 */
export function adjacentTileIndex(
  tiles: TreemapTile[],
  current: number,
  direction: TileDirection,
): number {
  if (tiles.length === 0) return -1;
  if (current < 0 || current >= tiles.length) return initialTileIndex(tiles);

  const fromTile = tiles[current];
  if (fromTile === undefined) return initialTileIndex(tiles);
  const from = tileCentre(fromTile);

  let best = -1;
  let bestDistance = Number.POSITIVE_INFINITY;

  tiles.forEach((tile, index) => {
    if (index === current) return;
    const to = tileCentre(tile);
    if (!isCandidate(direction, from, to)) return;

    const value = distance(direction, from, to);
    if (value < bestDistance) {
      best = index;
      bestDistance = value;
    }
  });

  if (best >= 0) return best;

  return direction === 'right' || direction === 'down'
    ? Math.min(tiles.length - 1, current + 1)
    : Math.max(0, current - 1);
}
