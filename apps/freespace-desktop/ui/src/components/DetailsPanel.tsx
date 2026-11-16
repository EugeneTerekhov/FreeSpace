import { Badge } from './Choice';
import { Breadcrumbs } from '../app/AppShell';
import { Card } from './Card';
import { EmptyState } from './Feedback';
import type { NodeRow } from '../api/dto';
import { t } from '../i18n';
import { formatBytes, formatDate, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import styles from './DetailsPanel.module.css';

export interface DetailsPanelProps {
  node: NodeRow | null;
  ancestors?: NodeRow[];
  emptyLabel?: string;
}

/** Read-only details of the selected row; sizes come from Rust unchanged. */
export function DetailsPanel({ node, ancestors = [], emptyLabel }: DetailsPanelProps) {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');

  if (node === null) {
    return (
      <EmptyState
        title={emptyLabel ?? t('details.empty')}
        description={t('details.empty_hint')}
      />
    );
  }

  return (
    <Card title={node.name}>
      <div className={styles.stack}>
        {ancestors.length > 0 ? (
          <Breadcrumbs
            items={ancestors.map((ancestor) => ({ label: ancestor.name }))}
          />
        ) : null}
        <p className={`mono ${styles.path}`}>{node.path}</p>
        <dl className={styles.grid}>
          <dt>{t('details.kind')}</dt>
          <dd>{t(`kind.${node.kind}`)}</dd>
          <dt>{t('details.size')}</dt>
          <dd>{formatBytes(node.on_disk_size, units)}</dd>
          <dt>{t('details.logical')}</dt>
          <dd>{formatBytes(node.logical_size, units)}</dd>
          <dt>{t('details.physical')}</dt>
          <dd>{formatBytes(node.physical_size, units)}</dd>
          {node.ads_size > 0 ? (
            <>
              <dt>{t('details.ads')}</dt>
              <dd>{formatBytes(node.ads_size, units)}</dd>
            </>
          ) : null}
          <dt>{t('details.items')}</dt>
          <dd>
            {formatNumber(node.child_count, language)} ·{' '}
            {t('tree.files', { count: formatNumber(node.file_count, language) })}
          </dd>
          <dt>{t('details.modified')}</dt>
          <dd>{formatDate(node.modified_at, language)}</dd>
        </dl>
        <div className={styles.badges}>
          {!node.accessible ? <Badge tone="warning">{t('tree.access_denied')}</Badge> : null}
          {node.attributes.hidden ? <Badge>{t('attr.hidden')}</Badge> : null}
          {node.attributes.system ? <Badge>{t('attr.system')}</Badge> : null}
          {node.attributes.compressed ? <Badge>{t('attr.compressed')}</Badge> : null}
          {node.attributes.sparse ? <Badge>{t('attr.sparse')}</Badge> : null}
          {node.attributes.reparse ? <Badge>{t('attr.reparse')}</Badge> : null}
        </div>
        {node.link_target !== null ? (
          <p className={`mono ${styles.path}`}>{node.link_target}</p>
        ) : null}
      </div>
    </Card>
  );
}
