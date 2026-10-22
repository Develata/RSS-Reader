import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { connect, newPage, evaluate, sleep } from './cdp_session.mjs';
import { pointerClick, fillField } from './server_page.mjs';

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
const requests = new Map();
const transfers = [];
const articleRendering = [];
client.on('Network.loadingFinished', ({ requestId, encodedDataLength }) => {
  const request = requests.get(requestId);
  if (request) transfers.push({ ...request, encodedDataLength });
});
client.on('Runtime.exceptionThrown', (event) => failures.push(event));
client.on('Runtime.consoleAPICalled', event => { if (event.type === 'error') failures.push(event); });
client.on('Network.responseReceived', ({ response, type }) => {
  // Dioxus normalizes away the query before reload. Pages returns the same
  // expected document 404 for both URLs; failed assets must still be reported.
  const url = new URL(response.url);
  const route = new URL(direct);
  const expectedFallback = response.status === 404 && type === 'Document'
    && url.origin === route.origin && url.pathname.startsWith(new URL(base).pathname + '/entries/');
  const sourceImage = type === 'Image' && url.origin !== route.origin;
  if (response.status >= 400 && !expectedFallback && !sourceImage && !response.url.endsWith('/favicon.ico')) {
    failures.push({ url: response.url, status: response.status, type });
  }
});
client.on('Network.loadingFailed', (event) => {
  if (!event.canceled && !externalImages.has(event.requestId)) failures.push(event);
});
client.on('Network.requestWillBeSent', ({ request, requestId, type }) => {
  requests.set(requestId, { url: request.url, type });
  if (/^https?:/.test(request.url) && new URL(request.url).origin !== new URL(base).origin) {
    // RSS article images can be remote; they are distinct from fetching feeds or executing code.
    if (type === 'Image') externalImages.set(requestId, request.url);
    else network.push(request.url);
  } else if (['Fetch', 'XHR'].includes(type) && /^https?:/.test(request.url)
      && !['/assets/', '/wasm/'].some(prefix => new URL(request.url).pathname.startsWith(new URL(base).pathname + prefix))) {
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
  await client.send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await client.send('Page.addScriptToEvaluateOnNewDocument', { source: `
    window.__pagesMeasure = { longTasks: [], memories: [] };
    new PerformanceObserver(list => {
      __pagesMeasure.longTasks.push(...list.getEntries().map(({startTime, duration}) => ({startTime, duration})));
    }).observe({type: 'longtask', buffered: true});
    for (const name of ['instantiate', 'instantiateStreaming']) {
      const original = WebAssembly[name];
      WebAssembly[name] = async function(...args) {
        const result = await original.apply(this, args);
        const instance = result.instance ?? result;
        for (const value of Object.values(instance.exports)) {
          if (value instanceof WebAssembly.Memory && !__pagesMeasure.memories.includes(value)) __pagesMeasure.memories.push(value);
        }
        return result;
      };
    }
    // An old/corrupt v0.1 browser library must neither be opened nor overwritten by the demo.
    if (!localStorage.getItem('rssr-web-state-v1')) localStorage.setItem('rssr-web-state-v1', 'old-library-left-untouched');
  ` });
  await client.send('Network.setCacheDisabled', { cacheDisabled: true });
  await client.send('Page.navigate', { url: direct });
  await waitFor(`!!document.querySelector('[data-slot="reader-body-html"]')`);
  assert.match(await evaluate(client, 'document.body.textContent'), /公开静态演示/);
  assert.ok((await evaluate(client, 'document.body.textContent')).includes(entries[1].title), 'reader shows the fetched RSS title');
  assert.ok(await evaluate(client, `document.querySelector('[data-slot="reader-body-html"]').textContent.trim().length > 100`), 'real article body renders');
  assert.equal(await evaluate(client, 'location.pathname'), new URL(direct).pathname);
  const startup = await evaluate(client, `({
    readyMs: performance.now(),
    wasmMemoryBytes: __pagesMeasure.memories.map(memory => memory.buffer.byteLength),
    longTasks: __pagesMeasure.longTasks,
    resources: performance.getEntriesByType('resource').map(({name, transferSize, encodedBodySize, decodedBodySize}) => ({name, transferSize, encodedBodySize, decodedBodySize}))
  })`);
  const heap = await client.send('Runtime.getHeapUsage');
  assert.ok(startup.wasmMemoryBytes.length > 0, 'measure the instantiated SQLite/WASM module');
  // Read state through the actual UI, mutate with a pointer, then start a fresh memory library.
  const starred = `document.querySelector('[data-action="toggle-starred"]')?.getAttribute('aria-pressed')`;
  await waitFor(`document.querySelector('[data-action="toggle-starred"]')?.disabled === false`);
  assert.equal(await evaluate(client, starred), 'false');
  await pointerClick(client, '[data-action="toggle-starred"]');
  await waitFor(`(${starred}) === 'true'`);
  await evaluate(client, 'globalThis.__pagesSmokeBeforeReload = true');
  await client.send('Page.reload');
  await waitFor(`!globalThis.__pagesSmokeBeforeReload && !!document.querySelector('[data-slot="reader-body-html"]')`);
  await waitFor(`document.querySelector('[data-action="toggle-starred"]')?.disabled === false`);
  assert.equal(await evaluate(client, starred), 'false', 'reloading resets the memory library');
  assert.equal(await evaluate(client, "localStorage.getItem('rssr-web-state-v1')"), 'old-library-left-untouched');
  // Real navigation must keep the prefix; a second home click invokes the guarded refresh.
  await pointerClick(client, '[data-action="activate-home"]');
  await waitFor(`location.pathname === ${JSON.stringify(new URL(base).pathname.replace(/\/$/, '') + '/entries')}`);
  await waitFor(`!!document.querySelector('[data-page="entries"] [data-action="activate-home"]')`);
  await pointerClick(client, '[data-action="activate-home"]');
  await waitFor(`document.body.textContent.includes('静态演示已禁用此网络操作')`);
  await pointerClick(client, '[data-nav="feeds"]');
  await waitFor(`!!document.querySelector('[data-page="feeds"] [data-feed-id="1"]')`);
  await pointerClick(client, '[data-action="refresh-feed"]');
  const feedError = `document.querySelector('[data-page="feeds"] > [data-layout="status-banner"][data-state="error"]')?.textContent`;
  await waitFor(`(${feedError})?.includes('静态演示已禁用此网络操作')`);
  await fillField(client, '#feed-url-input', 'https://example.com/pages-must-not-fetch.xml');
  await pointerClick(client, '[data-action="add-feed"]');
  // Since C4b a failure names its action first: "添加订阅失败：静态演示已禁用此网络操作…".
  await waitFor(`!document.querySelector('[data-action="add-feed"]').disabled && (${feedError})?.startsWith('添加订阅失败：静态演示已禁用此网络操作')`);
  assert.equal(await evaluate(client, `document.querySelectorAll('[data-layout="feed-card"]').length`), snapshot.feeds.length);
  const feedText = await evaluate(client, `document.querySelector('[data-page="feeds"]').textContent`);
  for (const { source } of snapshot.feeds) assert.ok(feedText.includes(source.title), `visible source: ${source.title}`);
  // All source articles must survive the SQL conversion and remain reachable with attribution.
  for (const [index, entry] of entries.entries()) {
    await client.send('Page.navigate', { url: `${base}/entries/${index + 1}` });
    await waitFor(`document.querySelector('[data-slot="reader-title"]')?.textContent === ${JSON.stringify(entry.title)}`);
    await waitFor(`!!document.querySelector('[data-slot="reader-body-html"]')`);
    assert.equal(await evaluate(client, `document.querySelector('[data-action="open-original"]').href`), entry.url);
    assert.ok(await evaluate(client, `document.querySelector('[data-slot="reader-body-html"]').textContent.trim().length > 100`));
    await evaluate(client, `Promise.race([
      Promise.all([...document.querySelectorAll('[data-slot="reader-body-html"] img')].map(img => img.complete ? Promise.resolve() : new Promise(resolve => {
        img.addEventListener('load', resolve, {once:true}); img.addEventListener('error', resolve, {once:true});
      }))), new Promise(resolve => setTimeout(resolve, 5000))
    ]).then(() => true)`);
    const rendering = await evaluate(client, `({
      title: document.querySelector('[data-slot="reader-title"]').textContent,
      math: document.querySelectorAll('[data-slot="reader-body-html"] math').length,
      mathLayout: [...document.querySelectorAll('[data-slot="reader-body-html"] math')].map(math => ({
        namespace: math.namespaceURI, width: math.getBoundingClientRect().width,
        height: math.getBoundingClientRect().height
      })),
      images: [...document.querySelectorAll('[data-slot="reader-body-html"] img')].map(img => ({
        src: img.currentSrc || img.src, alt: img.alt, complete: img.complete,
        naturalWidth: img.naturalWidth, width: img.getBoundingClientRect().width,
        formula: img.classList.contains('latex')
      }))
    })`);
    articleRendering.push(rendering);
    const raw = entry.content_html ?? '';
    assert.equal(rendering.math, (raw.match(/<math(?:\s|\/?>)/g) ?? []).length, `${entry.title}: every MathML formula survives`);
    assert.ok(rendering.mathLayout.every(math => math.namespace === 'http://www.w3.org/1998/Math/MathML' && math.width > 0 && math.height > 0), `${entry.title}: formulas have MathML layout`);
    assert.equal(rendering.images.filter(img => img.formula).length, (raw.match(/class="latex"/g) ?? []).length, `${entry.title}: formula images survive`);
    if (process.env.PAGES_SMOKE_LOG_DIR && (index === 0 || index === entries.length - 1)) {
      await evaluate(client, 'document.fonts.ready.then(() => true)');
      const { data } = await client.send('Page.captureScreenshot');
      await writeFile(join(process.env.PAGES_SMOKE_LOG_DIR, `article-${index + 1}.png`), Buffer.from(data, 'base64'));
      await evaluate(client, `document.querySelector('[data-slot="reader-body-html"]').scrollIntoView({block: 'start'})`);
      const body = await client.send('Page.captureScreenshot');
      await writeFile(join(process.env.PAGES_SMOKE_LOG_DIR, `article-${index + 1}-body.png`), Buffer.from(body.data, 'base64'));
    }
  }
  const persisted = await evaluate(client, `Object.values(localStorage)`);
  for (const { title } of entries) assert.ok(!persisted.some(value => value.includes(title)), 'no article library in localStorage');
  assert.deepEqual(network, [], 'demo must not fetch feeds or auto-refresh');
  assert.deepEqual(failures, [], 'bundle assets and runtime must load without errors');
  const result = { status: 'pass', fetched_at: snapshot.fetched_at, feeds: snapshot.feeds.length, articles: entries.length,
    storage: 'memory-sqlite', reloadResets: true, startup, heap, transfers, articleRendering,
    sourceHashes: snapshot.feeds.map(({ source, sha256 }) => ({ url: source.url, sha256 })), externalImages: [...externalImages.values()] };
  if (process.env.PAGES_SMOKE_LOG_DIR) await writeFile(join(process.env.PAGES_SMOKE_LOG_DIR, 'result.json'), JSON.stringify(result, null, 2));
  console.log(`Pages smoke passed: ${result.feeds} real feeds, ${result.articles} articles, reader, 404 deep link, prefix, reload reset, blocked add/refresh`);
} catch (error) {
  const logDir = process.env.PAGES_SMOKE_LOG_DIR;
  if (logDir) {
    await writeFile(join(logDir, 'failure.json'), JSON.stringify({ error: String(error), failures, network, articleRendering, externalImages: [...externalImages.values()] }, null, 2));
    try {
      const { data } = await client.send('Page.captureScreenshot');
      await writeFile(join(logDir, 'failure.png'), Buffer.from(data, 'base64'));
    } catch { /* Chrome itself may have exited. Preserve the original failure. */ }
  }
  throw error;
} finally {
  client.close();
}
