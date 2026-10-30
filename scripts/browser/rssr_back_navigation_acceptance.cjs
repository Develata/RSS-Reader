// Real native hosts only. Caller supplies an isolated process/library and, on
// Android, the dedicated backpolicytest package. No installation or data reset.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const { execFile, execFileSync, spawn } = require('node:child_process');
const { chromium, expect } = require('playwright/test');
const args = process.argv.slice(2);
assert.equal(args.length % 2, 0);
const options = Object.fromEntries(args.reduce((pairs, key, i) => {
  if (i % 2 === 0) pairs.push([key.replace(/^--/, ''), args[i + 1]]);
  return pairs;
}, []));
const action = value => `[data-action="${value}"]`;
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
assert.ok(options['reader-only'] === undefined || options['reader-only'] === 'true');
const readerOnly = options['reader-only'] === 'true';

(async () => {
  for (const key of ['process-file', 'artifact-dir', 'fixture-dir']) assert.ok(options[key], key);
  const identity = JSON.parse(await fs.readFile(options['process-file'], 'utf8'));
  assert.ok(Number.isSafeInteger(Number(identity.pid)) && Number(identity.pid) > 0);
  const android = identity.platform === 'android';
  assert.ok(android || identity.platform === 'windows');
  const endpoint = new URL(identity.cdp);
  assert.equal(endpoint.hostname, '127.0.0.1');
  assert.equal(endpoint.protocol, 'http:');
  assert.ok(Number.isInteger(Number(endpoint.port)) && Number(endpoint.port) > 0);
  const adb = (...params) => execFileSync('adb', ['-s', identity.serial, ...params], { encoding: 'utf8', windowsHide: true });
  if (android) {
    assert.equal(identity.package, 'io.github.develata.rssreader.backpolicytest');
    assert.equal(adb('shell', 'pidof', identity.package).trim(), String(identity.pid));
    assert.equal(sha(await fs.readFile(identity.apk)), identity.artifactSha256);
    const installed = /^package:(\/data\/app\/[A-Za-z0-9_+~=/.-]+\/base\.apk)$/.exec(adb('shell', 'pm', 'path', identity.package).trim());
    assert.ok(installed, 'single installed test APK');
    assert.equal(adb('shell', 'sha256sum', installed[1]).split(/\s+/)[0], identity.artifactSha256);
    if (!readerOnly) assert.equal(adb('shell', 'run-as', identity.package, 'sha256sum', 'files/native-back-save-lock').split(/\s+/)[0], identity.lockHelperSha256);
    assert.ok(adb('forward', '--list').split('\n').some(line => line.trim() === `${identity.serial} tcp:${endpoint.port} localabstract:webview_devtools_remote_${identity.pid}`));
  } else {
    assert.equal(sha(await fs.readFile(identity.exe)), identity.artifactSha256);
    const actual = execFileSync('powershell.exe', ['-NoProfile', '-Command',
      `(Get-CimInstance Win32_Process -Filter 'ProcessId = ${Number(identity.pid)}').ExecutablePath`
    ], { encoding: 'utf8', windowsHide: true }).trim();
    assert.equal(actual.toLowerCase(), path.resolve(identity.exe).toLowerCase());
    assert.ok(path.resolve(identity.exe).toLowerCase().startsWith(path.resolve(__dirname, '../../target').toLowerCase() + path.sep));
    const run = path.dirname(await fs.realpath(identity.exe));
    assert.ok((await fs.realpath(identity.profile)).toLowerCase().startsWith(run.toLowerCase() + path.sep));
    const ancestry = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-Command',
      `$owner = (Get-NetTCPConnection -LocalPort ${Number(endpoint.port)} -State Listen | Select-Object -First 1).OwningProcess; $chain = @(); for ($i = 0; $i -lt 12 -and $owner; $i++) { $p = Get-CimInstance Win32_Process -Filter "ProcessId = $owner"; if (!$p) { break }; $chain += [int]$p.ProcessId; $owner = $p.ParentProcessId }; ConvertTo-Json -InputObject $chain -Compress`
    ], { encoding: 'utf8', windowsHide: true }).trim());
    assert.ok(ancestry.includes(Number(identity.pid)), 'CDP belongs to the isolated app');
    const { DatabaseSync } = require('node:sqlite');
    const database = new DatabaseSync(path.join(run, 'RSS-Reader/library.sqlite3'), { readOnly: true });
    try {
      const rows = database.prepare('SELECT id,title FROM entries ORDER BY id').all();
      assert.equal(rows.length, identity.fixtureEntries);
      for (const [index, row] of rows.entries()) {
        assert.equal(row.id, index + 1);
        assert.equal(row.title, `阅读基线文章 ${String(row.id).padStart(5, '0')} — Reading baseline`);
      }
      const feeds = database.prepare('SELECT url FROM feeds').all();
      assert.equal(feeds.length, 6);
      assert.ok(feeds.every(feed => new URL(feed.url).origin === 'http://127.0.0.1:9299'));
    } finally { database.close(); }
  }
  const output = path.resolve(options['artifact-dir']);
  await fs.mkdir(output, { recursive: true });
  const report = { status: 'running', identity, scriptSha256: sha(await fs.readFile(__filename)), checks: [], pageErrors: [], consoleErrors: [] };
  const reportPath = path.join(output, 'acceptance.json');
  await fs.writeFile(reportPath, JSON.stringify(report, null, 2));
  let browser, page, server, releaseSave = async () => {};
  const check = async (name, run) => {
    try { await run(); report.checks.push({ name, pass: true }); console.log('PASS ' + name); }
    catch (error) { report.checks.push({ name, pass: false, error: String(error.stack) }); throw error; }
  };
  try {
    server = http.createServer(async (req, res) => {
      try {
        const match = /^\/(reading-baseline)\/([1-6])\.xml$/.exec(new URL(req.url, 'http://localhost').pathname);
        if (!match) { res.writeHead(404).end(); return; }
        const bytes = await fs.readFile(path.join(options['fixture-dir'], 'reading-baseline', match[2] + '.xml'));
        res.writeHead(200, { 'Content-Type': 'application/rss+xml' }).end(bytes);
      } catch (error) { report.serverError = String(error); res.destroy(error); }
    });
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(9299, '127.0.0.1', resolve); });
    browser = await chromium.connectOverCDP(identity.cdp);
    page = browser.contexts().flatMap(context => context.pages()).find(page => /dioxus/.test(page.url()));
    assert.ok(page, 'native Dioxus page');
    page.setDefaultTimeout(15000);
    page.on('pageerror', error => report.pageErrors.push(String(error)));
    page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
    report.webViewVersion = browser.version();
    const click = selector => page.locator(selector).first().click();
    const entries = () => page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
    const reader = () => page.locator('[data-page="reader"][data-state="loaded"][data-position-ready="true"]').waitFor();
    const settle = () => page.waitForTimeout(350);
    const home = async () => { await click(action('activate-home')); await entries(); await settle(); };
    const assertForeground = () => assert.ok(adb('shell', 'dumpsys', 'activity', 'activities').split('\n').some(line => /mResumedActivity|topResumedActivity/.test(line) && line.includes(identity.package + '/')), 'test package must own system input');
    const systemBack = () => { assertForeground(); return adb('shell', 'input', 'keyevent', '4'); };
    const systemBackPending = () => {
      assertForeground();
      return new Promise(resolve => execFile('adb', ['-s', identity.serial, 'shell', 'input', 'keyevent', '4'], { windowsHide: true }, error => resolve(error)));
    };
    const closeOverlay = async selector => { if (android) systemBack(); else await click(selector); await settle(); };
    const screenshot = async name => { await settle(); await page.screenshot({ path: path.join(output, name + '.png'), animations: 'disabled' }); };
    await entries();
    assert.ok((await page.locator('[data-slot="entry-card-title"]').allTextContents()).every(title => /^阅读基线文章 \d{5} — Reading baseline$/.test(title.trim())), 'isolated fixture titles');
    // Count only the extra native capture round trip, not the existing DOM click capture.
    await page.evaluate(android => {
      window.__backProbe = { count: 0, mode: 'normal', intercepted: 0, hintTransitions: [] };
      const dispatch = document.dispatchEvent;
      const ipc = window.ipc;
      const post = ipc.postMessage.bind(ipc);
      let hintVisible = false;
      const observer = new MutationObserver(() => {
        const probe = window.__backProbe;
        if (probe.started && !probe.leftReaderAt && !document.querySelector('[data-page="reader"]')) probe.leftReaderAt = performance.now();
        const visible = !!document.querySelector('[data-layout="exit-hint"]');
        if (visible !== hintVisible) {
          hintVisible = visible;
          probe.hintTransitions.push({ visible, at: performance.now() });
        }
      });
      observer.observe(document.documentElement, { subtree: true, childList: true });
      document.dispatchEvent = function (event) {
        const value = dispatch.call(this, event);
        if (event.type === 'rssr-capture-position') {
          window.__backProbe.count++;
          window.__backProbe.started = performance.now();
          window.__backProbe.leftReaderAt = null;
          if (window.__backProbe.mode !== 'normal') event.detail.fact = { __backProbe: true };
        }
        return value;
      };
      const wrappedPost = function (message) {
        const parsed = typeof message === 'string' ? JSON.parse(message) : message;
        if (parsed.method === 'query' && parsed.params?.data?.data?.__backProbe) {
          window.__backProbe.intercepted++;
          if (window.__backProbe.mode === 'hold') return;
          parsed.params.data = { method: 'return_error', data: 'intentional capture failure for acceptance' };
          return post(typeof message === 'string' ? JSON.stringify(parsed) : parsed);
        }
        return post(message);
      };
      // Android's Java interface method can be non-writable. Replace the window
      // binding, then verify the injection, instead of silently assigning a method.
      // Only Android needs failure/timeout injection. Windows checks synchronous
      // toolbar behavior through dispatch observation and keeps its IPC binding.
      window.__restoreBackProbe = () => { observer.disconnect(); document.dispatchEvent = dispatch; if (android) window.ipc = ipc; };
      if (android) {
        window.ipc = { postMessage: wrappedPost };
        if (window.ipc.postMessage !== wrappedPost) throw new Error('IPC probe was not installed');
      }
    }, android);
    const widths = android ? [await page.evaluate(() => innerWidth)] : [1280, 390];
    for (const width of widths) {
      if (!android) await page.setViewportSize({ width, height: 844 });
      await check('toolbar-position-and-image-priority-' + width, async () => {
        await screenshot(width + '-entries');
        const title = page.locator('[data-slot="entry-card-title"]').first();
        const text = await title.textContent();
        for (let i = 0; i < 3; i++) {
          await page.locator('[data-slot="entry-card-title"]').filter({ hasText: text }).first().click(); await reader(); await settle();
          await page.evaluate(() => scrollTo(0, 900)); await settle();
          if (i === 0) {
            await screenshot(width + '-reader');
            const image = page.locator('[data-page="reader"] img').first();
            await image.scrollIntoViewIfNeeded();
            const beforeImage = await page.evaluate(() => scrollY);
            await image.evaluate(node => node.addEventListener('click', () => { window.__readerImageActivationY = scrollY; }, {once:true, capture:true}));
            await image.click();
            await page.locator('[data-layout="reader-image-viewer"]').waitFor();
            await screenshot(width + '-image');
            // The modal makes the toolbar inert; Android uses system back, Windows Esc.
            if (android) systemBack(); else await page.keyboard.press('Escape');
            await expect(page.locator('[data-layout="reader-image-viewer"]')).toHaveCount(0);
            await reader();
            await expect(image).toBeFocused();
            const imagePosition = await page.evaluate(() => ({activated:window.__readerImageActivationY, restored:scrollY}));
            (report.imagePositions ??= []).push({width, beforeClick:beforeImage, ...imagePosition});
            assert.equal(typeof imagePosition.activated, 'number');
            assert.ok(Math.abs(imagePosition.restored - imagePosition.activated) <= 3, 'modal restores the position at actual image activation');
            await page.evaluate(() => scrollTo(0, 900)); await settle();
          }
          await page.evaluate(() => { window.__backProbe.count = 0; });
          await click('[data-nav="back"]'); await entries();
          assert.equal(await page.evaluate(() => window.__backProbe.count), 0, 'toolbar remains synchronous');
          await page.locator('[data-slot="entry-card-title"]').filter({ hasText: text }).first().click(); await reader(); await settle();
          assert.ok(Math.abs(await page.evaluate(() => scrollY) - 900) <= 3, 'reader position restored');
          await click('[data-nav="back"]'); await entries(); await settle();
        }
      });
      if (readerOnly) await check('reader-shortcuts-flags-and-return-' + width, async () => {
        await click('[data-slot="entry-card-title"]'); await reader(); await settle();
        const root = page.locator('[data-page="reader"]');
        const entry = await root.getAttribute('data-position-key');
        await page.evaluate(() => { window.__readerRuleBody = document.querySelector('[data-layout="reader-body"]'); });
        for (const [key, command] of [['m', 'mark-read'], ['f', 'toggle-starred']]) {
          const button = page.locator('[data-layout="reader-bottom-bar"] ' + action(command));
          const before = await button.getAttribute('data-state');
          await page.keyboard.press(key);
          await expect(button).not.toHaveAttribute('data-state', before);
          await page.keyboard.press(key);
          await expect(button).toHaveAttribute('data-state', before);
          assert.ok(await page.evaluate(() => window.__readerRuleBody === document.querySelector('[data-layout="reader-body"]')), 'flag writes retain the mounted body');
        }
        await page.keyboard.press('Control+ArrowRight');
        // Controlled exclusion facts, not an OS keyboard/IME acceptance claim.
        await page.evaluate(() => {
          for (const facts of [{repeat: true}, {isComposing: true}]) document.dispatchEvent(new KeyboardEvent('keydown', {key:'j', bubbles:true, ...facts}));
          const input = document.createElement('input'); input.id = 'reader-rule-input';
          document.body.append(input); input.focus();
        });
        await page.keyboard.press('ArrowRight'); await settle();
        await expect(root).toHaveAttribute('data-position-key', entry);
        await page.evaluate(() => document.getElementById('reader-rule-input').remove());
        for (const key of ['j', 'ArrowRight']) {
          await expect(page.locator('[data-nav="next-unread-entry"]')).toBeEnabled();
          await page.evaluate(() => scrollTo(0, 900)); await settle();
          await page.keyboard.press(key);
          await expect(root).not.toHaveAttribute('data-position-key', entry); await reader();
          await click('[data-nav="back"]');
          await expect(root).toHaveAttribute('data-position-key', entry); await reader(); await settle();
          assert.ok(Math.abs(await page.evaluate(() => scrollY) - 900) <= 3, 'key navigation captured position before switching');
        }
        await screenshot(width + '-reader-after-shortcuts');
        await page.evaluate(() => { delete window.__readerRuleBody; });
        await click('[data-nav="back"]'); await entries(); await settle();
      });
      if (!readerOnly) await check('source-directory-and-edit-dismiss-' + width, async () => {
        await click(action('open-source-filter')); await page.locator('#source-filter-dialog').waitFor();
        await screenshot(width + '-source');
        await closeOverlay('#source-filter-dialog button[aria-label="关闭来源筛选"]');
        await expect(page.locator('#source-filter-dialog')).toHaveCount(0); await entries();
        if (width < 760) {
          await click(action('open-directory')); await page.locator(action('close-directory')).waitFor();
          await screenshot(width + '-directory');
          await closeOverlay(action('close-directory')); await expect(page.locator(action('close-directory'))).toBeHidden();
          await entries();
        }
        await click('[data-nav="feeds"]'); await page.locator('[data-page="feeds"]').waitFor();
        await click(action('edit-feed')); await expect(page.locator('#feed-edit-url')).toBeEnabled();
        await screenshot(width + '-edit');
        await closeOverlay('#feed-edit-dialog button[aria-label="关闭来源编辑"]');
        await expect(page.locator('#feed-edit-dialog')).toHaveCount(0);
        await page.locator('[data-page="feeds"]').waitFor();
        await click('[data-nav="settings"]'); await page.locator('[data-page="settings"]').waitFor();
        await screenshot(width + '-settings');
        if (android) { systemBack(); await page.locator('[data-page="feeds"]').waitFor(); }
        await home();
      });
    }
    if (android && !readerOnly) {
      await check('saving-editor-consumes-system-back', async () => {
        await click('[data-nav="feeds"]'); await click(action('edit-feed'));
        await expect(page.locator('#feed-edit-url')).toBeEnabled();
        await page.locator('#feed-edit-name').fill('Back policy save gate fixture');
        // Feed editing writes SQLite without fetching the URL. Hold a genuine writer
        // in the isolated package; the helper verifies the fixture and rolls back.
        const lock = spawn('adb', ['-s', identity.serial, 'shell', '-T', 'run-as', identity.package, 'files/native-back-save-lock'], { windowsHide: true });
        let lockOutput = '', lockError = '';
        lock.stdout.on('data', bytes => { lockOutput += bytes; });
        lock.stderr.on('data', bytes => { lockError += bytes; });
        const closed = new Promise(resolve => {
          lock.once('close', code => resolve({ code }));
          lock.once('error', error => resolve({ error: String(error) }));
        });
        releaseSave = async () => {
          lock.stdin.end('\n');
          const outcome = await closed;
          report.saveLock = { ...outcome, stdout: lockOutput, stderr: lockError };
          assert.equal(outcome.code, 0, JSON.stringify(report.saveLock));
          assert.match(lockOutput, /RELEASED/);
          releaseSave = async () => {};
        };
        await expect.poll(() => lockOutput, { timeout: 5000 }).toContain('LOCKED');
        await click(action('save-feed-edit'));
        await expect(page.locator(action('save-feed-edit'))).toBeDisabled();
        await settle();
        systemBack(); await settle();
        await expect(page.locator('#feed-edit-dialog')).toBeVisible();
        await expect(page.locator(action('save-feed-edit'))).toBeDisabled();
        await page.locator('[data-page="feeds"]').waitFor();
        await screenshot('saving-editor-after-back');
        await releaseSave(); await expect(page.locator('#feed-edit-dialog')).toHaveCount(0);
        await home();
      });
      await check('system-capture-success-failure-timeout-and-merged-input', async () => {
        for (const mode of ['normal', 'fail', 'hold', 'normal']) {
          await click('[data-slot="entry-card-title"]'); await reader(); await settle();
          await page.evaluate(mode => { window.__backProbe.mode = mode; window.__backProbe.count = 0; window.__backProbe.intercepted = 0; }, mode);
          if (mode === 'hold') { assertForeground(); adb('shell', 'input', 'keyevent', '4', '4', '4'); } else systemBack();
          await entries();
          const result = await page.evaluate(() => ({ count: window.__backProbe.count, intercepted: window.__backProbe.intercepted, routeDelayMs: window.__backProbe.leftReaderAt - window.__backProbe.started, observationDelayMs: performance.now() - window.__backProbe.started }));
          assert.equal(result.count, 1, 'one capture per return even for a burst');
          assert.equal(result.intercepted, mode === 'normal' ? 0 : 1, 'failure/hold injection actually reached the capture reply');
          if (mode === 'hold') assert.ok(result.routeDelayMs >= 400 && result.routeDelayMs < 1500, '500 ms capture timeout plus host/DOM scheduling');
          report.checks.push({ name: 'capture-' + mode, pass: true, ...result });
          await page.evaluate(() => { window.__backProbe.mode = 'normal'; }); await settle();
        }
      });
      await check('route-change-cancels-capture-and-allows-next-return', async () => {
        await click('[data-slot="entry-card-title"]'); await reader(); await settle();
        await page.evaluate(() => { window.__backProbe.mode = 'hold'; window.__backProbe.count = 0; });
        const pendingBack = systemBackPending();
        await expect.poll(() => page.evaluate(() => window.__backProbe.count), { intervals: [10] }).toBe(1);
        assert.equal(await page.evaluate(() => !!window.__backProbe.leftReaderAt), false, 'leave the route while capture is still pending');
        await home(); assert.equal(await pendingBack, null); await page.waitForTimeout(600); await entries();
        report.cancelledCapture = await page.evaluate(() => ({ intercepted: window.__backProbe.intercepted, routeDelayMs: window.__backProbe.leftReaderAt - window.__backProbe.started }));
        assert.ok(report.cancelledCapture.routeDelayMs < 450, 'route changed before the capture timeout');
        await page.evaluate(() => { window.__backProbe.mode = 'normal'; });
        await click('[data-slot="entry-card-title"]'); await reader(); systemBack(); await entries();
      });
      const hint = () => page.locator('[data-layout="exit-hint"]');
      const resume = () => adb('shell', 'am', 'start', '-W', '-n', identity.package + '/dev.dioxus.main.MainActivity');
      await check('home-timeout-background-resume-and-stale-timer', async () => {
        systemBack(); await expect(hint()).toBeVisible(); await screenshot('exit-hint');
        await expect(hint()).toHaveCount(0, { timeout: 6500 });
        const lifetime = await page.evaluate(() => { const events = window.__backProbe.hintTransitions.slice(-2); return events[1].at - events[0].at; });
        assert.ok(lifetime >= 4500 && lifetime < 6500, 'five-second hint lifetime');
        report.hintLifetimeMs = lifetime;
        systemBack(); await expect(hint()).toBeVisible();
        systemBack(); await page.waitForTimeout(300);
        assert.ok(!adb('shell', 'dumpsys', 'activity', 'activities').split('\n').some(line => /mResumedActivity|topResumedActivity/.test(line) && line.includes(identity.package)), 'backgrounded');
        resume(); await expect(hint()).toHaveCount(0);
        // Preserve an old timeout, then show a new prompt after an independent resume.
        systemBack(); await expect(hint()).toBeVisible();
        adb('shell', 'input', 'keyevent', '3'); await page.waitForTimeout(1800); resume();
        await expect(hint()).toHaveCount(0);
        systemBack(); await expect(hint()).toBeVisible();
        await page.waitForTimeout(3400); await expect(hint()).toBeVisible();
        systemBack(); await page.waitForTimeout(300); resume(); await expect(hint()).toHaveCount(0);
        assert.equal(adb('shell', 'pidof', identity.package).trim(), String(identity.pid), 'same Activity process survived');
      });
    }
    assert.deepEqual(report.pageErrors, []);
    assert.deepEqual(report.consoleErrors, []);
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed'; report.error = String(error.stack); process.exitCode = 1;
    await page?.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
    console.error(error);
  } finally {
    await releaseSave().catch(error => { report.cleanupError = String(error); process.exitCode = 1; });
    await page?.evaluate(() => window.__restoreBackProbe?.()).catch(() => {});
    await fs.writeFile(reportPath, JSON.stringify(report, null, 2));
    await browser?.close();
    server?.closeAllConnections();
    await new Promise(resolve => server ? server.close(resolve) : resolve());
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
