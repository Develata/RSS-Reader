const assert = require('node:assert/strict');
const field = name => `[data-field="${name}"]`;
const action = name => `[data-action="${name}"]`;

module.exports = async ({ page, baseline, origin, settings, waitStatus, eventually, screenshot, width,
  requests, setRemote, getRemote, setMode, holdDownload, releaseDownload }) => {
  const cases = [];
  const draft = () => page.locator(field('custom-css')).inputValue();
  const applied = () => page.locator('#user-custom-css').textContent().catch(() => '');
  const feedback = () => page.locator('[data-page="settings"] [data-layout="status-banner"]').allTextContents();
  const preset = () => page.locator(field('preset-theme-select')).inputValue();
  const settle = () => page.evaluate(async () => {
    await navigator.locks.request('rssr-browser-state-v1', () => {});
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
  const ready = async () => {
    await page.locator(field('custom-css')).waitFor();
    await eventually(draft, (await settings()).custom_css, 'loaded draft');
    await settle();
  };
  const reset = async () => {
    setMode('success'); releaseDownload(); await page.reload(); await ready();
    await page.locator(field('webdav-endpoint')).fill(origin + '/dav/');
    await page.locator(field('webdav-remote-path')).fill('selected.json');
  };
  const clickPull = () => page.locator(action('pull-webdav')).click();
  const download = async () => { await clickPull(); await clickPull(); };
  const chooseCss = async css => {
    const choosing = page.waitForEvent('filechooser');
    await page.locator(action('import-custom-css-file')).click();
    await (await choosing).setFiles({ name: 'settings-lifecycle.css', mimeType: 'text/css', buffer: Buffer.from(css) });
  };
  const imported = async expected => {
    await waitStatus('已从 WebDAV 下载并导入配置');
    await eventually(settings, expected, 'persisted downloaded settings');
    await eventually(applied, expected.custom_css, 'applied downloaded CSS');
  };
  const armResult = async failure => page.evaluate(failure => {
    const original = navigator.locks.request.bind(navigator.locks);
    const probe = window.__settingsResult = { armed: true, held: false, done: false };
    navigator.locks.request = async function(name, ...args) {
      if (name !== 'rssr-browser-state-v1' || !probe.armed) return original(name, ...args);
      probe.armed = false;
      const value = await original(name, ...args);
      probe.held = true;
      await new Promise(resolve => { probe.release = resolve; });
      probe.done = true;
      if (failure) throw Error('fixture delayed settings failure');
      return value;
    };
    probe.restore = () => { navigator.locks.request = original; };
  }, failure);
  const releaseResult = async () => {
    await page.evaluate(() => window.__settingsResult.release());
    await page.waitForFunction(() => window.__settingsResult.done);
    await settle(); await page.waitForTimeout(100);
    await page.evaluate(() => window.__settingsResult.restore());
  };
  // Keep fixture archiving valid; all subscription identities and article flags stay intact.
  await page.evaluate(() => window.__rssrTestMutateSlice('rssr-web-state-v1', core => { core.settings.archive_after_months = 120; }));
  await reset();
  await page.locator(action('push-webdav')).click(); await waitStatus('配置已上传到 WebDAV');
  const config = structuredClone(getRemote());
  const remoteSettings = async name => ({ ...(await settings()), custom_css: `/* downloaded ${name} */` });

  for (const mode of ['edit', 'edit-back', 'preset-back', 'empty-number', 'save', 'save-late-error', 'download']) {
    await reset(); const before = await settings();
    await page.locator('[data-nav="feeds"]').click(); await page.locator('[data-page="feeds"]').waitFor(); await settle();
    const loaded = { ...before, custom_css: `/* initial load ${mode} */` };
    if (mode === 'empty-number') loaded.entries_page_size++;
    await page.evaluate(value => window.__rssrTestMutateSlice('rssr-web-state-v1', core => { core.settings = value; }), loaded);
    await armResult(mode === 'save-late-error');
    await page.locator('[data-nav="settings"]').click(); await page.waitForFunction(() => window.__settingsResult.held);
    assert.equal(await draft(), before.custom_css, 'initial read result is held before publication');
    if (mode === 'preset-back') {
      const choice = await preset(); await page.locator(field('preset-theme-select')).selectOption('newsprint');
      await page.locator(field('preset-theme-select')).selectOption(choice);
    } else if (mode === 'empty-number') {
      await page.locator(field('entries-page-size')).fill('');
    } else {
      await page.locator(field('custom-css')).fill('/* edit during initial load */');
      if (mode === 'edit-back') await page.locator(field('custom-css')).fill(before.custom_css);
    }
    await settle();
    const edited = await draft(), choice = await preset();
    const numberAfterEdit = await page.locator(field('entries-page-size')).inputValue();
    if (mode === 'empty-number') assert.equal(await page.getByText('当前输入不是有效数字', { exact: false }).count(), 1);
    if (mode.startsWith('save')) { await page.locator(action('save-settings')).click(); await waitStatus('设置已保存'); }
    if (mode === 'download') {
      setRemote({ ...config, settings: { ...loaded, custom_css: '/* downloaded after initial load */' } });
      await page.locator(field('webdav-endpoint')).fill(origin + '/dav/');
      await page.locator(field('webdav-remote-path')).fill('selected.json');
      await download(); await imported(getRemote().settings);
    }
    const saved = await settings(), message = await feedback();
    await releaseResult();
    if (baseline) {
      if (mode === 'save-late-error') await waitStatus('fixture delayed settings failure', 'error');
      else { assert.equal(await draft(), loaded.custom_css); assert.equal(await applied(), loaded.custom_css); }
    } else {
      assert.equal(await draft(), mode === 'download' ? saved.custom_css : mode === 'preset-back' ? loaded.custom_css : edited);
      if (mode !== 'download') assert.equal(await preset(), choice);
      assert.equal(await applied(), mode.startsWith('save') || mode === 'download' ? saved.custom_css : loaded.custom_css);
      if (mode.startsWith('save') || mode === 'download') assert.deepEqual(await feedback(), message);
      else if (mode !== 'preset-back') await waitStatus('尚未保存');
    }
    if (mode === 'empty-number') {
      assert.equal(await page.locator(field('entries-page-size')).inputValue(), baseline ? String(loaded.entries_page_size) : numberAfterEdit);
      assert.equal(await page.getByText('当前输入不是有效数字', { exact: false }).count(), baseline ? 0 : 1);
    }
    assert.deepEqual(await settings(), saved, 'delayed initial read never writes storage');
    cases.push({ name: 'initial-' + mode, result: baseline ? 'old result observed' : 'later intent preserved' });
  }

  for (const mode of ['edit', 'edit-back', 'preset-back', 'empty-number', 'save-during-download', 'duplicate-download', 'new-file', 'old-file']) {
    await reset(); const before = await settings(), loaded = await remoteSettings(mode);
    if (mode === 'empty-number') loaded.entries_page_size++;
    setRemote({ ...config, settings: loaded }); holdDownload();
    if (mode === 'old-file') {
      await page.evaluate(() => {
        const read = FileReader.prototype.readAsArrayBuffer;
        const probe = window.__oldSettingsCss = { held: false, done: false };
        FileReader.prototype.readAsArrayBuffer = function(...args) {
          FileReader.prototype.readAsArrayBuffer = read;
          probe.held = true;
          this.addEventListener('loadend', () => { probe.done = true; }, { once: true });
          probe.release = () => read.apply(this, args);
        };
      });
      await chooseCss('/* file started before download */');
      await page.waitForFunction(() => window.__oldSettingsCss.held);
    }
    const start = requests.length; await download();
    await eventually(() => requests.length, start + 1, 'download starts');
    const originalChoice = await preset();
    if (mode === 'preset-back') {
      await page.locator(field('preset-theme-select')).selectOption('newsprint');
      await page.locator(field('preset-theme-select')).selectOption(originalChoice);
    } else if (mode === 'empty-number') {
      await page.locator(field('entries-page-size')).fill('');
    } else if (mode === 'new-file') {
      await chooseCss('/* edit during download */');
      await waitStatus(baseline ? '已从文件载入并应用自定义 CSS' : '配置仍在下载');
    } else if (mode === 'old-file') {
      await page.evaluate(() => window.__oldSettingsCss.release());
      await page.waitForFunction(() => window.__oldSettingsCss.done); await settle(); await page.waitForTimeout(100);
      if (baseline) await waitStatus('已从文件载入并应用自定义 CSS');
      else { assert.equal(await draft(), before.custom_css); assert.deepEqual(await settings(), before); }
    } else if (mode !== 'duplicate-download') {
      await page.locator(field('custom-css')).fill('/* edit during download */');
      if (mode === 'edit-back') await page.locator(field('custom-css')).fill(before.custom_css);
    }
    await settle();
    const edited = await draft(), choice = await preset();
    const numberAfterEdit = await page.locator(field('entries-page-size')).inputValue();
    if (mode === 'empty-number') assert.equal(await page.getByText('当前输入不是有效数字', { exact: false }).count(), 1);
    if (mode === 'save-during-download') {
      await page.locator(action('save-settings')).click();
      await waitStatus(baseline ? '设置已保存' : '配置仍在下载');
      if (!baseline) assert.deepEqual(await settings(), before, 'blocked save does not persist');
    }
    if (mode === 'duplicate-download') {
      await download();
      if (!baseline) await waitStatus('配置仍在下载');
      await settle(); assert.equal(requests.length, start + (baseline ? 2 : 1), 'only one actual download admitted');
    }
    releaseDownload(); await imported(loaded);
    assert.equal(await draft(), baseline || ['preset-back', 'duplicate-download', 'old-file'].includes(mode) ? loaded.custom_css : edited);
    if (mode === 'empty-number') {
      assert.equal(await page.locator(field('entries-page-size')).inputValue(), baseline ? String(loaded.entries_page_size) : numberAfterEdit);
      assert.equal(await page.getByText('当前输入不是有效数字', { exact: false }).count(), baseline ? 0 : 1);
    }
    if (!baseline && !['duplicate-download', 'old-file'].includes(mode)) assert.equal(await preset(), choice);
    if (!baseline && ['edit', 'edit-back', 'empty-number', 'save-during-download', 'new-file'].includes(mode)) {
      await waitStatus('尚未保存');
      await page.locator(action('save-settings')).click(); await waitStatus('设置已保存');
      await eventually(settings, { ...before, custom_css: edited }, 'explicit save of retained whole draft');
    }
    cases.push({ name: 'download-' + mode, result: baseline ? 'old behavior observed' : 'draft and write gate verified' });
  }

  await reset(); const before = await settings(), loaded = await remoteSettings('after-save');
  setRemote({ ...config, settings: loaded });
  await page.locator(field('custom-css')).fill('/* save already committed */');
  await armResult(false); await page.locator(action('save-settings')).click();
  await page.waitForFunction(() => window.__settingsResult.held);
  assert.equal((await settings()).custom_css, '/* save already committed */');
  const start = requests.length; await download();
  if (baseline) await imported(loaded);
  else { await waitStatus('上一份设置仍在保存'); assert.equal(requests.length, start); }
  await releaseResult(); await waitStatus('设置已保存');
  if (baseline) {
    assert.equal((await settings()).custom_css, loaded.custom_css);
    assert.equal(await applied(), '/* save already committed */', 'baseline UI disagrees with persisted download');
  } else {
    await eventually(settings, { ...before, custom_css: '/* save already committed */' }, 'only original save persisted');
    assert.equal(await page.locator(action('pull-webdav')).getAttribute('data-state'), 'idle');
    await download(); await imported(loaded); assert.equal(await draft(), loaded.custom_css);
  }
  cases.push({ name: 'download-during-save', result: baseline ? 'applied theme diverges from persistence' : 'busy rejection then explicit confirmed retry' });

  for (const mode of ['missing', 'invalid']) {
    await reset(); const before = await settings(); setMode(mode);
    await page.locator(field('webdav-remote-path')).fill(mode === 'missing' ? 'missing.json' : 'retry.json');
    // The fixture runner only permits the known missing-path browser diagnostic.
    if (mode === 'missing') await page.locator(field('webdav-endpoint')).fill(origin + '/later/');
    await download(); await waitStatus(mode === 'missing' ? '远端配置不存在' : '下载配置失败', mode === 'missing' ? 'info' : 'error');
    assert.deepEqual(await settings(), before); await page.locator(action('save-settings')).click(); await waitStatus('设置已保存');
    cases.push({ name: 'release-gate-' + mode, result: 'explicit save succeeds afterwards' });
  }
  // The earlier intentional behavior differences must not change the final visual fixture.
  await reset(); setRemote({ ...config, settings: { ...config.settings, custom_css: '/* normal downloaded CSS */' } });
  await download(); await imported(getRemote().settings); await screenshot('loaded-' + width);
  return cases;
};
