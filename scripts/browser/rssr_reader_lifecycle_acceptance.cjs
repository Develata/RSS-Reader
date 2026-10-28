// Same-path Reader acceptance for Web or an explicitly selected isolated native process.
// Web uses the existing offline fixture; native startup/shutdown belongs to the caller.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('playwright');
const fixture = require('./directory_fixture.cjs');
const workspace = require('./rssr_workspace_acceptance.cjs');
const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, args) => {
  if (index % 2 === 0) pairs.push([value.slice(2), args[index + 1]]);
  return pairs;
}, []));
const action = name => `[data-action="${name}"]`;
const reader = '[data-page="reader"]';
const body = '[data-layout="reader-body"]';
const title = '[data-slot="reader-title"]';
const frames = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const ready = page => page.locator(`${reader}[data-state="loaded"][data-position-ready="true"]`).waitFor();

(async () => {
  const output = path.resolve(options['artifact-dir'] || process.env.ARTIFACT_DIR || 'target/reader-lifecycle-acceptance');
  await fs.mkdir(output, { recursive: true });
  const native = !!options.cdp;
  const database = native ? new (require('node:sqlite').DatabaseSync)(path.resolve(options.database), { readOnly: true }) : null;
  const browser = native ? await chromium.connectOverCDP(options.cdp) : await chromium.launch({ channel: 'chrome', headless: true });
  const context = native ? browser.contexts()[0] : await browser.newContext();
  if (!native) await fixture.install(context, path.resolve(__dirname, '../..'));
  const page = native ? context.pages().find(page => /dioxus/.test(page.url())) : await context.newPage();
  assert.ok(page, 'explicit native target exists');
  page.setDefaultTimeout(15000);
  const report = { native, results: [], errors: [], consoleErrors: [] };
  page.on('pageerror', error => report.errors.push(String(error)));
  page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
  const flags = async () => native
    ? database.prepare('SELECT id, is_read, is_starred, is_archived FROM entries ORDER BY id').all().map(entry => ({ id: entry.id, is_read: !!entry.is_read, is_starred: !!entry.is_starred, is_archived: !!entry.is_archived }))
    : page.evaluate(() => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-entry-flags-v1'))).entries.map(entry => ({ id: entry.id, is_read: entry.is_read, is_starred: entry.is_starred, is_archived: !!entry.is_archived })).sort((a, b) => a.id - b.id));
  const entryTitle = async id => native
    ? database.prepare('SELECT title FROM entries WHERE id = ?').get(id).title
    : page.evaluate(id => JSON.parse(localStorage.getItem(window.__rssrTestStateKey('rssr-web-state-v1'))).entries.find(entry => entry.id === id).title, id);
  const currentId = async () => Number((await page.locator(reader).getAttribute('data-position-key')).split(':')[1]);
  const back = async () => { if (native) await page.locator('[data-nav="back"]').click(); else await page.goBack(); };
  const openFirst = async () => {
    if (!native) await workspace.seed(page, fixture.origin);
    await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
    const expected = await page.locator('[data-slot="entry-card-title"]').first().innerText();
    if (native) assert.match(expected, /^阅读基线文章/, 'isolated native fixture');
    await page.locator('[data-slot="entry-card-title"]').first().click();
    await ready(page);
    assert.equal(await page.locator(title).innerText(), expected);
  };
  const scenario = async (name, run) => {
    try { const evidence = await run(); report.results.push({ name, pass: true, evidence }); console.log('PASS ' + name); }
    catch (error) { report.results.push({ name, pass: false, error: String(error.stack) }); if (!native) await page.screenshot({ path: path.join(output, 'failure.png') }); throw error; }
  };
  const snapshot = () => page.evaluate(() => ({
    key: document.querySelector('[data-page="reader"]').dataset.positionKey,
    title: document.querySelector('[data-slot="reader-title"]').textContent,
    body: document.querySelector('[data-layout="reader-body"]')?.innerHTML,
    status: document.querySelector('[data-layout="status-banner"]')?.textContent || '',
    read: document.querySelector('[data-action="mark-read"]')?.dataset.state,
    starred: document.querySelector('[data-action="toggle-starred"]')?.dataset.state,
  }));
  try {
    for (const width of [1536, 390]) await scenario('reader-flow-' + width, async () => {
      await page.setViewportSize({ width, height: 844 });
      await openFirst();
      const entryA = await currentId(), originalFlags = await flags();
      if (native) assert.equal(originalFlags.length, 5000);
      await page.evaluate(async () => { await document.fonts.ready; window.__readerOriginalBody = document.querySelector('[data-layout="reader-body"]'); window.__readerOriginalHtml = window.__readerOriginalBody.innerHTML; });
      await frames(page);
      // A hidden native WebView can return unpainted pixels; visual receipts use Web.
      if (!native) await page.screenshot({ path: path.join(output, `reader-${width}.png`), animations: 'disabled', caret: 'hide' });
      for (const [name, field, keys] of [['mark-read', 'is_read', ['m', 'M']], ['toggle-starred', 'is_starred', ['f', 'F']]]) {
        const initial = await page.locator(action(name)).getAttribute('data-state');
        for (const keyboard of [false, true]) for (let index = 0; index < 2; index++) {
          if (keyboard) { await page.locator('[data-layout="reader-shortcut-scope"]').focus(); await page.keyboard.press(keys[index]); }
          else await page.locator(action(name)).click();
          await page.waitForFunction(({ name, initial, changed }) => (document.querySelector(`[data-action="${name}"]`).dataset.state !== initial) === changed, { name, initial, changed: index === 0 });
          const expected = originalFlags.map(entry => entry.id === entryA && index === 0 ? { ...entry, [field]: !entry[field] } : entry);
          assert.deepEqual(await flags(), expected, 'only the current article flag is persisted');
          assert.equal(await page.evaluate(() => window.__readerOriginalBody === document.querySelector('[data-layout="reader-body"]') && window.__readerOriginalHtml === window.__readerOriginalBody.innerHTML), true, 'flag changes preserve the existing body node and content');
        }
      }
      const direction = await page.locator('[data-nav="next-unread-entry"]').isEnabled() ? 'next-unread-entry' : 'previous-unread-entry';
      await page.locator(`[data-nav="${direction}"]`).click(); await ready(page);
      const entryB = await currentId(); assert.notEqual(entryB, entryA);
      assert.equal(await page.locator(title).innerText(), await entryTitle(entryB), 'navigation title matches article identity');
      await back(); await ready(page); assert.equal(await currentId(), entryA);
      assert.equal(await page.locator(title).innerText(), await entryTitle(entryA));
      if (!native) { await page.goForward(); await ready(page); assert.equal(await currentId(), entryB); await back(); await ready(page); }
      await page.locator('[data-nav="back"]').click();
      await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
      assert.deepEqual(await flags(), originalFlags, 'round trip preserves all article flags');
      return { entryA, entryB, flagRowsCompared: originalFlags.length };
    });
    if (!native) {
      for (const failure of [false, true]) for (const roundTrip of [false, true]) {
        await scenario(`late-${failure ? 'failure' : 'success'}-${roundTrip ? 'A-B-A' : 'A-B'}`, async () => {
          await page.setViewportSize({ width: 1536, height: 844 }); await openFirst();
          const entryA = await currentId(), beforeFlags = await flags(), before = await snapshot();
          await page.evaluate(failure => {
            const request = navigator.locks.request.bind(navigator.locks);
            const probe = window.__readerResultProbe = { armed: true, held: false, done: false };
            navigator.locks.request = async function (name, ...args) {
              if (name !== 'rssr-browser-state-v1' || !probe.armed) return request(name, ...args);
              probe.armed = false;
              const result = failure ? undefined : await request(name, ...args);
              probe.held = true;
              await new Promise(resolve => { probe.release = resolve; });
              probe.done = true;
              if (failure) throw Error('Reader delayed write failure');
              return result;
            };
            probe.restore = () => { navigator.locks.request = request; };
          }, failure);
          try {
            await page.locator(action('toggle-starred')).click();
            await page.waitForFunction(() => window.__readerResultProbe.held);
            await frames(page);
            assert.deepEqual(await snapshot(), before, 'the intended command result is still held');
            const expectedFlags = beforeFlags.map(entry => !failure && entry.id === entryA ? { ...entry, is_starred: !entry.is_starred } : entry);
            assert.deepEqual(await flags(), expectedFlags, 'the held result belongs to the target write');
            await page.locator('[data-nav="next-unread-entry"]').click(); await ready(page);
            assert.notEqual(await currentId(), entryA);
            if (roundTrip) { await page.goBack(); await ready(page); assert.equal(await currentId(), entryA); }
            const expected = await snapshot();
            assert.equal(expected.status, '');
            await page.evaluate(() => window.__readerResultProbe.release());
            await page.waitForFunction(() => window.__readerResultProbe.done); await frames(page);
            assert.deepEqual(await snapshot(), expected, 'retired command cannot publish status or flags into the new visit');
            assert.deepEqual(await flags(), expectedFlags, 'result rejection does not undo a committed write');
          } finally { await page.evaluate(() => { window.__readerResultProbe.release?.(); window.__readerResultProbe.restore(); }); }
        });
      }
      await scenario('missing-article-and-recovery', async () => {
        await openFirst(); const entryA = await currentId();
        await page.goto(fixture.origin + '/entries/999999');
        await page.locator(`${reader}[data-state="error"]`).waitFor();
        assert.match(await page.locator('[data-layout="status-banner"]').innerText(), /文章不存在/);
        await page.locator('[data-nav="back"]').click(); await ready(page);
        assert.equal(await currentId(), entryA); assert.equal(await page.locator(title).innerText(), await entryTitle(entryA));
      });
    }
    assert.deepEqual(report.errors, []); assert.deepEqual(report.consoleErrors, []);
    console.log(JSON.stringify({ passed: report.results.length, errors: report.errors, consoleErrors: report.consoleErrors }));
  } finally {
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    database?.close(); await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
