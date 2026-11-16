import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

// jsdom does not implement matchMedia; the theme runtime expects it.
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false,
    }),
  });
}

// jsdom does not implement ResizeObserver or canvas; both are stubbed so the
// Treemap screen can be exercised without WebView2.
if (typeof globalThis.ResizeObserver !== 'function') {
  class ResizeObserverStub {
    constructor(private readonly callback: ResizeObserverCallback) {}

    observe(): void {
      this.callback(
        [
          {
            contentRect: {
              width: 800,
              height: 500,
              x: 0,
              y: 0,
              top: 0,
              left: 0,
              bottom: 500,
              right: 800,
            },
          } as ResizeObserverEntry,
        ],
        this,
      );
    }

    unobserve(): void {
      // No-op in the jsdom stub.
    }

    disconnect(): void {
      // No-op in the jsdom stub.
    }
  }

  Object.defineProperty(globalThis, 'ResizeObserver', {
    writable: true,
    value: ResizeObserverStub,
  });
}

if (typeof HTMLCanvasElement !== 'undefined') {
  Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', {
    writable: true,
    value: () => null,
  });
}

afterEach(() => {
  cleanup();
});
