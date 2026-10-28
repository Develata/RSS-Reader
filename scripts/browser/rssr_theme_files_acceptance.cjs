// Isolated Web/WASM file workflows. Native picker outcomes are covered by Rust adapter tests.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const crypto = require('node:crypto');
const { chromium } = require('playwright');
const fixture = require('./directory_fixture.cjs');
const { installStorageHelpers } = require('./storage_helpers.cjs');
const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, args) => {
  if (index % 2 === 0) pairs.push([value.slice(2), args[index + 1]]);
  return pairs;
}, []));
const field = name => `[data-field="${name}"]`;
const action = name => `[data-action="${name}"]`;
async function eventually(read, expected, label) {
  const deadline = Date.now() + 15000;
  for (;;) {
    try { assert.deepEqual(await read(), expected, label); return; }
    catch (error) { if (Date.now() >= deadline) throw error; }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
}
(async () => {
  const output = path.resolve(options['artifact-dir'] || 'target/theme-files-acceptance');
  const baseline = options.baseline === 'true';
  const report = { status: 'running', baseline, results: [], pageErrors: [], consoleErrors: [] };
  let browser, page;
  try {
    await fs.mkdir(output, { recursive: true });
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    report.scriptSha256 = crypto.createHash('sha256').update(await fs.readFile(__filename)).digest('hex');
    report.publicRoot = path.resolve(__dirname, '../..', process.env.PUBLIC_DIR || 'target/dx/rssr-app/release/web/public');
    await fs.access(path.join(report.publicRoot, 'index.html'));
    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const context = await browser.newContext({ acceptDownloads: true });
    await fixture.install(context, path.resolve(__dirname, '../..')); await installStorageHelpers(context);
    await context.addInitScript(() => {
      const io = window.__themeFiles = { mode: 'normal', reads: 0, errors: 0, aborts: 0, pending: [], urls: new Set(), failAppend: false };
      for (const method of ['readAsText', 'readAsArrayBuffer']) {
        const read = FileReader.prototype[method];
        FileReader.prototype[method] = function (...args) {
          if (io.mode === 'normal') return read.apply(this, args);
          io.reads++;
          if (io.mode === 'hold') {
            Object.defineProperty(this, 'readyState', { configurable: true, value: 1 });
            io.pending.push({ reader: this, read, args }); return;
          }
          Object.defineProperty(this, 'readyState', { configurable: true, value: 2 });
          Object.defineProperty(this, 'error', { configurable: true, value: new DOMException('fixture read denied', 'NotReadableError') });
          queueMicrotask(() => { this.dispatchEvent(new ProgressEvent('error')); io.errors++; });
        };
      }
      const abort = FileReader.prototype.abort;
      FileReader.prototype.abort = function () {
        const pending = io.pending.find(item => item.reader === this);
        if (pending) { pending.aborted = true; delete this.readyState; io.aborts++; }
        return abort.call(this);
      };
      io.release = () => {
        for (const pending of io.pending.splice(0)) {
          if (pending.aborted) continue;
          delete pending.reader.readyState;
          pending.read.apply(pending.reader, pending.args);
        }
      };
      const create = URL.createObjectURL, revoke = URL.revokeObjectURL;
      URL.createObjectURL = function (blob) { const url = create.call(this, blob); io.urls.add(url); return url; };
      URL.revokeObjectURL = function (url) { io.urls.delete(url); return revoke.call(this, url); };
      const append = Node.prototype.appendChild;
      Node.prototype.appendChild = function (node) {
        if (io.failAppend && node.download === 'rss-reader-theme.css') throw new DOMException('fixture append denied', 'HierarchyRequestError');
        return append.call(this, node);
      };
    });
    page = await context.newPage(); page.setDefaultTimeout(15000);
    page.on('pageerror', error => report.pageErrors.push(String(error)));
    page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
    const settings = () => page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-state-v1'))).settings);
    const flags = () => page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-entry-flags-v1'))).entries);
    const feedback = async () => (await page.locator('[data-page="settings"] [data-layout="status-banner"]').allTextContents()).filter(Boolean);
    const status = (text, tone = 'info') => page.waitForFunction(({ text, tone }) => {
      const banner = document.querySelector('[data-page="settings"] [data-layout="status-banner"]');
      return banner?.textContent.includes(text) && banner.dataset.state === tone;
    }, { text, tone });
    const ready = async () => {
      await page.locator(field('custom-css')).waitFor();
      await eventually(() => page.locator(field('custom-css')).inputValue(), (await settings()).custom_css.replace(/\r\n/g, '\n'), 'loaded draft');
    };
    const seed = async () => {
      await page.goto(fixture.origin + '/__codex/setup-local-auth?seed=directory-contract&next=/settings'); await ready();
      await page.evaluate(() => window.__rssrTestMutateSlice('rssr-web-state-v1', core => { core.settings.archive_after_months = 120; }));
      await page.reload(); await ready();
    };
    const pick = async buffer => {
      const chooser = page.waitForEvent('filechooser');
      await page.locator(action('import-custom-css-file')).click();
      const selected = await chooser;
      assert.equal(await page.locator('#custom-css-file-input').inputValue(), '', 'trigger resets the previous selection');
      await selected.setFiles(buffer === null ? [] : { name: 'theme-contract.css', mimeType: 'text/css', buffer });
    };
    const checkSaved = async expected => {
      await eventually(settings, expected, 'persisted settings');
      await eventually(() => page.evaluate(() => document.querySelector('#user-custom-css')?.textContent || ''), expected.custom_css, 'applied CSS');
    };
    const shot = async name => {
      await page.evaluate(async () => { window.scrollTo(0, 0); await document.fonts.ready; await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))); });
      await page.screenshot({ path: path.join(output, name + '.png'), fullPage: true, animations: 'disabled', caret: 'hide' });
    };
    for (const width of [1536, 390]) {
      await page.setViewportSize({ width, height: 844 }); await seed();
      const original = await settings(), originalFlags = await flags();
      const evidence = { width, pass: false }; report.results.push(evidence);
      await shot('initial-' + width);
      await pick(null); assert.deepEqual(await feedback(), []); await checkSaved(original);
      await pick(Buffer.from('body {')); await status('自定义 CSS 格式无效', 'error');
      assert.equal(await page.locator(field('custom-css')).inputValue(), original.custom_css); await checkSaved(original);
      await pick(Buffer.alloc(0)); await status('已从文件载入并应用自定义 CSS'); await checkSaved(original);
      const css = '/* 主题文件 — UTF-8 */\r\n[data-page="reader"] { --theme-file-test: 1; }\r\n';
      await pick(Buffer.from('\uFEFF' + css)); await status('已从文件载入并应用自定义 CSS');
      const imported = { ...original, custom_css: css }; await checkSaved(imported);
      await shot('imported-' + width);
      const priorFeedback = await feedback(); await pick(null);
      assert.deepEqual(await feedback(), priorFeedback); await checkSaved(imported);
      const again = '/* 再次选择同名文件 */';
      await pick(Buffer.from(again)); await status('已从文件载入并应用自定义 CSS');
      await checkSaved({ ...original, custom_css: again });
      // Export the current, unsaved draft, checking actual downloaded UTF-8 bytes.
      await page.locator(field('custom-css')).fill(css);
      const exported = await page.locator(field('custom-css')).inputValue();
      const downloadEvent = page.waitForEvent('download');
      await page.locator(action('export-custom-css-file')).click();
      const download = await downloadEvent; assert.equal(download.suggestedFilename(), 'rss-reader-theme.css');
      assert.deepEqual(await fs.readFile(await download.path()), Buffer.from(exported));
      await status('已导出当前自定义 CSS');
      assert.equal(await page.evaluate(() => window.__themeFiles.urls.size), 0);
      await checkSaved({ ...original, custom_css: again });
      // Failure after object-URL creation must report an error and release the URL.
      await page.evaluate(() => { window.__themeFiles.failAppend = true; });
      await page.locator(action('export-custom-css-file')).click(); await status('导出 CSS 文件失败', 'error');
      evidence.exportFailureLiveUrls = await page.evaluate(() => window.__themeFiles.urls.size);
      assert.equal(evidence.exportFailureLiveUrls, baseline ? 1 : 0);
      await page.evaluate(() => { window.__themeFiles.failAppend = false; for (const url of window.__themeFiles.urls) URL.revokeObjectURL(url); });
      const retryDownload = page.waitForEvent('download'); await page.locator(action('export-custom-css-file')).click();
      assert.deepEqual(await fs.readFile(await (await retryDownload).path()), Buffer.from(exported));
      await status('已导出当前自定义 CSS');
      await page.locator(field('custom-css')).fill(' \n ');
      await page.locator(action('export-custom-css-file')).click(); await status('当前没有可导出的自定义 CSS');
      assert.equal(await page.evaluate(() => window.__themeFiles.urls.size), 0);
      await page.reload(); await ready();
      // A malformed UTF-8 byte in a comment must not become a replacement character on disk.
      const beforeBad = await settings();
      await pick(Buffer.from([47, 42, 255, 42, 47]));
      if (baseline) {
        await status('已从文件载入并应用自定义 CSS');
        assert.equal((await settings()).custom_css, '/*\uFFFD*/'); evidence.invalidUtf8 = 'saved replacement character';
      } else {
        await status('CSS 文件不是有效 UTF-8', 'error'); await checkSaved(beforeBad);
        assert.equal(await page.locator(field('custom-css')).inputValue(), beforeBad.custom_css); evidence.invalidUtf8 = 'rejected without draft change';
      }
      await page.reload(); await ready();
      const beforeError = await settings(), oldFeedback = await feedback();
      await page.evaluate(() => { window.__themeFiles.mode = 'fail'; });
      await pick(Buffer.from('/* read failure */'));
      await page.waitForFunction(() => window.__themeFiles.errors === 1);
      if (baseline) { await page.waitForTimeout(150); assert.deepEqual(await feedback(), oldFeedback); evidence.readFailure = 'no feedback after error event'; }
      else { await status('载入 CSS 文件失败', 'error'); evidence.readFailure = 'reported'; }
      await checkSaved(beforeError);
      await page.evaluate(() => { window.__themeFiles.mode = 'normal'; });
      await pick(Buffer.from('/* 读取失败后重试 */')); await status('已从文件载入并应用自定义 CSS');
      await checkSaved({ ...beforeError, custom_css: '/* 读取失败后重试 */' });
      await page.reload(); await ready();
      // Dropping the page while reading must leave the next visit untouched.
      const beforeLeave = await settings();
      await page.evaluate(() => { window.__themeFiles.mode = 'hold'; });
      await pick(Buffer.from('/* obsolete file result */'));
      await page.waitForFunction(() => window.__themeFiles.reads === 1);
      await page.locator('[data-nav="feeds"]').click(); await page.locator('[data-page="feeds"]').waitFor();
      evidence.abortedOnLeave = await page.evaluate(() => window.__themeFiles.aborts);
      assert.equal(evidence.abortedOnLeave, baseline ? 0 : 1);
      await page.locator('[data-nav="settings"]').click(); await ready();
      await page.evaluate(() => { window.__themeFiles.mode = 'normal'; window.__themeFiles.release(); });
      await page.waitForTimeout(150); await checkSaved(beforeLeave); assert.deepEqual(await feedback(), []);
      assert.equal(await page.locator(field('custom-css')).inputValue(), beforeLeave.custom_css.replace(/\r\n/g, '\n'));
      // Persisting an imported file can fail; preserve its draft and retry explicitly.
      await page.evaluate(() => {
        window.__themeSetItem = Storage.prototype.setItem;
        Storage.prototype.setItem = function (key, value) {
          if (key.startsWith('rssr-web-state-v1')) throw new DOMException('theme fixture write failure', 'QuotaExceededError');
          return window.__themeSetItem.call(this, key, value);
        };
      });
      const retryCss = '/* 文件导入保存失败后的重试 */';
      try {
        await pick(Buffer.from(retryCss)); await status('草稿已保留，可修改后重试', 'error');
        await checkSaved(beforeLeave); assert.equal(await page.locator(field('custom-css')).inputValue(), retryCss);
      } finally { await page.evaluate(() => { Storage.prototype.setItem = window.__themeSetItem; }); }
      await page.locator(action('apply-custom-css')).click(); await status('已应用当前输入框中的自定义 CSS');
      await checkSaved({ ...beforeLeave, custom_css: retryCss });
      await page.reload(); await ready(); await checkSaved({ ...beforeLeave, custom_css: retryCss });
      assert.deepEqual(await flags(), originalFlags, 'all 72 article flags preserved');
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
      evidence.pass = true; evidence.articleCount = originalFlags.length;
      console.log('PASS theme-files-' + width);
    }
    assert.deepEqual(report.pageErrors, []); assert.deepEqual(report.consoleErrors, []); report.status = 'passed';
  } catch (error) { report.status = 'failed'; report.error = String(error.stack); throw error; }
  finally {
    try { await browser?.close(); }
    catch (error) { report.status = 'failed'; report.cleanupError = String(error); throw error; }
    finally { await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2)); }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
