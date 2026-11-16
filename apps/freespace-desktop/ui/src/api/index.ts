import { ApiClient } from './client';
import { createMockTransport } from './mockTransport';
import type { MockTransport } from './mockTransport';
import { TauriTransport, isTauriRuntime } from './transport';

/** Mock transport used by the browser preview and tests. */
let mockTransport: MockTransport | null = null;

/** Returns the shared mock transport for the browser preview. */
export function previewTransport(): MockTransport {
  mockTransport ??= createMockTransport();

  return mockTransport;
}

/** Creates a client for the current runtime. */
export function createApiClient(transport?: TauriTransport | MockTransport): ApiClient {
  if (transport !== undefined) {
    return new ApiClient(transport);
  }

  return new ApiClient(isTauriRuntime() ? new TauriTransport() : previewTransport());
}

export { ApiClient } from './client';
export { MockTransport, createMockTransport } from './mockTransport';
export { TauriTransport, isTauriRuntime, COMMANDS, EVENTS } from './transport';
export type { CommandName, EventName, Transport, Unlisten } from './transport';
export type * from './dto';
