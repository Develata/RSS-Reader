// bun scripts/browser/rssr_reading_position_acceptance.cjs
// Reuses the SPA regression server and installed Playwright (NODE_PATH / CHROME_BIN).
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const base = process.env.STATIC_BASE || 'http://127.0.0.1:8099';

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
  try {
    for (const width of [360, 1280]) {
      const page = await browser.newPage({ viewport: { width, height: 800 } });
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      page.setDefaultTimeout(15000);
      const ready = async () => {
        await page.locator('[data-page="reader"][data-position-ready="true"]').waitFor();
        await page.waitForTimeout(2300);
      };
      const y = () => page.evaluate(() => scrollY);
      await page.goto(`${base}/__codex/setup-local-auth?seed=reader-demo&next=/entries/1`);
      await ready();
      await page.evaluate(() => {
        const key = 'rssr-web-state-v1';
        const state = JSON.parse(localStorage.getItem(key));
        state.settings.archive_after_months = 0;
        localStorage.setItem(key, JSON.stringify(state));
        const contentKey = 'rssr-web-entry-content-v1';
        const content = JSON.parse(localStorage.getItem(contentKey));
        for (const entry of content.entries) {
          entry.content_html = Array.from({ length: 2000 }, (_, i) =>
            `<p>Block ${i}: ${'Reading position performance. '.repeat(12)}</p>`).join('');
        }
        localStorage.setItem(contentKey, JSON.stringify(content));
      });
      await page.goto(`${base}/entries`);
      await page.locator('a[href="/entries/1"]').first().click();
      await ready();
      await page.evaluate(() => {
        window.__positionReads = 0;
        const original = Element.prototype.getBoundingClientRect;
        Element.prototype.getBoundingClientRect = function () {
          if (this.closest('[data-layout="reader-body"]')) window.__positionReads++;
          return original.call(this);
        };
        scrollTo(0, document.documentElement.scrollHeight * 0.8);
      });
      for (let i = 0; i < 12; i++) {
        await page.mouse.wheel(0, i % 2 ? -180 : 240);
        await page.waitForTimeout(30);
      }
      await page.keyboard.press('PageDown');
      await page.waitForTimeout(300);
      assert.equal(await page.evaluate(() => window.__positionReads), 0,
        'continuous scrolling must not scan reader blocks');
      // Editing the shell search is outside the reader shortcut scope. Caret movement
      // must not scan a long article, even though an article remains open underneath.
      await page.locator('[data-action="toggle-search"]').click();
      await page.locator('[data-field="entry-search"]').fill('caret movement');
      await page.evaluate(() => { window.__positionReads = 0; });
      await page.keyboard.press('ArrowLeft');
      await page.keyboard.press('ArrowRight');
      await page.waitForTimeout(100);
      assert.equal(await page.evaluate(() => window.__positionReads), 0,
        'search caret movement must not scan reader blocks');
      assert.equal(new URL(page.url()).pathname, '/entries/1');
      await page.locator('[data-field="entry-search"]').fill('');
      await page.keyboard.press('Escape');
      await page.evaluate(() => document.querySelector('[data-layout="reader-shortcut-scope"]').focus({ preventScroll: true }));
      const forward = await page.locator('[data-nav="next-unread-entry"]').isEnabled();
      const arrow = forward ? 'ArrowRight' : 'ArrowLeft';
      await page.keyboard.press(`Control+${arrow}`);
      await page.evaluate(key => document.activeElement.dispatchEvent(
        new KeyboardEvent('keydown', { key, bubbles: true, isComposing: true })), arrow);
      await page.waitForTimeout(200);
      assert.equal(new URL(page.url()).pathname, '/entries/1', 'modified/composing keys must not navigate');
      assert.equal(await page.evaluate(() => window.__positionReads), 0,
        'modified/composing keys must not scan reader blocks');
      const saved = await y();
      assert(saved > 1000);
      // Browser history has no preceding DOM click: it must capture the departing article.
      await page.goBack();
      await page.locator('a[href="/entries/1"]').first().click();
      await ready();
      assert(Math.abs(await y() - saved) < 4, `browser-back capture: ${saved} -> ${await y()}`);
      const adjacent = forward ? 'next-unread-entry' : 'previous-unread-entry';
      await page.evaluate(() => document.querySelector('[data-layout="reader-shortcut-scope"]').focus({ preventScroll: true }));
      await page.keyboard.press(forward ? 'ArrowRight' : 'ArrowLeft');
      await page.waitForURL(url => /\/entries\/\d+$/.test(url.pathname) && url.pathname !== '/entries/1');
      await ready();
      await page.goBack();
      await ready();
      assert(Math.abs(await y() - saved) < 4, 'keyboard navigation capture');
      // Native host capture returns a fact synchronously, without changing history.
      const nativeFact = await page.evaluate(() => {
        const event = new CustomEvent('rssr-capture-position', { detail: { fact: null } });
        document.dispatchEvent(event);
        return event.detail.fact;
      });
      assert.equal(nativeFact.kind, 'capture');
      assert.equal(nativeFact.key, 'reader:1');
      assert(Math.abs(nativeFact.point.y - saved) < 4);
      for (const edge of ['top', 'bottom']) {
        await page.evaluate(edge => scrollTo(0, edge === 'top' ? 0 : document.documentElement.scrollHeight), edge);
        const expected = await y();
        await page.locator(`[data-nav="${adjacent}"]`).click();
        await ready();
        await page.goBack();
        await ready();
        assert(Math.abs(await y() - expected) < 4, `${edge} capture`);
      }
      console.log(JSON.stringify({ width, scrollLayoutReads: 0, editingLayoutReads: 0, composingKeys: 'pass', browserBack: 'pass',
        keyboard: 'pass', nativeCaptureBridge: 'pass', topAndBottom: 'pass' }));
      assert.deepEqual(errors, [], 'no uncaught browser errors');
      await page.close();
    }
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
