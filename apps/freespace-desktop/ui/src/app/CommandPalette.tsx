import { useState, type KeyboardEvent } from 'react';

import { cx } from '../lib/cx';
import { Dialog } from '../components/Dialog';
import styles from './palette.module.css';

export interface PaletteCommand {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
}

export interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  commands: PaletteCommand[];
  placeholder?: string;
  emptyLabel?: string;
}

/**
 * Searchable command list; Ctrl+K opens it from the shell. Focus is trapped by
 * `Dialog`, `[data-autofocus]` puts it into the search input, and the active
 * option is exposed with `aria-activedescendant`.
 */
export function CommandPalette({
  open,
  onClose,
  commands,
  placeholder = 'Type a command…',
  emptyLabel = 'No matching commands',
}: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);

  const needle = query.trim().toLowerCase();
  const filtered = commands.filter((command) =>
    `${command.label} ${command.hint ?? ''}`.toLowerCase().includes(needle),
  );
  const activeIndex =
    filtered.length === 0 ? -1 : Math.min(active, filtered.length - 1);
  const activeCommand = filtered[activeIndex];

  const run = (command: PaletteCommand | undefined) => {
    if (command === undefined) return;
    command.run();
    setQuery('');
    onClose();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActive((current) =>
        filtered.length === 0 ? 0 : (current + 1) % filtered.length,
      );
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActive((current) =>
        filtered.length === 0
          ? 0
          : (current - 1 + filtered.length) % filtered.length,
      );
    } else if (event.key === 'Home') {
      event.preventDefault();
      setActive(0);
    } else if (event.key === 'End') {
      event.preventDefault();
      setActive(Math.max(0, filtered.length - 1));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      run(activeCommand);
    } else if (event.key === 'Escape') {
      // The dialog owns Escape (close + focus restoration); only reset the query.
      setQuery('');
    }
  };

  return (
    <Dialog open={open} title="Command palette" onClose={onClose}>
      <input
        className={styles.input}
        aria-label={placeholder}
        placeholder={placeholder}
        role="combobox"
        aria-expanded={open}
        aria-controls="command-palette-list"
        aria-activedescendant={
          activeCommand === undefined ? undefined : `command-${activeCommand.id}`
        }
        data-autofocus="true"
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
          setActive(0);
        }}
        onKeyDown={onKeyDown}
      />
      <ul className={styles.list} id="command-palette-list" role="listbox">
        {filtered.map((command, index) => (
          <li
            key={command.id}
            id={`command-${command.id}`}
            role="option"
            aria-selected={index === activeIndex}
            className={cx(styles.item, index === activeIndex && styles.itemActive)}
            onMouseDown={(event) => {
              // Keep focus in the input so the trap and aria-activedescendant stay valid.
              event.preventDefault();
            }}
            onMouseEnter={() => {
              setActive(index);
            }}
            onClick={() => {
              run(command);
            }}
          >
            <span>{command.label}</span>
            {command.hint !== undefined ? (
              <span className={styles.hint}>{command.hint}</span>
            ) : null}
          </li>
        ))}
      </ul>
      <p className={styles.empty} aria-live="polite">
        {filtered.length === 0 ? emptyLabel : ''}
      </p>
    </Dialog>
  );
}
