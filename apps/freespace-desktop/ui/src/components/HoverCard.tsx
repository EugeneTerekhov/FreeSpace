import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

import type { RowAnchor } from './DataTable';
import styles from './HoverCard.module.css';

/** Width of a hover card; the height is measured after it renders. */
export const HOVER_CARD_WIDTH = 320;

const CARD_GAP = 14;
const CARD_MARGIN = 12;

interface CardPosition {
  left: number;
  top: number;
}

/** Keeps the card inside the viewport, beside the pointer or the focused row. */
function placeCard(anchor: RowAnchor, width: number, height: number): CardPosition {
  const viewportWidth = typeof window === 'undefined' ? 1280 : window.innerWidth;
  const viewportHeight = typeof window === 'undefined' ? 800 : window.innerHeight;
  const left =
    anchor.x + CARD_GAP + width + CARD_MARGIN <= viewportWidth
      ? anchor.x + CARD_GAP
      : Math.max(
          CARD_MARGIN,
          Math.min(anchor.x - CARD_GAP - width, viewportWidth - width - CARD_MARGIN),
        );
  const top = Math.max(
    CARD_MARGIN,
    Math.min(anchor.y - 14, viewportHeight - height - CARD_MARGIN),
  );

  return { left, top };
}

export interface HoverCardProps {
  /** Pointer position, or the row rectangle for a keyboard focus. */
  anchor: RowAnchor;
  /** Accessible name of the `tooltip` role; also tells which card this is. */
  label: string;
  busy?: boolean;
  children: ReactNode;
}

/**
 * Dense read-only card anchored to a table row.
 *
 * It renders into `document.body` so the table scroll container cannot clip
 * it, follows the viewport edges and never takes pointer events, so it cannot
 * steal the hover from the row underneath.
 */
export function HoverCard({ anchor, label, busy = false, children }: HoverCardProps) {
  const card = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<CardPosition>(() =>
    placeCard(anchor, HOVER_CARD_WIDTH, 220),
  );

  // Content arrives asynchronously, so the measured height changes after the
  // first paint. A resize observer keeps the card inside the viewport without
  // guessing which props changed.
  useLayoutEffect(() => {
    const element = card.current;
    if (element === null) return undefined;
    const update = () => {
      const next = placeCard(
        anchor,
        element.offsetWidth || HOVER_CARD_WIDTH,
        element.offsetHeight,
      );
      setPosition((current) =>
        current.left === next.left && current.top === next.top ? current : next,
      );
    };
    update();
    if (typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(update);
    observer.observe(element);

    return () => {
      observer.disconnect();
    };
  }, [anchor]);

  return createPortal(
    <div
      ref={card}
      role="tooltip"
      aria-label={label}
      aria-busy={busy || undefined}
      className={styles.card}
      style={{ left: position.left, top: position.top, width: HOVER_CARD_WIDTH }}
    >
      {children}
    </div>,
    document.body,
  );
}

/** Title line: the object name and an optional muted kind or owner. */
export function HoverCardHead({
  title,
  meta,
}: {
  title: string;
  meta?: ReactNode;
}) {
  return (
    <header className={styles.head}>
      <span className={styles.name} title={title}>
        {title}
      </span>
      {meta === undefined ? null : <span className={styles.meta}>{meta}</span>}
    </header>
  );
}

/** Muted monospace line, usually a path or an operation identifier. */
export function HoverCardPath({ children, title }: { children: ReactNode; title?: string }) {
  return (
    <p className={styles.path} title={title}>
      {children}
    </p>
  );
}

/** Two-column label/value grid of a hover card. */
export function HoverCardMetrics({ children }: { children: ReactNode }) {
  return <dl className={styles.metrics}>{children}</dl>;
}

/** Single-column list of label/value rows with the value on the right. */
export function HoverCardRows({ children }: { children: ReactNode }) {
  return <dl className={styles.rows}>{children}</dl>;
}

/** One label/value row: the label on the left, the value on the right edge. */
export function HoverCardRow({
  label,
  value,
}: {
  label: ReactNode;
  value: ReactNode;
}) {
  return (
    <div className={styles.row}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/** One dense label/value pair of the hover card. */
export function HoverCardMetric({ label, value }: { label: ReactNode; value: ReactNode }) {
  return (
    <div className={styles.metric}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/** Small muted note under the metrics. */
export function HoverCardNote({ children }: { children: ReactNode }) {
  return <p className={styles.note}>{children}</p>;
}

/** Error block: translated message, stable code and the raw detail text. */
export function HoverCardError({
  title,
  code,
  message,
  details,
}: {
  title: ReactNode;
  code?: string | undefined;
  message: ReactNode;
  details?: string | null | undefined;
}) {
  return (
    <div className={styles.error}>
      <span className={styles.errorTitle}>
        <span aria-hidden="true">✕</span>
        <span>{title}</span>
        {code === undefined ? null : <code className={styles.errorCode}>{code}</code>}
      </span>
      <span className={styles.errorMessage}>{message}</span>
      {details === undefined || details === null || details === '' ? null : (
        <code className={styles.errorDetails}>{details}</code>
      )}
    </div>
  );
}
