import { useEffect, useRef } from 'react';

export type HotkeyMap = Record<string, () => void>;

/** Builds a normalised combo such as `Ctrl+K` from a keyboard event. */
export function comboFromEvent(event: KeyboardEvent): string {
  const parts: string[] = [];
  if (event.ctrlKey || event.metaKey) parts.push('Ctrl');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');
  parts.push(event.key.length === 1 ? event.key.toUpperCase() : event.key);

  return parts.join('+');
}

/** Registers window-level shortcuts; the latest handlers win. */
export function useHotkeys(handlers: HotkeyMap): void {
  const latest = useRef(handlers);
  latest.current = handlers;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const handler = latest.current[comboFromEvent(event)];
      if (handler === undefined) return;

      event.preventDefault();
      handler();
    };

    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
    };
  }, []);
}
