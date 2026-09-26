// Uses the existing SPA regression server; supply NODE_PATH and CHROME_BIN as needed.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const base = process.env.STATIC_BASE || 'http://127.0.0.1:8099';
const artifacts = process.env.ARTIFACT_DIR || 'target/global-review/browser';

(async () => {
  await fs.mkdir(artifacts, { recursive: true });
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
  try {
    for (const width of [360, 1280]) {
      for (const theme of ['none', 'atlas-sidebar', 'newsprint', 'amethyst-glass', 'midnight-ledger']) {
        const page = await browser.newPage({ viewport: { width, height: 800 } });
        const errors = [];
        page.on('pageerror', error => errors.push(error.message));
        page.setDefaultTimeout(15000);
        await page.goto(`${base}/__codex/setup-local-auth?seed=reader-demo&next=/entries/1`);
        await page.locator('[data-page="reader"][data-position-ready="true"]').waitFor();
        const css = theme === 'none' ? '' : await fs.readFile(`assets/themes/${theme}.css`, 'utf8');
        await page.evaluate(css => {
          const key = 'rssr-web-state-v1', state = JSON.parse(localStorage.getItem(key));
          state.settings.custom_css = css;
          state.settings.archive_after_months = 0;
          localStorage.setItem(key, JSON.stringify(state));
        }, css);
        await page.reload();
        const nav = page.locator('[data-layout="app-nav-shell"]');
        const toggle = page.locator('[data-action="toggle-nav"]');
        await toggle.waitFor();
        const expanded = await nav.boundingBox();
        const target = await toggle.boundingBox();
        assert(target.width >= 44 && target.height >= 44, 'touch target');
        assert(target.x >= expanded.x + expanded.width - target.width - 20, 'right edge toggle');
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'expanded overflow');
        if (['none', 'atlas-sidebar'].includes(theme)) await page.screenshot({ path: `${artifacts}/${theme}-${width}-expanded.png` });
        await page.locator('[data-action="toggle-search"]').click();
        await page.locator('[data-field="entry-search"]').fill('preserved query');
        await toggle.click();
        assert.equal(await nav.getAttribute('data-state'), 'collapsed');
        assert.equal(await toggle.getAttribute('aria-expanded'), 'false');
        assert.equal(await toggle.getAttribute('aria-label'), '向右展开导航');
        assert.equal(await nav.locator('button, a, input').count(), 1, 'only expand control remains');
        assert(await toggle.evaluate(e => e === document.activeElement), 'focus stays on toggle');
        const collapsed = await nav.boundingBox();
        assert(collapsed.width <= 45 && collapsed.height <= 45, 'collapsed header footprint');
        assert(collapsed.x <= expanded.x + 1, 'collapse toward left');
        assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'collapsed overflow');
        if (['none', 'atlas-sidebar'].includes(theme)) await page.screenshot({ path: `${artifacts}/${theme}-${width}-collapsed.png` });
        await page.keyboard.press('Enter');
        assert.equal(await nav.getAttribute('data-state'), 'normal');
        await page.locator('[data-action="toggle-search"]').click();
        assert.equal(await page.locator('[data-field="entry-search"]').inputValue(), 'preserved query');
        await page.keyboard.press('Escape');
        // A rendered article DOM node survives shell changes and flag updates.
        assert(await page.evaluate(() => {
          window.__bodyNode = document.querySelector('[data-slot="reader-body-html"]')?.firstChild;
          return !!window.__bodyNode;
        }), 'fixture has rendered HTML');
        await page.locator('[data-action="toggle-starred"]').click();
        await page.waitForTimeout(200);
        assert(await page.evaluate(() => window.__bodyNode === document.querySelector('[data-slot="reader-body-html"]')?.firstChild), 'body not replaced by flag change');
        await page.locator('[data-nav="settings"]').click();
        await page.locator('[data-page="settings"]').waitFor();
        await toggle.click();
        await page.goBack();
        await page.locator('[data-page="reader"]').waitFor();
        assert.equal(await nav.getAttribute('data-state'), 'collapsed', 'shell state survives routes');
        await page.reload();
        await toggle.waitFor();
        assert.equal(await nav.getAttribute('data-state'), 'normal', 'new session starts expanded');
        assert.deepEqual(errors, [], 'no uncaught browser errors');
        console.log(JSON.stringify({ width, theme, expanded, collapsed, status: 'pass' }));
        await page.close();
      }
    }
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
