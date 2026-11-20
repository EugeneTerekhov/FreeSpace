import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Check, File, Trash2, TriangleAlert } from 'lucide-react';

import { api } from '../api/runtime';
import type {
  DuplicateGroupView,
  DuplicateMemberView,
  DuplicateTask,
  FilterSet,
  UiError,
} from '../api/dto';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Badge, Checkbox } from '../components/Choice';
import { ExportAction } from '../components/ExportAction';
import { Banner, EmptyState, ErrorState, Progress } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { Field, Select } from '../components/Fields';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { emptyFilter } from '../lib/filters';
import { t } from '../i18n';
import { formatBytes, formatDateTimeShort, formatNumber } from '../i18n/format';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import styles from './duplicates.module.css';

const TERMINAL = new Set(['completed', 'partial', 'failed', 'cancelled']);
const RULES = ['oldest', 'newest', 'shortest', 'original-dir'];

/** Copy kept by a rule, mirrored locally so the preselection follows the select. */
function keptMember(
  group: DuplicateGroupView,
  rule: string,
): DuplicateMemberView | undefined {
  const [first, ...rest] = group.members;
  if (first === undefined) return undefined;

  switch (rule) {
    case 'newest':
      return rest.reduce(
        (best, member) => (member.modified_at > best.modified_at ? member : best),
        first,
      );
    case 'shortest':
      return rest.reduce(
        (best, member) => (member.path.length < best.path.length ? member : best),
        first,
      );
    case 'original-dir':
      return (
        group.members.find((member) => member.node_id === group.keep_node_id) ?? first
      );
    case 'oldest':
    default:
      return rest.reduce(
        (best, member) => (member.modified_at < best.modified_at ? member : best),
        first,
      );
  }
}

/** Everything except the kept copy is preselected for the preview. */
function selectionFor(
  groups: readonly DuplicateGroupView[],
  rule: string,
): Record<string, ReadonlySet<number>> {
  return Object.fromEntries(
    groups.map((group) => {
      if (group.stale || !group.deletable) return [group.id, new Set<number>()];

      const kept = keptMember(group, rule);

      return [
        group.id,
        new Set(
          group.members
            .filter((member) => member.node_id !== kept?.node_id)
            .map((member) => member.node_id),
        ),
      ];
    }),
  );
}

/** Duplicate groups with an explicit keep rule and a guarded delete preview. */
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
  const ruleRef = useRef(rule);
  ruleRef.current = rule;

  const applyGroups = useCallback((nextGroups: DuplicateGroupView[]) => {
    setGroups(nextGroups);
    setSelection(selectionFor(nextGroups, ruleRef.current));
  }, []);

  const start = async () => {
    if (generation === null) return;
    setLoading(true);
    setError(null);
    setGuardError(null);
    setTask(null);
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
          applyGroups(page.groups);
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
  }, [applyGroups, taskId]);

  const visibleGroups = useMemo(() => {
    const needle = filter.text.trim().toLowerCase();
    if (needle === '') return groups;

    return groups.filter((group) =>
      group.members.some((member) => member.path.toLowerCase().includes(needle)),
    );
  }, [filter.text, groups]);

  const selectedMembers = useMemo(
    () =>
      visibleGroups.flatMap((group) =>
        group.members.filter(
          (member) => selection[group.id]?.has(member.node_id) === true,
        ),
      ),
    [selection, visibleGroups],
  );
  const targetPaths = useMemo(
    () => selectedMembers.map((member) => member.path),
    [selectedMembers],
  );
  const selectedSize = useMemo(
    () => selectedMembers.reduce((sum, member) => sum + member.size, 0),
    [selectedMembers],
  );
  const copies = useMemo(
    () => visibleGroups.reduce((sum, group) => sum + group.members.length, 0),
    [visibleGroups],
  );
  const reclaimable = useMemo(
    () => visibleGroups.reduce((sum, group) => sum + group.reclaimable_bytes, 0),
    [visibleGroups],
  );
  const terminal = task !== null && TERMINAL.has(task.state);
  const searching = loading || (task !== null && !terminal);

  const changeRule = (nextRule: string) => {
    setRule(nextRule);
    setGuardError(null);
    setSelection(selectionFor(groups, nextRule));
  };

  const toggle = (group: DuplicateGroupView, nodeId: number, checked: boolean) => {
    setGuardError(null);
    const members = new Set(selection[group.id] ?? []);
    if (checked) members.add(nodeId);
    else members.delete(nodeId);
    // A group always contributes at least one copy to the preview: deleting all
    // copies is never allowed, and the previous flow also kept a group selected.
    if (members.size >= group.members.length || members.size === 0) {
      setGuardError(t('duplicates.keep_one'));
      return;
    }
    setSelection((current) => ({ ...current, [group.id]: members }));
  };

  if (generation === null) {
    return (
      <EmptyState
        title={t('analysis.scan_required')}
        actions={
          <Button variant="primary" onClick={() => useShellStore.getState().setScreen('overview')}>
            {t('scan.start')}
          </Button>
        }
      />
    );
  }

  return (
    <div className={styles.stack}>
      <p className={styles.context} data-testid="duplicates-context">
        {t('duplicates_ui.context', {
          groups: formatNumber(task?.groups_count ?? visibleGroups.length, language),
          copies: formatNumber(copies, language),
          size: formatBytes(reclaimable, units),
        })}
      </p>

      <StatRow>
        <Stat
          label={t('duplicates_ui.stat_groups')}
          value={
            <span data-testid="duplicates-groups">
              {formatNumber(task?.groups_count ?? visibleGroups.length, language)}
            </span>
          }
        />
        <Stat
          label={t('duplicates_ui.stat_copies')}
          value={<span data-testid="duplicates-copies">{formatNumber(copies, language)}</span>}
        />
        <Stat
          label={t('duplicates_ui.stat_reclaimable')}
          value={
            <span data-testid="duplicates-reclaimable">{formatBytes(reclaimable, units)}</span>
          }
        />
        <Stat
          label={t('duplicates_ui.stat_selected')}
          value={
            <span data-testid="duplicates-selected">
              {formatNumber(selectedMembers.length, language)} · {formatBytes(selectedSize, units)}
            </span>
          }
        />
      </StatRow>

      <Panel title={t('nav.duplicates')}>
        <div className={styles.toolbar}>
          <Field id="duplicates-min-size" label={t('duplicates.min_size')}>
            <input
              id="duplicates-min-size"
              className={styles.input}
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
              onChange={(event) => changeRule(event.target.value)}
            />
          </Field>
          <span className={styles.spacer} />
          <ExportAction sections={['duplicates']} />
          <Button variant="primary" loading={searching} onClick={() => void start()}>
            {t('duplicates.search')}
          </Button>
        </div>
        <p className={styles.ruleHint}>{t('duplicates_ui.rule_hint')}</p>
      </Panel>

      <FilterBar filter={filter} onChange={setFilter} />

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void start()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {searching ? (
        <Panel title={t('duplicates.searching')}>
          <Progress
            label={t('duplicates.searching')}
            state="indeterminate"
          />
          {task !== null ? (
            <p className={styles.context}>
              {t('duplicates.hashed', { count: formatNumber(task.hashed, language) })}
            </p>
          ) : null}
        </Panel>
      ) : null}

      {terminal && task !== null ? (
        task.state === 'partial' ? (
          <Banner tone="warning">{t('duplicates_ui.partial')}</Banner>
        ) : task.state === 'failed' ? (
          <Banner tone="error">
            {t(task.error?.message_key ?? 'common.error')}
            {task.error === null || task.error === undefined ? '' : ` · ${task.error.code}`}
          </Banner>
        ) : task.state === 'completed' ? (
          <Banner tone="info">
            {t('duplicates.hashed', { count: formatNumber(task.hashed, language) })}
          </Banner>
        ) : null
      ) : null}

      {guardError !== null ? (
        <p className={styles.guard} role="alert">
          {guardError}
        </p>
      ) : null}

      {task !== null &&
      (task.state === 'completed' || task.state === 'partial') &&
      visibleGroups.length === 0 ? (
        <EmptyState
          title={filter.text.trim() === '' ? t('duplicates.empty') : t('tree.no_matches')}
        />
      ) : null}

      {visibleGroups.map((group, index) => {
        const verified = group.members.every((member) => member.verified);

        return (
          <Panel
            key={group.id}
            title={
              <span className={styles.mono}>
                <span data-testid={`duplicates-group-${index + 1}-size`}>
                  {formatBytes(group.size, units)}
                </span>
                {' · '}
                {group.hash.slice(0, 12)}…
              </span>
            }
            actions={
              group.stale ? (
                <Badge tone="warning">
                  <TriangleAlert size={11} strokeWidth={2} aria-hidden="true" />
                  {t('duplicates.stale')}
                </Badge>
              ) : verified ? (
                <Badge tone="success">
                  <Check size={11} strokeWidth={2.4} aria-hidden="true" />
                  {t('duplicates.identical')}
                </Badge>
              ) : (
                <Badge tone="info">
                  {t('duplicates.reclaimable', {
                    size: formatBytes(group.reclaimable_bytes, units),
                  })}
                </Badge>
              )
            }
          >
            <table className={styles.groupTable} aria-label={t('nav.duplicates')}>
              <thead>
                <tr>
                  <th scope="col" className={styles.checkCell}>
                    <span className={styles.visuallyHidden}>{t('duplicates.delete')}</span>
                  </th>
                  <th scope="col">{t('top.path')}</th>
                  <th scope="col">{t('top.modified')}</th>
                  <th scope="col" className={styles.sizeCell}>
                    {t('top.size')}
                  </th>
                  <th scope="col">{t('unlock.state')}</th>
                </tr>
              </thead>
              <tbody>
                {group.members.map((member) => {
                  const dropped = selection[group.id]?.has(member.node_id) === true;
                  const disabled = group.stale || !group.deletable;

                  return (
                    <tr
                      key={member.node_id}
                      className={dropped ? styles.dropRow : styles.keepRow}
                    >
                      <td className={styles.checkCell}>
                        <Checkbox
                          id={`duplicate-${group.id}-${member.node_id}`}
                          checked={dropped}
                          disabled={disabled}
                          aria-label={`${t('duplicates.delete')}: ${member.name}`}
                          label={<span className={styles.visuallyHidden}>{t('duplicates.delete')}</span>}
                          onChange={(event) => {
                            toggle(group, member.node_id, event.target.checked);
                          }}
                        />
                      </td>
                      <td>
                        <span className={styles.memberPath} title={member.path}>
                          <span
                            className={dropped ? styles.fileDrop : styles.fileKeep}
                            aria-hidden="true"
                          >
                            {dropped ? (
                              <Trash2 size={13} strokeWidth={1.7} />
                            ) : (
                              <File size={13} strokeWidth={1.7} />
                            )}
                          </span>
                          <span className={styles.pathCell}>{member.path}</span>
                        </span>
                      </td>
                      <td>{formatDateTimeShort(member.modified_at)}</td>
                      <td className={styles.sizeCell}>{formatBytes(member.size, units)}</td>
                      <td className={styles.stateCell}>
                        {dropped ? (
                          <Badge tone="danger">{t('duplicates.delete')}</Badge>
                        ) : (
                          <Badge tone="success">{t('duplicates.keep')}</Badge>
                        )}
                        {member.reason === '' ? null : (
                          <span className={styles.reason}>{member.reason}</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Panel>
        );
      })}

      {groups.length > 0 ? (
        <Panel>
          <div className={styles.summary}>
            <span>
              {t('duplicates_ui.selected', {
                count: formatNumber(selectedMembers.length, language),
                total: formatNumber(copies, language),
                size: formatBytes(selectedSize, units),
              })}
            </span>
            <span className={styles.summarySpacer} />
            <Button
              variant="danger"
              disabled={targetPaths.length === 0}
              onClick={() => {
                if (taskId === null) return;
                void operation.prepare({
                  kind: 'delete',
                  targets: targetPaths,
                  delete_mode: 'recycle_bin',
                  reason: 'duplicate cleanup',
                  task_id: taskId,
                });
              }}
            >
              {t('analysis.delete_selected', { count: targetPaths.length })}
            </Button>
          </div>
        </Panel>
      ) : null}

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
