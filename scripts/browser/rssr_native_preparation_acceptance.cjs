// Current formal Web + real rssr-web/SQLite. Only response timing and FileReader
// completion are controlled; this does not use the retired BrowserStore.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const { chromium, expect } = require('playwright/test');
const fixture = require('./directory_fixture.cjs');
const field = name => `[data-field="${name}"]`;
const action = name => `[data-action="${name}"]`;

(async () => {
  const output = path.resolve(process.env.ARTIFACT_DIR || 'target/native-preparation-acceptance');
  await fs.mkdir(output, { recursive: true });
  const report = { status: 'running', results: [], pageErrors: [], consoleErrors: [], patches: [] };
  await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
  let browser, context, page, dav, releasePull = () => {}, releasePatch = () => {};
  let remote, pullGate = Promise.resolve(), pullCount = 0;
  try {
    dav = http.createServer(async (request, response) => {
      try {
        if (request.method === 'PUT') {
          const chunks = []; for await (const chunk of request) chunks.push(chunk);
          remote = JSON.parse(Buffer.concat(chunks).toString('utf8'));
          response.writeHead(201).end(); return;
        }
        assert.equal(request.method, 'GET');
        const payload = JSON.stringify(remote); pullCount++;
        await pullGate;
        response.writeHead(200, { 'Content-Type': 'application/json' }).end(payload);
      } catch (error) { report.serverError = String(error); response.destroy(error); }
    });
    await new Promise((resolve, reject) => { dav.once('error', reject); dav.listen(0, '127.0.0.1', resolve); });
    browser = await chromium.launch({ executablePath: process.env.CHROME_BIN, channel: process.env.CHROME_BIN ? undefined : 'chrome', headless: true });
    context = await browser.newContext({ acceptDownloads: true });
    await fixture.install(context, path.resolve(__dirname, '../..'));
    await context.addInitScript(() => {
      const probe = window.__fileProbe = { hold: false, pending: [] };
      const read = FileReader.prototype.readAsArrayBuffer;
      FileReader.prototype.readAsArrayBuffer = function (blob) {
        if (!probe.hold) return read.call(this, blob);
        probe.pending.push(() => new Promise(resolve => {
          this.addEventListener('loadend', resolve, { once: true });
          read.call(this, blob);
        }));
      };
    });
    page = await context.newPage(); page.setDefaultTimeout(15000);
    page.on('pageerror', error => report.pageErrors.push(String(error)));
    page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
    page.on('request', request => {
      if (request.method() === 'PATCH' && new URL(request.url()).pathname === '/api/v1/settings') report.patches.push(request.postDataJSON());
    });
    const settings = async () => {
      const response = await context.request.get(fixture.origin + '/api/v1/settings');
      assert.equal(response.status(), 200); return response.json();
    };
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const status = text => expect(page.locator('[data-page="settings"] [data-layout="settings-save-region"] [data-layout="status-banner"]')).toContainText(text);
    const idle = () => expect(page.locator(action('save-settings'))).toHaveAttribute('data-state', 'idle');
    const save = async () => { await page.locator(action('save-settings')).click(); await status('设置已保存'); await idle(); };
    const stored = expected => expect.poll(settings).toEqual(expected);
    const hints = () => page.locator('[data-section="reading-preferences"] [data-state="invalid"]').count();
    const record = async (name, run) => {
      try { await run(); report.results.push({ name, pass: true }); console.log('PASS ' + name); }
      catch (error) { report.results.push({ name, pass: false, error: String(error.stack) }); throw error; }
    };
    const remount = async () => {
      await page.locator('[data-nav="feeds"]').click(); await page.locator('[data-page="feeds"]').waitFor();
      await page.locator('[data-nav="settings"]').click();
      await expect(page.locator(field('entries-page-size'))).toHaveValue(String((await settings()).entries_page_size));
    };
    const pick = async css => {
      const choosing = page.waitForEvent('filechooser'); await page.locator(action('import-custom-css-file')).click();
      await (await choosing).setFiles({ name: 'theme.css', mimeType: 'text/css', buffer: Buffer.isBuffer(css) ? css : Buffer.from(css) });
    };
    for (const width of [1536, 390]) {
      await page.setViewportSize({ width, height: 844 });
      await page.goto('about:blank');
      await page.goto(fixture.origin + '/__codex/setup-local-auth?seed=directory-contract&workspace=true&next=/settings');
      await expect(page.locator(field('entries-page-size'))).toHaveValue('8');

      await record('per-field-save-and-later-edit-' + width, async () => {
        const before = await settings();
        let patchSeen = false;
        const gate = new Promise(resolve => { releasePatch = resolve; });
        const hold = async route => {
          if (route.request().method() !== 'PATCH') return route.continue();
          patchSeen = true; await gate; await route.continue();
        };
        await page.route('**/api/v1/settings', hold);
        try {
          await page.locator(field('entries-page-size')).fill('0');
          await page.locator(action('save-settings')).click();
          await expect.poll(() => patchSeen).toBe(true);
          await page.locator(field('refresh-interval')).fill('75');
          assert.deepEqual(await settings(), before, 'held save has not stored');
          releasePatch(); await status('尚未保存'); await idle();
          await stored({ ...before, entries_page_size: 100 });
          await expect(page.locator(field('entries-page-size'))).toHaveValue('100');
          await expect(page.locator(field('refresh-interval'))).toHaveValue('75');
          assert.deepEqual(Object.fromEntries(Object.entries(report.patches.at(-1)).filter(([,v]) => v !== null)), { entries_page_size: 100 }, 'only the edited field was sent');
        } finally { releasePatch(); await page.unroute('**/api/v1/settings', hold); }
        await save(); await stored({ ...before, entries_page_size: 100, refresh_interval_minutes: 75 });
      });

      await record('equal-value-save-retires-parse-feedback-' + width, async () => {
        const before = await settings();
        for (const name of ['refresh-interval', 'archive-after-months', 'entries-page-size', 'reader-font-scale']) await page.locator(field(name)).fill('');
        await expect.poll(hints).toBe(4);
        await save(); await stored(before); await expect.poll(hints).toBe(0);
      });

      await record('download-merges-per-field-and-retains-edit-feedback-' + width, async () => {
        await page.locator(field('webdav-endpoint')).fill(`http://127.0.0.1:${dav.address().port}/`);
        await page.locator(field('webdav-remote-path')).fill('settings.json');
        await page.locator(action('push-webdav')).click(); await status('配置已上传');
        const before = await settings();
        const target = { ...before, theme: 'dark', refresh_interval_minutes: 90, entries_page_size: 120 };
        remote.settings = target;
        pullGate = new Promise(resolve => { releasePull = resolve; });
        const count = pullCount;
        await page.locator(action('pull-webdav')).click(); await status('再次点击');
        await page.locator(action('pull-webdav')).click();
        await expect.poll(() => pullCount).toBe(count + 1);
        await page.locator(field('entries-page-size')).fill('80');
        await page.locator(field('entries-page-size')).fill('');
        await expect.poll(hints).toBe(1);
        releasePull(); await status('已从 WebDAV 下载并导入配置'); await stored(target);
        await expect(page.locator(field('theme-mode'))).toHaveValue('dark');
        await expect(page.locator(field('refresh-interval'))).toHaveValue('90');
        await expect.poll(hints).toBe(1); // Pending page size owns its parse feedback.
        await save(); await stored({ ...target, entries_page_size: 80 }); await expect.poll(hints).toBe(0);
      });

      await record('theme-utf8-bom-export-and-stale-file-' + width, async () => {
        await remount();
        const before = await settings();
        const css = '\uFEFF/* 中文 🦀 */\r\nbody { --pr40-color: #123456; }';
        const savedCss = css.replace(/^\uFEFF/, '');
        await page.locator('[data-layout="theme-advanced"] summary').click();
        const patchesBeforeImport = report.patches.length;
        await pick(css); await status('已从文件载入并预览自定义 CSS'); await stored(before);
        assert.equal(report.patches.length, patchesBeforeImport, 'import only previews, without saving other fields');
        const download = page.waitForEvent('download'); await page.locator(action('export-custom-css-file')).click();
        const exported = await download;
        assert.equal(exported.suggestedFilename(), 'rss-reader-theme.css');
        assert.deepEqual(await fs.readFile(await exported.path()), Buffer.from(savedCss));
        await save(); await stored({ ...before, custom_css: savedCss });
        await pick(Buffer.from([47, 42, 255, 42, 47])); await status('CSS 文件不是有效 UTF-8');
        await stored({ ...before, custom_css: savedCss });
        await page.evaluate(() => { window.__fileProbe.hold = true; });
        await pick('/* obsolete */'); await page.waitForFunction(() => window.__fileProbe.pending.length === 1);
        await page.evaluate(() => { window.__fileProbe.hold = false; });
        await pick('/* latest */'); await status('已从文件载入并预览自定义 CSS');
        await stored({ ...before, custom_css: savedCss });
        const patches = report.patches.length;
        await page.evaluate(async () => {
          await Promise.all(window.__fileProbe.pending.splice(0).map(resume => resume()));
        });
        await settle();
        await stored({ ...before, custom_css: savedCss });
        assert.equal(report.patches.length, patches, 'obsolete read did not issue a save');
        await expect(page.locator(field('custom-css'))).toHaveValue('/* latest */');
        await save(); await stored({ ...before, custom_css: '/* latest */' });
      });
      await page.screenshot({ path: path.join(output, `settings-${width}.png`) });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
    }
    assert.deepEqual(report.pageErrors, []); assert.deepEqual(report.consoleErrors, []);
    assert.equal(report.serverError, undefined);
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed'; report.error = String(error.stack);
    await page?.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
    process.exitCode = 1;
  } finally {
    releasePatch(); releasePull();
    if (context) await fixture.get({ context: () => context })?.close();
    await browser?.close();
    if (dav) await new Promise(resolve => dav.close(resolve));
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    if (report.error) console.error(report.error);
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
