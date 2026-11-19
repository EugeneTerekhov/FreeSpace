import type { Language } from '../api/dto';
import { en } from './generated/en';
import { ru } from './generated/ru';
import type { MessageKey } from './generated/keys';

export type { MessageKey };

type Catalog = Record<string, string>;

const catalogs: Record<Language, Catalog> = { ru, en };
const missing = new Set<string>();
let language: Language = 'ru';

/** Applies the active language to the document. */
export function setLanguage(next: Language): void {
  language = next;
  if (typeof document !== 'undefined') {
    document.documentElement.lang = next;
  }
}

/** Current interface language. */
export function getLanguage(): Language {
  return language;
}

/**
 * Translates a key and substitutes `%{name}` placeholders.
 *
 * A missing translation falls back to English and is recorded for the
 * missing-key test; it never renders `undefined`.
 */
export function t(
  key: MessageKey,
  variables?: Record<string, string | number>,
): string;
export function t(
  key: string,
  variables?: Record<string, string | number>,
): string;
export function t(
  key: string,
  variables?: Record<string, string | number>,
): string {
  const template = catalogs[language][key] ?? catalogs.en[key];

  if (template === undefined) {
    missing.add(key);
    return key;
  }

  if (variables === undefined) {
    return template;
  }

  return template.replace(/%\{(\w+)\}/g, (match, name: string) => {
    const value = variables[name];
    return value === undefined ? match : String(value);
  });
}

/** Keys requested but absent from both catalogues. */
export function missingKeys(): string[] {
  return [...missing].sort();
}

/**
 * Translates a count-aware message using the CLDR plural rules of the active
 * language.
 *
 * Looks for `<base>_<category>` (`_one`, `_few`, `_many`, `_other`), then falls
 * back to `<base>`. `count` is always supplied to the placeholders.
 */
export function tPlural(
  base: string,
  count: number,
  variables: Record<string, string | number> = {},
): string {
  const category = new Intl.PluralRules(language).select(count);
  const candidate = `${base}_${category}`;
  const key = catalogs[language][candidate] === undefined ? base : candidate;

  return t(key, { count, ...variables });
}

/** Clears the missing-key registry; used by tests. */
export function resetMissingKeys(): void {
  missing.clear();
}
