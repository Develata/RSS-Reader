// Run against the SPA regression server with an isolated browser profile.
// NODE_PATH resolves the locally installed Playwright; CHROME_BIN is optional.
const assert = require('node:assert/strict');
const { installStorageHelpers } = require('./storage_helpers.cjs');
const rows = '[data-layout="entry-card"]';
const action = name => `[data-action="${name}"]`;
const selected = `${rows}[data-selected="true"]`;
const rail = '[data-layout="entry-directory-rail"]';

async function ready(page) {
  await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
  await page.locator(`${rows} ${action('select-entry')}`).first().waitFor();
}

async function seed(page, base) {
  await installStorageHelpers(page.context());
  await page.goto(`${base}/__codex/setup-local-auth?seed=directory-contract&next=/entries`);
  await ready(page);
  await page.evaluate(async () => {
    await window.__rssrTestMutateSlice('rssr-web-state-v1', state => {
      state.settings.entries_page_size = 8;
      state.settings.archive_after_months = 120;
    });
    await window.__rssrTestMutateSlice('rssr-web-app-state-v2', state => {
      state.entries_workspace.show_archived = false;
    });
  });
  await page.reload();
  await ready(page);
}

async function selection(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  assert.equal(await page.locator(rows).count(), 8);
  assert.equal(await page.locator(action('mark-selected-read')).isDisabled(), true);
  assert.equal(await page.locator(action('archive-selected')).isDisabled(), true);
  await page.locator(`${rows}[data-position-entry="72"] ${action('select-entry')}`).check();
  await page.locator(`${rows}[data-position-entry="71"] ${action('select-entry')}`).check();
  assert.equal(await page.locator(selected).count(), 2);
  assert.equal(await page.locator(action('select-current-page')).getAttribute('aria-checked'), 'mixed');
  const august = page.locator(`${rail} [data-directory-kind="group"]`).nth(1);
  await august.click();
  await page.waitForFunction(() => document.querySelector('[data-directory-consumer="rail"]').dataset.directoryMode === 'manual');
  assert.equal(await page.locator(selected).count(), 2, 'directory expansion preserves selection');
  await page.locator(`${rail} [data-directory-kind="item"]`).nth(1).click();
  assert.equal(await page.locator(selected).count(), 2, 'same-page jump preserves selection');
  await page.locator(action('mark-selected-read')).click();
  await page.waitForFunction(() => document.querySelector('[data-position-entry="72"]').dataset.read === 'true' && document.querySelector('[data-position-entry="71"]').dataset.read === 'true');
  assert.equal(await page.locator(`${rows}[data-read="true"]`).count(), 2, 'batch read does not expand to all matches');
  assert.equal(await page.locator(selected).count(), 0);
  assert.equal(await page.locator(rail).getAttribute('data-directory-mode'), 'manual', 'flag update does not reset directory browsing');
  await page.locator(action('select-current-page')).click();
  assert.equal(await page.locator(selected).count(), 8);
  await page.locator(action('entry-page-next')).click();
  await page.waitForFunction(() => document.querySelector('[data-page="entries"]').dataset.positionPage === '2');
  assert.equal(await page.locator(selected).count(), 0, 'paging clears selection');
  await page.locator(action('entry-page-previous')).click();
  await page.waitForFunction(() => document.querySelector('[data-page="entries"]').dataset.positionPage === '1');
  for (const id of [70, 69]) await page.locator(`${rows}[data-position-entry="${id}"] ${action('select-entry')}`).check();
  await page.locator(action('archive-selected')).click();
  await page.waitForFunction(() => !document.querySelector('[data-position-entry="70"]') && !document.querySelector('[data-position-entry="69"]'));
  assert.equal(await page.locator(rows).count(), 8, 'page refills after archive');
  await page.reload(); await ready(page);
  assert.equal(await page.locator('[data-position-entry="70"]').count(), 0, 'archive persists across reload');
}

async function sources(page) {
  await page.locator(action('select-entry')).first().check();
  await page.locator(action('open-source-filter')).click();
  const chooser = page.locator('#source-filter-dialog');
  await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
  assert.equal(await page.locator(action('apply-source-filter')).isDisabled(), true, 'empty source selection cannot apply');
  await chooser.getByRole('button', { name: '取消', exact: true }).click();
  assert.equal(await page.locator(selected).count(), 1);
  assert.equal(await page.evaluate(() => document.activeElement.id), 'source-filter-trigger', 'cancel restores focus');
  await page.locator(action('open-source-filter')).click();
  await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
  await chooser.getByRole('checkbox', { name: 'Alpha', exact: true }).check();
  assert.equal(await chooser.locator(action('edit-feed')).count(), 0, 'article source popup is filtering only');
  await page.locator(action('apply-source-filter')).click();
  await page.waitForFunction(() => document.querySelector('.source-filter-trigger span').textContent === '…');
  assert.equal(await page.locator(selected).count(), 0);
  assert.deepEqual(await page.locator(`${rows} [data-slot="entry-card-meta"]`).allTextContents().then(texts => [...new Set(texts.map(text => text.split(' · ')[0]))]), ['Alpha']);
}

async function editing(page) {
  await page.locator('[data-nav="feeds"]').first().click();
  await page.locator('[data-feed-id="1"]').waitFor();
  assert.equal((await page.locator('[data-stat="unread"] [data-slot="stat-card-value"]').innerText()).trim(), '70');
  assert.equal((await page.locator('[data-stat="archived"] [data-slot="stat-card-value"]').innerText()).trim(), '2');
  const oldURL = await page.locator('[data-feed-id="1"]').getAttribute('data-feed-url');
  const occupiedURL = await page.locator('[data-feed-id="2"]').getAttribute('data-feed-url');
  await page.locator('[data-feed-id="1"] [data-action="edit-feed"]').click();
  await page.locator('#feed-edit-name').fill('少数派 · 精选');
  await page.locator('#feed-edit-url').fill(occupiedURL);
  await page.locator(action('save-feed-edit')).click();
  await page.locator('#feed-edit-dialog [role="alert"]').waitFor();
  assert.equal(await page.locator('#feed-edit-name').inputValue(), '少数派 · 精选', 'failed save preserves draft');
  assert.equal(await page.locator('[data-feed-id="1"]').getAttribute('data-feed-title'), 'Alpha');
  await page.locator('#feed-edit-url').fill(`${oldURL}&edited=1`);
  await page.locator(action('save-feed-edit')).click();
  await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
  await page.waitForFunction(() => document.querySelector('[data-feed-id="1"]').dataset.feedTitle === '少数派 · 精选');
  await page.locator('[data-feed-id="1"] [data-action="edit-feed"]').click();
  assert.equal(await page.locator('#feed-edit-name').inputValue(), '少数派 · 精选');
  assert.equal(await page.locator('#feed-edit-url').inputValue(), `${oldURL}&edited=1`);
  await page.getByRole('button', { name: '使用原名', exact: true }).click();
  assert.equal(await page.locator('#feed-edit-name').inputValue(), '');
  await page.locator('#feed-edit-name').fill('少数派 · 精选');
  await page.keyboard.press('Escape');
  await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
  await page.locator('[data-action="activate-home"]').first().click(); await ready(page);
  assert.equal(await page.locator('.source-filter-trigger span').innerText(), '…');
  assert.equal(await page.locator('[data-position-entry="72"]').getAttribute('data-read'), 'true');
  assert.equal(await page.locator('.entries-count').innerText(), '12 篇', 'URL edit keeps selected source scope');
  assert.equal(await page.locator(`${rows} [data-slot="entry-card-meta"]`).first().innerText(), '少数派 · 精选 · 2026-09-02');
}

async function responsive(page) {
  for (const [width, height] of [[1920, 1080], [1536, 864], [760, 1000], [390, 844], [360, 800]]) {
    await page.setViewportSize({ width, height });
    await page.locator(action('select-current-page')).scrollIntoViewIfNeeded();
    const metrics = await page.evaluate(() => {
      const r = s => document.querySelector(s).getBoundingClientRect().toJSON();
      return { width: innerWidth, scrollWidth: document.documentElement.scrollWidth,
        box: r('.entry-select-box[type="checkbox"]'), hit: r('.entry-select-hit'), pager: r('[data-layout="entry-pagination"]'),
        previous: r('[data-action="entry-page-previous"]'), next: r('[data-action="entry-page-next"]'),
        railVisible: !!document.querySelector('[data-layout="entry-directory-rail"]').getClientRects().length };
    });
    assert.ok(metrics.scrollWidth <= width + 1, `${width}px has no horizontal overflow`);
    assert.equal(metrics.box.width, 16); assert.equal(metrics.hit.width, 44);
    assert.ok(metrics.pager.bottom <= height - 10 && metrics.pager.bottom >= height - 20);
    assert.ok(metrics.next.x - metrics.previous.x < 140, 'paging buttons stay close');
    assert.equal(metrics.railVisible, width > 1000);
    if (width <= 1000) {
      await page.locator(action('open-directory')).click();
      assert.equal(await page.locator(rail).isVisible(), true);
      await page.locator(action('close-directory')).click();
      assert.equal(await page.locator(rail).isVisible(), false);
    }
  }
  await page.locator(action('select-entry')).first().focus();
  await page.keyboard.press('Space');
  assert.equal(await page.locator(selected).count(), 1, 'keyboard can select an article');
  await page.keyboard.press('Space');
  assert.equal(await page.locator(selected).count(), 0);
  await page.locator(action('open-source-filter')).click();
  assert.equal(await page.locator('#source-filter-dialog').isVisible(), true);
  await page.setViewportSize({ width: 1536, height: 864 });
  await page.setViewportSize({ width: 360, height: 800 });
  const box = await page.locator('#source-filter-dialog').boundingBox();
  assert.ok(box.x >= 0 && box.x + box.width <= 361 && box.y >= 0 && box.y + box.height <= 801);
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('#source-filter-dialog').count(), 0);
}

module.exports = { seed, selection, sources, editing, responsive };
if (require.main === module) (async () => {
  const { chromium } = require('playwright');
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
  try {
    const page = await browser.newPage({ viewport: { width: 1536, height: 864 } });
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    page.setDefaultTimeout(15000);
    await seed(page, process.env.STATIC_BASE || 'http://127.0.0.1:8097');
    for (const [name, run] of Object.entries({ selection, sources, editing, responsive })) {
      await run(page); console.log(`PASS ${name}`);
    }
    assert.deepEqual(errors, []);
    console.log('PASS no page errors');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
