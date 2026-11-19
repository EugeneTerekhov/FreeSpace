/**
 * Copies text to the system clipboard.
 *
 * WebView2 exposes the asynchronous clipboard API on the application origin;
 * the temporary textarea is a fallback for environments where it is missing or
 * denied. The caller decides what to tell the user on failure.
 */
export async function copyText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText !== undefined) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.top = '0';
  area.style.left = '0';
  area.style.opacity = '0';
  document.body.appendChild(area);
  area.select();
  const copied = document.execCommand('copy');
  document.body.removeChild(area);

  if (!copied) {
    throw new Error('the clipboard is unavailable');
  }
}
