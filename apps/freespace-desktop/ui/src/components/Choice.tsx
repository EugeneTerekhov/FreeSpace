import {
  useEffect,
  useRef,
  type InputHTMLAttributes,
  type ReactNode,
} from 'react';

import { cx } from '../lib/cx';
import styles from './kit.module.css';

export interface ToggleProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}

/** Switch with `role="switch"` and an explicit accessible name. */
export function Toggle({
  checked,
  onCheckedChange,
  label,
  disabled = false,
}: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={cx(styles.toggle, checked && styles.on)}
      onClick={() => {
        onCheckedChange(!checked);
      }}
    >
      <span className={styles.track} aria-hidden="true">
        <span className={styles.thumb} />
      </span>
      <span>{label}</span>
    </button>
  );
}

export interface CheckboxProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'type'> {
  label: ReactNode;
  indeterminate?: boolean;
}

/** Native checkbox with an optional indeterminate state. */
export function Checkbox({
  label,
  indeterminate = false,
  id,
  ...rest
}: CheckboxProps) {
  const reference = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (reference.current !== null) {
      reference.current.indeterminate = indeterminate;
    }
  }, [indeterminate]);

  return (
    <label className={styles.checkRow} htmlFor={id}>
      <input ref={reference} id={id} type="checkbox" {...rest} />
      <span>{label}</span>
    </label>
  );
}

export interface RadioProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'type'> {
  label: ReactNode;
}

/** Native radio button; group it inside a fieldset with a legend. */
export function Radio({ label, id, ...rest }: RadioProps) {
  return (
    <label className={styles.checkRow} htmlFor={id}>
      <input id={id} type="radio" {...rest} />
      <span>{label}</span>
    </label>
  );
}

export interface SegmentedOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface SegmentedProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: SegmentedOption[];
}

/** Radio group that renders as a segmented control. */
export function Segmented({ label, value, onChange, options }: SegmentedProps) {
  const enabled = options.filter((option) => option.disabled !== true);

  const move = (direction: 1 | -1) => {
    if (enabled.length === 0) return;
    const current = enabled.findIndex((option) => option.value === value);
    const next = enabled[(current + direction + enabled.length) % enabled.length];
    if (next !== undefined) onChange(next.value);
  };

  return (
    <div
      role="radiogroup"
      aria-label={label}
      className={styles.segmented}
      onKeyDown={(event) => {
        if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
          event.preventDefault();
          move(1);
        } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
          event.preventDefault();
          move(-1);
        }
      }}
    >
      {options.map((option) => {
        const selected = option.value === value;

        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            disabled={option.disabled === true}
            className={cx(styles.segment, selected && styles.segmentSelected)}
            onClick={() => {
              onChange(option.value);
            }}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

export interface TabItem {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface TabsProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  items: TabItem[];
}

/** Tab list with arrow-key navigation; panels are rendered by the caller. */
export function Tabs({ label, value, onChange, items }: TabsProps) {
  const enabled = items.filter((item) => item.disabled !== true);

  const move = (direction: 1 | -1) => {
    if (enabled.length === 0) return;
    const current = enabled.findIndex((item) => item.value === value);
    const next = enabled[(current + direction + enabled.length) % enabled.length];
    if (next !== undefined) onChange(next.value);
  };

  return (
    <div
      role="tablist"
      aria-label={label}
      className={styles.tabs}
      onKeyDown={(event) => {
        if (event.key === 'ArrowRight') {
          event.preventDefault();
          move(1);
        } else if (event.key === 'ArrowLeft') {
          event.preventDefault();
          move(-1);
        } else if (event.key === 'Home') {
          event.preventDefault();
          const first = enabled[0];
          if (first !== undefined) onChange(first.value);
        } else if (event.key === 'End') {
          event.preventDefault();
          const last = enabled.at(-1);
          if (last !== undefined) onChange(last.value);
        }
      }}
    >
      {items.map((item) => {
        const selected = item.value === value;

        return (
          <button
            key={item.value}
            type="button"
            role="tab"
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            disabled={item.disabled === true}
            className={cx(styles.tab, selected && styles.tabSelected)}
            onClick={() => {
              onChange(item.value);
            }}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}

export interface ChipProps {
  children: ReactNode;
  selected?: boolean;
  onClick?: () => void;
  onRemove?: () => void;
  removeLabel?: string;
}

/** Filter chip; selection is exposed with `aria-pressed`. */
export function Chip({
  children,
  selected = false,
  onClick,
  onRemove,
  removeLabel = 'Remove',
}: ChipProps) {
  return (
    <span className={cx(styles.chip, selected && styles.chipSelected)}>
      {onClick !== undefined ? (
        <button
          type="button"
          aria-pressed={selected}
          onClick={onClick}
          className={styles.chipButton}
        >
          {children}
        </button>
      ) : (
        <span>{children}</span>
      )}
      {onRemove !== undefined ? (
        <button
          type="button"
          className={styles.chipRemove}
          aria-label={removeLabel}
          onClick={onRemove}
        >
          <span aria-hidden="true">×</span>
        </button>
      ) : null}
    </span>
  );
}

export type BadgeTone = 'neutral' | 'success' | 'warning' | 'danger' | 'info';

const BADGE_CLASS: Record<BadgeTone, string> = {
  neutral: styles.neutral ?? '',
  success: styles.toneSuccess ?? '',
  warning: styles.toneWarning ?? '',
  danger: styles.toneDanger ?? '',
  info: styles.toneInfo ?? '',
};

/** Small status label; the text always carries the meaning. */
export function Badge({
  tone = 'neutral',
  children,
}: {
  tone?: BadgeTone;
  children: ReactNode;
}) {
  return <span className={cx(styles.badge, BADGE_CLASS[tone])}>{children}</span>;
}
