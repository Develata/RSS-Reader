import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { connect, newPage, evaluate, sleep } from './cdp_session.mjs';

const base = process.env.PAGES_TEST_BASE ?? 'http://127.0.0.1:8093/RSS-Reader';
const direct = `${base}/entries/2?pages-smoke=1`;
const snapshot = JSON.parse(await readFile(process.env.PAGES_SNAPSHOT_PATH ?? 'assets/pages-demo/snapshot.json', 'utf8'));
const entries = snapshot.feeds.flatMap(({ feed }) => feed.entries);
assert.ok(entries.length >= 2, 'the demo needs real articles for its direct reader link');
assert.equal((await fetch(`${base}/`)).status, 200);
assert.equal((await fetch(direct)).status, 404, 'simulate real Pages deep-link status');
const client = connect((await newPage()).webSocketDebuggerUrl);
const failures = [];
const network = [];
const externalImages = new Map();
client.on('Runtime.exceptionThrown', (event) => failures.push(event));
client.on('Network.responseReceived', ({ response, type }) => {
  // Dioxus normalizes away the query before reload. Pages returns the same
  // expected document 404 for both URLs; failed assets must still be reported.
  const url = new URL(response.url);
  const route = new URL(direct);
  const expectedFallback = response.status === 404 && type === 'Document'
    && url.origin === route.origin && url.pathname === route.pathname;
  const sourceImage = type === 'Image' && url.origin !== route.origin;
  if (response.status >= 400 && !expectedFallback && !sourceImage && !response.url.endsWith('/favicon.ico')) {
    failures.push({ url: response.url, status: response.status, type });
  }
});
client.on('Network.loadingFailed', (event) => {
  if (!event.canceled && !externalImages.has(event.requestId)) failures.push(event);
});
client.on('Network.requestWillBeSent', ({ request, requestId, type }) => {
  if (/^https?:/.test(request.url) && new URL(request.url).origin !== new URL(base).origin) {
    // RSS article images can be remote; they are distinct from fetching feeds or executing code.
    if (type === 'Image') externalImages.set(requestId, request.url);
    else network.push(request.url);
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
  assert.ok((await evaluate(client, 'document.body.textContent')).includes(entries[1].title), 'reader shows the fetched RSS title');
  assert.ok(await evaluate(client, `document.querySelector('[data-slot="reader-body-html"]').textContent.trim().length > 100`), 'real article body renders');
  assert.equal(await evaluate(client, 'location.pathname'), new URL(direct).pathname);
  // An actual mutation must survive reload, rather than reapplying the seed.
  const starred = `(() => {
    const commit = JSON.parse(localStorage.getItem('rssr-web-commit-v1'));
    if (!commit) return null;
    const flags = JSON.parse(localStorage.getItem('rssr-web-entry-flags-v1' + (commit.revisions[2] % 2 ? '-next' : '')));
    return flags.entries.find(entry => entry.id === 2)?.is_starred ?? false;
  })()`;
  await waitFor(`document.querySelector('[data-action="toggle-starred"]') !== null`);
  assert.equal(await evaluate(client, starred), false);
  await evaluate(client, `document.querySelector('[data-action="toggle-starred"]').click()`);
  await waitFor(`(${starred}) === true`);
  await evaluate(client, 'globalThis.__pagesSmokeBeforeReload = true');
  await client.send('Page.reload');
  await waitFor(`!globalThis.__pagesSmokeBeforeReload && !!document.querySelector('[data-slot="reader-body-html"]')`);
  assert.equal(await evaluate(client, starred), true, 'reload must preserve the real article mutation');
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
  // Since C4b a failure names its action first: "添加订阅失败：静态演示已禁用此网络操作…".
  await waitFor(`!document.querySelector('[data-action="add-feed"]').disabled && (${feedError})?.startsWith('添加订阅失败：静态演示已禁用此网络操作')`);
  assert.equal(await evaluate(client, `document.querySelectorAll('[data-layout="feed-card"]').length`), snapshot.feeds.length);
  const feedText = await evaluate(client, `document.querySelector('[data-page="feeds"]').textContent`);
  for (const { source } of snapshot.feeds) assert.ok(feedText.includes(source.title), `visible source: ${source.title}`);
  const stored = await evaluate(client, `(() => {
    const commit = JSON.parse(localStorage.getItem('rssr-web-commit-v1'));
    const core = JSON.parse(localStorage.getItem('rssr-web-state-v1' + (commit.revisions[0] % 2 ? '-next' : '')));
    return core.entries.map(({ title, url, has_content }) => ({ title, url, has_content }));
  })()`);
  assert.deepEqual(stored, entries.map(({ title, url }) => ({ title, url, has_content: true })), 'all fetched articles and original links reach the real store');
  assert.deepEqual(network, [], 'demo must not fetch feeds or auto-refresh');
  assert.deepEqual(failures, [], 'bundle assets and runtime must load without errors');
  const result = { status: 'pass', fetched_at: snapshot.fetched_at, feeds: snapshot.feeds.length, articles: entries.length,
    sourceHashes: snapshot.feeds.map(({ source, sha256 }) => ({ url: source.url, sha256 })), externalImages: [...externalImages.values()] };
  if (process.env.PAGES_SMOKE_LOG_DIR) await writeFile(join(process.env.PAGES_SMOKE_LOG_DIR, 'result.json'), JSON.stringify(result, null, 2));
  console.log(`Pages smoke passed: ${result.feeds} real feeds, ${result.articles} articles, reader, 404 deep link, prefix, persistence, blocked add/refresh`);
} catch (error) {
  const logDir = process.env.PAGES_SMOKE_LOG_DIR;
  if (logDir) {
    await writeFile(join(logDir, 'failure.json'), JSON.stringify({ error: String(error), failures, network, externalImages: [...externalImages.values()] }, null, 2));
    try {
      const { data } = await client.send('Page.captureScreenshot');
      await writeFile(join(logDir, 'failure.png'), Buffer.from(data, 'base64'));
    } catch { /* Chrome itself may have exited. Preserve the original failure. */ }
  }
  throw error;
} finally {
  client.close();
}
