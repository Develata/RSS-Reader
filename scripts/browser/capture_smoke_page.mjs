// Capture a real, settled page. Virtual-time budgets can expire before Web Locks or
// asynchronous storage initialization complete, leaving only the setup page in the artifact.
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { connect, evaluate, navigate, newPage, selectorExists, sleep } from './cdp_session.mjs';

const [url, selector, outputPrefix, chromeBin = process.env.CHROME_BIN ?? 'google-chrome'] = process.argv.slice(2);
if (!url || !selector || !outputPrefix) {
  throw new Error('Usage: capture_smoke_page.mjs URL READY_SELECTOR OUTPUT_PREFIX [CHROME_BIN]');
}
const timeoutMs = Number(process.env.SMOKE_PAGE_TIMEOUT_MS ?? 45000);
if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) throw new Error('Invalid page timeout');
const profile = await mkdtemp(path.join(tmpdir(), 'rssr-smoke-page-'));
let client;
let browser;
let exited;
let timeout;
try {
  browser = spawn(chromeBin, [
    '--headless=new', '--disable-gpu', '--no-sandbox', '--disable-dev-shm-usage',
    '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank',
  ], { stdio: ['ignore', 'ignore', 'inherit'] });
  let launchError;
  browser.once('error', error => { launchError = error; });
  exited = new Promise(resolve => browser.once('exit', resolve));
  const capture = async () => {
    let port;
    while (!port) {
      if (launchError) throw launchError;
      if (browser.exitCode !== null || browser.signalCode !== null) throw new Error('Chrome exited before readiness');
      try {
        port = (await readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0];
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
      }
      if (!port) await sleep(100);
    }
    const page = await newPage('about:blank', `http://127.0.0.1:${port}`);
    client = connect(page.webSocketDebuggerUrl);
    const errors = [];
    client.on('Runtime.exceptionThrown', event => errors.push(event.exceptionDetails?.text));
    await client.send('Page.enable');
    await client.send('Runtime.enable');
    await client.send('Emulation.setDeviceMetricsOverride', {
      width: 1440, height: 1200, deviceScaleFactor: 1, mobile: false,
    });
    await navigate(client, url);
    await selectorExists(client, selector, timeoutMs);
    await evaluate(client, 'document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))');
    if (errors.length) throw new Error(`Page exceptions: ${JSON.stringify(errors)}`);
    await writeFile(`${outputPrefix}.html`, await evaluate(client, 'document.documentElement.outerHTML'));
    const screenshot = await client.send('Page.captureScreenshot', { format: 'png' });
    await writeFile(`${outputPrefix}.png`, Buffer.from(screenshot.data, 'base64'));
  };
  await Promise.race([
    capture(),
    new Promise((_, reject) => {
      timeout = setTimeout(() => reject(new Error(`Page did not become ready within ${timeoutMs}ms: ${selector}`)), timeoutMs);
    }),
  ]);
} finally {
  clearTimeout(timeout);
  client?.close();
  if (browser?.pid && browser.exitCode === null && browser.signalCode === null) {
    browser.kill('SIGTERM');
    const killTimer = setTimeout(() => browser.kill('SIGKILL'), 5000);
    try { await exited; } finally { clearTimeout(killTimer); }
  }
  await rm(profile, { recursive: true, force: true });
}
