import { useState } from 'react';

import type { RenamePreview, UiError } from '../api/dto';
import { applyRename, requestRenamePreview } from '../api/filesystem';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Badge, Checkbox } from '../components/Choice';
import { Banner, EmptyState, Loading } from '../components/Feedback';
import { Field } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import local from './fileops.module.css';

/**
 * Bulk rename with an old-to-new diff and conflict highlighting.
 *
 * The preview is the only thing the user edits: the pattern, the replacement
 * and the optional scope produce a plan, conflicts disable the apply button and
 * the token is single-use, so a second click can never rename twice.
 */
export function Rename() {
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const [targets, setTargets] = useState('');
  const [pattern, setPattern] = useState('');
  const [replacement, setReplacement] = useState('');
  const [scope, setScope] = useState('');
  const [preview, setPreview] = useState<RenamePreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [applying, setApplying] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [applied, setApplied] = useState<number | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<UiError | null>(null);

  const previewNow = async () => {
    const targetPaths = parseTargets(targets);
    if (targetPaths.length === 0) {
      setPreview(null);
      setError(formError('rename.files'));
      return;
    }
    if (pattern.trim() === '') {
      setPreview(null);
      setError(formError('rename.pattern'));
      return;
    }

    setLoading(true);
    setError(null);
    setNotice(null);
    setConfirmed(false);
    setApplied(null);
    try {
      setPreview(
        await requestRenamePreview({
          targets: targetPaths,
          regex: pattern,
          replacement,
          scope: scope.trim() === '' ? null : scope.trim(),
          case: 'preserve',
        }),
      );
    } catch (cause) {
      setError(normalizeError(cause));
      setPreview(null);
    } finally {
      setLoading(false);
    }
  };

  const apply = async () => {
    if (preview === null || !confirmed || applying || applied !== null) return;

    setApplying(true);
    setError(null);
    try {
      const result = await applyRename(preview.preview_id, preview.confirmation_token);
      setApplied(result.renamed);
      setNotice(
        t('rename.done', {
          count: formatNumber(result.renamed, language),
        }),
      );
      setPreview(null);
      setConfirmed(false);
    } catch (cause) {
      const normalized = normalizeError(cause);
      setError(normalized);
      setConfirmed(false);
      if (normalized.code === 'E_PREVIEW_EXPIRED') {
        // The token is single-use: require a fresh preview before retrying.
        setPreview(null);
      }
    } finally {
      setApplying(false);
    }
  };

  const blocked = preview !== null && preview.conflicts.length > 0;
  const canApply = preview !== null && !blocked && confirmed && applied === null && !applying;

  return (
    <div className={local.stack}>
      <Panel title={t('nav.rename')} hint={t('fileops.rename.hint')}>
        <div className={local.form}>
          <Field id="rename-targets" label={t('rename.files')}>
            <textarea
              id="rename-targets"
              className={local.textarea}
              value={targets}
              spellCheck={false}
              onChange={(event) => {
                setTargets(event.target.value);
              }}
            />
          </Field>
        </div>
        <div className={local.fields}>
          <Field id="rename-pattern" label={t('rename.pattern')}>
            <input
              id="rename-pattern"
              className={local.input}
              value={pattern}
              spellCheck={false}
              onChange={(event) => {
                setPattern(event.target.value);
              }}
            />
          </Field>
          <Field id="rename-replacement" label={t('rename.replacement')}>
            <input
              id="rename-replacement"
              className={local.input}
              value={replacement}
              spellCheck={false}
              onChange={(event) => {
                setReplacement(event.target.value);
              }}
            />
          </Field>
          <Field id="rename-scope" label={t('long_paths.destination')}>
            <input
              id="rename-scope"
              className={local.input}
              value={scope}
              spellCheck={false}
              onChange={(event) => {
                setScope(event.target.value);
              }}
            />
          </Field>
          <div className={local.actions}>
            <Button variant="primary" loading={loading} onClick={() => void previewNow()}>
              {t('rename.preview')}
            </Button>
          </div>
        </div>
        {notice !== null ? <Banner tone="info">{notice}</Banner> : null}
      </Panel>

      {error !== null ? (
        <Banner
          tone="error"
          action={
            <Button variant="ghost" loading={loading} onClick={() => void previewNow()}>
              {t('rename.preview')}
            </Button>
          }
        >
          {t(error.message_key)}
        </Banner>
      ) : null}

      {loading && preview === null ? (
        <Panel title={t('rename.preview')}>
          <Loading label={t('common.loading')} rows={2} />
        </Panel>
      ) : null}

      {preview === null && !loading && error === null && notice === null ? (
        <EmptyState title={t('rename.preview')} description={t('fileops.rename.empty')} />
      ) : null}

      {preview !== null ? (
        <Panel
          title={t('rename.preview')}
          actions={
            blocked ? (
              <Badge tone="danger">{t('fileops.rename.blocked')}</Badge>
            ) : (
              <Badge tone="success">{t('journal.result.success')}</Badge>
            )
          }
        >
          <StatRow>
            <Stat
              label={t('fileops.rename.changes')}
              value={formatNumber(preview.change_count, language)}
            />
            <Stat
              label={t('fileops.rename.conflicts')}
              value={formatNumber(preview.conflicts.length, language)}
            />
            <Stat label={t('top.size')} value={formatBytes(preview.total_size, units)} />
          </StatRow>
          <p className={local.note}>
            {t('rename.plan', {
              count: formatNumber(preview.change_count, language),
              conflicts: formatNumber(preview.conflicts.length, language),
            })}
          </p>
          {blocked ? (
            <Banner tone="error">{t('fileops.rename.blocked')}</Banner>
          ) : null}
          {preview.requires_elevation ? (
            <Banner tone="warning">{t('confirm.elevation_required')}</Banner>
          ) : null}
          {preview.warnings.length > 0 ? (
            <ul className={local.warnings}>
              {preview.warnings.map((warning) => (
                <li key={warning}>{t(warning)}</li>
              ))}
            </ul>
          ) : null}
          <table className={local.table} aria-label={t('rename.preview')}>
            <thead>
              <tr>
                <th scope="col">{t('details.path')}</th>
                <th scope="col">{t('fileops.rename.before')}</th>
                <th scope="col">{t('fileops.rename.after')}</th>
                <th scope="col">{t('fileops.rename.new_path')}</th>
                <th scope="col">{t('journal.result_label')}</th>
              </tr>
            </thead>
            <tbody>
              {preview.rows.map((row) => (
                <tr key={row.old_path} aria-invalid={row.conflict || undefined}>
                  <td>
                    <span className={local.pathText} title={row.old_path}>
                      {row.old_path}
                    </span>
                  </td>
                  <td>
                    <span className={local.pathText} title={row.old_name}>
                      {row.old_name}
                    </span>
                  </td>
                  <td>
                    <span className={local.pathText} title={row.new_name}>
                      {row.new_name}
                    </span>
                  </td>
                  <td>
                    <span className={local.pathText} title={row.new_path}>
                      {row.new_path}
                    </span>
                  </td>
                  <td>
                    {row.conflict ? (
                      <Badge tone="danger">{t('error.validation')}</Badge>
                    ) : (
                      <Badge tone="success">{t('journal.result.success')}</Badge>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {preview.conflicts.length > 0 ? (
            <ul className={local.warnings}>
              {preview.conflicts.map((conflict) => (
                <li key={`${conflict.path}-${conflict.reason}`}>
                  {t(conflict.reason_key)}
                  {conflict.target === null ? '' : ` · ${conflict.target}`}
                </li>
              ))}
            </ul>
          ) : null}

          <Checkbox
            id="rename-confirm"
            checked={confirmed}
            disabled={blocked || applying}
            label={t('rename.confirm')}
            onChange={(event) => {
              setConfirmed(event.target.checked);
            }}
          />

          <div className={local.actionsEnd}>
            <Button
              variant="primary"
              loading={applying}
              disabled={!canApply}
              onClick={() => void apply()}
            >
              {t('rename.apply')}
            </Button>
          </div>
        </Panel>
      ) : null}
    </div>
  );
}

function parseTargets(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line !== '');
}

function formError(details: string): UiError {
  return {
    code: 'E_VALIDATION',
    category: 'validation',
    message_key: 'error.validation',
    details,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };
}
