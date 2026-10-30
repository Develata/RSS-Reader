// Real native WebView/CDP transport regression; this does not operate OS file dialogs.
// Caller owns the isolated process, copied reading fixture, loopback feeds and cleanup.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const { DatabaseSync } = require('node:sqlite');
const { chromium } = require('playwright');
const args = process.argv.slice(2);
assert.equal(args.length % 2, 0, 'options require values');
const options = Object.fromEntries(args.reduce((pairs, value, index) => {
  if (index % 2 === 0) pairs.push([value.replace(/^--/, ''), args[index + 1]]);
  return pairs;
}, []));
const field = name => `[data-field="${name}"]`;
const action = name => `[data-action="${name}"]`;
const normalize = value => value.replace(/\r\n?/g, '\n');
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
async function eventually(read, expected, label) {
  const deadline = Date.now() + 10000;
  for (;;) {
    try { assert.deepEqual(await read(), expected, label); return; }
    catch (error) { if (Date.now() >= deadline) throw error; }
    await new Promise(resolve => setTimeout(resolve, 30));
  }
}

(async () => {
  assert.ok(options['artifact-dir'], '--artifact-dir is required');
  const output = path.resolve(options['artifact-dir']);
  await fs.mkdir(output, { recursive: true });
  const report = {
    status: 'running', baseline: options.baseline === 'true',
    scriptSha256: sha256(await fs.readFile(__filename)), checks: [], pageErrors: [], consoleErrors: [],
    boundary: 'Native CDP inputs and real Rust transport/SQLite; no OS picker or real IME claim',
  };
  let browser, database, page;
  const check = (name, pass) => { report.checks.push({ name, pass }); assert.ok(pass, name); };
  // Replace a previous success even if process, database or CDP startup fails.
  await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
  try {
    assert.equal(process.platform, 'win32', 'process identity check currently supports Windows');
    assert.ok(options['process-file'], '--process-file is required');
    const identity = JSON.parse(await fs.readFile(options['process-file'], 'utf8'));
    assert.ok(Number.isSafeInteger(identity.pid) && identity.pid > 0);
    const cdp = new URL(identity.cdp);
    assert.equal(cdp.protocol, 'http:');
    assert.equal(cdp.hostname, '127.0.0.1');
    assert.ok(Number(cdp.port) > 0);
    const workspace = path.resolve(__dirname, '../..');
    const exe = await fs.realpath(identity.exe);
    const run = path.dirname(exe);
    const target = await fs.realpath(path.join(workspace, 'target'));
    assert.ok(run.toLowerCase().startsWith(target.toLowerCase() + path.sep), 'isolated EXE under target/');
    assert.equal(path.basename(exe), 'rssr-app.exe');
    const profile = await fs.realpath(identity.profile);
    assert.ok(profile.toLowerCase().startsWith(run.toLowerCase() + path.sep), 'isolated WebView profile');
    const processInfo = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-Command',
      `Get-CimInstance Win32_Process -Filter "ProcessId = ${identity.pid}" | Select-Object ProcessId,ExecutablePath | ConvertTo-Json -Compress`
    ], { encoding: 'utf8', windowsHide: true }).trim());
    assert.equal(processInfo.ExecutablePath.toLowerCase(), exe.toLowerCase(), 'running EXE identity');
    // CDP must belong to this app's WebView subprocess, not another browser.
    const ancestry = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-Command',
      `$owner = (Get-NetTCPConnection -LocalPort ${Number(cdp.port)} -State Listen | Select-Object -First 1).OwningProcess; $chain = @(); for ($i = 0; $i -lt 12 -and $owner; $i++) { $p = Get-CimInstance Win32_Process -Filter "ProcessId = $owner"; if (!$p) { break }; $chain += [int]$p.ProcessId; $owner = $p.ParentProcessId }; ConvertTo-Json -InputObject $chain -Compress`
    ], { encoding: 'utf8', windowsHide: true }).trim());
    assert.ok(ancestry.includes(identity.pid), 'CDP owner descends from the isolated app');
    const databasePath = await fs.realpath(path.join(run, 'RSS-Reader/library.sqlite3'));
    assert.ok(databasePath.toLowerCase().startsWith(run.toLowerCase() + path.sep), 'copied database');
    Object.assign(report, {
      identity,
      exeSha256: sha256(await fs.readFile(exe)),
      databasePath, ancestry,
    });
    database = new DatabaseSync(databasePath, { readOnly: true });
    const settings = () => JSON.parse(database.prepare("SELECT value FROM app_settings WHERE key='user_settings'").get().value);
    const flags = () => database.prepare('SELECT id,is_read,is_starred,is_archived FROM entries ORDER BY id').all().map(row => ({ ...row }));
    const feeds = () => database.prepare('SELECT id,url FROM feeds ORDER BY id').all().map(row => ({ ...row }));
    const catalog = database.prepare('SELECT id,title FROM entries ORDER BY id').all();
    assert.equal(catalog.length, 5000);
    for (const [index, entry] of catalog.entries()) {
      assert.equal(entry.id, index + 1);
      assert.equal(entry.title, `阅读基线文章 ${String(entry.id).padStart(5, '0')} — Reading baseline`);
    }
    const originalFlags = flags(), originalFeeds = feeds();
    assert.equal(originalFeeds.length, 6);
    assert.ok(originalFeeds.every(feed => new URL(feed.url).hostname === '127.0.0.1'));
    browser = await chromium.connectOverCDP(identity.cdp);
    const pages = browser.contexts().flatMap(context => context.pages()).filter(page => /dioxus/.test(page.url()));
    assert.equal(pages.length, 1);
    page = pages[0]; page.setDefaultTimeout(10000);
    page.on('pageerror', error => report.pageErrors.push(String(error)));
    page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
    report.webViewVersion = browser.version();
    await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
    const visible = await page.locator('[data-layout="entry-card"][data-position-entry]').evaluateAll(cards => cards.map(card => ({
      id: Number(card.dataset.positionEntry), title: card.querySelector('[data-slot="entry-card-title"]')?.textContent.trim(),
    })));
    assert.ok(visible.length > 0);
    for (const entry of visible) assert.equal(entry.title, catalog[entry.id - 1]?.title);
    report.fixture = { entries: catalog.length, visibleChecked: visible.length, feeds: originalFeeds.length };
    await page.locator('[data-nav="settings"]').click();
    await page.locator('[data-layout="theme-advanced"] summary').click();
    await page.locator(field('custom-css')).waitFor();
    const original = settings();
    assert.ok(original.archive_after_months >= 1, 'prepare a legal archive threshold before this probe');
    await eventually(() => page.locator(field('custom-css')).inputValue(), normalize(original.custom_css), 'initial loaded CSS');
    report.initial = original;
    report.decoder = await page.evaluate(() => ({
      exists: window.interpreter?.c instanceof TextDecoder,
      ignoreBOM: window.interpreter?.c?.ignoreBOM,
    }));
    check('locked interpreter decoder exists', report.decoder.exists);
    if (!report.baseline) check('startup preserves BOM before page edits', report.decoder.ignoreBOM === true);
    await page.evaluate(() => {
      const decoder = window.interpreter.c;
      const own = Object.getOwnPropertyDescriptor(decoder, 'decode');
      const decode = decoder.decode;
      window.__rssrTransportProbe = { reads: [], restore: () => {
        if (own) Object.defineProperty(decoder, 'decode', own); else delete decoder.decode;
      } };
      decoder.decode = function (bytes, ...args) {
        const raw = new TextDecoder('utf-8', { ignoreBOM: true }).decode(bytes);
        const actual = decode.call(this, bytes, ...args);
        if (raw.startsWith('\uFEFF')) window.__rssrTransportProbe.reads.push({ raw, actual });
        return actual;
      };
    });
    const fields = () => page.locator('[data-field]').evaluateAll(es => Object.fromEntries(es.map(e => [e.dataset.field, e.value])));
    const expectedFields = css => ({
      'theme-mode': original.theme, 'startup-view': original.startup_view,
      'refresh-interval': String(original.refresh_interval_minutes), 'archive-after-months': String(original.archive_after_months),
      'entries-page-size': String(original.entries_page_size), 'reader-font-scale': String(original.reader_font_scale),
      'custom-css': normalize(css),
      'webdav-endpoint': '', 'webdav-remote-path': 'config/rss-reader.json',
    });
    const apply = async raw => {
      const css = normalize(raw);
      const beforePreview = settings();
      await page.locator(field('custom-css')).fill(raw);
      await page.locator(action('apply-custom-css')).click();
      await eventually(() => page.locator('#user-custom-css').textContent(), css, 'explicit preview');
      assert.deepEqual(settings(), beforePreview, 'preview does not store settings');
      await page.locator(action('save-settings')).click();
      await eventually(settings, { ...original, custom_css: css }, 'exact SQLite settings');
      return css;
    };
    const plain = '/* native transport 中文 */\n:root { --rssr-transport-probe: 1; }\n';
    await apply(plain);
    await eventually(fields, expectedFields(plain), 'ordinary text fields');
    check('ordinary text applies and preserves every field', true);
    // Stable same-path render comparison, separate from foreground OS screenshots.
    await page.locator('[data-nav="feeds"]').click();
    await page.locator('[data-page="feeds"]').waitFor();
    await page.locator('[data-nav="settings"]').click();
    await page.locator('[data-layout="theme-advanced"] summary').click();
    await eventually(fields, expectedFields(plain), 'page reload before comparison');
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await page.evaluate(async () => { await document.fonts.ready; window.scrollTo(0, 0); await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))); });
      await page.screenshot({ path: path.join(output, `ordinary-${width}.png`), animations: 'disabled', caret: 'hide' });
      check('no horizontal overflow ' + width, await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
    }
    await page.setViewportSize({ width: 1280, height: 900 });
    for (const [index, raw] of [
      '\uFEFF/* 中文 BOM 😀 */\r\n:root { --rssr-transport-probe: 2; }\r\n',
      '\uFEFF\uFEFF/* consecutive BOM */\n:root { --rssr-transport-probe: 3; }\n',
      '/* interior \uFEFF and emoji 😀 */\n:root { --rssr-transport-probe: 4; }\n',
    ].entries()) {
      const css = await apply(raw);
      if (report.baseline && index === 0) {
        await page.waitForFunction(() => window.__rssrTransportProbe.reads.length > 0);
        report.corruptedFields = await fields();
        report.transportReads = await page.evaluate(() => window.__rssrTransportProbe.reads);
        check('old decoder drops exactly the leading BOM', report.transportReads.some(r => r.raw === '\uFEFF' + r.actual));
        check('old field alignment is corrupted', report.corruptedFields['custom-css'] !== css && report.corruptedFields['theme-mode'] !== original.theme);
        break;
      }
      await eventually(fields, expectedFields(css), 'BOM and neighboring fields ' + index);
      await eventually(() => page.locator('#user-custom-css').textContent(), css, 'applied CSS retains exact text');
      check('BOM/Unicode field and applied-style alignment ' + index, true);
    }
    report.transportReads = await page.evaluate(() => window.__rssrTransportProbe.reads);
    if (!report.baseline) {
      check('actual BOM-prefixed Rust batches were decoded', report.transportReads.length >= 2);
      check('all BOM-prefixed tables are preserved', report.transportReads.every(r => r.raw === r.actual));
      const saved = settings();
      await page.locator('[data-nav="feeds"]').click();
      await page.locator('[data-page="feeds"]').waitFor();
      await page.locator('[data-nav="settings"]').click();
      await page.locator('[data-layout="theme-advanced"] summary').click();
      await eventually(fields, expectedFields(saved.custom_css), 'saved state after page remount');
      await page.locator(action('save-settings')).click();
      await page.waitForFunction(() => document.querySelector('[data-page="settings"] [data-layout="status-banner"]')?.textContent.includes('设置已保存。'));
      await eventually(settings, saved, 'subsequent Save does not corrupt data');
      check('remount and subsequent Save remain usable', true);
    }
    assert.deepEqual(flags(), originalFlags); assert.deepEqual(feeds(), originalFeeds);
    check('all 5000 flags and 6 subscriptions unchanged', true);
    check('no page or console errors', report.pageErrors.length === 0 && report.consoleErrors.length === 0);
    report.finalSettings = settings(); report.finalFlags = flags(); report.finalFeeds = feeds();
    report.status = report.baseline ? 'baseline-reproduced' : 'pass';
  } catch (error) {
    report.status = 'fail'; report.error = String(error.stack); process.exitCode = 1;
  } finally {
    const cleanup = async (name, run) => {
      try { await run(); }
      catch (error) {
        (report.cleanupErrors ??= []).push({ name, error: String(error) });
        report.status = 'fail'; process.exitCode = 1;
      }
    };
    if (page) await cleanup('restore decoder observation', () => page.evaluate(() => {
      window.__rssrTransportProbe?.restore(); delete window.__rssrTransportProbe;
    }));
    if (database) await cleanup('close SQLite reader', () => database.close());
    if (browser) await cleanup('disconnect CDP', () => browser.close());
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify({ status: report.status, checks: report.checks.length, error: report.error }));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
