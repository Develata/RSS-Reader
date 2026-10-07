// No npm dependencies. Reuses the repository CDP driver and the cfg(test) rssr-web harness.
// Usage: CHROME_BIN=... RSSR_PROXY_TEST_PUBLIC_DIR=... node scripts/browser/feed_proxy_isolation.mjs OUTPUT [--expect-vulnerable]
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { connect, evaluate, navigate, newPage, sleep, waitFor } from './cdp_session.mjs';

const output = path.resolve(process.argv[2] ?? 'target/feed-proxy-isolation');
const vulnerable = process.argv.includes('--expect-vulnerable');
const publicDir = process.env.RSSR_PROXY_TEST_PUBLIC_DIR;
assert(publicDir, 'Set RSSR_PROXY_TEST_PUBLIC_DIR to the real rssr-app web bundle');
await readFile(path.join(publicDir, 'index.html'));
await mkdir(output, { recursive: true });
const run = await mkdtemp(path.join(output, 'run-'));
const profile = path.join(run, 'chrome-profile');
await mkdir(profile);
const report = { expectVulnerable: vulnerable, run, cases: [], compatibility: [] };
const children = [];
let client;
let serverLog = '';
let browserLog = '';
let deadline;

function launch(command, args, env = process.env, stdin = 'ignore') {
  const child = spawn(command, args, { env, windowsHide: true, stdio: [stdin, 'pipe', 'pipe'] });
  child.done = new Promise(resolve => child.once('close', (code, signal) => resolve({ code, signal })));
  child.on('error', error => { child.launchError = error; });
  children.push(child);
  return child;
}

async function poll(check, label, ms = 120000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const result = await check();
    if (result) return result;
    await sleep(100);
  }
  throw new Error(`Timed out: ${label}`);
}

async function main() {
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('RSS_READER_WEB_')));
  Object.assign(env, {
    RSSR_PROXY_ISOLATION_TEST: '1', RSS_READER_WEB_USERNAME: 'isolation-test',
    RSS_READER_WEB_PASSWORD: 'fixture-password-only',
    RSS_READER_WEB_SESSION_SECRET: 'feed-proxy-isolation-test-secret-0123456789',
    RSS_READER_WEB_AUTH_STATE_FILE: path.join(run, 'auth.json'),
    RSS_READER_WEB_STATIC_DIR: path.resolve(publicDir),
    TEMP: run, TMP: run, TMPDIR: run,
  });
  const server = launch(process.env.CARGO ?? 'cargo', ['test', '--locked', '-p', 'rssr-web',
    'proxy::isolation_tests::browser_fixture_server', '--', '--exact', '--ignored', '--nocapture'], env, 'pipe');
  server.stdout.on('data', data => { serverLog += data; });
  server.stderr.on('data', data => { serverLog += data; });
  const base = await poll(() => {
    if (server.launchError) throw server.launchError;
    if (server.exitCode !== null) throw new Error(`Fixture server exited: ${serverLog}`);
    return serverLog.match(/RSSR_PROXY_TEST_URL=(http:\/\/127\.0\.0\.1:\d+)/)?.[1];
  }, 'fixture server');
  report.base = base;
  const proxy = name => `${base}/feed-proxy?url=${encodeURIComponent(`https://fixture.example.invalid/${name}`)}`;
  const unauthenticated = await fetch(proxy('html'), { redirect: 'manual' });
  assert.equal(unauthenticated.status, 303, 'production authentication must protect the fixture route');

  const browser = launch(process.env.CHROME_BIN ?? 'google-chrome', ['--headless=new',
    '--disable-gpu', '--disable-dev-shm-usage', '--disable-background-networking',
    '--no-first-run', '--no-default-browser-check', '--remote-debugging-port=0',
    `--user-data-dir=${profile}`, 'about:blank']);
  browser.stderr.on('data', data => { browserLog += data; });
  const port = await poll(async () => {
    if (browser.launchError) throw browser.launchError;
    if (browser.exitCode !== null) throw new Error(`Browser exited: ${browserLog}`);
    try { return (await readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]; }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }, 'isolated Chrome');
  client = connect((await newPage('about:blank', `http://127.0.0.1:${port}`)).webSocketDebuggerUrl);
  report.browser = await client.send('Browser.getVersion');
  await client.send('Page.enable');
  await client.send('Runtime.enable');
  await client.send('Network.enable');
  const proxyResponses = new Map();
  client.on('Network.responseReceived', ({ response }) => {
    const url = new URL(response.url);
    if (url.pathname === '/feed-proxy') {
      const upstream = url.searchParams.get('url');
      proxyResponses.set(upstream, (proxyResponses.get(upstream) ?? 0) + 1);
    }
  });
  // All test content stays local, and a direct-feed fallback cannot hide a proxy regression.
  await client.send('Network.setBlockedURLs', { urls: ['https://*', 'http://fixture.example.invalid/*'] });
  await navigate(client, `${base}/login`);
  const login = await evaluate(client, `fetch('/login', {method:'POST', body: new URLSearchParams({username:'isolation-test', password:'fixture-password-only', next:'/sentinel'})}).then(r => r.url)`);
  assert.equal(login, `${base}/sentinel`);
  const sentinel = 'isolated-value-never-private';
  const seed = async () => {
    await navigate(client, `${base}/sentinel`);
    await evaluate(client, `localStorage.setItem('rssr-proxy-sentinel', ${JSON.stringify(sentinel)}); localStorage.removeItem('rssr-proxy-read')`);
  };
  const inspect = () => evaluate(client, `(() => {
    const state = {url: location.href, executed: window.__proxyExecuted === true, read: window.__proxyRead ?? null};
    try { state.storage = localStorage.getItem('rssr-proxy-sentinel'); } catch(e) { state.storageError = e.name; }
    try { localStorage.setItem('rssr-proxy-cdp-write', 'probe'); state.canWrite = true; } catch(e) { state.writeError = e.name; }
    return state;
  })()`);

  // Positive controls prove these same harmless scripts really execute and reach test storage.
  for (const name of ['html', 'svg', 'xhtml']) {
    await seed();
    await navigate(client, `${base}/control?url=${encodeURIComponent(`https://fixture.example.invalid/${name}`)}`);
    const state = await inspect();
    assert(state.executed, `positive control ${name} must execute`);
    assert.equal(state.read, sentinel);
    assert.equal(state.storage, 'changed-by-fixture');
    report.cases.push({ name, context: 'control', ...state });
  }

  for (const name of ['html', 'svg', 'xhtml', 'xml', 'sniff', 'error']) {
    await seed();
    await navigate(client, proxy(name));
    const state = await inspect();
    assert.equal(state.url, proxy(name), 'must inspect the actual proxy document');
    assert.equal(state.executed, vulnerable, name);
    if (vulnerable) {
      assert.equal(state.read, sentinel, name);
      assert.equal(state.storage, 'changed-by-fixture', name);
    } else {
      assert.equal(state.read, null, name);
      // CDP evaluation is privileged: even that context must not gain app-origin storage.
      assert.equal(state.storageError, 'SecurityError', name);
      assert.equal(state.writeError, 'SecurityError', name);
    }
    await navigate(client, `${base}/sentinel`);
    const stored = await evaluate(client, `({sentinel:localStorage.getItem('rssr-proxy-sentinel'), read:localStorage.getItem('rssr-proxy-read')})`);
    assert.equal(stored.sentinel, vulnerable ? 'changed-by-fixture' : sentinel, name);
    assert.equal(stored.read, vulnerable ? sentinel : null, name);
    report.cases.push({ name, context: 'top-level', ...state, stored });
    console.log(`top-level ${name}: ${vulnerable ? 'vulnerability reproduced' : 'isolated'}`);
  }

  if (!vulnerable) {
    await seed();
    await evaluate(client, `new Promise(resolve => {const f=document.createElement('iframe');f.id='probe';f.onload=resolve;f.src=${JSON.stringify(proxy('html'))};document.body.append(f)})`);
    assert.equal(await evaluate(client, `(() => {try {return document.querySelector('#probe').contentWindow.localStorage.getItem('rssr-proxy-sentinel')} catch(e) {return e.name}})()`), 'SecurityError');
    assert.equal(await evaluate(client, `localStorage.getItem('rssr-proxy-sentinel')`), sentinel);
    report.cases.push({ name: 'html', context: 'iframe', storageError: 'SecurityError' });
  }

  // Fetch still supplies raw data and metadata to the existing Rust client.
  for (const [name, status] of [['rss.xml', 200], ['atom.xml', 200], ['latin.xml', 200], ['landing', 200], ['error', 404], ['not-modified', 304], ['oversized', 502]]) {
    const data = await evaluate(client, `fetch(${JSON.stringify(proxy(name))}).then(async r => ({status:r.status, headers:Object.fromEntries(r.headers), bytes:Array.from(new Uint8Array(await r.arrayBuffer()))}))`);
    assert.equal(data.status, status, name);
    if (status !== 502) {
      assert.equal(data.headers.etag, '"fixture-v1"');
      assert.equal(data.headers['last-modified'], 'Wed, 07 Oct 2026 00:00:00 GMT');
    }
    if (name === 'latin.xml') assert(new TextDecoder('iso-8859-1').decode(Uint8Array.from(data.bytes)).includes('Café Feed'));
    if (name === 'landing') assert.equal(data.headers['x-rssr-final-url'], 'https://fixture.example.invalid/moved/index.html');
    report.compatibility.push({ name, status, headers: data.headers, byteLength: data.bytes.length });
  }
  const localStatus = await evaluate(client, `fetch('/feed-proxy?url=http://127.0.0.1/private').then(r=>r.status)`);
  assert.equal(localStatus, 400, 'production local-target rejection');

  // The unchanged production WASM bundle exercises real subscription/discovery/refresh parsing.
  await navigate(client, `${base}/feeds`);
  await waitFor(client, `document.querySelector('[data-field="feed-url-input"]') !== null`, 60000);
  for (const [name, title] of [['rss.xml', 'RSS Fixture'], ['atom.xml', 'Atom Fixture'], ['latin.xml', 'Café Feed'], ['landing', 'Discovered Feed']]) {
    const url = `https://fixture.example.invalid/${name}`;
    await evaluate(client, `(() => {const input=document.querySelector('[data-field="feed-url-input"]');input.value=${JSON.stringify(url)};input.dispatchEvent(new Event('input',{bubbles:true}));document.querySelector('[data-action="add-feed"]').click()})()`);
    const card = `Array.from(document.querySelectorAll('li[data-layout="feed-card"]')).find(c=>c.textContent.includes(${JSON.stringify(title)}))`;
    await waitFor(client, `Boolean(${card})`, 60000);
    const refreshedUrl = name === 'landing' ? 'https://fixture.example.invalid/moved/rss.xml' : url;
    const beforeRefresh = proxyResponses.get(refreshedUrl) ?? 0;
    await evaluate(client, `(${card}).querySelector('[data-action="refresh-feed"]').click()`);
    await poll(() => (proxyResponses.get(refreshedUrl) ?? 0) > beforeRefresh, `real proxy refresh: ${name}`, 30000);
    await waitFor(client, `(${card})?.getAttribute('data-refresh-state') === 'success'`, 60000);
    await evaluate(client, `(${card}).querySelector('[data-nav="feed-entries"]').click()`);
    await waitFor(client, `document.body.textContent.includes(${JSON.stringify(`${title} Entry`)})`, 60000);
    report.compatibility.push({ name, title, add: true, refresh: true, entry: true, refreshResponseObserved: true });
    await navigate(client, `${base}/feeds`);
    await waitFor(client, `document.querySelector('[data-field="feed-url-input"]') !== null`);
  }
  const screenshot = await client.send('Page.captureScreenshot', { format: 'png' });
  await writeFile(path.join(run, 'feeds.png'), Buffer.from(screenshot.data, 'base64'));
  report.pass = true;
  server.stdin.end();
  assert.deepEqual(await server.done, { code: 0, signal: null });
}

try {
  await Promise.race([main(), new Promise((_, reject) => { deadline = setTimeout(() => reject(new Error('Acceptance exceeded 8 minutes')), 480000); })]);
} catch (error) {
  report.pass = false;
  report.error = error.stack;
  process.exitCode = 1;
} finally {
  clearTimeout(deadline);
  if (client) {
    try { await client.send('Browser.close'); } catch { /* socket can close before the reply */ }
    client.close();
  }
  for (const child of children) {
    if (child.launchError || child.exitCode !== null || child.signalCode !== null) continue;
    // Only the Rust fixture server uses stdin (EOF) for shutdown. Chrome closes its
    // stdin on Linux; writing to it during Browser.close races with exit and raises EPIPE.
    if (child.stdin && !child.stdin.writableEnded) child.stdin.end();
    const timer = setTimeout(() => child.kill(), 5000);
    await child.done;
    clearTimeout(timer);
  }
  await writeFile(path.join(run, 'server.log'), serverLog);
  await writeFile(path.join(run, 'chrome.log'), browserLog);
  await writeFile(path.join(run, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ pass: report.pass, report: path.join(run, 'result.json'), error: report.error }));
}
