// CDP acceptance for an isolated native process with the 5000-entry reading fixture.
// The caller owns process startup/shutdown and supplies its copied SQLite database.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { DatabaseSync } = require('node:sqlite');
const { chromium } = require('playwright');
const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, args) => {
  if (index % 2 === 0) pairs.push([value.slice(2), args[index + 1]]);
  return pairs;
}, []));
const row = id => `[data-layout="entry-card"][data-position-entry="${id}"]`;
const action = name => `[data-action="${name}"]`;

(async () => {
  const output = path.resolve(options['artifact-dir']);
  await fs.mkdir(output, { recursive: true });
  const database = new DatabaseSync(path.resolve(options.database), { readOnly: true });
  const browser = await chromium.connectOverCDP(options.cdp);
  const page = browser.contexts()[0].pages().find(page => /dioxus/.test(page.url()));
  assert.ok(page, 'isolated native WebView exists');
  page.setDefaultTimeout(15000);
  const report = { mode: options.mode, checks: [], errors: [], status: 'running' };
  page.on('pageerror', error => report.errors.push(String(error)));
  const check = (name, result) => { report.checks.push({ name, pass: !!result }); assert.ok(result, name); };
  const storedWorkspace = () => JSON.parse(database.prepare('SELECT value FROM app_settings WHERE key = ?').get('app_state_v2').value).entries_workspace;
  const storedEntry = id => database.prepare('SELECT is_read, is_starred, is_archived FROM entries WHERE id = ?').get(Number(id));
  const waitStored = async predicate => {
    const deadline = Date.now() + 15000;
    while (!predicate()) {
      if (Date.now() >= deadline) throw Error('SQLite did not commit the expected state');
      await new Promise(resolve => setTimeout(resolve, 25));
    }
  };
  try {
    await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
    await page.locator('[data-slot="entry-card-title"]').first().waitFor();
    check('reading fixture identity', (await page.locator('[data-slot="entry-card-title"]').first().innerText()).startsWith('阅读基线文章'));
    const expectedPath = path.join(output, 'expected.json');
    if (options.mode === 'reopen') {
      const expected = JSON.parse(await fs.readFile(expectedPath, 'utf8'));
      check('grouping survives process restart', await page.locator(action('group-by-source')).getAttribute('aria-pressed') === 'true');
      check('final All filter survives process restart', await page.getByRole('button', { name: '仅未读', exact: true }).getAttribute('aria-pressed') === 'false');
      await page.locator('details.entries-view-options > summary').click();
      check('archive visibility survives process restart', !await page.getByRole('checkbox', { name: '显示已归档文章' }).isChecked());
      await page.locator('details.entries-view-options > summary').click();
      for (const id of expected.ids) check('archived row stays hidden after restart ' + id, await page.locator(row(id)).count() === 0);
      check('SQLite retains exact flags', expected.ids.every(id => storedEntry(id).is_read === 1 && storedEntry(id).is_archived === 1) && storedEntry(expected.ids[0]).is_starred === 1);
      check('unselected row remains unchanged', JSON.stringify(storedEntry(expected.otherId)) === JSON.stringify(expected.otherFlags));
    } else {
      assert.equal(options.mode, 'write');
      check('fresh 5000-entry fixture', database.prepare('SELECT count(*) AS count FROM entries').get().count === 5000);
      await page.locator('details.entries-view-options > summary').click();
      await page.getByRole('checkbox', { name: '显示已归档文章' }).uncheck();
      await page.locator('details.entries-view-options > summary').click();
      const ids = await page.locator('[data-layout="entry-card"]').evaluateAll(items => items.slice(0, 3).map(item => item.dataset.positionEntry));
      const [first, second, otherId] = ids;
      const otherFlags = storedEntry(otherId);
      check('fixture starts unread and unstarred', [first, second].every(id => storedEntry(id).is_read === 0 && storedEntry(id).is_starred === 0));
      await page.locator(`${row(first)} ${action('toggle-starred')}`).click();
      await page.waitForFunction(selector => document.querySelector(selector)?.getAttribute('aria-pressed') === 'true', `${row(first)} ${action('toggle-starred')}`);
      check('single star persisted', storedEntry(first).is_starred === 1);
      await page.locator(`${row(first)} ${action('mark-read')}`).click();
      await page.waitForFunction(selector => document.querySelector(selector)?.dataset.read === 'true', row(first));
      check('single read persisted', storedEntry(first).is_read === 1);
      await page.locator(`${row(first)} ${action('mark-read')}`).click();
      await page.waitForFunction(selector => document.querySelector(selector)?.dataset.read === 'false', row(first));
      check('single read can be toggled back', storedEntry(first).is_read === 0);
      for (const id of [first, second]) await page.locator(`${row(id)} ${action('select-entry')}`).check();
      await page.locator(action('mark-selected-read')).click();
      await page.waitForFunction(ids => ids.every(id => document.querySelector(`[data-position-entry="${id}"]`)?.dataset.read === 'true')
        && document.querySelectorAll('[data-layout="entry-card"][data-selected="true"]').length === 0, [first, second]);
      check('batch marks only selected ids', storedEntry(first).is_read === 1 && storedEntry(second).is_read === 1 && storedEntry(otherId).is_read === otherFlags.is_read);
      for (const id of [first, second]) await page.locator(`${row(id)} ${action('select-entry')}`).check();
      await page.locator(action('archive-selected')).click();
      await page.waitForFunction(ids => ids.every(id => !document.querySelector(`[data-position-entry="${id}"]`)), [first, second]);
      check('batch archive persisted', storedEntry(first).is_archived === 1 && storedEntry(second).is_archived === 1);
      check('archive refills the page', await page.locator('[data-layout="entry-card"]').count() === 50);
      await page.getByRole('button', { name: '仅未读', exact: true }).click();
      await page.getByRole('button', { name: '仅未读', exact: true }).click();
      await page.locator(action('group-by-source')).click();
      await waitStored(() => { const state = storedWorkspace(); return state.read_filter === 'all' && state.grouping_mode === 'source' && !state.show_archived; });
      check('final preferences committed before restart', storedWorkspace().read_filter === 'all');
      await fs.writeFile(expectedPath, JSON.stringify({ ids: [first, second], otherId, otherFlags }, null, 2));
    }
    check('no uncaught native errors', report.errors.length === 0);
    report.status = 'pass';
  } catch (error) {
    report.status = 'fail'; report.failure = String(error.stack);
    throw error;
  } finally {
    await fs.writeFile(path.join(output, options.mode + '.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify({ mode: options.mode, status: report.status, checks: report.checks.length }));
    database.close(); await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
