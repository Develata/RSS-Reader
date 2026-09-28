import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { connect, newPage, evaluate, sleep } from './cdp_session.mjs';

const base = process.env.PAGES_TEST_BASE ?? 'http://127.0.0.1:8093/RSS-Reader';
const direct = `${base}/entries/2?pages-smoke=1`;
assert.equal((await fetch(`${base}/`)).status, 200);
assert.equal((await fetch(direct)).status, 404, 'simulate real Pages deep-link status');
const client = connect((await newPage()).webSocketDebuggerUrl);
const failures = [];
const network = [];
client.on('Runtime.exceptionThrown', (event) => failures.push(event));
client.on('Network.responseReceived', ({ response }) => {
  if (response.status >= 400 && response.url !== direct && !response.url.endsWith('/favicon.ico')) failures.push(response.url);
});
client.on('Network.loadingFailed', (event) => {
  if (!event.canceled) failures.push(event);
});
client.on('Network.requestWillBeSent', ({ request }) => {
  if (/^https?:/.test(request.url) && new URL(request.url).origin !== new URL(base).origin) {
    network.push(request.url);
  }
});
async function waitFor(expression) {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (await evaluate(client, expression)) return;
    await sleep(200);
  }
  throw new Error(`Timed out: ${expression}`);
}
try {
  await client.send('Runtime.enable');
  await client.send('Network.enable');
  await client.send('Page.enable');
  await client.send('Page.navigate', { url: direct });
  await waitFor(`!!document.querySelector('[data-slot="reader-body-html"]')`);
  assert.match(await evaluate(client, 'document.body.textContent'), /公开静态演示/);
  assert.equal(await evaluate(client, 'location.pathname'), new URL(direct).pathname);
  // An actual mutation must survive reload, rather than reapplying the seed.
  const starred = `(() => {
    const commit = JSON.parse(localStorage.getItem('rssr-web-commit-v1'));
    if (!commit) return null;
    const flags = JSON.parse(localStorage.getItem('rssr-web-entry-flags-v1' + (commit.revisions[2] % 2 ? '-next' : '')));
    return flags.entries.find(entry => entry.id === 2).is_starred;
  })()`;
  await waitFor(`document.querySelector('[data-action="toggle-starred"][data-state="starred"]') !== null`);
  await evaluate(client, `document.querySelector('[data-action="toggle-starred"]').click()`);
  await waitFor(`(${starred}) === false`);
  await evaluate(client, 'globalThis.__pagesSmokeBeforeReload = true');
  await client.send('Page.reload');
  await waitFor(`!globalThis.__pagesSmokeBeforeReload && !!document.querySelector('[data-slot="reader-body-html"]')`);
  assert.equal(await evaluate(client, starred), false, 'reload must not reapply the starred seed');
  // Real navigation must keep the prefix; a second home click invokes the guarded refresh.
  await evaluate(client, `document.querySelector('[data-action="activate-home"]').click()`);
  await waitFor(`location.pathname === ${JSON.stringify(new URL(base).pathname.replace(/\/$/, '') + '/entries')}`);
  await waitFor(`!!document.querySelector('[data-page="entries"] [data-action="activate-home"]')`);
  await evaluate(client, `document.querySelector('[data-action="activate-home"]').click()`);
  await waitFor(`document.body.textContent.includes('静态演示已禁用此网络操作')`);
  await evaluate(client, `document.querySelector('[data-nav="feeds"]').click()`);
  await waitFor(`!!document.querySelector('[data-page="feeds"] [data-feed-id="1"]')`);
  await evaluate(client, `document.querySelector('[data-action="refresh-feed"]').click()`);
  const feedError = `document.querySelector('[data-page="feeds"] > [data-layout="status-banner"][data-state="error"]')?.textContent`;
  await waitFor(`(${feedError})?.includes('静态演示已禁用此网络操作')`);
  await evaluate(client, `(() => {
    const input = document.querySelector('#feed-url-input');
    input.value = 'https://example.com/pages-must-not-fetch.xml';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  })()`);
  await evaluate(client, `document.querySelector('[data-action="add-feed"]').click()`);
  await waitFor(`!document.querySelector('[data-action="add-feed"]').disabled && (${feedError})?.startsWith('静态演示已禁用此网络操作')`);
  assert.equal(await evaluate(client, `document.querySelectorAll('[data-layout="feed-card"]').length`), 1);
  assert.deepEqual(network, [], 'demo must not fetch feeds or auto-refresh');
  assert.deepEqual(failures, [], 'bundle assets and runtime must load without errors');
  console.log('Pages smoke passed: actual reader, 404 deep link, prefix, persistence, blocked add/refresh');
} catch (error) {
  const logDir = process.env.PAGES_SMOKE_LOG_DIR;
  if (logDir) {
    await writeFile(join(logDir, 'failure.json'), JSON.stringify({ error: String(error), failures, network }, null, 2));
    try {
      const { data } = await client.send('Page.captureScreenshot');
      await writeFile(join(logDir, 'failure.png'), Buffer.from(data, 'base64'));
    } catch { /* Chrome itself may have exited. Preserve the original failure. */ }
  }
  throw error;
} finally {
  client.close();
}
