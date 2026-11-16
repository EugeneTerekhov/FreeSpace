import type { UiError } from '../api/dto';

const recent: string[] = [];

/** Normalises an unknown failure into the documented error envelope. */
export function normalizeError(error: unknown): UiError {
  if (isUiError(error)) {
    return error;
  }

  const details = error instanceof Error ? error.message : String(error);

  return {
    code: 'E_INTERNAL',
    category: 'internal',
    message_key: 'error.internal',
    details,
    retryable: true,
    task_id: null,
    journal_entry_id: null,
  };
}

/** Creates a validation envelope for client-side form checks. */
export function validationError(details: string): UiError {
  return {
    code: 'E_VALIDATION',
    category: 'validation',
    message_key: 'error.validation',
    details,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };
}

/** Returns true when a value already looks like a `UiError`. */
export function isUiError(value: unknown): value is UiError {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<UiError>;

  return (
    typeof candidate.code === 'string' &&
    typeof candidate.category === 'string' &&
    typeof candidate.message_key === 'string' &&
    typeof candidate.retryable === 'boolean'
  );
}

/** Keeps the last 50 local log lines; nothing leaves the machine. */
export function logLocal(
  level: 'debug' | 'info' | 'warn' | 'error',
  message: string,
  data?: unknown,
): void {
  const line = `${new Date().toISOString()} ${level.toUpperCase()} ${message}${
    data === undefined ? '' : ` ${safeJson(data)}`
  }`;
  recent.push(line);
  if (recent.length > 50) recent.shift();

  if (import.meta.env.DEV) {
    const method = level === 'debug' ? 'log' : level;
    console[method](message, data ?? '');
  }
}

/** Recent log lines, newest last. */
export function recentLogs(): string[] {
  return [...recent];
}

function safeJson(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
