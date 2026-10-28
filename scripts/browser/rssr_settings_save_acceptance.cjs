// Same-path Settings acceptance; native mode requires an isolated fixture process.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('playwright');
const fixture = require('./directory_fixture.cjs');
const { installStorageHelpers } = require('./storage_helpers.cjs');
const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, args) => {
  if (index % 2 === 0) pairs.push([value.slice(2), args[index + 1]]);
  return pairs;
}, []));
const action = name => `[data-action="${name}"]`;
const field = name => `[data-field="${name}"]`;
const banner = '[data-page="settings"] [data-layout="status-banner"]';
const settle = page => page.evaluate(async () => {
  await document.fonts.ready;
  await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
});
async function eventually(read, expected, label) {
  const deadline = Date.now() + 15000;
  for (;;) {
    const actual = await read();
    try { assert.deepEqual(actual, expected, label); return actual; }
    catch (error) { if (Date.now() >= deadline) throw error; }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
}
async function holdWrites(page, interaction) {
  await page.evaluate(() => {
    window.__settingsWriteHeld = false;
    void navigator.locks.request('rssr-browser-state-v1', async () => {
      window.__settingsWriteHeld = true;
      await new Promise(resolve => { window.__settingsReleaseWrite = resolve; });
    });
  });
  await page.waitForFunction(() => window.__settingsWriteHeld);
  try { await interaction(); }
  finally { await page.evaluate(() => window.__settingsReleaseWrite()); }
}

(async () => {
  const output = path.resolve(options['artifact-dir'] || process.env.ARTIFACT_DIR || 'target/settings-save-acceptance');
  await fs.mkdir(output, { recursive: true });
  const native = !!options.cdp;
  const database = native ? new (require('node:sqlite').DatabaseSync)(path.resolve(options.database), { readOnly: true }) : null;
  const browser = native ? await chromium.connectOverCDP(options.cdp) : await chromium.launch({ channel: 'chrome', headless: true });
  const context = native ? browser.contexts()[0] : await browser.newContext();
  if (!native) { await fixture.install(context, path.resolve(__dirname, '../..')); await installStorageHelpers(context); }
  const page = native ? context.pages().find(page => /dioxus/.test(page.url())) : await context.newPage();
  assert.ok(page, 'explicit native target exists');
  page.setDefaultTimeout(15000);
  const report = { native, results: [], pageErrors: [], consoleErrors: [] };
  page.on('pageerror', error => report.pageErrors.push(String(error)));
  page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
  const settings = () => native
    ? JSON.parse(database.prepare("SELECT value FROM app_settings WHERE key = 'user_settings'").get().value)
    : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-state-v1'))).settings);
  const flags = () => native
    ? database.prepare('SELECT id, is_read, is_starred, is_archived FROM entries ORDER BY id').all().map(entry => ({ ...entry }))
    : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-entry-flags-v1'))).entries);
  const ready = () => page.locator(field('theme-mode')).waitFor();
  const seed = async () => {
    if (native) {
      await page.locator('[data-nav="settings"]').click();
    } else {
      await page.goto(fixture.origin + '/__codex/setup-local-auth?seed=directory-contract&next=/settings');
      await ready();
      // The directory fixture uses legacy archive=0; Settings saves need valid input.
      await page.evaluate(() => window.__rssrTestMutateSlice('rssr-web-state-v1', core => { core.settings.archive_after_months = 120; }));
      await page.reload();
    }
    await ready();
    let saved = await settings();
    await eventually(() => page.locator(field('entries-page-size')).inputValue(), String(saved.entries_page_size), 'loaded saved settings');
    if (native && saved.archive_after_months === 0 && options.mode !== 'reopen') {
      await page.locator(field('archive-after-months')).fill('120');
      await page.locator(action('save-settings')).click(); await status('设置已保存。');
      saved = { ...saved, archive_after_months: 120 }; await checkSaved(saved);
    }
    return saved;
  };
  const status = async (text, tone = 'info') => {
    await page.waitForFunction(({ text, tone }) => {
      const banner = document.querySelector('[data-page="settings"] [data-layout="status-banner"]');
      return banner?.dataset.state === tone && banner.textContent.includes(text)
        && document.querySelector('[data-action="save-settings"]').dataset.state === 'idle';
    }, { text, tone });
  };
  const checkSaved = async expected => {
    await eventually(settings, expected, 'exact persisted settings');
    await eventually(() => page.evaluate(() => document.querySelector('#user-custom-css')?.textContent || ''), expected.custom_css, 'applied CSS snapshot');
    assert.equal(await page.locator('.app-shell').evaluate((element, theme) => element.classList.contains('theme-' + theme), expected.theme), true);
  };
  const scenario = async (name, run) => {
    try { const evidence = await run(); report.results.push({ name, pass: true, evidence }); console.log('PASS ' + name); }
    catch (error) {
      report.results.push({ name, pass: false, error: String(error.stack), saved: await settings(), dom: await page.evaluate(() => ({
        status: document.querySelector('[data-layout="status-banner"]')?.outerHTML,
        fields: Array.from(document.querySelectorAll('[data-field]'), element => [element.dataset.field, element.value]),
      })) });
      if (!native) await page.screenshot({ path: path.join(output, 'failure.png') }); throw error;
    }
  };
  try {
    if (options.mode === 'reopen') {
      assert.ok(native && options.expect, 'reopen needs native fixture and prior receipt');
      const receipt = JSON.parse(await fs.readFile(options.expect, 'utf8'));
      await scenario('process-reopen', async () => {
        await seed(); await checkSaved(receipt.finalSettings);
        assert.equal(await page.locator(field('theme-mode')).inputValue(), receipt.finalSettings.theme);
        assert.equal(await page.locator(field('entries-page-size')).inputValue(), String(receipt.finalSettings.entries_page_size));
        assert.deepEqual(await flags(), receipt.finalFlags, 'all article flags survive process restart');
        return { settings: receipt.finalSettings, flagRowsCompared: receipt.finalFlags.length };
      });
    } else {
      for (const width of [1536, 390]) await scenario('settings-flow-' + width, async () => {
        await page.setViewportSize({ width, height: 844 });
        const original = await seed(), originalFlags = await flags();
        assert.equal(originalFlags.length, native ? 5000 : 72, 'isolated fixture');
        await settle(page);
        if (!native) await page.screenshot({ path: path.join(output, `settings-${width}.png`), animations: 'disabled', caret: 'hide' });
        assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), true, 'no horizontal overflow');
        await page.locator(field('custom-css')).fill('body {');
        await page.locator(action('save-settings')).click();
        await status('自定义 CSS 格式无效', 'error');
        assert.deepEqual(await settings(), original, 'invalid CSS never persists');
        await page.locator(field('custom-css')).fill(original.custom_css);
        await page.locator(field('entries-page-size')).fill('201');
        await page.locator(action('save-settings')).click();
        await status('草稿已保留，可修改后重试。', 'error');
        assert.match(await page.locator(banner).innerText(), /文章页每页数量/);
        assert.deepEqual(await settings(), original, 'application validation failure preserves saved settings');
        assert.equal(await page.locator(field('entries-page-size')).inputValue(), '201');
        await page.locator(field('entries-page-size')).fill('0');
        const expected = { ...original, entries_page_size: 100, theme: original.theme === 'dark' ? 'light' : 'dark' };
        await page.locator(field('theme-mode')).selectOption(expected.theme);
        await page.locator(action('save-settings')).click();
        await status('已回退为默认值 100。'); await checkSaved(expected);
        assert.equal(await page.locator(field('entries-page-size')).inputValue(), '100');
        expected.custom_css = '/* settings save acceptance */';
        await page.locator(field('custom-css')).fill(expected.custom_css);
        await page.locator(action('apply-custom-css')).click();
        await status('已应用当前输入框中的自定义 CSS。'); await checkSaved(expected);
        await page.locator(field('preset-theme-select')).selectOption('newsprint');
        await page.locator(action('apply-selected-theme')).click();
        await status('已应用示例主题：Newsprint。');
        expected.custom_css = await fs.readFile(path.resolve(__dirname, '../../assets/themes/newsprint.css'), 'utf8');
        await checkSaved(expected);
        assert.equal(await page.locator(field('preset-theme-select')).inputValue(), 'newsprint');
        await page.locator(action('clear-custom-css')).click();
        await status('已清空自定义 CSS。'); expected.custom_css = ''; await checkSaved(expected);
        if (native) {
          await page.locator(action('activate-home')).click();
          await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
          await page.locator('[data-nav="settings"]').click();
        }
        else await page.reload();
        await ready(); await checkSaved(expected);
        await eventually(() => page.locator(field('theme-mode')).inputValue(), expected.theme, 'saved theme reloads');
        assert.equal(await page.locator(field('entries-page-size')).inputValue(), '100');
        assert.deepEqual(await flags(), originalFlags, 'settings operations preserve all article flags');
        return { settings: expected, flagRowsCompared: originalFlags.length };
      });
      if (!native) {
        await scenario('pending-draft-and-shared-gate', async () => {
          const original = await seed();
          await page.locator(field('entries-page-size')).fill('0');
          await page.locator(field('theme-mode')).selectOption('dark');
          await holdWrites(page, async () => {
            await page.locator(action('save-settings')).click();
            await page.waitForFunction(async () => (await navigator.locks.query()).pending.length > 0);
            assert.equal(await page.locator(action('save-settings')).isDisabled(), true);
            await page.locator(field('theme-mode')).selectOption('light');
            await page.locator(field('preset-theme-select')).selectOption('newsprint');
            await page.locator(action('apply-selected-theme')).click();
            assert.match(await page.locator(banner).innerText(), /上一份设置仍在保存/);
            assert.deepEqual(await settings(), original, 'blocked write has not committed');
          });
          await status('尚未保存。');
          const saved = { ...original, theme: 'dark', entries_page_size: 100 };
          await checkSaved(saved);
          assert.equal(await page.locator(field('theme-mode')).inputValue(), 'light');
          assert.equal(await page.locator(field('entries-page-size')).inputValue(), '0', 'later edit keeps the entire draft without field merging');
          assert.equal(await page.locator(field('preset-theme-select')).inputValue(), 'newsprint');
          const laterCss = await fs.readFile(path.resolve(__dirname, '../../assets/themes/newsprint.css'), 'utf8');
          assert.equal(await page.locator(field('custom-css')).inputValue(), laterCss.replace(/\r\n/g, '\n'), 'later theme input remains editable');
          await page.locator(action('save-settings')).click(); await status('已回退为默认值 100。');
          const final = { ...saved, theme: 'light', custom_css: laterCss }; await checkSaved(final);
          assert.doesNotMatch(await page.locator(banner).innerText(), /尚未保存/);
          await page.reload(); await ready(); await checkSaved(final);
          return { submittedTheme: saved.theme, finalTheme: final.theme, savedPageSize: final.entries_page_size };
        });
        await scenario('storage-failure-and-retry', async () => {
          const original = await seed();
          await page.locator(field('theme-mode')).selectOption('dark');
          await page.evaluate(() => {
            window.__settingsSetItem = Storage.prototype.setItem;
            Storage.prototype.setItem = function (key, value) {
              if (key.startsWith('rssr-web-state-v1')) throw new DOMException('settings acceptance failure', 'QuotaExceededError');
              return window.__settingsSetItem.call(this, key, value);
            };
          });
          try {
            await page.locator(action('save-settings')).click(); await status('草稿已保留，可修改后重试。', 'error');
            await checkSaved(original);
            assert.equal(await page.locator(field('theme-mode')).inputValue(), 'dark');
          } finally { await page.evaluate(() => { Storage.prototype.setItem = window.__settingsSetItem; }); }
          await page.locator(action('save-settings')).click(); await status('设置已保存。');
          const expected = { ...original, theme: 'dark' }; await checkSaved(expected);
          await page.reload(); await ready(); await checkSaved(expected);
          return { theme: expected.theme };
        });
      }
    }
    report.finalSettings = await settings(); report.finalFlags = await flags();
    assert.deepEqual(report.pageErrors, [], 'no uncaught page errors');
    assert.deepEqual(report.consoleErrors, [], 'no console errors');
  } finally {
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    database?.close(); await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
