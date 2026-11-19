import { useCallback, useEffect, useState } from 'react';

import type {
  Density,
  DuplicateKeepRule,
  Language,
  PathValidation,
  ReducedMotion,
  SettingsView,
  ThemePreference,
  UiError,
  UiSettings,
  UiSettingsPatch,
  UnitSystem,
} from '../api/dto';
import {
  settingsGet,
  settingsReset,
  settingsSet,
  settingsValidatePaths,
} from '../api/settingsOps';
import { normalizeError, validationError } from '../app/errors';
import { Button } from '../components/Button';
import { Segmented, Toggle } from '../components/Choice';
import { Banner, ErrorState, Loading } from '../components/Feedback';
import { Field, Select } from '../components/Fields';
import { setLanguage, t } from '../i18n';
import { formatBytes } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { About } from './About';
import { Elevation } from './Elevation';
import { Panel } from './ScreenFrame';
import { ScreenFrameTabs } from './ScreenTabs';
import local from './settings-screen.module.css';

const MAX_DAYS = 3650;
const MAX_INTERVAL = 86_400;
const KEEP_RULES: DuplicateKeepRule[] = ['oldest', 'newest', 'shortest-path', 'original-dir'];

/**
 * English fallback for option labels whose catalogue keys are not translated
 * yet. `t()` still records each miss; once the keys land in `locales/*.toml`
 * the map is never used again.
 */
const FALLBACK_LABELS: Record<string, string> = {
  'settings.language.ru': 'Russian',
  'settings.language.en': 'English',
  'settings.theme.system': 'System',
  'settings.theme.dark': 'Dark',
  'settings.theme.light': 'Light',
  'settings.density': 'Density',
  'settings.density.comfortable': 'Comfortable',
  'settings.density.compact': 'Compact',
  'settings.reduced_motion': 'Reduced motion',
  'settings.reduced_motion.system': 'System',
  'settings.reduced_motion.on': 'On',
  'settings.reduced_motion.off': 'Off',
};

function label(key: string): string {
  const text = t(key);

  return text === key ? (FALLBACK_LABELS[key] ?? text) : text;
}

function splitList(value: string): string[] {
  return value
    .split(/\r?\n|;/)
    .map((item) => item.trim())
    .filter((item) => item !== '');
}

function parseKeepRule(value: string): DuplicateKeepRule {
  return KEEP_RULES.find((rule) => rule === value) ?? 'oldest';
}

function isIntegerInRange(value: string, min: number, max: number): boolean {
  if (value.trim() === '') return false;
  const parsed = Number(value);

  return Number.isInteger(parsed) && parsed >= min && parsed <= max;
}

/**
 * Application settings.
 *
 * Interface preferences are persisted immediately through `settings_set`, so
 * the theme, language, density and motion change without a restart. The legacy
 * 001 fields keep the explicit *Save* action documented for the settings screen.
 */
export function SettingsPanel() {
  const [view, setView] = useState<SettingsView | null>(null);
  const [ui, setUi] = useState<UiSettings | null>(null);
  const [exclusions, setExclusions] = useState('');
  const [duplicateRule, setDuplicateRule] = useState<DuplicateKeepRule>('oldest');
  const [outdatedDays, setOutdatedDays] = useState('180');
  const [quarantineDays, setQuarantineDays] = useState('30');
  const [thresholdBytes, setThresholdBytes] = useState(String(1024 * 1024 * 1024));
  const [intervalSecs, setIntervalSecs] = useState('60');
  const [confirmDestructive, setConfirmDestructive] = useState(true);
  const [contextMenu, setContextMenu] = useState(false);
  const [pathResults, setPathResults] = useState<PathValidation[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [validating, setValidating] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const hydrate = useCallback((next: SettingsView) => {
    setView(next);
    setUi(next.ui);
    setExclusions(next.exclusions.join('; '));
    setDuplicateRule(parseKeepRule(next.duplicate_keep_rule));
    setOutdatedDays(String(next.outdated_threshold_days));
    setQuarantineDays(String(next.quarantine_retention_days));
    setThresholdBytes(String(next.monitor_threshold_bytes));
    setIntervalSecs(String(next.monitor_interval_secs));
    setConfirmDestructive(next.confirm_destructive_actions);
    setContextMenu(next.context_menu_enabled);
    setPathResults(null);
    useSettingsStore.getState().setSettings(next.ui);
    setLanguage(next.ui.language);
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      hydrate(await settingsGet());
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setLoading(false);
    }
  }, [hydrate]);

  useEffect(() => {
    void load();
  }, [load]);

  const applyUi = async (patch: UiSettingsPatch) => {
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      const next = await settingsSet({ ui: patch });
      setView(next);
      setUi(next.ui);
      useSettingsStore.getState().setSettings(next.ui);
      setLanguage(next.ui.language);
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setSaving(false);
    }
  };

  const daysValid = isIntegerInRange(outdatedDays, 0, MAX_DAYS);
  const quarantineValid = isIntegerInRange(quarantineDays, 0, MAX_DAYS);
  const thresholdValid = isIntegerInRange(thresholdBytes, 0, Number.MAX_SAFE_INTEGER);
  const intervalValid = isIntegerInRange(intervalSecs, 1, MAX_INTERVAL);
  const formValid = daysValid && quarantineValid && thresholdValid && intervalValid;

  const save = async () => {
    if (!formValid) {
      setNotice(null);
      setError(validationError(t('error.validation')));
      return;
    }
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      hydrate(
        await settingsSet({
          exclusions: splitList(exclusions),
          duplicate_keep_rule: duplicateRule,
          outdated_threshold_days: Number(outdatedDays),
          quarantine_retention_days: Number(quarantineDays),
          monitor_threshold_bytes: Number(thresholdBytes),
          monitor_interval_secs: Number(intervalSecs),
          confirm_destructive_actions: confirmDestructive,
          context_menu_enabled: contextMenu,
        }),
      );
      setNotice(t('settings.saved'));
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setSaving(false);
    }
  };

  const reset = async () => {
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      hydrate(await settingsReset('all'));
      setNotice(t('settings.saved'));
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setSaving(false);
    }
  };

  const validatePaths = async () => {
    setValidating(true);
    setError(null);
    try {
      const candidates = splitList(exclusions).filter(
        (entry) => !entry.includes('*') && !entry.includes('?'),
      );
      setPathResults(await settingsValidatePaths(candidates));
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setValidating(false);
    }
  };

  if (loading && view === null) {
    return (
      <Panel title={t('nav.settings')}>
        <Loading label={t('common.loading')} rows={4} />
      </Panel>
    );
  }

  if (view === null || ui === null) {
    return (
      <ErrorState
        title={t(error?.message_key ?? 'error.internal')}
        code={error?.code}
        details={error?.details ?? undefined}
        actions={
          <Button variant="primary" loading={loading} onClick={() => void load()}>
            {t('error.retry')}
          </Button>
        }
      />
    );
  }

  const invalidPaths = (pathResults ?? []).filter((result) => !result.valid);

  return (
    <div className={local.page}>
      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" loading={loading} onClick={() => void load()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      <Panel
        title={t('nav.settings')}
        actions={
          <Button variant="secondary" loading={saving} onClick={() => void reset()}>
            {t('filter.reset')}
          </Button>
        }
      >
        <div className={local.section}>
          <h3 className={local.sectionTitle}>{t('system.settings.interface')}</h3>
          <div className={local.formGrid}>
            <Field id="settings-language" label={t('settings.language')}>
              <Select
                id="settings-language"
                value={ui.language}
                options={[
                  { value: 'ru', label: label('settings.language.ru') },
                  { value: 'en', label: label('settings.language.en') },
                ]}
                onChange={(event) => {
                  void applyUi({ language: event.target.value as Language });
                }}
              />
            </Field>

            <div className={local.settingItem}>
              <span className={local.hint}>{t('settings.theme')}</span>
              <Segmented
                label={t('settings.theme')}
                value={ui.theme}
                onChange={(value) => {
                  void applyUi({ theme: value as ThemePreference });
                }}
                options={[
                  { value: 'system', label: label('settings.theme.system') },
                  { value: 'dark', label: label('settings.theme.dark') },
                  { value: 'light', label: label('settings.theme.light') },
                ]}
              />
            </div>

            <div className={local.settingItem}>
              <span className={local.hint}>{label('settings.density')}</span>
              <Segmented
                label={label('settings.density')}
                value={ui.density}
                onChange={(value) => {
                  void applyUi({ density: value as Density });
                }}
                options={[
                  { value: 'comfortable', label: label('settings.density.comfortable') },
                  { value: 'compact', label: label('settings.density.compact') },
                ]}
              />
            </div>

            <div className={local.settingItem}>
              <span className={local.hint}>{t('settings.units')}</span>
              <Segmented
                label={t('settings.units')}
                value={ui.units}
                onChange={(value) => {
                  void applyUi({ units: value as UnitSystem });
                }}
                options={[
                  { value: 'binary', label: formatBytes(1024, 'binary') },
                  { value: 'decimal', label: formatBytes(1000, 'decimal') },
                ]}
              />
            </div>

            <div className={local.settingItem}>
              <span className={local.hint}>{label('settings.reduced_motion')}</span>
              <Segmented
                label={label('settings.reduced_motion')}
                value={ui.reduced_motion}
                onChange={(value) => {
                  void applyUi({ reduced_motion: value as ReducedMotion });
                }}
                options={[
                  { value: 'system', label: label('settings.reduced_motion.system') },
                  { value: 'on', label: label('settings.reduced_motion.on') },
                  { value: 'off', label: label('settings.reduced_motion.off') },
                ]}
              />
            </div>
          </div>
        </div>

        <div className={local.section}>
          <h3 className={local.sectionTitle}>{t('system.settings.cleanup')}</h3>
          <div className={local.formGrid}>
            <Field id="settings-duplicate-rule" label={t('settings.duplicate_rule')}>
              <Select
                id="settings-duplicate-rule"
                value={duplicateRule}
                options={[
                  { value: 'oldest', label: t('duplicates.rule.oldest') },
                  { value: 'newest', label: t('duplicates.rule.newest') },
                  { value: 'shortest-path', label: t('duplicates.rule.shortest') },
                  { value: 'original-dir', label: t('duplicates.rule.original-dir') },
                ]}
                onChange={(event) => {
                  setDuplicateRule(event.target.value as DuplicateKeepRule);
                }}
              />
            </Field>

            <Field
              id="settings-outdated-days"
              label={t('settings.outdated_days')}
              {...(daysValid ? {} : { error: t('error.validation') })}
            >
              <input
                id="settings-outdated-days"
                className={`${local.control} ${local.number}`}
                type="number"
                min={0}
                max={MAX_DAYS}
                step={1}
                value={outdatedDays}
                onChange={(event) => {
                  setOutdatedDays(event.target.value);
                }}
              />
            </Field>

            <Field
              id="settings-quarantine-days"
              label={t('settings.quarantine_days')}
              {...(quarantineValid ? {} : { error: t('error.validation') })}
            >
              <input
                id="settings-quarantine-days"
                className={`${local.control} ${local.number}`}
                type="number"
                min={0}
                max={MAX_DAYS}
                step={1}
                value={quarantineDays}
                onChange={(event) => {
                  setQuarantineDays(event.target.value);
                }}
              />
            </Field>
          </div>

          <div className={local.toggleList}>
            <Toggle
              checked={confirmDestructive}
              onCheckedChange={setConfirmDestructive}
              label={t('settings.confirm')}
            />
            <Toggle
              checked={contextMenu}
              onCheckedChange={setContextMenu}
              label={t('settings.context_menu')}
            />
          </div>
        </div>

        <div className={local.section}>
          <h3 className={local.sectionTitle}>{t('nav.monitor')}</h3>
          <div className={local.formGrid}>
            <Field
              id="settings-monitor-threshold"
              label={t('settings.monitor_threshold')}
              {...(thresholdValid ? {} : { error: t('error.validation') })}
            >
              <input
                id="settings-monitor-threshold"
                className={`${local.control} ${local.number}`}
                type="number"
                min={0}
                step={1024 * 1024}
                value={thresholdBytes}
                onChange={(event) => {
                  setThresholdBytes(event.target.value);
                }}
              />
            </Field>

            <Field
              id="settings-monitor-interval"
              label={t('settings.monitor_interval')}
              {...(intervalValid ? {} : { error: t('error.validation') })}
            >
              <input
                id="settings-monitor-interval"
                className={`${local.control} ${local.number}`}
                type="number"
                min={1}
                max={MAX_INTERVAL}
                step={1}
                value={intervalSecs}
                onChange={(event) => {
                  setIntervalSecs(event.target.value);
                }}
              />
            </Field>
          </div>
        </div>
      </Panel>

      <Panel title={t('system.settings.paths')}>
        <Field id="settings-exclusions" label={t('settings.exclusions')}>
          <textarea
            id="settings-exclusions"
            className={local.control}
            rows={3}
            value={exclusions}
            onChange={(event) => {
              setExclusions(event.target.value);
            }}
          />
        </Field>

        <div className={local.footer}>
          <Button variant="secondary" loading={validating} onClick={() => void validatePaths()}>
            {t('details.inspect')}
          </Button>
          {pathResults !== null && invalidPaths.length === 0 ? (
            <Banner tone="info">{t('status.ready')}</Banner>
          ) : null}
        </div>

        {invalidPaths.length > 0 ? (
          <>
            <Banner tone="error">{t('error.validation')}</Banner>
            <ul className={local.list}>
              {invalidPaths.map((result) => (
                <li key={`${result.path}-${result.error_key ?? ''}`}>
                  <code className={local.code}>{result.path}</code>
                  {result.error_key !== null ? ` — ${t(result.error_key)}` : ''}
                </li>
              ))}
            </ul>
          </>
        ) : null}
      </Panel>

      <Panel title={t('system.settings.apply')}>
        <p className={local.note}>{t('settings.change_hint')}</p>
        <p className={local.note}>{t('settings.restart_hint')}</p>
        {notice !== null ? <Banner tone="info">{notice}</Banner> : null}
        <div className={local.footer}>
          <Button
            variant="primary"
            loading={saving}
            disabled={!formValid}
            onClick={() => void save()}
          >
            {t('settings.save')}
          </Button>
        </div>
      </Panel>
    </div>
  );
}

const SETTINGS_TABS = [
  { id: 'general', labelKey: 'nav.settings' },
  { id: 'elevation', labelKey: 'nav.elevation' },
  { id: 'about', labelKey: 'nav.about' },
] as const;

type SettingsTab = (typeof SETTINGS_TABS)[number]['id'];

/** System: settings, elevation and about as three tabs (T080). */
export function Settings() {
  const [tab, setTab] = useState<SettingsTab>('general');

  return (
    <ScreenFrameTabs
      idBase="settings"
      tabs={SETTINGS_TABS.map((item) => ({ id: item.id, label: t(item.labelKey) }))}
      active={tab}
      onChange={(id) => {
        setTab(id as SettingsTab);
      }}
    >
      {(id) => {
        switch (id) {
          case 'general':
            return <SettingsPanel />;
          case 'elevation':
            return <Elevation />;
          default:
            return <About />;
        }
      }}
    </ScreenFrameTabs>
  );
}
