import { useCallback, useEffect, useMemo, useState } from 'react';

import { api } from '../api/runtime';
import type {
  DuplicateGroupView,
  DuplicateTask,
  FilterSet,
  UiError,
} from '../api/dto';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Badge, Checkbox } from '../components/Choice';
import { EmptyState, ErrorState, Progress } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { emptyFilter } from '../lib/filters';
import { Field, Select } from '../components/Fields';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { t } from '../i18n';
import { formatBytes, formatDate, formatNumber } from '../i18n/format';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './analysis.module.css';

const TERMINAL = new Set(['completed', 'partial', 'failed', 'cancelled']);
const RULES = ['oldest', 'newest', 'shortest', 'original-dir'];

/** Duplicate groups with explicit copy selection and a guarded delete preview. */
export function Duplicates() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [minSize, setMinSize] = useState(1);
  const [algorithm, setAlgorithm] = useState('sha256');
  const [rule, setRule] = useState('oldest');
  const [filter, setFilter] = useState<FilterSet>(emptyFilter());
  const [taskId, setTaskId] = useState<string | null>(null);
  const [task, setTask] = useState<DuplicateTask | null>(null);
  const [groups, setGroups] = useState<DuplicateGroupView[]>([]);
  const [selection, setSelection] = useState<Record<string, ReadonlySet<number>>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [guardError, setGuardError] = useState<string | null>(null);
  const operation = useOperation();

  const applySelection = useCallback((nextGroups: DuplicateGroupView[]) => {
    setGroups(nextGroups);
    setSelection(
      Object.fromEntries(
        nextGroups.map((group) => [
          group.id,
          new Set(
            group.members
              .filter((member) => member.selected_for_deletion)
              .map((member) => member.node_id),
          ),
        ]),
      ),
    );
  }, []);

  const start = async () => {
    if (generation === null) return;
    setLoading(true);
    setError(null);
    setGuardError(null);
    setGroups([]);
    setSelection({});
    try {
      const started = await api.duplicatesStart({
        path: useSnapshotStore.getState().volume?.mount_point ?? 'C:\\',
        min_size: minSize,
        algorithm,
        keep_rule: rule,
        exclusions: [],
      });
      setTaskId(started.task_id);
    } catch (cause) {
      setError(normalizeError(cause));
      setLoading(false);
    }
  };

  useEffect(() => {
    if (taskId === null) return undefined;
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async () => {
      try {
        const status = await api.duplicatesStatus(taskId);
        if (!active) return;
        setTask(status);
        if (TERMINAL.has(status.state)) {
          const page = await api.duplicatesGroupsPage(taskId, { pageSize: 500 });
          if (!active) return;
          applySelection(page.groups);
          setLoading(false);
        } else {
          timer = setTimeout(() => void tick(), 400);
        }
      } catch (cause) {
        if (active) {
          setError(normalizeError(cause));
          setLoading(false);
        }
      }
    };

    void tick();

    return () => {
      active = false;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [applySelection, taskId]);

  const visibleGroups = useMemo(() => {
    const needle = filter.text.trim().toLowerCase();
    if (needle === '') return groups;

    return groups.filter((group) =>
      group.members.some((member) => member.path.toLowerCase().includes(needle)),
    );
  }, [filter.text, groups]);

  const targetPaths = useMemo(
    () =>
      visibleGroups.flatMap((group) =>
        group.members
          .filter((member) => selection[group.id]?.has(member.node_id) === true)
          .map((member) => member.path),
      ),
    [selection, visibleGroups],
  );

  const toggle = (group: DuplicateGroupView, nodeId: number, checked: boolean) => {
    setGuardError(null);
    setSelection((current) => {
      const members = new Set(current[group.id] ?? []);
      if (checked) members.add(nodeId);
      else members.delete(nodeId);
      if (members.size >= group.members.length) {
        setGuardError(t('duplicates.keep_one'));
        return current;
      }
      if (members.size === 0) {
        setGuardError(t('duplicates.keep_one'));
        return current;
      }
      return { ...current, [group.id]: members };
    });
  };

  if (generation === null) {
    return (
      <EmptyState
        title={t('analysis.scan_required')}
        description={t('scan.required')}
        actions={
          <Button variant="primary" onClick={() => useShellStore.getState().setScreen('scan')}>
            {t('scan.start')}
          </Button>
        }
      />
    );
  }

  return (
    <div className={styles.stack}>
      <Card title={t('nav.duplicates')}>
        <p>{t('duplicates.intro')}</p>
        <div className={styles.toolbar}>
          <Field id="duplicates-min-size" label={t('duplicates.min_size')}>
            <input
              id="duplicates-min-size"
              className={local.input}
              type="number"
              min={1}
              value={minSize}
              onChange={(event) => setMinSize(Math.max(1, Number(event.target.value)))}
            />
          </Field>
          <Field id="duplicates-algorithm" label={t('duplicates.algorithm')}>
            <Select
              id="duplicates-algorithm"
              value={algorithm}
              options={[
                { value: 'sha256', label: 'SHA-256' },
                { value: 'md5', label: 'MD5' },
              ]}
              onChange={(event) => setAlgorithm(event.target.value)}
            />
          </Field>
          <Field id="duplicates-rule" label={t('duplicates.keep_rule')}>
            <Select
              id="duplicates-rule"
              value={rule}
              options={RULES.map((value) => ({ value, label: t(`duplicates.rule.${value}`) }))}
              onChange={(event) => setRule(event.target.value)}
            />
          </Field>
          <span className={styles.spacer} />
          <Button variant="primary" loading={loading} onClick={() => void start()}>
            {t('duplicates.search')}
          </Button>
        </div>
      </Card>

      <FilterBar screen="duplicates" filter={filter} onChange={setFilter} />

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={<Button variant="primary" onClick={() => void start()}>{t('error.retry')}</Button>}
        />
      ) : null}

      {loading || (task !== null && !TERMINAL.has(task.state)) ? (
        <Card title={t('duplicates.searching')}>
          <Progress label={t('duplicates.searching')} state="indeterminate" />
          {task !== null ? (
            <p className={local.numeric}>
              {t('duplicates.hashed', { count: formatNumber(task.hashed, language) })}
            </p>
          ) : null}
        </Card>
      ) : null}

      {task !== null && TERMINAL.has(task.state) ? (
        <div className={styles.metaRow}>
          <span>{t('duplicates.groups', { count: task.groups_count })}</span>
          <span>{t('duplicates.reclaimable', { size: formatBytes(task.reclaimable_bytes, units) })}</span>
          <span>{t('duplicates.hashed', { count: formatNumber(task.hashed, language) })}</span>
          <Button
            variant="danger"
            disabled={targetPaths.length === 0}
            onClick={() => {
              if (taskId === null) return;
              void operation.prepare({
                kind: 'delete',
                targets: targetPaths,
                delete_mode: 'quarantine',
                reason: 'duplicate cleanup',
                task_id: taskId,
              });
            }}
          >
            {t('analysis.delete_selected', { count: targetPaths.length })}
          </Button>
        </div>
      ) : null}

      {guardError !== null ? <p role="alert">{guardError}</p> : null}

      {task !== null && TERMINAL.has(task.state) && visibleGroups.length === 0 ? (
        <EmptyState title={t('duplicates.empty')} />
      ) : null}

      {visibleGroups.map((group) => (
        <Card
          key={group.id}
          title={
            <span className={local.numeric}>
              {formatBytes(group.size, units)} · {group.hash.slice(0, 12)}…
            </span>
          }
          actions={
            group.stale ? (
              <Badge tone="warning">{t('duplicates.stale')}</Badge>
            ) : (
              <Badge tone="info">
                {t('duplicates.reclaimable', { size: formatBytes(group.reclaimable_bytes, units) })}
              </Badge>
            )
          }
        >
          <table className={local.table} aria-label={t('nav.duplicates')}>
            <thead>
              <tr>
                <th scope="col">{t('duplicates.delete')}</th>
                <th scope="col">{t('top.path')}</th>
                <th scope="col" className={local.numeric}>{t('top.modified')}</th>
                <th scope="col">{t('duplicates.kept_hint')}</th>
              </tr>
            </thead>
            <tbody>
              {group.members.map((member) => (
                <tr key={member.node_id}>
                  <td>
                    <Checkbox
                      id={`duplicate-${group.id}-${member.node_id}`}
                      checked={selection[group.id]?.has(member.node_id) === true}
                      disabled={group.stale || !group.deletable}
                      label={<span className={local.visuallyHidden}>{t('duplicates.delete')}</span>}
                      onChange={(event) => {
                        toggle(group, member.node_id, event.target.checked);
                      }}
                    />
                  </td>
                  <td className={styles.pathCell} title={member.path}>
                    {member.path}
                  </td>
                  <td className={local.numeric}>
                    {formatDate(member.modified_at, language)}
                  </td>
                  <td>{member.reason}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Card>
      ))}

      {operation.progress !== null ? (
        <OperationProgressView
          progress={operation.progress}
          onCancel={() => void operation.cancel()}
          onUndo={() => void operation.undo()}
        />
      ) : null}
      <OperationPreviewDialog
        open={operation.preview !== null}
        preview={operation.preview}
        busy={operation.busy}
        error={operation.error}
        onCancel={operation.close}
        onConfirm={(mode, extra) => void operation.confirm(mode, extra)}
      />
    </div>
  );
}
