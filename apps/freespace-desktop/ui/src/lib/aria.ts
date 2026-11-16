/** Returns the `aria-describedby` value for a field with a hint and an error. */
export function describedBy(
  id: string,
  hint?: string,
  error?: string,
): string | undefined {
  const ids = [
    hint !== undefined ? `${id}-hint` : null,
    error !== undefined ? `${id}-error` : null,
  ].filter((value): value is string => value !== null);

  return ids.length > 0 ? ids.join(' ') : undefined;
}
