import { ApiClient } from '../../src/api/client';
import { MockTransport } from '../../src/api/mockTransport';
import { previewTransport } from '../../src/api';

/** Creates a fresh client and transport, optionally seeded. */
export function createMockClient(seed?: (transport: MockTransport) => void): {
  transport: MockTransport;
  client: ApiClient;
} {
  const transport = new MockTransport();
  seed?.(transport);

  return { transport, client: new ApiClient(transport) };
}

/** Transport with a finished generation, as after a scan. */
export function scannedTransport(): MockTransport {
  const transport = new MockTransport();
  transport.generation = 2;

  return transport;
}

/** Shared transport used by the screens in unit tests. */
export function sharedTransport(): MockTransport {
  return previewTransport();
}
