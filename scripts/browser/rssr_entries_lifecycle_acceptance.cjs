// Same-path baseline/candidate acceptance for Entries request lifecycle extraction.
// Uses the real Web build and same-origin server with an isolated fixture library.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('playwright');
const fixture = require('./directory_fixture.cjs');
const workspace = require('./rssr_workspace_acceptance.cjs');

const rows = '[data-layout="entry-card"]';
const action = name => `[data-action="${name}"]`;
const ready = page => page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();

async function searchRefreshReturn(page, width) {
  await page.setViewportSize({ width, height: 844 });
  await workspace.seed(page, fixture.origin);
  const title = await page.locator(`${rows} [data-slot="entry-card-title"]`).first().innerText();
  await page.locator(action('toggle-search')).click();
  const search = page.locator('[data-field="entry-search"]');
  await search.fill('no matching lifecycle article');
  await page.waitForFunction(() => document.querySelectorAll('[data-layout="entry-card"]').length === 0);
  await search.fill(title);
  await page.waitForFunction(() => document.querySelectorAll('[data-layout="entry-card"]').length === 1);
  assert.equal(await page.locator(`${rows} [data-slot="entry-card-title"]`).innerText(), title);
  // Queue several scopes, then require the latest complete list.
  await search.fill('no matching lifecycle article');
  await search.fill(title);
  await search.fill('');
  await page.waitForFunction(() => document.querySelectorAll('[data-layout="entry-card"]').length === 8);
  await search.press('Escape');

  const feedRequested = page.waitForRequest(request => request.method() === 'POST' && new URL(request.url()).pathname === '/api/v1/jobs/refresh-all');
  await page.locator(action('activate-home')).click();
  await feedRequested;
  await page.waitForFunction(() => ['finished', 'error'].includes(document.querySelector('[data-slot="manual-refresh-status"]').dataset.state));
  await ready(page);
  assert.equal(await page.locator(rows).count(), 8, 'refresh retains the query scope');
  await page.locator(action('entry-page-next')).click();
  await page.waitForFunction(() => document.querySelector('[data-page="entries"]').dataset.positionPage === '2');
  const expected = await page.locator(rows).evaluateAll(items => items.map(item => item.dataset.positionEntry));
  await page.locator(`${rows} [data-slot="entry-card-title"]`).first().click();
  await page.locator('[data-page="reader"][data-position-ready="true"]').waitFor();
  await page.locator('[data-nav="back"]').click();
  await ready(page);
  await page.waitForFunction(() => document.querySelector('[data-page="entries"]').dataset.positionPage === '2');
  assert.deepEqual(await page.locator(rows).evaluateAll(items => items.map(item => item.dataset.positionEntry)), expected);
  return { width, restoredPage: 2, entryIds: expected };
}

(async () => {
  const repo = path.resolve(__dirname, '../..');
  const output = path.resolve(process.env.ARTIFACT_DIR || 'target/entries-lifecycle-20261027/acceptance');
  await fs.mkdir(output, { recursive: true });
  const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN, channel: process.env.CHROME_BIN ? undefined : 'chrome', headless: true });
  const context = await browser.newContext({ viewport: { width: 1536, height: 864 } });
  await fixture.install(context, repo);
  const page = await context.newPage();
  page.setDefaultTimeout(15000);
  const results = [], pageErrors = [], consoleErrors = [], expectedConsoleErrors = [];
  let currentScenario;
  page.on('pageerror', error => pageErrors.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error') return;
    const record = {text:message.text(), url:message.location().url};
    if (currentScenario === 'pendingWrites' && record.url === fixture.origin + '/api/v1/entries/72/read'
        && record.text === 'Failed to load resource: the server responded with a status of 503 (Service Unavailable)') {
      expectedConsoleErrors.push(record);
    } else consoleErrors.push(record);
  });
  const scenario = async (name, run) => {
    currentScenario = name;
    console.log('START ' + name);
    try {
      const evidence = await run();
      results.push({ name, pass: true, evidence });
      console.log('PASS ' + name);
    } catch (error) {
      results.push({ name, pass: false, error: String(error) });
      await page.screenshot({ path: path.join(output, 'failure.png') });
      throw error;
    }
  };
  try {
    await workspace.seed(page, fixture.origin);
    for (const [name, width, height] of [['desktop', 1536, 864], ['mobile', 390, 844]]) {
      await page.setViewportSize({ width, height });
      await page.evaluate(async () => {
        await document.fonts.ready;
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      });
      await page.screenshot({ path: path.join(output, name + '.png'), animations: 'disabled', caret: 'hide' });
    }
    await page.setViewportSize({ width: 1536, height: 864 });
    for (const name of ['selection', 'sources', 'queryStability', 'pendingWrites', 'preferenceOrdering', 'sourceIdentity']) {
      await scenario(name, () => workspace[name](page));
    }
    for (const width of [1536, 390]) {
      await scenario('search-refresh-return-' + width, () => searchRefreshReturn(page, width));
    }
    assert.deepEqual(pageErrors, [], 'no uncaught page errors');
    assert.deepEqual(consoleErrors, [], 'no unexpected console errors');
    assert.equal(expectedConsoleErrors.length, 1, 'the one explicitly refused write is recorded');
    console.log(JSON.stringify({ passed: results.length, pageErrors, consoleErrors }));
  } finally {
    await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify({ results, pageErrors, consoleErrors, expectedConsoleErrors }, null, 2));
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
