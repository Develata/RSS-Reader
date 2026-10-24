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
  await page.waitForFunction(() => document.querySelector('#source-filter-all').indeterminate);
  assert.equal(await chooser.locator('#source-filter-all').getAttribute('aria-checked'), 'mixed');
  await chooser.locator('#source-filter-all').press('Space');
  assert.equal(await chooser.locator('#source-filter-all').isChecked(), true, 'mixed state selects all with the keyboard');
  await page.waitForFunction(() => !document.querySelector('#source-filter-all').indeterminate);
  await chooser.locator('#source-filter-all').uncheck();
  assert.equal(await chooser.locator('#source-filter-all').getAttribute('aria-checked'), 'false');
  await chooser.getByRole('checkbox', { name: 'Alpha', exact: true }).check();
  assert.equal(await chooser.locator(action('edit-feed')).count(), 0, 'article source popup is filtering only');
  await page.locator(action('apply-source-filter')).click();
  await page.waitForFunction(() => document.querySelector('.source-filter-trigger span').textContent === '…');
  assert.equal(await page.locator(selected).count(), 0);
  assert.deepEqual(await page.locator(`${rows} [data-slot="entry-card-meta"]`).allTextContents().then(texts => [...new Set(texts.map(text => text.split(' · ')[0]))]), ['Alpha']);
  await page.locator(action('open-source-filter')).click();
  await page.waitForFunction(() => document.querySelector('#source-filter-all').indeterminate);
  await page.keyboard.press('Escape');
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

async function editEdges(page) {
  await page.locator('[data-nav="feeds"]').first().click();
  const feed = page.locator('[data-feed-id="1"]');
  await feed.locator(action('remove-feed')).click();
  assert.equal(await feed.getAttribute('data-state'), 'confirm');
  await feed.locator(action('edit-feed')).click();
  await page.locator('#feed-edit-name').waitFor();
  await page.keyboard.press('Escape');
  assert.equal(await feed.getAttribute('data-state'), 'idle', 'editing cancels the previous delete confirmation');
  await feed.locator(action('remove-feed')).click();
  assert.equal(await feed.getAttribute('data-state'), 'confirm', 'the next delete click only asks for confirmation');
  await feed.locator(action('edit-feed')).click();
  await page.locator('#feed-edit-name').waitFor();
  await page.keyboard.press('Escape');
  await page.evaluate(async () => {
    await window.__rssrTestMutateSlice('rssr-web-state-v1', state => {
      state.feeds.find(feed => feed.id === 1).title = 'LongSourceName'.repeat(35);
    });
  });
  await page.reload();
  await feed.locator(action('edit-feed')).click();
  await page.locator('#feed-edit-name').waitFor();
  for (const width of [1536, 360]) {
    await page.setViewportSize({ width, height: 800 });
    const metrics = await page.locator('#feed-edit-dialog').evaluate(dialog => ({
      width: dialog.clientWidth, scrollWidth: dialog.scrollWidth,
      right: dialog.getBoundingClientRect().right,
      resetRight: dialog.querySelector('.text-button').getBoundingClientRect().right,
    }));
    assert.ok(metrics.scrollWidth <= metrics.width + 1, `long original name fits ${width}px dialog`);
    assert.ok(metrics.resetRight <= metrics.right, 'reset button remains within the dialog');
  }
  await page.getByRole('button', { name: '使用原名', exact: true }).click();
  assert.equal(await page.locator('#feed-edit-name').inputValue(), '');
  await page.keyboard.press('Escape');
}

async function queryStability(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  await seed(page, new URL(page.url()).origin);
  const first = page.locator(`${rows}[data-position-entry="72"]`);
  const original = await first.innerText();
  const marker = 'A fresh query loaded this title';
  await page.evaluate(async marker => {
    await window.__rssrTestMutateSlice('rssr-web-state-v1', state => {
      state.entries.find(entry => entry.id === 72).title = marker;
    });
  }, marker);
  // Intentionally cross the archive cutoff clock's second boundary. This
  // mutation remains invisible until an operation actually reloads the query.
  await page.waitForTimeout(1100);
  await first.locator(action('select-entry')).check();
  await page.waitForTimeout(250);
  assert.equal(await first.innerText(), original, 'selection must not reload the list');
  await page.waitForTimeout(1100);
  await page.locator(action('close-directory')).click();
  await page.locator(action('open-directory')).click();
  await page.waitForTimeout(250);
  assert.equal(await first.innerText(), original, 'directory visibility must not reload the list');
  assert.equal(await page.locator(selected).count(), 1, 'directory visibility preserves selection');
  await page.waitForTimeout(1100);
  await page.locator(action('entry-page-next')).click();
  await page.waitForFunction(() => document.querySelector('[data-page="entries"]').dataset.positionPage === '2');
  await page.locator(action('entry-page-previous')).click();
  await first.waitFor();
  await page.waitForTimeout(250);
  assert.equal(await first.innerText(), original, 'local pagination must not reload the list');
  assert.equal(await page.locator(selected).count(), 0, 'paging still clears selection');
  await first.locator(action('select-entry')).check();
  await page.getByRole('button', { name: '仅未读', exact: true }).click();
  await first.getByText(marker, { exact: true }).waitFor();
  assert.equal(await page.locator(selected).count(), 0, 'a filter change still queries and clears selection');
}

async function importNames(page) {
  for (const format of ['config', 'opml']) {
    await seed(page, new URL(page.url()).origin);
    await page.locator('[data-nav="feeds"]').first().click();
    const feed = page.locator('[data-feed-id="1"]');
    await feed.locator(action('edit-feed')).click();
    await page.locator('#feed-edit-name').fill('Imported alias');
    await page.locator(action('save-feed-edit')).click();
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
    await page.locator(action(`export-${format}`)).click();
    await page.waitForFunction(format => document.querySelector(`#${format}-text`).value.includes('Imported alias'), format);
    await page.locator(action(`import-${format}`)).click();
    if (format === 'config') await page.locator(action('import-config')).click();
    await page.getByRole('status').filter({ hasText: format === 'config' ? '配置包已导入' : 'OPML 已导入' }).waitFor();
    assert.equal(await feed.getAttribute('data-feed-title'), 'Imported alias');
    await feed.locator(action('edit-feed')).click();
    await page.locator('#feed-edit-name').waitFor();
    assert.equal(await page.locator('#feed-edit-name').inputValue(), 'Imported alias');
    assert.equal(await page.locator('.feed-edit-original small').innerText(), '原始名称：Alpha', `${format} preserves the remote title`);
    await page.getByRole('button', { name: '使用原名', exact: true }).click();
    await page.locator(action('save-feed-edit')).click();
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
    await page.waitForFunction(() => document.querySelector('[data-feed-id="1"]').dataset.feedTitle === 'Alpha');
  }
}

async function sourceIdentity(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  await seed(page, new URL(page.url()).origin);
  await page.locator('[data-nav="feeds"]').first().click();
  const secondURL = await page.locator('[data-feed-id="2"]').getAttribute('data-feed-url');
  for (const id of [1, 2]) {
    await page.locator(`[data-feed-id="${id}"] ${action('edit-feed')}`).click();
    await page.locator('#feed-edit-name').fill('Same name');
    await page.locator(action('save-feed-edit')).click();
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
  }
  await page.locator('[data-action="activate-home"]').first().click(); await ready(page);
  await page.locator(action('group-by-source')).click();
  const groups = page.locator(`${rail} [data-directory-kind="group"]`);
  await groups.filter({ hasText: 'Same name' }).first().waitFor();
  assert.equal(await groups.count(), 6, 'renaming sources must not merge their directory groups');
  assert.equal(await groups.filter({ hasText: 'Same name' }).count(), 2);
  const anchors = await groups.evaluateAll(items => items.map(item => item.dataset.directoryAnchor));
  assert.equal(new Set(anchors).size, 6, 'source anchors are unique even with identical names');
  await page.locator(action('select-current-page')).click();
  assert.equal(await page.locator(selected).count(), 8, 'source grouping still selects exactly this page');
  await page.locator(action('open-source-filter')).click();
  const chooser = page.locator('#source-filter-dialog');
  await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
  await chooser.getByRole('searchbox', { name: '搜索来源' }).fill(secondURL);
  await chooser.getByRole('checkbox', { name: 'Same name', exact: true }).check();
  await page.locator(action('apply-source-filter')).click();
  await page.waitForFunction(() => document.querySelector('.entries-count').textContent === '12 篇');
  assert.deepEqual(await page.locator(rows).evaluateAll(items => items.map(item => Number(item.dataset.positionEntry))), [71, 65, 59, 53, 47, 41, 35, 29], 'the second source remains independently selectable');
}

async function holdWrites(page, interaction) {
  await page.evaluate(() => {
    window.__workspaceWriteHeld = false;
    void navigator.locks.request('rssr-browser-state-v1', async () => {
      window.__workspaceWriteHeld = true;
      await new Promise(resolve => { window.__workspaceReleaseWrite = resolve; });
    });
  });
  await page.waitForFunction(() => window.__workspaceWriteHeld);
  try { await interaction(); }
  finally { await page.evaluate(() => window.__workspaceReleaseWrite()); }
}

async function pendingWrites(page) {
  await seed(page, new URL(page.url()).origin);
  const first = page.locator(`${rows}[data-position-entry="72"]`);
  await first.locator(action('select-entry')).check();
  await holdWrites(page, async () => {
    await first.locator(action('mark-read')).click();
    await page.waitForFunction(async () => (await navigator.locks.query()).pending.length > 0);
    assert.equal(await first.locator(action('mark-read')).isDisabled(), true, 'the pending row rejects repeated writes');
    assert.equal(await first.locator(action('toggle-starred')).isDisabled(), true);
    assert.equal(await page.locator(`${rows}[data-position-entry="71"] ${action('mark-read')}`).isDisabled(), false, 'other rows remain usable');
    assert.equal(await page.locator(action('mark-selected-read')).isDisabled(), true, 'batch waits for outstanding row writes');
    assert.equal(await page.locator(action('archive-selected')).isDisabled(), true);
  });
  await page.waitForFunction(() => document.querySelector('[data-position-entry="72"]').dataset.read === 'true' && !document.querySelector('[data-action="mark-selected-read"]').disabled);
  await first.locator(action('mark-read')).click();
  await page.waitForFunction(() => document.querySelector('[data-position-entry="72"]').dataset.read === 'false' && !document.querySelector('[data-position-entry="72"] [data-action="mark-read"]').disabled);
  await page.locator(`${rows}[data-position-entry="71"] ${action('select-entry')}`).check();
  await holdWrites(page, async () => {
    await page.locator(action('archive-selected')).click();
    await page.waitForFunction(async () => (await navigator.locks.query()).pending.length > 0);
    assert.equal(await page.locator(`${rows} [data-layout="entry-card-actions"] button`).evaluateAll(buttons => buttons.every(button => button.disabled)), true, 'batch writes block overlapping per-row actions');
  });
  await page.waitForFunction(() => !document.querySelector('[data-position-entry="72"]') && !document.querySelector('[data-position-entry="71"]'));
  await seed(page, new URL(page.url()).origin);
  await page.evaluate(() => {
    window.__workspaceOriginalSetItem = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key, value) {
      if (key.startsWith('rssr-web-entry-flags-v1')) throw new DOMException('review write failure', 'QuotaExceededError');
      return window.__workspaceOriginalSetItem.call(this, key, value);
    };
  });
  try {
    await first.locator(action('mark-read')).click();
    await page.locator('[data-layout="status-banner"][data-state="error"]').waitFor({ state: 'attached' });
    assert.equal(await first.locator(action('mark-read')).isDisabled(), false, 'a failed write unlocks retry');
    assert.equal(await first.getAttribute('data-read'), 'false', 'a failed write preserves flags');
  } finally {
    await page.evaluate(() => { Storage.prototype.setItem = window.__workspaceOriginalSetItem; });
  }
  await first.locator(action('mark-read')).click();
  await page.waitForFunction(() => document.querySelector('[data-position-entry="72"]').dataset.read === 'true');
}

module.exports = { seed, selection, sources, editing, responsive, editEdges, queryStability, importNames, sourceIdentity, pendingWrites };
if (require.main === module) (async () => {
  const { chromium } = require('playwright');
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
  try {
    const page = await browser.newPage({ viewport: { width: 1536, height: 864 } });
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    page.setDefaultTimeout(15000);
    await seed(page, process.env.STATIC_BASE || 'http://127.0.0.1:8097');
    for (const [name, run] of Object.entries({ selection, sources, editing, responsive, editEdges, queryStability, importNames, sourceIdentity, pendingWrites })) {
      await run(page); console.log(`PASS ${name}`);
    }
    assert.deepEqual(errors, []);
    console.log('PASS no page errors');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
