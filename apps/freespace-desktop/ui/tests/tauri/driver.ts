// Lifecycle of the WebDriver stack behind the Tauri end-to-end suite.
//
// `tauri-driver` is an intermediary node that speaks WebDriver on port 4444 and
// forwards to the platform driver (Microsoft Edge WebDriver on Windows). Both
// must be running before the first session is created, so the suite owns them:
// `onPrepare` starts the pair and `onComplete` stops it. A driver that is
// already listening on the port (a developer started one manually) is reused
// and left running.

import { spawn, type ChildProcess } from 'node:child_process';
import net from 'node:net';

import { download as downloadEdgeDriver } from 'edgedriver';

const HOST = '127.0.0.1';
const PORT = 4444;
const START_TIMEOUT = 30_000;

let driver: ChildProcess | undefined;

/** Resolves to true when something accepts TCP connections on the port. */
function isPortOpen(timeout = 500): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.connect({ host: HOST, port: PORT });

    const finish = (open: boolean): void => {
      socket.removeAllListeners();
      socket.destroy();
      resolve(open);
    };

    socket.setTimeout(timeout);
    socket.once('connect', () => finish(true));
    socket.once('timeout', () => finish(false));
    socket.once('error', () => finish(false));
  });
}

/** Starts `tauri-driver` unless it is already listening; waits until it is. */
export async function startDriver(): Promise<void> {
  if (await isPortOpen()) return;

  // Edge WebDriver ships as the npm `edgedriver` package: the helper downloads
  // the build that matches the installed Edge and caches it between runs.
  const nativeDriver = process.env.FREESPACE_EDGEDRIVER ?? (await downloadEdgeDriver());
  const command = process.env.FREESPACE_TAURI_DRIVER ?? 'tauri-driver';

  const child = spawn(
    command,
    ['--port', String(PORT), '--native-driver', nativeDriver],
    { stdio: ['ignore', 'ignore', 'inherit'] },
  );

  let failure: Error | undefined;
  child.once('error', (error) => {
    failure =
      (error as NodeJS.ErrnoException).code === 'ENOENT'
        ? new Error(
            `cannot run "${command}". Install it with ` +
              '`cargo install tauri-driver --locked` or set FREESPACE_TAURI_DRIVER.',
          )
        : error;
  });
  child.once('exit', (code) => {
    failure ??= new Error(`"${command}" exited with code ${code ?? 'unknown'}`);
  });

  const deadline = Date.now() + START_TIMEOUT;
  while (Date.now() < deadline) {
    if (await isPortOpen()) {
      driver = child;
      return;
    }
    if (failure !== undefined) {
      child.kill();
      throw failure;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }

  child.kill();
  throw new Error(
    `"${command}" did not start listening on ${HOST}:${PORT} within ${START_TIMEOUT} ms`,
  );
}

/** Stops the driver started by `startDriver`; a reused one stays alive. */
export function stopDriver(): void {
  driver?.kill();
  driver = undefined;
}
