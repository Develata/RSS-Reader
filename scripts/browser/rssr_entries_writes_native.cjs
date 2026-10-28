// CDP acceptance for an isolated native process with the 5000-entry reading fixture.
// The caller owns process startup/shutdown and supplies its copied SQLite database.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { isDeepStrictEqual } = require('node:util');
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
  const storedEntries = () => database.prepare('SELECT id, is_read, is_starred, is_archived FROM entries ORDER BY id').all().map(entry => ({ ...entry }));
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
      check('write receipt contains all 5000 entries', Array.isArray(expected.entries) && expected.entries.length === 5000);
      check('grouping survives process restart', await page.locator(action('group-by-source')).getAttribute('aria-pressed') === 'true');
      check('final All filter survives process restart', await page.getByRole('button', { name: '仅未读', exact: true }).getAttribute('aria-pressed') === 'false');
      await page.locator('details.entries-view-options > summary').click();
      check('archive visibility survives process restart', !await page.getByRole('checkbox', { name: '显示已归档文章' }).isChecked());
      await page.locator('details.entries-view-options > summary').click();
      for (const id of expected.ids) check('archived row stays hidden after restart ' + id, await page.locator(row(id)).count() === 0);
      check('SQLite retains exact flags for every entry', isDeepStrictEqual(storedEntries(), expected.entries));
    } else {
      assert.equal(options.mode, 'write');
      const expectedEntries = storedEntries();
      check('fresh 5000-entry fixture', expectedEntries.length === 5000);
      const checkFlags = (name, ids, changes) => {
        const selectedIds = new Set(ids.map(Number));
        for (const entry of expectedEntries) if (selectedIds.has(entry.id)) Object.assign(entry, changes);
        check(name, isDeepStrictEqual(storedEntries(), expectedEntries));
      };
      await page.locator('details.entries-view-options > summary').click();
      await page.getByRole('checkbox', { name: '显示已归档文章' }).uncheck();
      await page.locator('details.entries-view-options > summary').click();
      const [first, second] = await page.locator('[data-layout="entry-card"]').evaluateAll(items => items.slice(0, 2).map(item => item.dataset.positionEntry));
      check('fixture starts unread and unstarred', [first, second].every(id => storedEntry(id).is_read === 0 && storedEntry(id).is_starred === 0));
      await page.locator(`${row(first)} ${action('toggle-starred')}`).click();
      await page.waitForFunction(selector => document.querySelector(selector)?.getAttribute('aria-pressed') === 'true', `${row(first)} ${action('toggle-starred')}`);
      checkFlags('single star changes only its target flag', [first], { is_starred: 1 });
      await page.locator(`${row(first)} ${action('mark-read')}`).click();
      await page.waitForFunction(selector => document.querySelector(selector)?.dataset.read === 'true', row(first));
      checkFlags('single read changes only its target flag', [first], { is_read: 1 });
      await page.locator(`${row(first)} ${action('mark-read')}`).click();
      await page.waitForFunction(selector => document.querySelector(selector)?.dataset.read === 'false', row(first));
      checkFlags('single read can be toggled back without other changes', [first], { is_read: 0 });
      for (const id of [first, second]) await page.locator(`${row(id)} ${action('select-entry')}`).check();
      await page.locator(action('mark-selected-read')).click();
      await page.waitForFunction(ids => ids.every(id => document.querySelector(`[data-position-entry="${id}"]`)?.dataset.read === 'true')
        && document.querySelectorAll('[data-layout="entry-card"][data-selected="true"]').length === 0, [first, second]);
      checkFlags('batch marks only selected ids', [first, second], { is_read: 1 });
      for (const id of [first, second]) await page.locator(`${row(id)} ${action('select-entry')}`).check();
      await page.locator(action('archive-selected')).click();
      await page.waitForFunction(ids => ids.every(id => !document.querySelector(`[data-position-entry="${id}"]`)), [first, second]);
      checkFlags('batch archive changes only selected archive flags', [first, second], { is_archived: 1 });
      check('archive refills the page', await page.locator('[data-layout="entry-card"]').count() === 50);
      await page.getByRole('button', { name: '仅未读', exact: true }).click();
      await page.getByRole('button', { name: '仅未读', exact: true }).click();
      await page.locator(action('group-by-source')).click();
      await waitStored(() => { const state = storedWorkspace(); return state.read_filter === 'all' && state.grouping_mode === 'source' && !state.show_archived; });
      check('final preferences committed before restart', storedWorkspace().read_filter === 'all');
      await fs.writeFile(expectedPath, JSON.stringify({ ids: [first, second], entries: expectedEntries }, null, 2));
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
