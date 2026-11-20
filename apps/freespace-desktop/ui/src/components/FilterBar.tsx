import { useState } from 'react';

import type { FilterSet } from '../api/dto';
import { Button } from './Button';
import { Chip } from './Choice';
import { SearchField } from './Fields';
import { t } from '../i18n';
import { emptyFilter } from '../lib/filters';
import styles from './FilterBar.module.css';

const CATEGORIES = [
  'video',
  'audio',
  'image',
  'archive',
  'document',
  'code',
  'executable',
  'database',
  'font',
  'other',
] as const;

export interface FilterCategoryOption {
  id: string;
  labelKey: string;
}

export interface FilterBarProps {
  filter: FilterSet;
  onChange: (filter: FilterSet) => void;
  disabled?: boolean;
  /** Extra filter chips (for example cleanup reasons) appended to file types. */
  extraCategories?: readonly FilterCategoryOption[];
}

/** Shared filter bar with explicit active conditions. */
export function FilterBar({
  filter,
  onChange,
  disabled = false,
  extraCategories = [],
}: FilterBarProps) {
  const [extension, setExtension] = useState('');
  const [minSizeMb, setMinSizeMb] = useState(
    filter.min_size === null ? '' : String(Math.round(filter.min_size / (1024 * 1024))),
  );

  const categoryLabelKey = (category: string) =>
    extraCategories.find((item) => item.id === category)?.labelKey ??
    `treemap.category.${category}`;

  const active = [
    filter.text !== '' ? { id: 'text', label: filter.text } : null,
    ...filter.categories.map((category) => ({
      id: `category-${category}`,
      label: t(categoryLabelKey(category)),
    })),
    ...filter.extensions.map((item) => ({ id: `extension-${item}`, label: `.${item}` })),
    filter.min_size !== null
      ? {
          id: 'min-size',
          label: `${t('filter.min_size')}: ${Math.round(filter.min_size / (1024 * 1024))} MB`,
        }
      : null,
  ].filter((item): item is { id: string; label: string } => item !== null);

  const toggleCategory = (category: string) => {
    const categories = filter.categories.includes(category)
      ? filter.categories.filter((item) => item !== category)
      : [...filter.categories, category];
    onChange({ ...filter, categories });
  };

  const clear = () => {
    setExtension('');
    setMinSizeMb('');
    onChange(emptyFilter());
  };

  return (
    <section className={styles.bar} aria-label={t('filter.title')}>
      <div className={styles.row}>
        <SearchField
          className={styles.search}
          label={t('filter.text')}
          placeholder={t('tree.filter_hint')}
          value={filter.text}
          onValueChange={(text) => {
            onChange({ ...filter, text });
          }}
        />
        <input
          className={styles.input}
          aria-label={t('filter.extension')}
          placeholder={t('filter.extension')}
          value={extension}
          onChange={(event) => {
            setExtension(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Enter') return;
            const value = extension.trim().replace(/^\./, '').toLowerCase();
            if (value === '') return;
            onChange({
              ...filter,
              extensions: filter.extensions.includes(value)
                ? filter.extensions
                : [...filter.extensions, value],
            });
            setExtension('');
          }}
        />
        <input
          className={styles.input}
          type="number"
          min={0}
          aria-label={t('filter.min_size')}
          placeholder={t('filter.min_size')}
          value={minSizeMb}
          onChange={(event) => {
            const value = event.target.value;
            setMinSizeMb(value);
            const parsed = Number(value);
            onChange({
              ...filter,
              min_size: value === '' || Number.isNaN(parsed) ? null : Math.max(0, parsed) * 1024 * 1024,
            });
          }}
        />
        <span className={styles.spacer} />
        <Button variant="ghost" disabled={disabled} onClick={clear}>
          {t('filter.reset')}
        </Button>
      </div>
      <div className={styles.row}>
        <span className={styles.label}>{t('filter.categories')}</span>
        {[...CATEGORIES, ...extraCategories.map((item) => item.id)].map((category) => (
          <Chip
            key={category}
            selected={filter.categories.includes(category)}
            onClick={() => {
              toggleCategory(category);
            }}
          >
            {t(categoryLabelKey(category))}
          </Chip>
        ))}
      </div>
      {active.length > 0 ? (
        <div className={styles.row} role="group" aria-label={t('filter.active')}>
          <span className={styles.label}>{t('filter.active')}</span>
          {active.map((item) => (
            <Chip
              key={item.id}
              selected
              removeLabel={`${t('filter.reset')}: ${item.label}`}
              onRemove={() => {
                if (item.id.startsWith('category-')) {
                  toggleCategory(item.id.slice('category-'.length));
                } else if (item.id.startsWith('extension-')) {
                  const value = item.id.slice('extension-'.length);
                  onChange({
                    ...filter,
                    extensions: filter.extensions.filter((entry) => entry !== value),
                  });
                } else if (item.id === 'min-size') {
                  setMinSizeMb('');
                  onChange({ ...filter, min_size: null });
                } else {
                  onChange({ ...filter, text: '' });
                }
              }}
            >
              {item.label}
            </Chip>
          ))}
        </div>
      ) : null}
    </section>
  );
}
