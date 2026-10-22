// C6: a real packaged page must send its own build id; an actual older page cannot save.
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { connect, evaluate, navigate, newPage, selectorExists, sleep, submitLoginForm, waitFor } from './cdp_session.mjs';
import { pointerClick, VISIBLE } from './server_page.mjs';

const [cdpBase, base, buildId, mode, report] = process.argv.slice(2);
assert.ok(cdpBase && base && buildId && ['matching', 'old-page'].includes(mode) && report);
const target = await newPage('about:blank', cdpBase);
const client = connect(target.webSocketDebuggerUrl);
const errors = [], writes = [], replies = new Map();
client.on('Runtime.exceptionThrown', (event) => errors.push(event.exceptionDetails));
client.on('Runtime.consoleAPICalled', (event) => { if (event.type === 'error') errors.push(event); });
client.on('Network.requestWillBeSent', ({ requestId, request }) => {
  if (new URL(request.url).pathname === '/api/v1/settings' && request.method === 'PATCH') {
    writes.push({ id: requestId, build: Object.entries(request.headers).find(([key]) => key.toLowerCase() === 'x-rssr-build')?.[1] });
  }
});
client.on('Network.responseReceived', ({ requestId, response }) => replies.set(requestId, response.status));
try {
  await client.send('Page.enable');
  await client.send('Runtime.enable');
  await client.send('Network.enable');
  await client.send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false });
  await navigate(client, `${base}/settings`);
  await submitLoginForm(client, 'smoke', 'package-smoke-pass');
  await selectorExists(client, '[data-page="settings"] [data-field="theme-mode"]');
  const cookies = (await client.send('Network.getCookies', { urls: [base] })).cookies;
  const settings = async () => {
    const response = await fetch(`${base}/api/v1/settings`, { headers: { Cookie: cookies.map(({ name, value }) => `${name}=${value}`).join('; ') } });
    assert.equal(response.status, 200);
    return await response.json();
  };
  const before = await settings();
  const theme = before.theme === 'dark' ? 'light' : 'dark';
  await evaluate(client, `(() => {
    const select = document.querySelector('[data-field="theme-mode"]');
    select.value = ${JSON.stringify(theme)};
    select.dispatchEvent(new Event('change', {bubbles:true}));
  })()`);
  await waitFor(client, `!document.querySelector('[data-action="save-settings"]').disabled`);
  await pointerClick(client, '[data-action="save-settings"]');
  for (let attempt = 0; attempt < 150 && !writes.some((write) => replies.has(write.id)); attempt++) await sleep(100);
  assert.equal(writes.length, 1, 'one user save must produce exactly one write');
  assert.equal(writes[0].build, buildId, 'page must send its compiled identifier, without header injection');
  if (mode === 'old-page') {
    assert.equal(replies.get(writes[0].id), 409);
    await selectorExists(client, '[data-slot="app-notice"][data-notice="reload"]');
    assert.ok(await evaluate(client, `['[data-notice="reload"]', '[data-action="reload-page"]'].every(selector => {
      const element = document.querySelector(selector); return element && ${VISIBLE}(element);
    })`), 'build recovery and its action must be visible');
    assert.equal((await settings()).theme, before.theme, 'old page must not modify the server library');
  } else {
    assert.equal(replies.get(writes[0].id), 200);
    assert.equal((await settings()).theme, theme);
  }
  assert.deepEqual(errors, []);
  await writeFile(report, JSON.stringify({ status: 'pass', mode, buildId, writes, runtimeErrors: errors }, null, 2));
  console.log(`${mode}: real page build header, visible recovery/write and library state passed`);
} finally {
  client.close();
  const version = await (await fetch(`${cdpBase}/json/version`)).json();
  const browser = connect(version.webSocketDebuggerUrl);
  await browser.send('Browser.close').catch(() => {});
  browser.close();
}
