import { useCallback, useEffect, useRef, useState } from 'react';

import type { AttributeFlags, SizeDetailsView, UiError } from '../api/dto';
import { fetchSizeDetails, findNodeByPath } from '../api/filesystem';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Badge } from '../components/Choice';
import { EmptyState, ErrorState, Loading } from '../components/Feedback';
import { Field } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './filesystem.module.css';

const ATTRIBUTE_LABEL: Record<keyof AttributeFlags, string> = {
  read_only: 'permissions.attributes',
  hidden: 'attr.hidden',
  system: 'attr.system',
  compressed: 'attr.compressed',
  sparse: 'attr.sparse',
  reparse: 'attr.reparse',
};

const ATTRIBUTE_NAMES: readonly (keyof AttributeFlags)[] = [
  'read_only',
  'hidden',
  'system',
  'compressed',
  'sparse',
  'reparse',
];

/** Logical, on-disk, physical and allocated size of the selected node. */
export function SizeDetails() {
  const generation = useSnapshotStore((state) => state.generation);
  const pendingNodeId = useShellStore((state) => state.pendingNodeId);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const [path, setPath] = useState('');
  const [view, setView] = useState<SizeDetailsView | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [copied, setCopied] = useState(false);
  const autoLoaded = useRef<number | null>(null);

  const inspect = useCallback(
    async (target: string) => {
      if (generation === null) return;
      const cleaned = target.trim();
      if (cleaned === '') {
        setView(null);
        setError(missingPathError());
        return;
      }

      setLoading(true);
      setError(null);
      setCopied(false);
      try {
        const node = await findNodeByPath(generation, cleaned);
        if (node === null) throw notFoundError(cleaned);
        const details = await fetchSizeDetails(generation, node.id);
        setView(details);
        setPath(details.path);
      } catch (cause) {
        setError(normalizeError(cause));
        setView(null);
      } finally {
        setLoading(false);
      }
    },
    [generation],
  );

  const inspectNode = useCallback(
    async (nodeId: number) => {
      if (generation === null) return;
      setLoading(true);
      setError(null);
      setCopied(false);
      try {
        const details = await fetchSizeDetails(generation, nodeId);
        setView(details);
        setPath(details.path);
      } catch (cause) {
        setError(normalizeError(cause));
        setView(null);
      } finally {
        setLoading(false);
      }
    },
    [generation],
  );

  useEffect(() => {
    if (generation !== null && pendingNodeId !== null) {
      void inspectNode(pendingNodeId);
    }
  }, [generation, inspectNode, pendingNodeId]);

  useEffect(() => {
    if (generation === null) {
      autoLoaded.current = null;
      setView(null);
      setError(null);
      return;
    }
    if (pendingNodeId !== null) return;
    if (autoLoaded.current === generation) return;
    autoLoaded.current = generation;
    const mountPoint = useSnapshotStore.getState().volume?.mount_point ?? 'C:\\';
    setPath(mountPoint);
    void inspect(mountPoint);
  }, [generation, inspect, pendingNodeId]);

  const copyPath = async () => {
    if (view === null) return;
    try {
      await navigator.clipboard.writeText(view.path);
    } catch {
      // The host may deny clipboard access; the path is still visible and focusable.
    }
    setCopied(true);
  };

  if (generation === null) {
    return (
      <EmptyState
        title={t('analysis.scan_required')}
        description={t('scan.required')}
        actions={
          <Button
            variant="primary"
            onClick={() => {
              useShellStore.getState().setScreen('scan');
            }}
          >
            {t('scan.start')}
          </Button>
        }
      />
    );
  }

  return (
    <div className={styles.stack}>
      <Card title={t('nav.size_details')}>
        <p className={local.note}>{t('details.intro')}</p>
        <div className={styles.toolbar}>
          <Field id="size-details-path" label={t('details.path')}>
            <input
              id="size-details-path"
              className={local.input}
              value={path}
              spellCheck={false}
              onChange={(event) => {
                setPath(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  void inspect(path);
                }
              }}
            />
          </Field>
          <Button variant="primary" loading={loading} onClick={() => void inspect(path)}>
            {t('details.inspect')}
          </Button>
          {copied ? <span role="status">{t('about.copied')}</span> : null}
        </div>
      </Card>

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void inspect(path)}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {loading && view === null ? (
        <Card title={t('details.intro')}>
          <Loading label={t('common.loading')} />
        </Card>
      ) : null}

      {!loading && view === null && error === null ? (
        <EmptyState title={t('details.empty')} description={t('details.empty_hint')} />
      ) : null}

      {view !== null ? (
        <Card
          title={t('details.path')}
          actions={
            <Button variant="ghost" onClick={() => void copyPath()}>
              {t('about.copy')}
            </Button>
          }
        >
          <p className={local.mono}>{view.path}</p>
          <div className={local.statusRow}>
            {view.estimated ? (
              <Badge tone="warning">{t('common.warning')}</Badge>
            ) : null}
            {ATTRIBUTE_NAMES.filter((name) => view.attributes[name]).map((name) => (
              <Badge key={name}>{t(ATTRIBUTE_LABEL[name])}</Badge>
            ))}
          </div>

          <dl className={local.metrics}>
            <Metric label={t('details.logical')} value={formatBytes(view.logical_size, units)} />
            <Metric label={t('details.size')} value={formatBytes(view.on_disk_size, units)} />
            <Metric label={t('details.physical')} value={formatBytes(view.physical_size, units)} />
            <Metric
              label={t('details.allocated')}
              value={formatBytes(view.allocated_size, units)}
            />
            <Metric
              label={t('tree.hardlink')}
              value={formatBytes(view.hardlink_contribution, units)}
            />
            <Metric label={t('details.ads')} value={formatBytes(view.ads_size, units)} />
            <Metric
              label={t('attr.compressed')}
              value={formatBytes(view.compression_savings, units)}
            />
            <Metric
              label={t('attr.sparse')}
              value={formatBytes(view.sparse_savings, units)}
            />
          </dl>

          <dl className={local.metrics}>
            <Metric label={t('details.kind')} value={t(view.metric_label)} />
            {view.link_target !== null ? (
              <Metric label={t('details.reparse')} value={view.link_target} mono />
            ) : null}
          </dl>
        </Card>
      ) : null}
    </div>
  );
}

function Metric({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className={local.metric}>
      <dt className={local.metricLabel}>{label}</dt>
      <dd className={mono ? `${local.mono} ${local.metricValue}` : local.metricValue}>
        {value}
      </dd>
    </div>
  );
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

function notFoundError(path: string): Error {
  const envelope: UiError = {
    code: 'E_NOT_FOUND',
    category: 'io',
    message_key: 'error.not_found',
    details: path,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };

  return Object.assign(new Error(path), envelope);
}
