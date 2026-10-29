// Real form notifications and accepted/rejected result projections, on Web and native CDP.
const assert = require('node:assert/strict');
const field = name => `[data-field="${name}"]`;
const action = name => `[data-action="${name}"]`;
const enums = [
  { field: 'theme-mode', key: 'theme', values: ['light', 'dark', 'system'] },
  { field: 'list-density', key: 'list_density', values: ['compact', 'comfortable'] },
  { field: 'startup-view', key: 'startup_view', values: ['last_feed', 'all'] },
];
const numbers = [
  { field: 'refresh-interval', key: 'refresh_interval_minutes', valid: [1, 525600], invalid: 525601 },
  { field: 'archive-after-months', key: 'archive_after_months', valid: [1, 1200], invalid: 1201 },
  { field: 'entries-page-size', key: 'entries_page_size', valid: [1, 200], invalid: 201 },
  { field: 'reader-font-scale', key: 'reader_font_scale', valid: [0.8, 1.5], invalid: 2 },
];
const controls = [...enums, ...numbers];

module.exports = async ({ page, baseline, settings, waitStatus, eventually, screenshot, width, origin,
  requests, setRemote, getRemote, holdDownload, releaseDownload }) => {
  const cases = [];
  const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const values = () => page.locator('[data-section="reading-preferences"] [data-field]').evaluateAll(es => Object.fromEntries(es.map(e => [e.dataset.field, e.value])));
  const expectedValues = s => Object.fromEntries(controls.map(c => [c.field, String(s[c.key])]));
  const hints = () => page.locator('[data-section="reading-preferences"] [data-field]').evaluateAll(es => es.filter(e => e.parentElement.querySelector('[data-state="invalid"]')).map(e => e.dataset.field));
  const record = (name, evidence = {}) => cases.push({ name, ...evidence });
  const idle = () => eventually(() => page.locator(action('save-settings')).getAttribute('data-state'), 'idle', 'save gate released');
  const save = async expected => {
    await page.locator(action('save-settings')).click(); await waitStatus('设置已保存'); await idle();
    await eventually(settings, expected, 'saved entire settings snapshot'); await settle();
  };
  const remount = async () => {
    await page.locator('[data-nav="feeds"]').click(); await page.locator('[data-page="feeds"]').waitFor();
    await page.locator('[data-nav="settings"]').click();
    await eventually(values, expectedValues(await settings()), 'initial projection');
    await page.locator(field('webdav-endpoint')).fill(origin + '/form/');
    await page.locator(field('webdav-remote-path')).fill('settings.json');
  };
  const startDownload = async () => {
    await page.locator(action('pull-webdav')).click(); await waitStatus('再次点击');
    await page.locator(action('pull-webdav')).click();
  };
  const imported = async expected => {
    await waitStatus('已从 WebDAV 下载并导入配置'); await eventually(settings, expected, 'downloaded settings persisted'); await idle(); await settle();
  };
  // This is fixture preparation through the real Save path, not a product validation exception.
  if ((await settings()).archive_after_months === 0) {
    const prepared = { ...(await settings()), archive_after_months: 120 };
    await page.locator(field('archive-after-months')).fill('120'); await save(prepared);
  }
  const original = await settings(); let expected = { ...original };
  await remount();
  await page.evaluate(() => {
    const events = [];
    const listener = event => {
      if (event.target?.closest('[data-section="reading-preferences"]')) events.push({ type: event.type, field: event.target.dataset.field, trusted: event.isTrusted });
    };
    document.addEventListener('input', listener); document.addEventListener('change', listener);
    window.__rssrFormProbe = { events, stop: () => { document.removeEventListener('input', listener); document.removeEventListener('change', listener); } };
  });
  const clearEvents = () => page.evaluate(() => {
    // User blur can legitimately emit change; observe only the subsequent result projection.
    document.activeElement?.blur(); window.__rssrFormProbe.events.length = 0;
  });
  const events = () => page.evaluate(() => window.__rssrFormProbe.events);
  try {
    await remount(); assert.deepEqual(await events(), [], 'initial programmatic projection emits no edit notifications');
    assert.deepEqual(await settings(), original, 'loading does not save');
    await screenshot(`form-${width}-initial`); record('initial-projection-no-edit-or-save');
    for (const control of enums) for (const value of control.values) {
      await page.locator(field(control.field)).selectOption(value);
      assert.deepEqual(await settings(), expected, 'selection is not auto-save');
      expected = { ...expected, [control.key]: value }; await save(expected);
      await eventually(values, expectedValues(expected), 'enum round trip');
      record(`enum-${control.field}-${value}`);
    }
    for (const control of numbers) {
      for (const value of control.valid) {
        await page.locator(field(control.field)).fill(String(value));
        assert.deepEqual(await settings(), expected, 'parsed numeric input is not auto-save');
        expected = { ...expected, [control.key]: value }; await save(expected);
        await eventually(values, expectedValues(expected), 'numeric bound round trip'); record(`numeric-${control.field}-${value}`);
      }
      // Empty input is delivered by the real number control; Rust retains the last parsed value.
      // Integer fractional/overflow forms are valid HTML number text but do not parse as u32.
      const unparsed = control.key === 'reader_font_scale' ? [''] : ['', '1.5', '4294967296'];
      for (const raw of unparsed) {
        await page.locator(field(control.field)).fill(raw);
        await eventually(hints, [control.field], 'unparsed input hint');
        assert.deepEqual(await settings(), expected, 'unparsed input does not overwrite saved values');
        await clearEvents(); await save(expected);
        await eventually(hints, baseline ? [control.field] : [], 'accepted save retires field feedback');
        assert.deepEqual(await events(), [], 'save projection emits no input/change');
        record(`unparsed-save-${control.field}-${raw || 'empty'}`, { feedback: baseline ? 'old stale hint observed' : 'retired', persistedLastParsedValue: true });
        await page.locator(field(control.field)).fill(String(expected[control.key])); await eventually(hints, [], 'valid input recovers');
      }
      await page.locator(field(control.field)).fill(String(control.invalid));
      await page.locator(action('save-settings')).click(); await waitStatus('草稿已保留，可修改后重试。', 'error'); await idle();
      assert.deepEqual(await settings(), expected, 'out-of-range save rejected without storage mutation');
      assert.equal(await page.locator(field(control.field)).inputValue(), String(control.invalid));
      assert.deepEqual(await hints(), [], 'range rejection is not a parse failure');
      await page.locator(field(control.field)).fill(String(expected[control.key])); await save(expected);
      record(`range-error-retry-${control.field}`);
    }
    await page.locator(field('entries-page-size')).fill('0');
    assert.deepEqual(await settings(), expected, 'zero remains a draft until explicit save');
    await clearEvents(); expected = { ...expected, entries_page_size: 100 }; await save(expected);
    await eventually(values, expectedValues(expected), 'zero normalizes only in saved result');
    assert.deepEqual(await events(), []); record('zero-page-size-normalization-no-notification');

    await page.locator(action('push-webdav')).click(); await waitStatus('配置已上传');
    const config = structuredClone(getRemote());
    const A = { ...original, theme: 'light', list_density: 'compact', startup_view: 'last_feed', refresh_interval_minutes: 30, archive_after_months: 12, entries_page_size: 80, reader_font_scale: 1 };
    const B = { ...original, theme: 'dark', list_density: 'comfortable', startup_view: 'all', refresh_interval_minutes: 60, archive_after_months: 24, entries_page_size: 120, reader_font_scale: 1.25 };
    const download = async target => {
      setRemote({ ...config, settings: target }); await clearEvents(); await startDownload(); await imported(target);
      await eventually(values, expectedValues(target), 'accepted download projection');
      assert.deepEqual(await events(), [], 'download projection emits no input/change');
    };
    await download(A); await remount();
    for (const control of numbers) await page.locator(field(control.field)).fill('');
    const allNumeric = numbers.map(c => c.field); await eventually(hints, allNumeric, 'all parse errors visible');
    await download(A); await eventually(hints, baseline ? allNumeric : [], 'equal download retires old hints');
    record('equal-download-retires-feedback', { feedback: baseline ? 'old stale hints observed' : 'retired' });
    await download(B); await eventually(hints, [], 'different values hide old feedback');
    await download(A); await eventually(hints, baseline ? allNumeric : [], 'A-B-A never revives feedback');
    record('download-roundtrip-feedback', { feedback: baseline ? 'old hints revived' : 'retired permanently' });
    await page.locator(field('entries-page-size')).fill('');
    await eventually(hints, baseline ? allNumeric : ['entries-page-size'], 'new field error cannot revive other retired errors');
    record('new-error-after-backfill-is-field-local');
    await screenshot(`form-${width}-feedback`);

    for (const control of controls) {
      await remount(); const before = await settings(); const target = before.theme === 'light' ? B : A;
      setRemote({ ...config, settings: target }); holdDownload();
      const requestCount = requests.length; await startDownload();
      await eventually(() => requests.length, requestCount + 1, 'real HTTP response held');
      await clearEvents();
      const event = control.values ? 'change' : 'input';
      await page.locator(field(control.field)).dispatchEvent(event);
      await page.locator(field(control.field)).dispatchEvent(event);
      await settle(); releaseDownload(); await imported(target);
      await eventually(values, expectedValues(before), 'duplicate same-value events protect entire draft');
      assert.equal((await events()).length, 2); assert.deepEqual(await hints(), []);
      record(`held-download-duplicate-${control.field}`, { notifications: 'two synthetic DOM events through real Rust handler', stored: 'downloaded', draft: 'later editing intent preserved' });
    }
    for (const control of numbers) {
      await remount(); const before = await settings(); const target = before.theme === 'light' ? B : A;
      setRemote({ ...config, settings: target }); holdDownload();
      const requestCount = requests.length; await startDownload();
      await eventually(() => requests.length, requestCount + 1, 'real HTTP response held');
      await page.locator(field(control.field)).fill(''); await eventually(hints, [control.field], 'later parse error');
      const draft = await values(); releaseDownload(); await imported(target);
      await eventually(values, draft, 'rejected backfill preserves later numeric input');
      assert.deepEqual(await hints(), [control.field]); record(`held-download-unparsed-${control.field}`);
    }
    await remount(); await download(original); await remount();
    await eventually(hints, [], 'remount feedback is clean');
    assert.deepEqual(await settings(), original, 'restore fixture settings');
    await screenshot(`form-${width}-restored`);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), true);
    return cases;
  } finally {
    releaseDownload();
    await page.evaluate(() => { window.__rssrFormProbe?.stop(); delete window.__rssrFormProbe; });
  }
};
