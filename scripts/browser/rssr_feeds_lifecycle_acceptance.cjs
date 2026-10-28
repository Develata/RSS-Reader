// Real Web/WASM or isolated native acceptance. All refresh traffic stays on loopback fixtures.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require('playwright');
const fixture = require('./directory_fixture.cjs');
const { installStorageHelpers } = require('./storage_helpers.cjs');
const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, args) => {
  if (index % 2 === 0) pairs.push([value.slice(2), args[index + 1]]);
  return pairs;
}, []));
const action = name => `[data-action="${name}"]`;
const banner = '[data-page="feeds"] > [data-layout="status-banner"]';
const cards = '[data-layout="feed-card"]';
const frames = page => page.evaluate(async () => {
  await document.fonts.ready;
  await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
});
async function eventually(read, expected, label) {
  const deadline = Date.now() + 15000;
  for (;;) {
    try { assert.deepEqual(await read(), expected, label); return; }
    catch (error) { if (Date.now() >= deadline) throw error; }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
}
(async () => {
  const output = path.resolve(options['artifact-dir'] || process.env.ARTIFACT_DIR || 'target/feeds-lifecycle-acceptance');
  const native = !!options.cdp;
  const report = { native, status: 'running', results: [], pageErrors: [], consoleErrors: [] };
  let database, browser, page, server;
  let responseMode = 'success', held = false, release, requestCount = 0;
  let gate = Promise.resolve();
  const respond = async url => {
    requestCount++;
    if (held) await gate;
    if (url.pathname.endsWith('/new-feed.xml')) return { status: 200, body: '<rss version="2.0"><channel><title>Lifecycle New Feed</title><link>http://127.0.0.1/</link><description>Fixture</description></channel></rss>' };
    return responseMode === 'failure' ? { status: 200, body: 'invalid feed fixture' } : { status: 304, body: '' };
  };
  try {
    await fs.mkdir(output, { recursive: true });
    // Replace any previous receipt before acquiring resources or validating the target.
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    database = native ? new (require('node:sqlite').DatabaseSync)(path.resolve(options.database), { readOnly: true }) : null;
    browser = native ? await chromium.connectOverCDP(options.cdp) : await chromium.launch({ channel: 'chrome', headless: true });
    const context = native ? browser.contexts()[0] : await browser.newContext();
    const origin = native ? options['feed-origin'] : fixture.origin;
    if (!native) {
      await fixture.install(context, path.resolve(__dirname, '../..')); await installStorageHelpers(context);
      await context.route(url => url.origin === origin && (url.pathname.startsWith('/__codex/feed/') || url.pathname === '/new-feed.xml'), async route => {
        const response = await respond(new URL(route.request().url()));
        await route.fulfill({ ...response, contentType: 'application/rss+xml; charset=utf-8' });
      });
    }
    page = native ? context.pages().find(page => /dioxus/.test(page.url())) : await context.newPage();
    assert.ok(page, 'explicit target page'); page.setDefaultTimeout(15000);
    page.on('pageerror', error => report.pageErrors.push(String(error)));
    page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
    const flags = async () => native
      ? database.prepare('SELECT id, is_read, is_starred, is_archived FROM entries ORDER BY id').all().map(row => ({ ...row }))
      : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-entry-flags-v1'))).entries);
    const feedRows = async () => native
      ? database.prepare('SELECT id, url FROM feeds WHERE is_deleted = 0 ORDER BY id').all().map(row => ({ ...row }))
      : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-state-v1'))).feeds.filter(feed => !feed.is_deleted).map(({ id, url }) => ({ id, url })).sort((a, b) => a.id - b.id));
    const feedback = () => page.locator(banner).textContent();
    const waitFeedback = async (text, tone) => {
      await page.waitForFunction(({ text, tone }) => {
        const node = document.querySelector('[data-page="feeds"] > [data-layout="status-banner"]');
        return node?.textContent.includes(text) && (!tone || node.dataset.state === tone);
      }, { text, tone });
      return feedback();
    };
    const count = name => page.locator(`[data-stat="${name}"] [data-slot="stat-card-value"]`).textContent();
    const scenario = async (name, run) => {
      try { const evidence = await run(); report.results.push({ name, pass: true, evidence }); console.log('PASS ' + name); }
      catch (error) { report.results.push({ name, pass: false, error: String(error.stack) }); throw error; }
    };
    if (native) {
      const catalog = database.prepare('SELECT id, title FROM entries ORDER BY id').all();
      assert.equal(catalog.length, 5000);
      for (const [index, row] of catalog.entries()) {
        assert.equal(row.id, index + 1); assert.equal(row.title, `阅读基线文章 ${String(row.id).padStart(5, '0')} — Reading baseline`);
      }
      await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
      const visible = await page.locator('[data-layout="entry-card"][data-position-entry]').evaluateAll(rows => rows.map(row => ({ id: Number(row.dataset.positionEntry), title: row.querySelector('[data-slot="entry-card-title"]').textContent.trim() })));
      assert.ok(visible.length > 0);
      for (const row of visible) assert.equal(row.title, catalog[row.id - 1]?.title, 'UI matches isolated native fixture');
      const base = new URL(origin);
      assert.equal(base.hostname, '127.0.0.1'); assert.equal(base.protocol, 'http:');
      for (const row of await feedRows()) assert.equal(new URL(row.url).origin, base.origin, 'native fixture only contacts the owned loopback server');
      server = http.createServer(async (request, response) => {
        try { const data = await respond(new URL(request.url, origin)); response.writeHead(data.status, { 'Content-Type': 'application/rss+xml' }); response.end(data.body); }
        catch (error) { response.destroy(error); }
      });
      await new Promise((resolve, reject) => { server.once('error', reject); server.listen(Number(base.port), '127.0.0.1', resolve); });
    }
    for (const width of [1536, 390]) await scenario('feeds-flow-' + width, async () => {
      await page.setViewportSize({ width, height: 844 }); responseMode = 'success';
      if (!native) await page.goto(origin + '/__codex/setup-local-auth?seed=directory-contract&next=/feeds');
      else await page.locator('[data-nav="feeds"]').click();
      await eventually(() => count('feeds'), '6', 'snapshot feed count');
      await eventually(() => page.locator(cards).count(), 6, 'snapshot cards');
      const originalFlags = await flags(), originalFeeds = await feedRows();
      assert.equal(originalFlags.length, native ? 5000 : 72);
      assert.equal(await count('entries'), String(originalFlags.length));
      await frames(page);
      if (!native) await page.screenshot({ path: path.join(output, `feeds-${width}.png`), animations: 'disabled', caret: 'hide' });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
      const first = page.locator(`${cards}[data-feed-id="1"]`);
      const title = await first.getAttribute('data-feed-title');
      await first.locator(action('refresh-feed')).click();
      const success = await waitFeedback(title, 'info');
      assert.ok(success.length > title.length);
      await eventually(feedback, '', 'successful refresh feedback expires');
      responseMode = 'failure'; await first.locator(action('refresh-feed')).click();
      const error = await waitFeedback('刷新订阅失败', 'error');
      await page.waitForTimeout(3300);
      assert.equal(await feedback(), error, 'error feedback outlives the success interval');
      await eventually(feedback, '', 'error feedback expires');
      responseMode = 'success'; await first.locator(action('refresh-feed')).click();
      await waitFeedback(title, 'info');
      await first.locator(action('remove-feed')).click();
      const confirmation = await waitFeedback('再次点击即可删除订阅');
      await page.waitForTimeout(3300);
      assert.equal(await feedback(), confirmation, 'old refresh timer cannot clear another message with the same revision');
      await page.locator(action('export-config')).click(); await waitFeedback('已导出配置包 JSON');
      const beforeRequests = requestCount;
      held = true; gate = new Promise(resolve => { release = resolve; });
      try {
        await page.locator(action('refresh-all')).click();
        await eventually(async () => requestCount > beforeRequests, true, 'shell refresh reached the fixture');
        assert.equal(await page.locator(action('refresh-all')).isDisabled(), true);
      } finally { held = false; release(); }
      await eventually(() => page.locator(action('refresh-all')).isDisabled(), false, 'shell refresh releases busy');
      await page.locator('[data-field="feed-url-input"]').fill(origin + '/new-feed.xml');
      await page.locator(action('add-feed')).click();
      await eventually(() => count('feeds'), '7', 'add completion reloads snapshot');
      const added = page.locator(cards).filter({ has: page.locator('[data-slot="feed-card-title"]', { hasText: 'Lifecycle New Feed' }) });
      await added.locator(action('remove-feed')).click(); await added.locator(action('remove-feed')).click();
      await eventually(() => count('feeds'), '6', 'remove completion reloads snapshot');
      await eventually(feedRows, originalFeeds, 'only the added fixture feed was removed');
      assert.equal(await count('entries'), String(originalFlags.length));
      assert.deepEqual(await flags(), originalFlags, 'all article flags survive refresh and feed mutations');
      return { feedCount: 6, articleCount: originalFlags.length, errorSurvivedSuccessInterval: true, newerConfirmationPreserved: true };
    });
    assert.deepEqual(report.pageErrors, []); assert.deepEqual(report.consoleErrors, []);
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed'; report.error = String(error.stack);
    if (!native && page) await page.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
    throw error;
  } finally {
    held = false; release?.();
    // Attempt every cleanup even if another fails; report writes must not prevent release.
    const cleanup = await Promise.allSettled([
      (async () => { if (server?.listening) await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve())); })(),
      (async () => database?.close())(),
      (async () => browser?.close())(),
    ]);
    const errors = cleanup.filter(result => result.status === 'rejected').map(result => result.reason);
    if (errors.length) { report.status = 'failed'; report.cleanupErrors = errors.map(String); }
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    if (errors.length) throw new AggregateError(errors, 'acceptance cleanup failed');
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
