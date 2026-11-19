import { useId, useState, type ReactNode } from 'react';

import styles from './kit.module.css';

export interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
}

/** Hover/focus tooltip wired with `aria-describedby`. */
export function Tooltip({ content, children }: TooltipProps) {
  const id = useId();
  const [open, setOpen] = useState(false);

  return (
    <span
      className={styles.tooltipWrap}
      onMouseEnter={() => {
        setOpen(true);
      }}
      onMouseLeave={() => {
        setOpen(false);
      }}
      onFocus={() => {
        setOpen(true);
      }}
      onBlur={() => {
        setOpen(false);
      }}
    >
      <span aria-describedby={open ? id : undefined}>{children}</span>
      {open ? (
        <span role="tooltip" id={id} className={styles.tooltip}>
          {content}
        </span>
      ) : null}
    </span>
  );
}

/** Keyboard hint; never the only way to discover an action. */
export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className={styles.kbd}>{children}</kbd>;
}
