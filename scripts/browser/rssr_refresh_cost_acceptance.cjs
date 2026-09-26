const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const { installStorageHelpers } = require('./storage_helpers.cjs');
const base = process.env.STATIC_BASE || 'http://127.0.0.1:8099';
const artifacts = process.env.ARTIFACT_DIR || 'target/refresh-cost/browser';

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
      let mode = 'original';
      await context.route('**/feed-proxy?**', route => {
        const body = mode === 'oversized' ? 'x'.repeat(8 * 1024 * 1024 + 1)
          : `<rss version="2.0"><channel><title>Refresh fixture</title><description>Feed</description><item><guid>refresh-cost-entry</guid><title>Refresh cost article</title><description>Body ${mode}</description></item></channel></rss>`;
        return route.fulfill({ status: 200, contentType: 'application/rss+xml; charset=utf-8', body });
      });
      const page = await context.newPage();
      await page.goto(`${base}/__codex/setup-local-auth?seed=reader-demo&next=/feeds`);
      const card = page.locator('[data-layout="feed-card"]').first();
      await card.waitFor();
      const read = () => page.evaluate(() => navigator.locks.request('rssr-browser-state-v1', () => ({
        head: JSON.parse(localStorage.getItem('rssr-web-commit-v1')),
        core: JSON.parse(localStorage.getItem(__rssrTestStateKey('rssr-web-state-v1'))),
        content: JSON.parse(localStorage.getItem(__rssrTestStateKey('rssr-web-entry-content-v1'))),
        flags: JSON.parse(localStorage.getItem(__rssrTestStateKey('rssr-web-entry-flags-v1'))),
      })));
      const refresh = async () => {
        const before = await card.getAttribute('data-last-fetched-at');
        await card.locator('[data-action="refresh-feed"]').click();
        await page.waitForFunction(value => document.querySelector('[data-layout="feed-card"]')
          ?.getAttribute('data-last-fetched-at') !== value, before);
      };
      await refresh();
      let before = await read();
      const entry = before.core.entries.find(entry => entry.title === 'Refresh cost article');
      assert(entry, 'first refresh inserts the fixture article');
      const count = await card.getAttribute('data-entry-count');
      const unread = await card.getAttribute('data-unread-count');
      await refresh();
      let after = await read();
      assert.equal(after.head.revisions[3], before.head.revisions[3], 'same body does not publish the content slice');
      assert.deepEqual(after.content, before.content, 'body and content timestamp remain unchanged');
      assert.deepEqual(after.flags, before.flags);
      assert.equal(await card.getAttribute('data-entry-count'), count);
      assert.equal(await card.getAttribute('data-unread-count'), unread);

      const reader = await context.newPage();
      await reader.goto(`${base}/__codex/setup-local-auth?next=/entries/${entry.id}`);
      const body = reader.locator('[data-slot="reader-body-html"], [data-slot="reader-body-text"]');
      await body.filter({ hasText: 'Body original' }).waitFor();
      before = await read();
      mode = 'updated';
      await refresh();
      after = await read();
      assert.equal(after.head.revisions[3], before.head.revisions[3] + 1);
      assert(after.content.entries.find(content => content.entry_id === entry.id).content_text.includes('Body updated'));
      assert((await body.textContent()).includes('Body original'), 'refresh keeps the open reader snapshot');
      assert.equal(await card.getAttribute('data-entry-count'), count, 'changed content does not count as new');

      before = after;
      const successAt = await card.getAttribute('data-last-success-at');
      mode = 'oversized';
      await refresh();
      assert((await card.getAttribute('data-fetch-error')).includes('8 MiB'));
      after = await read();
      assert.deepEqual(after.content, before.content);
      assert.deepEqual(after.core.entries, before.core.entries);
      assert.deepEqual(after.flags, before.flags);
      assert.equal(await card.getAttribute('data-last-success-at'), successAt);
      assert.equal(await card.getAttribute('data-entry-count'), count);
      mode = 'updated';
      await refresh();
      assert.equal(await card.getAttribute('data-fetch-error'), '');
      assert.deepEqual((await read()).content, before.content, 'retry of unchanged body clears error without rewriting content');
      await reader.reload();
      await body.filter({ hasText: 'Body updated' }).waitFor();
      for (const current of [page, reader]) {
        assert(await current.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      }
      assert.deepEqual(errors, []);
      await page.screenshot({ path: `${artifacts}/refresh-${width}.png`, fullPage: true });
      await reader.screenshot({ path: `${artifacts}/reader-${width}.png`, fullPage: true });
      console.log(JSON.stringify({ width, unchanged: 'pass', changed: 'pass', oversizedAndRetry: 'pass', readerSnapshot: 'pass', consoleErrors: errors.length }));
      await context.close();
    }
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
