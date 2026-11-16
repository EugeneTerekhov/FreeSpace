import { afterEach, describe, expect, it } from 'vitest';

import {
  getLanguage,
  missingKeys,
  resetMissingKeys,
  setLanguage,
  t,
  tPlural,
} from '../../src/i18n';
import {
  formatBytes,
  formatDate,
  formatNumber,
  formatPercent,
} from '../../src/i18n/format';
import { ru } from '../../src/i18n/generated/ru';
import { en } from '../../src/i18n/generated/en';

afterEach(() => {
  setLanguage('ru');
  resetMissingKeys();
});

describe('i18n runtime', () => {
  it('switches language without losing keys', () => {
    setLanguage('en');
    expect(getLanguage()).toBe('en');
    expect(t('app.name')).toBe('FreeSpace');
    expect(t('scan.objects', { count: 5 })).toBe('objects: 5');

    setLanguage('ru');
    expect(t('scan.objects', { count: 5 })).toBe('объектов: 5');
  });

  it('keeps RU and EN key sets identical', () => {
    expect(Object.keys(ru).sort()).toEqual(Object.keys(en).sort());
    expect(Object.keys(ru).length).toBeGreaterThan(300);
  });

  it('records missing keys and falls back to the key itself', () => {
    setLanguage('ru');
    expect(t('missing.key')).toBe('missing.key');
    expect(missingKeys()).toContain('missing.key');

    resetMissingKeys();
    expect(missingKeys()).toEqual([]);
  });

  it('leaves unknown placeholders untouched', () => {
    setLanguage('en');
    expect(t('scan.objects', {})).toBe('objects: %{count}');
  });

  it('pluralizes counts with the CLDR rules of the language', () => {
    setLanguage('ru');
    expect(tPlural('scan.objects', 1)).toBe('1 объект');
    expect(tPlural('scan.objects', 2)).toBe('2 объекта');
    expect(tPlural('scan.objects', 5)).toBe('5 объектов');
    expect(tPlural('scan.objects', 21)).toBe('21 объект');

    setLanguage('en');
    expect(tPlural('scan.objects', 1)).toBe('1 object');
    expect(tPlural('scan.objects', 5)).toBe('5 objects');
  });

  it('keeps a preformatted count when the caller supplies one', () => {
    setLanguage('en');
    expect(tPlural('scan.objects', 1200, { count: '1,200' })).toBe('1,200 objects');
  });

  it('contains no Cyrillic in the English catalogue', () => {
    const offenders = Object.entries(en).filter(([, value]) =>
      /[\u0400-\u04FF]/u.test(value),
    );

    expect(offenders).toEqual([]);
  });
});

describe('formatting', () => {
  it('formats binary and decimal byte counts', () => {
    expect(formatBytes(1024, 'binary')).toBe('1 KiB');
    expect(formatBytes(1536, 'binary')).toBe('1.5 KiB');
    expect(formatBytes(1000, 'decimal')).toBe('1 KB');
    expect(formatBytes(0)).toBe('0 B');
  });

  it('formats numbers, percentages and dates', () => {
    expect(formatNumber(1_234_567, 'en')).toBe('1,234,567');
    expect(formatPercent(0.426, 'en')).toBe('43%');
    expect(formatDate(null)).toBe('—');
    expect(formatDate(0, 'en')).not.toBe('—');
  });
});
