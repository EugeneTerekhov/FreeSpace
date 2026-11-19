import { useMemo, useState } from 'react';

import { api } from '../api/runtime';
import type { FilterSet } from '../api/dto';
import { Button } from './Button';
import { Chip } from './Choice';
import { SearchField, Select } from './Fields';
import { t } from '../i18n';
import { emptyFilter } from '../lib/filters';
import { useSettingsStore } from '../stores/settingsStore';
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

export interface FilterBarProps {
  /**
   * Persistence key for saved sets. Absorbed screens keep their historical key
   * (`top_files`, `owners`, …) so filters do not mix between tabs.
   */
  screen: string;
  filter: FilterSet;
  onChange: (filter: FilterSet) => void;
  disabled?: boolean;
}

/** Shared filter bar with saved sets and explicit active conditions. */
export function FilterBar({ screen, filter, onChange, disabled = false }: FilterBarProps) {
  const settings = useSettingsStore((state) => state.settings);
  const savedSets = useMemo(
    () => (settings?.saved_filters ?? {}) as Record<string, FilterSet[] | undefined>,
    [settings],
  );
  const saved = useMemo(() => savedSets[screen] ?? [], [savedSets, screen]);
  const [extension, setExtension] = useState('');
  const [minSizeMb, setMinSizeMb] = useState(
    filter.min_size === null ? '' : String(Math.round(filter.min_size / (1024 * 1024))),
  );

  const save = async () => {
    const name = `Filter ${saved.length + 1}`;
    const next = [...saved, { ...filter, name }];
    const updated = await api.appSettingsSet({
      saved_filters: {
        ...savedSets,
        [screen]: next,
      },
    });
    useSettingsStore.getState().setSettings(updated);
  };

  const active = [
    filter.text !== '' ? { id: 'text', label: filter.text } : null,
    ...filter.categories.map((category) => ({
      id: `category-${category}`,
      label: t(`treemap.category.${category}`),
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
          disabled={disabled}
          onValueChange={(text) => {
            onChange({ ...filter, text });
          }}
        />
        <Select
          aria-label={t('filter.saved')}
          disabled={disabled || saved.length === 0}
          value=""
          options={[
            { value: '', label: t('filter.saved') },
            ...saved.map((item, index) => ({
              value: String(index),
              label: item.name ?? `Filter ${index + 1}`,
            })),
          ]}
          onChange={(event) => {
            const index = Number(event.target.value);
            const next = saved[index];
            if (next !== undefined) onChange({ ...next });
          }}
        />
        <input
          className={styles.input}
          aria-label={t('filter.extension')}
          placeholder={t('filter.extension')}
          value={extension}
          disabled={disabled}
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
          disabled={disabled}
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
        <Button variant="secondary" disabled={disabled} onClick={() => void save()}>
          {t('filter.save')}
        </Button>
      </div>
      <div className={styles.row}>
        <span className={styles.label}>{t('filter.categories')}</span>
        {CATEGORIES.map((category) => (
          <Chip
            key={category}
            selected={filter.categories.includes(category)}
            onClick={() => {
              toggleCategory(category);
            }}
          >
            {t(`treemap.category.${category}`)}
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
