import {
  useId,
  useRef,
  useState,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
} from 'react';

import { cx } from '../lib/cx';
import { t } from '../i18n';
import styles from './kit.module.css';

export interface FieldProps {
  id: string;
  label: string;
  hint?: string;
  error?: string;
  required?: boolean;
  children: ReactNode;
}

/** Label, control, hint and error with the documented ARIA wiring. */
export function Field({
  id,
  label,
  hint,
  error,
  required = false,
  children,
}: FieldProps) {
  return (
    <div className={styles.field}>
      <label className={styles.label} htmlFor={id}>
        {label}
        {required ? (
          <span className={styles.required} aria-hidden="true">
            {' *'}
          </span>
        ) : null}
      </label>
      {children}
      {hint !== undefined ? (
        <span className={styles.hint} id={`${id}-hint`}>
          {hint}
        </span>
      ) : null}
      {error !== undefined ? (
        <span className={styles.errorText} id={`${id}-error`} role="alert">
          {error}
        </span>
      ) : null}
    </div>
  );
}

export interface SearchFieldProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'value' | 'onChange'> {
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  loading?: boolean;
}

/** Search input; Escape and the clear button reset the value. */
export function SearchField({
  label,
  value,
  onValueChange,
  loading = false,
  className,
  ...rest
}: SearchFieldProps) {
  return (
    <div className={cx(styles.searchWrap, className)}>
      <input
        {...rest}
        type="search"
        role="searchbox"
        aria-label={label}
        className={styles.input}
        value={value}
        aria-busy={loading || undefined}
        onChange={(event) => {
          onValueChange(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Escape' && value !== '') {
            event.stopPropagation();
            onValueChange('');
          }
        }}
      />
      {value !== '' ? (
        <button
          type="button"
          className={styles.clear}
          aria-label={t('common.clear')}
          onClick={() => {
            onValueChange('');
          }}
        >
          <span aria-hidden="true">×</span>
        </button>
      ) : null}
      {loading ? <span className={styles.spinner} aria-hidden="true" /> : null}
    </div>
  );
}

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface SelectProps
  extends Omit<SelectHTMLAttributes<HTMLSelectElement>, 'children'> {
  options: SelectOption[];
}

/** Native select with token styling. */
export function Select({ options, className, ...rest }: SelectProps) {
  return (
    <select {...rest} className={cx(styles.select, className)}>
      {options.map((option) => (
        <option key={option.value} value={option.value} disabled={option.disabled}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

export interface ComboboxOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface ComboboxProps {
  id?: string;
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  options: ComboboxOption[];
  placeholder?: string;
  disabled?: boolean;
  emptyLabel?: string;
}

/** Single-select combobox with filtering and keyboard navigation. */
export function Combobox({
  id,
  label,
  value,
  onValueChange,
  options,
  placeholder,
  disabled = false,
  emptyLabel = 'No matches',
}: ComboboxProps) {
  const generatedId = useId();
  const controlId = id ?? `combobox-${generatedId}`;
  const listId = `${controlId}-listbox`;
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const wrapper = useRef<HTMLDivElement>(null);

  const needle = value.trim().toLowerCase();
  const filtered = options.filter((option) =>
    option.label.toLowerCase().includes(needle),
  );
  const activeIndex = filtered.length === 0 ? -1 : Math.min(active, filtered.length - 1);
  const activeOption = filtered[activeIndex];

  const select = (option: ComboboxOption) => {
    onValueChange(option.value);
    setOpen(false);
  };

  return (
    <div
      ref={wrapper}
      className={styles.combobox}
      onBlur={(event) => {
        const related = event.relatedTarget;
        if (!(related instanceof Node) || !wrapper.current?.contains(related)) {
          setOpen(false);
        }
      }}
    >
      <input
        id={controlId}
        className={styles.input}
        role="combobox"
        aria-label={label}
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-autocomplete="list"
        aria-activedescendant={
          open && activeOption !== undefined
            ? `${controlId}-option-${activeIndex}`
            : undefined
        }
        placeholder={placeholder}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          onValueChange(event.target.value);
          setOpen(true);
          setActive(0);
        }}
        onFocus={() => {
          setOpen(true);
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown') {
            event.preventDefault();
            setOpen(true);
            setActive((current) =>
              filtered.length === 0 ? 0 : (current + 1) % filtered.length,
            );
          } else if (event.key === 'ArrowUp') {
            event.preventDefault();
            setOpen(true);
            setActive((current) =>
              filtered.length === 0
                ? 0
                : (current - 1 + filtered.length) % filtered.length,
            );
          } else if (event.key === 'Enter' && open && activeOption !== undefined) {
            event.preventDefault();
            select(activeOption);
          } else if (event.key === 'Escape') {
            setOpen(false);
          }
        }}
      />
      {open ? (
        <div className={styles.popover} id={listId} role="listbox" aria-label={label}>
          {filtered.length === 0 ? (
            <div className={styles.emptyOption}>{emptyLabel}</div>
          ) : (
            filtered.map((option, index) => (
              <button
                key={option.value}
                id={`${controlId}-option-${index}`}
                type="button"
                role="option"
                aria-selected={option.value === value}
                className={cx(
                  styles.option,
                  index === activeIndex && styles.optionActive,
                )}
                disabled={option.disabled === true}
                onMouseDown={(event) => {
                  event.preventDefault();
                }}
                onMouseEnter={() => {
                  setActive(index);
                }}
                onClick={() => {
                  select(option);
                }}
              >
                {option.label}
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}
