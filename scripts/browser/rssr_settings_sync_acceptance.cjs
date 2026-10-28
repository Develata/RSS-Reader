// Actual Web/WASM or isolated native UI against an owned loopback WebDAV fixture.
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
const field = name => `[data-field="${name}"]`;
async function eventually(read, expected, label) {
  const deadline = Date.now() + 15000;
  for (;;) {
    try { assert.deepEqual(await read(), expected, label); return; }
    catch (error) { if (Date.now() >= deadline) throw error; }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
}

(async () => {
  const output = path.resolve(options['artifact-dir'] || 'target/settings-sync-acceptance');
  const native = !!options.cdp;
  const origin = 'http://127.0.0.1:9277';
  const report = { native, status: 'running', results: [], requests: [], pageErrors: [], consoleErrors: [], expectedConsoleErrors: [] };
  let browser, page, database, server, remote, release;
  let responseMode = 'success', gate = Promise.resolve();
  try {
    await fs.mkdir(output, { recursive: true });
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    database = native ? new (require('node:sqlite').DatabaseSync)(path.resolve(options.database), { readOnly: true }) : null;
    browser = native ? await chromium.connectOverCDP(options.cdp) : await chromium.launch({ channel: 'chrome', headless: true });
    const context = native ? browser.contexts()[0] : await browser.newContext();
    if (!native) {
      await fixture.install(context, path.resolve(__dirname, '../..')); await installStorageHelpers(context);
      // The intercepted application origin has no real listener. Forward fixture requests
      // through Playwright so Chromium's synthetic-origin address space cannot block them.
      // Native mode uses the actual HTTP client; this Web fixture does not test CORS policy.
      await context.route(origin + '/**', async route => {
        const response = await route.fetch(); await route.fulfill({ response });
      });
    }
    page = native ? context.pages().find(page => /dioxus/.test(page.url())) : await context.newPage();
    assert.ok(page, 'explicit target page'); page.setDefaultTimeout(15000);
    page.on('pageerror', error => report.pageErrors.push(String(error)));
    page.on('console', message => {
      if (message.type() !== 'error') return;
      const expected = message.location().url === origin + '/later/missing.json' && /status of 404/.test(message.text());
      report[expected ? 'expectedConsoleErrors' : 'consoleErrors'].push(message.text());
    });
    const settings = () => native
      ? JSON.parse(database.prepare("SELECT value FROM app_settings WHERE key = 'user_settings'").get().value)
      : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-state-v1'))).settings);
    const flags = () => native
      ? database.prepare('SELECT id, is_read, is_starred, is_archived FROM entries ORDER BY id').all().map(row => ({ ...row }))
      : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-entry-flags-v1'))).entries);
    const feeds = () => native
      ? database.prepare('SELECT id, url FROM feeds WHERE is_deleted = 0 ORDER BY id').all().map(row => ({ ...row }))
      : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-state-v1'))).feeds.filter(row => !row.is_deleted).map(({ id, url }) => ({ id, url })).sort((a, b) => a.id - b.id));
    if (native) {
      // No navigation or mutation until the supplied database and current UI both match the fixture.
      const catalog = database.prepare('SELECT id, title FROM entries ORDER BY id').all();
      assert.equal(catalog.length, 5000);
      for (const [index, row] of catalog.entries()) {
        assert.equal(row.id, index + 1); assert.equal(row.title, `阅读基线文章 ${String(row.id).padStart(5, '0')} — Reading baseline`);
      }
      await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
      const visible = await page.locator('[data-layout="entry-card"][data-position-entry]').evaluateAll(rows => rows.map(row => ({ id: Number(row.dataset.positionEntry), title: row.querySelector('[data-slot="entry-card-title"]').textContent.trim() })));
      assert.ok(visible.length > 0);
      for (const row of visible) assert.equal(row.title, catalog[row.id - 1]?.title, 'UI matches isolated fixture');
      for (const row of await feeds()) assert.equal(new URL(row.url).origin, origin, 'fixture feeds use loopback only');
      report.preflight = { articleCount: catalog.length, visibleRowsChecked: visible.length };
    }
    server = http.createServer(async (request, response) => {
      response.setHeader('Access-Control-Allow-Origin', '*');
      response.setHeader('Access-Control-Allow-Methods', 'GET, PUT, OPTIONS');
      response.setHeader('Access-Control-Allow-Headers', 'content-type');
      try {
        if (request.method === 'OPTIONS') { response.writeHead(204); response.end(); return; }
        if (/^\/feed-\d+\.xml$/.test(request.url)) { response.writeHead(304); response.end(); return; }
        const record = { method: request.method, path: request.url };
        report.requests.push(record);
        if (request.method === 'PUT') {
          const chunks = []; for await (const chunk of request) chunks.push(chunk);
          remote = JSON.parse(Buffer.concat(chunks).toString('utf8'));
          record.feedCount = remote.feeds.length;
          response.writeHead(201); response.end(); return;
        }
        assert.equal(request.method, 'GET');
        const mode = responseMode, payload = JSON.stringify(remote);
        await gate;
        response.writeHead(mode === 'missing' ? 404 : 200, { 'Content-Type': 'application/json' });
        response.end(mode === 'invalid' ? 'invalid config fixture' : payload);
      } catch (error) { report.serverError = String(error.stack); response.destroy(error); }
    });
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(9277, '127.0.0.1', resolve); });
    const waitStatus = (text, tone = 'info') => page.waitForFunction(({ text, tone }) => {
      const banner = document.querySelector('[data-page="settings"] [data-layout="status-banner"]');
      return banner?.dataset.state === tone && banner.textContent.includes(text);
    }, { text, tone });
    const pullState = expected => eventually(() => page.locator(action('pull-webdav')).getAttribute('data-state'), expected, 'pull confirmation');
    const clickPull = () => page.locator(action('pull-webdav')).click();
    const confirmOnly = async () => {
      const before = report.requests.length;
      await clickPull(); await pullState('confirm');
      await waitStatus('再次点击才会执行');
      assert.equal(report.requests.length, before, 'first click sends no request');
    };
    const screenshot = async name => {
      if (native) return;
      // Capture the whole layout from the top: scrolling a tall grid card into view
      // before an element screenshot otherwise places its heading under the sticky nav.
      await page.evaluate(async () => { window.scrollTo(0, 0); await document.fonts.ready; await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))); });
      await page.screenshot({ path: path.join(output, name + '.png'), fullPage: true, animations: 'disabled', caret: 'hide' });
    };
    for (const width of [1536, 390]) {
      const start = report.requests.length;
      await page.setViewportSize({ width, height: 844 }); responseMode = 'success';
      if (native) { await page.locator('[data-nav="feeds"]').click(); await page.locator('[data-nav="settings"]').click(); }
      else await page.goto(fixture.origin + '/__codex/setup-local-auth?seed=directory-contract&next=/settings');
      await page.locator(field('theme-mode')).waitFor();
      const saved = await settings(), originalFlags = await flags(), originalFeeds = await feeds();
      await eventually(() => page.locator(field('entries-page-size')).inputValue(), String(saved.entries_page_size), 'initial load');
      assert.equal(originalFlags.length, native ? 5000 : 72); assert.equal(originalFeeds.length, 6);
      assert.equal(await page.locator(field('webdav-endpoint')).inputValue(), '');
      assert.equal(await page.locator(field('webdav-remote-path')).inputValue(), 'config/rss-reader.json');
      await screenshot(`sync-${width}`);
      await page.locator(field('webdav-endpoint')).fill(origin + '/dav/');
      await page.locator(field('webdav-remote-path')).fill('config.json');
      await confirmOnly(); await screenshot(`confirmation-${width}`);
      await page.locator(field('webdav-endpoint')).fill(origin + '/other/'); await pullState('idle');
      await confirmOnly();
      await page.locator(field('webdav-remote-path')).fill('selected.json'); await pullState('idle');
      await confirmOnly();
      await page.locator(action('push-webdav')).click(); await pullState('idle');
      await waitStatus('配置已上传到 WebDAV：6 个订阅');
      assert.deepEqual(report.requests.slice(start), [{ method: 'PUT', path: '/other/selected.json', feedCount: 6 }]);
      assert.deepEqual(remote.settings, saved, 'upload persisted settings');
      assert.equal(remote.feeds.length, 6);
      assert.ok(!('entries' in remote) && !('endpoint' in remote) && !('webdav_endpoint' in remote.settings));
      // Change only settings in the uploaded config; keep the exact subscription set.
      remote.settings = { ...saved, archive_after_months: 120, theme: saved.theme === 'dark' ? 'light' : 'dark' };
      gate = new Promise(resolve => { release = resolve; });
      await confirmOnly(); await clickPull(); await pullState('idle');
      await eventually(() => report.requests.length, start + 2, 'confirmed download reaches fixture');
      assert.deepEqual(report.requests.at(-1), { method: 'GET', path: '/other/selected.json' });
      await page.locator(field('webdav-endpoint')).fill(origin + '/later/');
      await page.locator(field('webdav-remote-path')).fill('later.json');
      release(); gate = Promise.resolve();
      await waitStatus('已从 WebDAV 下载并导入配置');
      await eventually(settings, remote.settings, 'download persists exact settings');
      await eventually(() => page.locator(field('theme-mode')).inputValue(), remote.settings.theme, 'download updates draft');
      await eventually(() => page.locator('.app-shell').evaluate((node, theme) => node.classList.contains('theme-' + theme), remote.settings.theme), true, 'download updates applied theme');
      assert.equal(await page.locator(field('webdav-remote-path')).inputValue(), 'later.json');
      responseMode = 'missing';
      await page.locator(field('webdav-remote-path')).fill('missing.json');
      await confirmOnly(); await clickPull(); await pullState('idle'); await waitStatus('远端配置不存在');
      assert.deepEqual(await settings(), remote.settings);
      responseMode = 'invalid';
      await page.locator(field('webdav-remote-path')).fill('retry.json');
      await confirmOnly(); await clickPull(); await pullState('idle'); await waitStatus('下载配置失败', 'error');
      assert.deepEqual(await settings(), remote.settings);
      responseMode = 'success';
      await confirmOnly(); await clickPull(); await waitStatus('已从 WebDAV 下载并导入配置');
      assert.deepEqual(report.requests.slice(start).map(({ method, path }) => ({ method, path })), [
        { method: 'PUT', path: '/other/selected.json' }, { method: 'GET', path: '/other/selected.json' },
        { method: 'GET', path: '/later/missing.json' }, { method: 'GET', path: '/later/retry.json' }, { method: 'GET', path: '/later/retry.json' },
      ]);
      assert.deepEqual(await flags(), originalFlags, 'all article flags preserved');
      assert.deepEqual(await feeds(), originalFeeds, 'all feed identities and URLs preserved');
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
      report.results.push({ width, pass: true, articleCount: originalFlags.length, feedCount: originalFeeds.length, requests: report.requests.length - start });
      console.log('PASS settings-sync-' + width);
    }
    assert.deepEqual(report.pageErrors, []); assert.deepEqual(report.consoleErrors, []); assert.equal(report.serverError, undefined);
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed'; report.error = String(error.stack); throw error;
  } finally {
    release?.();
    const cleanup = await Promise.allSettled([
      (async () => { if (server?.listening) { server.closeAllConnections(); await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve())); } })(),
      (async () => database?.close())(), (async () => browser?.close())(),
    ]);
    const errors = cleanup.filter(result => result.status === 'rejected').map(result => result.reason);
    if (errors.length) { report.status = 'failed'; report.cleanupErrors = errors.map(String); }
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    if (errors.length) throw new AggregateError(errors, 'acceptance cleanup failed');
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
