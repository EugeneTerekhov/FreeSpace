import { useCallback, useState } from 'react';

import type {
  FileAttributeName,
  PermissionsChanges,
  PermissionsPreview,
  PermissionsView,
  UiError,
} from '../api/dto';
import {
  applyPermissions,
  fetchPermissions,
  requestPermissionsPreview,
} from '../api/filesystem';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Badge, Checkbox, Toggle } from '../components/Choice';
import { Banner, EmptyState, Loading } from '../components/Feedback';
import { Field } from '../components/Fields';
import { t } from '../i18n';
import styles from './screens.module.css';
import local from './filesystem.module.css';

type AttributeChange = 'set' | 'clear';

const ATTRIBUTE_NAMES: readonly FileAttributeName[] = [
  'read-only',
  'hidden',
  'system',
  'compressed',
  'sparse',
];

/** Every wire attribute, used to describe a backend diff. */
const ALL_ATTRIBUTE_NAMES: readonly FileAttributeName[] = [
  'read-only',
  'hidden',
  'system',
  'archive',
  'temporary',
  'sparse',
  'compressed',
  'not-content-indexed',
];

const ATTRIBUTE_BITS: Record<FileAttributeName, number> = {
  'read-only': 0x1,
  hidden: 0x2,
  system: 0x4,
  archive: 0x20,
  temporary: 0x100,
  sparse: 0x200,
  compressed: 0x800,
  'not-content-indexed': 0x2000,
};

const ATTRIBUTE_LABEL: Record<FileAttributeName, string> = {
  // Four labels are missing from the catalogue and use the closest existing
  // keys; they are reported for the Phase 11 i18n pass.
  'read-only': 'permissions.attributes',
  hidden: 'attr.hidden',
  system: 'attr.system',
  archive: 'treemap.category.archive',
  temporary: 'cleanup.reason.temp',
  sparse: 'attr.sparse',
  compressed: 'attr.compressed',
  'not-content-indexed': 'permissions.attributes_hint',
};

interface DiffRow {
  id: string;
  targetKey: string;
  before: string | null;
  after: string | null;
  beforeKey: string | null;
  afterKey: string | null;
}

/** NTFS owner, attributes and ACL actions with a preview diff. */
export function Permissions() {
  const [path, setPath] = useState('');
  const [view, setView] = useState<PermissionsView | null>(null);
  const [owner, setOwner] = useState('');
  const [copyFrom, setCopyFrom] = useState('');
  const [resetAcl, setResetAcl] = useState(false);
  const [attributeChanges, setAttributeChanges] = useState<
    Partial<Record<FileAttributeName, AttributeChange>>
  >({});
  const [preview, setPreview] = useState<PermissionsPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [previewing, setPreviewing] = useState(false);
  const [applying, setApplying] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async (target: string) => {
    const trimmed = target.trim();
    if (trimmed === '') {
      setView(null);
      setPreview(null);
      setError(missingPathError());
      return;
    }

    setLoading(true);
    setError(null);
    setPreview(null);
    setConfirmed(false);
    try {
      const next = await fetchPermissions(trimmed);
      setView(next);
      setPath(next.path);
      setOwner(next.owner ?? '');
      setCopyFrom('');
      setResetAcl(false);
      setAttributeChanges({});
    } catch (cause) {
      setView(null);
      setError(normalizeError(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  const ownerChanged =
    view !== null && owner.trim() !== '' && owner.trim() !== (view.owner ?? '');
  const hasChanges =
    view !== null &&
    (ownerChanged ||
      copyFrom.trim() !== '' ||
      resetAcl ||
      Object.keys(attributeChanges).length > 0);

  const toggleAttribute = (name: FileAttributeName, next: boolean) => {
    if (view === null) return;
    const current = (view.attribute_mask & ATTRIBUTE_BITS[name]) !== 0;
    setAttributeChanges((changes) => {
      const attributes = { ...changes };
      if (next === current) {
        delete attributes[name];
      } else {
        attributes[name] = next ? 'set' : 'clear';
      }

      return attributes;
    });
    setPreview(null);
    setConfirmed(false);
  };

  const previewNow = async () => {
    if (view === null || !hasChanges) return;
    const changes = buildChanges(owner, ownerChanged, resetAcl, copyFrom, attributeChanges);

    setPreviewing(true);
    setError(null);
    setNotice(null);
    setConfirmed(false);
    try {
      setPreview(await requestPermissionsPreview(view.path, changes));
    } catch (cause) {
      setPreview(null);
      setError(normalizeError(cause));
    } finally {
      setPreviewing(false);
    }
  };

  const apply = async () => {
    if (preview === null || !confirmed || applying) return;

    setApplying(true);
    setError(null);
    try {
      const result = await applyPermissions(
        preview.preview_id,
        preview.confirmation_token,
      );
      setNotice(
        t('permissions.updated', {
          succeeded: result.changed,
          failed: result.failed,
        }),
      );
      setPreview(null);
      setConfirmed(false);
      setAttributeChanges({});
      if (view !== null) void load(view.path);
    } catch (cause) {
      const normalized = normalizeError(cause);
      setError(normalized);
      setConfirmed(false);
      if (normalized.code === 'E_PREVIEW_EXPIRED') {
        setPreview(null);
      }
    } finally {
      setApplying(false);
    }
  };

  const diffRows = preview === null ? [] : buildDiffRows(preview);

  return (
    <div className={styles.stack}>
      <Card title={t('nav.permissions')}>
        <p className={local.note}>{t('permissions.intro')}</p>
        <div className={styles.toolbar}>
          <Field id="permissions-path" label={t('details.path')}>
            <input
              id="permissions-path"
              className={local.input}
              value={path}
              spellCheck={false}
              onChange={(event) => {
                setPath(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  void load(path);
                }
              }}
            />
          </Field>
          <Button variant="primary" loading={loading} onClick={() => void load(path)}>
            {t('details.inspect')}
          </Button>
        </div>
        {notice !== null ? <p role="status">{notice}</p> : null}
      </Card>

      {error !== null ? (
        <Banner tone="error">
          {t(error.message_key)}
          {error.details !== null ? `: ${error.details}` : ''}
        </Banner>
      ) : null}

      {loading ? (
        <Card title={t('permissions.acl')}>
          <Loading label={t('common.loading')} />
        </Card>
      ) : null}

      {!loading && view === null && error === null ? (
        <EmptyState title={t('details.empty')} description={t('permissions.intro')} />
      ) : null}

      {view !== null ? (
        <>
          {view.requires_elevation ? (
            <Banner tone="warning">{t('confirm.elevation_required')}</Banner>
          ) : null}

          <div className={local.columns}>
            <Card title={t('permissions.acl')}>
              <p className={local.mono}>{view.path}</p>
              <dl className={local.metrics}>
                <div className={local.metric}>
                  <dt className={local.metricLabel}>{t('owners.owner')}</dt>
                  <dd className={local.mono}>{view.owner ?? '—'}</dd>
                </div>
                <div className={local.metric}>
                  <dt className={local.metricLabel}>{t('owners.sid')}</dt>
                  <dd className={local.mono}>{view.owner_sid ?? '—'}</dd>
                </div>
                <div className={local.metric}>
                  <dt className={local.metricLabel}>{t('details.kind')}</dt>
                  <dd className={local.mono}>
                    {t(view.is_directory ? 'kind.directory' : 'kind.file')}
                  </dd>
                </div>
              </dl>

              <div className={local.statusRow}>
                {view.is_protected ? (
                  <Badge tone="warning">{t('confirm.system')}</Badge>
                ) : null}
                {view.is_reparse_point ? (
                  <Badge tone="info">{t('attr.reparse')}</Badge>
                ) : null}
              </div>

              <div className={local.form}>
                <Field id="permissions-owner" label={t('permissions.owner')}>
                  <input
                    id="permissions-owner"
                    className={local.input}
                    value={owner}
                    onChange={(event) => {
                      setOwner(event.target.value);
                      setResetAcl(false);
                      setCopyFrom('');
                      setPreview(null);
                      setConfirmed(false);
                    }}
                  />
                </Field>
                <Field id="permissions-copy" label={t('permissions.copy_from')}>
                  <input
                    id="permissions-copy"
                    className={local.input}
                    value={copyFrom}
                    spellCheck={false}
                    onChange={(event) => {
                      setCopyFrom(event.target.value);
                      if (event.target.value.trim() !== '') {
                        setOwner('');
                        setResetAcl(false);
                      }
                      setPreview(null);
                      setConfirmed(false);
                    }}
                  />
                </Field>
              </div>
              <Checkbox
                id="permissions-reset"
                checked={resetAcl}
                label={t('permissions.reset')}
                onChange={(event) => {
                  setResetAcl(event.target.checked);
                  if (event.target.checked) {
                    setOwner('');
                    setCopyFrom('');
                  }
                  setPreview(null);
                  setConfirmed(false);
                }}
              />
            </Card>

            <Card title={t('permissions.attributes')}>
              <p className={local.note}>{t('permissions.attributes_hint')}</p>
              <div className={local.toggles}>
                {ATTRIBUTE_NAMES.map((name) => (
                  <Toggle
                    key={name}
                    checked={attributeValue(
                      view.attribute_mask,
                      attributeChanges,
                      name,
                    )}
                    label={t(ATTRIBUTE_LABEL[name])}
                    onCheckedChange={(next) => {
                      toggleAttribute(name, next);
                    }}
                  />
                ))}
              </div>
              <div className={local.statusRow}>
                {ATTRIBUTE_NAMES.filter(
                  (name) => (view.attribute_mask & ATTRIBUTE_BITS[name]) !== 0,
                ).map((name) => (
                  <Badge key={name}>{t(ATTRIBUTE_LABEL[name])}</Badge>
                ))}
              </div>
              <div className={local.actions}>
                <Button
                  variant="secondary"
                  loading={previewing}
                  disabled={!hasChanges}
                  onClick={() => void previewNow()}
                >
                  {t('rename.preview')}
                </Button>
              </div>
            </Card>
          </div>

          {preview !== null ? (
            <Card title={t('rename.preview')}>
              {preview.requires_elevation ? (
                <Banner tone="warning">{t('confirm.elevation_required')}</Banner>
              ) : null}
              <table className={local.table} aria-label={t('rename.preview')}>
                <thead>
                  <tr>
                    <th scope="col">{t('confirm.target')}</th>
                    <th scope="col">{t('snapshots.change.changed')}</th>
                  </tr>
                </thead>
                <tbody>
                  {diffRows.map((row) => (
                    <tr key={row.id}>
                      <td>{t(row.targetKey)}</td>
                      <td className={local.changeValue}>
                        {changeValue(row.before, row.beforeKey)}
                        {' → '}
                        {changeValue(row.after, row.afterKey)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {preview.warnings.length > 0 ? (
                <ul className={local.warnings}>
                  {preview.warnings.map((warning) => (
                    <li key={warning}>{t(warning)}</li>
                  ))}
                </ul>
              ) : null}

              <Checkbox
                id="permissions-confirm"
                checked={confirmed}
                disabled={applying}
                label={t('permissions.confirm')}
                onChange={(event) => {
                  setConfirmed(event.target.checked);
                }}
              />
              <div className={local.actionsEnd}>
                <Button
                  variant="primary"
                  loading={applying}
                  disabled={!confirmed || applying}
                  onClick={() => void apply()}
                >
                  {t('settings.save')}
                </Button>
              </div>
            </Card>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

function buildChanges(
  owner: string,
  ownerChanged: boolean,
  resetAcl: boolean,
  copyFrom: string,
  attributeChanges: Partial<Record<FileAttributeName, AttributeChange>>,
): PermissionsChanges {
  const changes: PermissionsChanges = {};
  if (ownerChanged) changes.owner = owner.trim();
  if (resetAcl) changes.reset = true;
  if (copyFrom.trim() !== '') changes.copy_from = copyFrom.trim();

  const set: FileAttributeName[] = [];
  const clear: FileAttributeName[] = [];
  for (const name of ALL_ATTRIBUTE_NAMES) {
    const change = attributeChanges[name];
    if (change === 'set') set.push(name);
    else if (change === 'clear') clear.push(name);
  }
  if (set.length > 0 || clear.length > 0) {
    changes.attributes = { set, clear };
  }

  return changes;
}

function buildDiffRows(preview: PermissionsPreview): DiffRow[] {
  const rows: DiffRow[] = [];
  if (preview.owner !== null) {
    rows.push({
      id: 'owner',
      targetKey: 'permissions.owner',
      before: preview.current.owner,
      after: preview.owner,
      beforeKey: null,
      afterKey: null,
    });
  }
  if (preview.acl_action === 'reset') {
    rows.push({
      id: 'reset',
      targetKey: 'permissions.reset',
      before: null,
      after: null,
      beforeKey: 'permissions.acl',
      afterKey: 'permissions.reset',
    });
  }
  if (preview.acl_action === 'copy') {
    rows.push({
      id: 'copy',
      targetKey: 'permissions.copy_from',
      before: null,
      after: preview.copy_from,
      beforeKey: null,
      afterKey: null,
    });
  }
  for (const change of preview.attribute_changes) {
    rows.push({
      id: `attribute-${change.attribute}`,
      targetKey: ATTRIBUTE_LABEL[change.attribute],
      before: null,
      after: null,
      beforeKey: change.before ? 'permissions.set' : 'permissions.clear',
      afterKey: change.after ? 'permissions.set' : 'permissions.clear',
    });
  }

  return rows;
}

function attributeValue(
  mask: number,
  changes: Partial<Record<FileAttributeName, AttributeChange>>,
  name: FileAttributeName,
): boolean {
  const change = changes[name];
  if (change !== undefined) return change === 'set';

  return (mask & ATTRIBUTE_BITS[name]) !== 0;
}

function changeValue(value: string | null, key: string | null): string {
  if (key !== null) return t(key);

  return value ?? '—';
}

function missingPathError(): UiError {
  return {
    code: 'E_VALIDATION',
    category: 'validation',
    message_key: 'details.path_required',
    details: null,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };
}
