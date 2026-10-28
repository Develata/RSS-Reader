const assert = require('node:assert/strict');
const field = name => `[data-field="${name}"]`;
const action = name => `[data-action="${name}"]`;

module.exports = async ({ page, baseline, pick, settings, status, feedback, checkSaved, ready, shot, width }) => {
  const cases = [];
  const lateCss = '/* older file must not replace the newer intent */';
  const newerCss = '/* newer selected file */';
  const hold = async css => {
    await page.evaluate(() => { window.__themeFiles.mode = 'hold'; });
    await pick(Buffer.from(css));
    await page.waitForFunction(() => window.__themeFiles.pending.length === 1);
    await page.evaluate(() => { window.__themeFiles.mode = 'normal'; });
  };
  const release = async (error = false) => {
    await page.evaluate(async error => {
      const pending = window.__themeFiles.pending.shift();
      if (!pending || pending.aborted) throw Error('expected live held read');
      if (error) {
        Object.defineProperty(pending.reader, 'readyState', { configurable: true, value: 2 });
        Object.defineProperty(pending.reader, 'error', { configurable: true, value: new DOMException('older read failed', 'NotReadableError') });
        pending.reader.dispatchEvent(new ProgressEvent('error'));
      } else {
        delete pending.reader.readyState;
        await new Promise(resolve => {
          pending.reader.addEventListener('loadend', resolve, { once: true });
          pending.read.apply(pending.reader, pending.args);
        });
      }
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      await navigator.locks.request('rssr-browser-state-v1', () => {});
    }, error);
    // Let result publication after the storage promise settle before testing absence of effects.
    await page.waitForTimeout(100);
  };
  const reload = async () => { await page.reload(); await ready(); };
  const draft = () => page.locator(field('custom-css')).inputValue();
  for (const outcome of ['success', 'read-error', 'invalid-css']) {
    await reload();
    await hold(outcome === 'invalid-css' ? 'body {' : lateCss);
    await pick(Buffer.from(newerCss)); await status('已从文件载入并应用自定义 CSS');
    const saved = await settings(), message = await feedback();
    assert.equal(saved.custom_css, newerCss);
    if (outcome === 'success') await shot('newer-file-' + width);
    await release(outcome === 'read-error');
    if (baseline && outcome === 'success') {
      await checkSaved({ ...saved, custom_css: lateCss }); assert.equal(await draft(), lateCss);
    } else {
      await checkSaved(saved); assert.equal(await draft(), newerCss);
      if (baseline) await status(outcome === 'read-error' ? 'older read failed' : '自定义 CSS 格式无效', 'error');
      else assert.deepEqual(await feedback(), message, 'obsolete success/error cannot publish status');
    }
    cases.push({ name: 'newer-file-before-old-' + outcome, result: baseline ? 'old result was accepted' : 'old result ignored' });
  }
  for (const edit of ['css-away-and-back', 'appearance', 'preset', 'clear', 'explicit-save']) {
    const lateCss = `/* older file before ${edit} */`;
    await reload(); const saved = await settings();
    await hold(lateCss);
    if (edit === 'css-away-and-back') {
      await page.locator(field('custom-css')).fill('/* intermediate edit */');
      await page.locator(field('custom-css')).fill(saved.custom_css);
    } else if (edit === 'appearance') {
      await page.locator(field('theme-mode')).selectOption(saved.theme === 'dark' ? 'light' : 'dark');
    } else if (edit === 'preset') {
      await page.locator(field('preset-theme-select')).selectOption('newsprint');
    } else if (edit === 'clear') {
      await page.locator(action('clear-custom-css')).click(); await status('已清空自定义 CSS');
    } else { await page.locator(action('save-settings')).click(); await status('设置已保存'); }
    const expectedSaved = await settings(), expectedDraft = await draft();
    const expectedPreset = await page.locator(field('preset-theme-select')).inputValue();
    const message = await feedback();
    await release();
    if (baseline) {
      await status('已从文件载入并应用自定义 CSS'); assert.equal(await draft(), lateCss); assert.equal((await settings()).custom_css, lateCss);
    } else {
      await checkSaved(expectedSaved); assert.equal(await draft(), expectedDraft);
      assert.equal(await page.locator(field('preset-theme-select')).inputValue(), expectedPreset);
      assert.deepEqual(await feedback(), message);
    }
    cases.push({ name: edit, result: baseline ? 'old file replaced later intent' : 'later intent preserved' });
  }
  // Superseding at picker opening matters even when the new picker is later cancelled.
  await reload(); const beforeCancel = await settings();
  const cancelCss = '/* old file before cancelled picker */';
  await hold(cancelCss);
  const choosing = page.waitForEvent('filechooser'); await page.locator(action('import-custom-css-file')).click();
  const chooser = await choosing;
  await release();
  await chooser.setFiles([]);
  if (baseline) await checkSaved({ ...beforeCancel, custom_css: cancelCss });
  else { await checkSaved(beforeCancel); assert.equal(await draft(), beforeCancel.custom_css); assert.deepEqual(await feedback(), []); }
  cases.push({ name: 'new-picker-before-selection-or-cancel', result: baseline ? 'old file still applied' : 'old file permanently invalidated' });
  await reload(); const beforeFailure = await settings();
  const failureCss = '/* old file before invalid selection */';
  await hold(failureCss); await pick(Buffer.from('body {')); await status('自定义 CSS 格式无效', 'error');
  const failure = await feedback(); await release();
  if (baseline) await checkSaved({ ...beforeFailure, custom_css: failureCss });
  else { await checkSaved(beforeFailure); assert.deepEqual(await feedback(), failure); }
  cases.push({ name: 'newer-invalid-file', result: baseline ? 'old file revived' : 'old file remains invalid' });
  // Already-started persistence keeps the existing pending gate and later-draft behavior.
  await reload(); const beforeSave = await settings();
  await page.evaluate(() => {
    window.__themeLockHeld = false;
    void navigator.locks.request('rssr-browser-state-v1', async () => {
      window.__themeLockHeld = true; await new Promise(resolve => { window.__themeReleaseLock = resolve; });
    });
  });
  await page.waitForFunction(() => window.__themeLockHeld);
  try {
    await pick(Buffer.from('/* accepted save */'));
    await page.waitForFunction(() => document.querySelector('[data-action="save-settings"]').dataset.state === 'pending');
    await pick(Buffer.from(newerCss)); await status('上一份设置仍在保存');
    assert.equal(await draft(), newerCss); assert.deepEqual(await settings(), beforeSave);
  } finally { await page.evaluate(() => window.__themeReleaseLock()); }
  await status('尚未保存');
  await checkSaved({ ...beforeSave, custom_css: '/* accepted save */' }); assert.equal(await draft(), newerCss);
  await page.locator(action('apply-custom-css')).click(); await status('已应用当前输入框中的自定义 CSS');
  await checkSaved({ ...beforeSave, custom_css: newerCss });
  cases.push({ name: 'already-started-save', result: 'old save finishes, later draft requires explicit save' });
  return cases;
};
