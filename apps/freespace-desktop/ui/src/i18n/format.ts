import type { Language, UnitSystem } from '../api/dto';

const BINARY_UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'] as const;
const DECIMAL_UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'] as const;

/** Formats a byte count with the selected unit system. */
export function formatBytes(bytes: number, units: UnitSystem = 'binary'): string {
  const base = units === 'binary' ? 1024 : 1000;
  const labels = units === 'binary' ? BINARY_UNITS : DECIMAL_UNITS;
  let value = Math.max(0, bytes);
  let index = 0;

  while (value >= base && index < labels.length - 1) {
    value /= base;
    index += 1;
  }

  const digits = index === 0 ? 0 : value >= 100 ? 0 : value >= 10 ? 1 : 2;
  const formatted = new Intl.NumberFormat('en-US', {
    minimumFractionDigits: 0,
    maximumFractionDigits: digits,
  }).format(value);

  return `${formatted} ${labels[index] ?? 'B'}`;
}

/** Formats an integer with locale-aware grouping. */
export function formatNumber(
  value: number,
  language: Language = 'ru',
  options?: Intl.NumberFormatOptions,
): string {
  return new Intl.NumberFormat(language, options).format(value);
}

/** Formats a 0..1 ratio as a percentage. */
export function formatPercent(
  ratio: number,
  language: Language = 'ru',
  digits = 0,
): string {
  return new Intl.NumberFormat(language, {
    style: 'percent',
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(Math.max(0, Math.min(1, ratio)));
}

/** Formats a Unix-millisecond timestamp in local time. */
export function formatDate(
  timestamp: number | null,
  language: Language = 'ru',
): string {
  if (timestamp === null) return '—';

  return new Intl.DateTimeFormat(language, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp));
}
