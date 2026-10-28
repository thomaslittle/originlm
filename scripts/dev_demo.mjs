/**
 * Demo dev server: Vite with hot module replacement in front of the real native helper.
 * The helper is scripts/run_live_demo.mjs (passive discovery and metadata-only
 * provider checks with the host's permission flags); nothing here is sample data. The UI
 * needs the helper's pairing flow, exactly as in the built demo.
 *
 *   npm run dev          -> http://127.0.0.1:5173/
 */
import { spawn, execFileSync } from 'node:child_process';
import { createServer as createNetServer } from 'node:net';
import { createServer } from 'vite';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const exampleMode = process.argv.includes('--example');
const portFlag = process.argv.indexOf('--port');
const port = portFlag < 0 ? 5173 : Number(process.argv[portFlag + 1]);
if (!Number.isInteger(port) || port < 1 || port > 65535) {
  console.error('Usage: npm run dev -- [--port <1-65535>] [--example]');
  process.exit(2);
}

function freePort() {
  return new Promise((resolvePort, reject) => {
    const server = createNetServer();
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port: found } = server.address();
      server.close((error) => (error ? reject(error) : resolvePort(found)));
    });
  });
}

const helperPort = await freePort();
const helperUrl = `http://127.0.0.1:${helperPort}`;
// run_live_demo.mjs builds the helper, grants the host permissions, and serves the API. Its assets
// are unused here because Vite serves the UI.
const host = exampleMode ? null : spawn(process.execPath, [resolve(root, 'scripts/run_live_demo.mjs')], {
  cwd: root,
  stdio: 'inherit',
  windowsHide: true,
  env: { ...process.env, LAI_DEMO_PORT: String(helperPort) },
});

function stopHost() {
  if (!host || host.exitCode !== null) return;
  // On Windows a plain kill would orphan the helper the demo script started.
  if (process.platform === 'win32') {
    try {
      execFileSync('taskkill', ['/pid', String(host.pid), '/T', '/F'], { stdio: 'ignore' });
    } catch {
      // Already gone.
    }
  } else host.kill('SIGTERM');
}

let helperAlive = !exampleMode;
host?.once('exit', (code) => {
  helperAlive = false;
  if (!shuttingDown) console.error(`The native helper exited (code ${code}). The UI keeps running and falls back to labeled example data.`);
});

let shuttingDown = false;
let vite;
async function shutdown(code = 0) {
  if (shuttingDown) return;
  shuttingDown = true;
  stopHost();
  await vite?.close();
  process.exit(code);
}
for (const signal of ['SIGINT', 'SIGTERM', 'SIGBREAK']) process.once(signal, () => void shutdown(0));
process.once('exit', stopHost);

// The helper builds first (cargo), so wait for it to answer before starting the UI.
for (let attempt = 0; helperAlive; attempt++) {
  if (shuttingDown) break;
  try {
    if ((await fetch(`${helperUrl}/runtime/info`)).ok) break;
  } catch {
    // Not listening yet.
  }
  if (attempt > 600) {
    console.error('The native helper did not start within 5 minutes; continuing with the example-data fallback.');
    break;
  }
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 500));
}

// The helper rejects unknown origins by design, so the proxy presents itself as the helper.
const toHelper = {
  target: helperUrl,
  changeOrigin: true,
  configure: (proxy) => proxy.on('proxyReq', (request) => request.setHeader('origin', helperUrl)),
};

vite = await createServer({
  configFile: resolve(root, 'apps/demo/vite.config.ts'),
  server: {
    port,
    strictPort: true,
    host: '127.0.0.1',
    proxy: exampleMode ? undefined : { '/runtime': toHelper, '/v1': toHelper },
  },
});
await vite.listen();
vite.printUrls();
console.log(exampleMode
  ? `Example data only: ${vite.resolvedUrls.local[0]}`
  : `Open the demo at ${vite.resolvedUrls.local[0]} and choose "Show current pairing code", then pair.`);
