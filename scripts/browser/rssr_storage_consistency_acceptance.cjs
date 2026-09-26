const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const { installStorageHelpers } = require('./storage_helpers.cjs');
const base = process.env.STATIC_BASE || 'http://127.0.0.1:8099';
const artifacts = process.env.ARTIFACT_DIR || 'target/p2-storage/browser';

(async () => {
  await fs.mkdir(artifacts, { recursive: true });
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
  try {
    for (const width of [360, 1280]) {
      const context = await browser.newContext({ viewport: { width, height: 800 } });
      await installStorageHelpers(context);
      const errors = [];
      context.on('page', page => {
        page.setDefaultTimeout(15000);
        page.on('pageerror', error => errors.push(error.message));
      });
      const a = await context.newPage();
      const ready = (page, id) => page.locator(`[data-page="reader"][data-position-key="reader:${id}"][data-position-ready="true"]`).waitFor();
      await a.goto(`${base}/__codex/setup-local-auth?seed=reader-demo&next=/entries/1`);
      await ready(a, 1);
      // Prepare via actual commands. Editing an active slice behind BrowserStore's revision
      // cache can race background work and is not a valid participating writer.
      await a.locator('[data-action="mark-read"][data-state="read"]').click();
      await a.locator('[data-action="mark-read"][data-state="unread"]').waitFor();
      await a.locator('[data-action="toggle-starred"][data-state="unstarred"]').waitFor();
      const b = await context.newPage();
      // Session auth is tab-local. Do not seed again: both tabs must share the same library.
      await b.goto(`${base}/__codex/setup-local-auth?next=/entries/2`);
      await ready(b, 2);
      await b.locator('[data-action="toggle-starred"][data-state="starred"]').click();
      await b.locator('[data-action="toggle-starred"][data-state="unstarred"]').waitFor();
      const star = page => page.locator('[data-action="toggle-starred"]');
      const starred = page => page.locator('[data-action="toggle-starred"][data-state="starred"]').waitFor();
      const flags = () => a.evaluate(() => navigator.locks.request('rssr-browser-state-v1', () =>
        JSON.parse(localStorage.getItem(__rssrTestStateKey('rssr-web-entry-flags-v1'))).entries));
      await star(a).click(); await starred(a);
      await star(b).click(); await starred(b);
      let saved = await flags();
      assert(saved.find(f => f.id === 1).is_starred, `stale tab B preserves tab A star: ${JSON.stringify(saved)}`);
      assert(saved.find(f => f.id === 2).is_starred, 'tab B star persisted');
      // Both tabs now read the same entry; simultaneous changes must preserve separate bits.
      await b.goto(`${base}/entries/1`); await ready(b, 1);
      await Promise.all([a.locator('[data-action="mark-read"]').click(), star(b).click()]);
      await a.locator('[data-action="mark-read"][data-state="read"]').waitFor();
      await b.locator('[data-action="toggle-starred"][data-state="unstarred"]').waitFor();
      saved = await flags();
      assert(saved.find(f => f.id === 1).is_read);
      assert(!saved.find(f => f.id === 1).is_starred);
      assert(saved.find(f => f.id === 2).is_starred);
      // Reject only the publication point, after the new flags have been staged.
      const before = saved;
      await b.evaluate(() => {
        window.__set = Storage.prototype.setItem;
        Storage.prototype.setItem = function(k, v) {
          if (k === 'rssr-web-commit-v1') throw new DOMException('test quota', 'QuotaExceededError');
          return __set.call(this, k, v);
        };
      });
      await star(b).click();
      await b.locator('[data-state="error"]').first().waitFor();
      await b.evaluate(() => { Storage.prototype.setItem = __set; delete window.__set; });
      assert.deepEqual(await flags(), before, 'failed publication leaves committed flags unchanged');
      await b.reload(); await ready(b, 1);
      await b.locator('[data-action="toggle-starred"][data-state="unstarred"]').waitFor();
      await b.locator('[data-action="mark-read"][data-state="read"]').waitFor();
      await star(b).click(); await starred(b);
      // Navigating back to the list rereads authoritative counts after the other tab wrote.
      await a.locator('[data-nav="feeds"]').click();
      await a.getByText('本地文章 2 · 未读 1', { exact: true }).waitFor();
      assert.deepEqual(errors, []);
      assert(await a.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      await a.screenshot({ path: `${artifacts}/counts-${width}.png`, fullPage: true });
      await b.screenshot({ path: `${artifacts}/reader-${width}.png`, fullPage: true });
      console.log(JSON.stringify({ width, staleTabs: 'pass', concurrentBits: 'pass', failedPublication: 'pass', retryAndCounts: 'pass' }));
      await context.close();
    }
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
