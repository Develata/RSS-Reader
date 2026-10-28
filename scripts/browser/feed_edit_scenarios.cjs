const assert = require('node:assert/strict');
const path = require('node:path');
const action = name => `[data-action="${name}"]`;

module.exports = async ({ page, database, native, width, output }) => {
  const dialog = page.locator('#feed-edit-dialog');
  const name = page.locator('#feed-edit-name');
  const url = page.locator('#feed-edit-url');
  const save = page.locator(action('save-feed-edit'));
  const feed = page.locator('[data-layout="feed-card"][data-feed-id="1"]');
  const readFeeds = async () => native
    ? database.prepare('SELECT id, url, title, custom_title FROM feeds WHERE is_deleted = 0 ORDER BY id').all().map(row => ({ ...row }))
    : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-state-v1'))).feeds
      .filter(feed => !feed.is_deleted).map(({ id, url, title, custom_title }) => ({ id, url, title, custom_title: custom_title ?? null })).sort((a, b) => a.id - b.id));
  const originals = await readFeeds();
  const original = originals.find(feed => feed.id === 1);
  const occupied = originals.find(feed => feed.id === 2);
  assert.ok(original && occupied);
  const open = async () => {
    await feed.locator(action('edit-feed')).click();
    await name.waitFor();
    await page.waitForFunction(() => document.querySelector('#feed-edit-dialog')?.open === true);
  };
  const close = async () => { await page.keyboard.press('Escape'); await dialog.waitFor({ state: 'detached' }); };
  const saved = async () => {
    await dialog.waitFor({ state: 'detached' });
    await page.waitForFunction(() => document.querySelector('[data-page="feeds"] > [data-layout="status-banner"]')?.textContent === '来源已更新。');
  };
  await feed.locator(action('remove-feed')).click();
  assert.equal(await feed.getAttribute('data-state'), 'confirm');
  await open();
  assert.equal(await feed.getAttribute('data-state'), 'idle', 'opening edit clears deletion confirmation');
  assert.equal(await name.inputValue(), original.custom_title || '');
  assert.equal(await url.inputValue(), original.url);
  await page.evaluate(async () => { await document.fonts.ready; await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))); });
  if (!native) await page.screenshot({ path: path.join(output, `edit-${width}.png`), animations: 'disabled', caret: 'hide' });
  const box = await dialog.boundingBox();
  assert.ok(box.x >= 0 && box.x + box.width <= width + 1 && box.y >= 0 && box.y + box.height <= 845, 'edit dialog fits viewport');
  await name.fill('取消的草稿'); await close();
  await open();
  assert.equal(await name.inputValue(), original.custom_title || '', 'cancel does not persist the draft');
  const title = `模块化验收 ${width}`;
  await name.fill(title); await url.fill(occupied.url); await save.click();
  await dialog.locator('[role="alert"]').waitFor();
  assert.equal(await name.inputValue(), title, 'failed save retains the title draft');
  assert.equal(await url.inputValue(), occupied.url, 'failed save retains the URL draft');
  assert.equal(await save.isDisabled(), false, 'failed save releases the gate');
  assert.deepEqual(await readFeeds(), originals, 'failed save does not persist changes');
  const changed = new URL(original.url); changed.searchParams.set('codex_edit', String(width));
  await url.fill(changed.href);
  if (!native) {
    await page.evaluate(() => {
      window.__feedEditHeld = false;
      void navigator.locks.request('rssr-browser-state-v1', async () => {
        window.__feedEditHeld = true;
        await new Promise(resolve => { window.__feedEditRelease = resolve; });
      });
    });
    await page.waitForFunction(() => window.__feedEditHeld);
    try {
      await save.click();
      await page.waitForFunction(() => document.querySelector('[data-action="save-feed-edit"]')?.disabled === true);
      assert.equal(await name.isDisabled(), true); assert.equal(await url.isDisabled(), true);
      assert.equal(await page.getByRole('button', { name: '关闭来源编辑', exact: true }).isDisabled(), true);
      assert.equal(await dialog.getByRole('button', { name: '取消', exact: true }).isDisabled(), true);
      // Exercise native cancel even when disabling the focused button loses focus.
      await page.evaluate(() => document.activeElement?.blur());
      await page.keyboard.press('Escape');
      assert.equal(await dialog.evaluate(node => node.open), true, 'saving consumes Escape without closing');
      await dialog.locator('form').evaluate(form => form.requestSubmit());
      assert.equal(await save.isDisabled(), true, 'queued submission remains gated');
      assert.deepEqual(await readFeeds(), originals, 'held write has not committed');
    } finally { await page.evaluate(() => window.__feedEditRelease()); }
  } else await save.click();
  await saved();
  await open();
  assert.equal(await name.inputValue(), title); assert.equal(await url.inputValue(), changed.href);
  const persisted = (await readFeeds()).find(feed => feed.id === 1);
  assert.deepEqual(persisted, { ...original, url: changed.href, custom_title: title });
  await dialog.getByRole('button', { name: '使用原名', exact: true }).click();
  assert.equal(await name.inputValue(), '');
  await url.fill(original.url); await save.click(); await saved();
  assert.equal((await readFeeds()).find(feed => feed.id === 1).custom_title, null, 'reset restores the original source name');
  if (original.custom_title !== null) {
    await open(); await name.fill(original.custom_title); await save.click(); await saved();
  }
  await page.locator(action('import-config')).click();
  await open(); await close();
  await page.locator(action('import-config')).click();
  await page.waitForFunction(() => document.querySelector('[data-page="feeds"] > [data-layout="status-banner"]')?.textContent.includes('再次点击才会执行'));
  await open(); await close();
  assert.deepEqual(await readFeeds(), originals, 'original feed identities, URLs and titles are restored');
  return { canceledDraftPreserved: true, failedSaveRetried: true, customNameReset: true, pendingSaveGate: native ? 'unit-and-web-coverage' : 'web-lock-verified' };
};
