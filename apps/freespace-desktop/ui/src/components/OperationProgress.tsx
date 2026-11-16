import { useEffect } from 'react';

import type { OperationProgress } from '../api/dto';
import { useLiveAnnouncer } from '../app/live';
import { Button } from './Button';
import { Badge } from './Choice';
import { Banner, LiveRegions, Progress } from './Feedback';
import { t } from '../i18n';
import styles from './OperationProgress.module.css';

const TERMINAL = new Set(['completed', 'partial', 'failed', 'cancelled']);

export interface OperationProgressViewProps {
  progress: OperationProgress;
  onCancel: () => void;
  onUndo: () => void;
}

/** Live operation state with cancel and undo. */
export function OperationProgressView({
  progress,
  onCancel,
  onUndo,
}: OperationProgressViewProps) {
  const finished = TERMINAL.has(progress.state);
  const percent =
    progress.total === 0 ? 0 : Math.round((progress.processed / progress.total) * 100);
  const stateLabel = t(`operation.state.${progress.state}`);
  const { polite, assertive, announce } = useLiveAnnouncer();

  useEffect(() => {
    announce(`${t('operation.progress')}: ${stateLabel} · ${percent}%`, {
      urgency: progress.state === 'failed' ? 'assertive' : 'polite',
      priority: finished ? 'status' : 'progress',
    });
  }, [announce, finished, percent, progress.state, stateLabel]);

  return (
    <section className={styles.panel} aria-label={t('operation.progress')}>
      <LiveRegions polite={polite} assertive={assertive} />
      <div className={styles.header}>
        <span className={styles.title}>{t('operation.progress')}</span>
        <Badge
          tone={
            progress.state === 'failed'
              ? 'danger'
              : progress.state === 'partial' || progress.state === 'cancelling'
                ? 'warning'
                : progress.state === 'completed'
                  ? 'success'
                  : 'info'
          }
        >
          {stateLabel}
        </Badge>
      </div>
      <Progress
        label={t('operation.progress')}
        value={percent}
        state={progress.state === 'failed' ? 'error' : progress.state === 'cancelled' ? 'cancelled' : 'normal'}
      />
      <p className={styles.meta}>
        {progress.processed} / {progress.total}
        {progress.current_target !== null ? ` · ${progress.current_target}` : ''}
      </p>
      {progress.errors.length > 0 ? (
        <Banner tone="error">
          {t('operation.errors', { count: progress.errors.length })}
        </Banner>
      ) : null}
      <div className={styles.actions}>
        {!finished ? (
          <Button variant="ghost" onClick={onCancel}>
            {t('operation.cancel')}
          </Button>
        ) : null}
        {progress.undo_available ? (
          <Button variant="secondary" onClick={onUndo}>
            {t('operation.undo')}
          </Button>
        ) : finished ? (
          <span className={styles.hint}>{t('operation.undo_unavailable')}</span>
        ) : null}
      </div>
    </section>
  );
}
