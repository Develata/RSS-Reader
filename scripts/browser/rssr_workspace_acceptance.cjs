// Run against the isolated real-server fixture; all article state is stored by the server.
// NODE_PATH resolves the locally installed Playwright; CHROME_BIN is optional.
const assert = require('node:assert/strict');
const { expect } = require('playwright/test');
const fixture = require('./directory_fixture.cjs');
const { installWorkspaceResponseProbe } = require('./workspace_response_probe.cjs');
const rows = '[data-layout="entry-card"]';
const action = name => `[data-action="${name}"]`;
const selected = `${rows}[data-selected="true"]`;
const rail = '[data-layout="entry-directory-rail"]';

async function ready(page) {
  await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
  await page.locator(`${rows} ${action('select-entry')}`).first().waitFor();
}

async function seed(page, base) {
  // Replacing the fixture library invalidates its session. Stop the old document
  // first so a delayed old request cannot trigger recovery during the reseed.
  await page.goto('about:blank');
  await page.goto(`${base}/__codex/setup-local-auth?seed=directory-contract&workspace=true&next=/entries`);
  await ready(page);
}

async function selection(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  assert.equal(await page.locator(rows).count(), 8);
  assert.equal(await page.locator(action('mark-selected-read')).isDisabled(), true);
  assert.equal(await page.locator(action('archive-selected')).isDisabled(), true);
  await page.locator(`${rows}[data-position-entry="72"] ${action('select-entry')}`).check();
  await page.locator(`${rows}[data-position-entry="71"] ${action('select-entry')}`).check();
  assert.equal(await page.locator(selected).count(), 2);
  assert.equal(await page.locator(action('select-current-page')).getAttribute('aria-checked'), 'mixed');
  const august = page.locator(`${rail} [data-directory-kind="group"]`).nth(1);
  await august.click();
  await page.waitForFunction(() => document.querySelector('[data-directory-consumer="rail"]').dataset.directoryMode === 'manual');
  assert.equal(await page.locator(selected).count(), 2, 'directory expansion preserves selection');
  await page.locator(`${rail} [data-directory-kind="item"]`).nth(1).click();
  assert.equal(await page.locator(selected).count(), 2, 'same-page jump preserves selection');
  await page.locator(action('mark-selected-read')).click();
  await page.waitForFunction(() => document.querySelector('[data-position-entry="72"]').dataset.read === 'true' && document.querySelector('[data-position-entry="71"]').dataset.read === 'true');
  assert.equal(await page.locator(`${rows}[data-read="true"]`).count(), 2, 'batch read does not expand to all matches');
  assert.equal(await page.locator(selected).count(), 0);
  assert.equal(await page.locator(rail).getAttribute('data-directory-mode'), 'manual', 'flag update does not reset directory browsing');
  await page.locator(action('select-current-page')).click();
  assert.equal(await page.locator(selected).count(), 8);
  await page.locator(action('entry-page-next')).click();
  await page.waitForFunction(() => document.querySelector('[data-page="entries"]').dataset.positionPage === '2');
  assert.equal(await page.locator(selected).count(), 0, 'paging clears selection');
  await page.locator(action('entry-page-previous')).click();
  await page.waitForFunction(() => document.querySelector('[data-page="entries"]').dataset.positionPage === '1');
  for (const id of [70, 69]) await page.locator(`${rows}[data-position-entry="${id}"] ${action('select-entry')}`).check();
  await page.locator(action('archive-selected')).click();
  await page.waitForFunction(() => !document.querySelector('[data-position-entry="70"]') && !document.querySelector('[data-position-entry="69"]'));
  assert.equal(await page.locator(rows).count(), 8, 'page refills after archive');
  await page.reload(); await ready(page);
  assert.equal(await page.locator('[data-position-entry="70"]').count(), 0, 'archive persists across reload');
}

async function sources(page) {
  await page.locator(action('select-entry')).first().check();
  await page.locator(action('open-source-filter')).click();
  const chooser = page.locator('#source-filter-dialog');
  await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
  assert.equal(await page.locator(action('apply-source-filter')).isDisabled(), true, 'empty source selection cannot apply');
  await chooser.getByRole('button', { name: '取消', exact: true }).click();
  assert.equal(await page.locator(selected).count(), 1);
  assert.equal(await page.evaluate(() => document.activeElement.id), 'source-filter-trigger', 'cancel restores focus');
  await page.locator(action('open-source-filter')).click();
  await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
  await chooser.getByRole('checkbox', { name: 'Alpha', exact: true }).check();
  await page.waitForFunction(() => document.querySelector('#source-filter-all').indeterminate);
  assert.equal(await chooser.locator('#source-filter-all').getAttribute('aria-checked'), 'mixed');
  await chooser.locator('#source-filter-all').press('Space');
  assert.equal(await chooser.locator('#source-filter-all').isChecked(), true, 'mixed state selects all with the keyboard');
  await page.waitForFunction(() => !document.querySelector('#source-filter-all').indeterminate);
  await chooser.locator('#source-filter-all').uncheck();
  assert.equal(await chooser.locator('#source-filter-all').getAttribute('aria-checked'), 'false');
  await chooser.getByRole('checkbox', { name: 'Alpha', exact: true }).check();
  assert.equal(await chooser.locator(action('edit-feed')).count(), 0, 'article source popup is filtering only');
  await page.locator(action('apply-source-filter')).click();
  await page.waitForFunction(() => document.querySelector('.source-filter-trigger span').textContent === '…');
  // The selected-scope label is immediate, while the real server list is async.
  // Wait for this fixture's filtered total before inspecting the returned rows.
  await page.waitForFunction(() => document.querySelector('.entries-count').textContent === '12 篇');
  assert.equal(await page.locator(selected).count(), 0);
  assert.deepEqual(await page.locator(`${rows} [data-slot="entry-card-meta"]`).allTextContents().then(texts => [...new Set(texts.map(text => text.split(' · ')[0]))]), ['Alpha']);
  await page.locator(action('open-source-filter')).click();
  await page.waitForFunction(() => document.querySelector('#source-filter-all').indeterminate);
  await page.keyboard.press('Escape');
}

async function editing(page) {
  await page.locator('[data-nav="feeds"]').first().click();
  await page.locator('[data-feed-id="1"]').waitFor();
  assert.equal((await page.locator('[data-stat="unread"] [data-slot="stat-card-value"]').innerText()).trim(), '70');
  assert.equal((await page.locator('[data-stat="archived"] [data-slot="stat-card-value"]').innerText()).trim(), '2');
  const oldURL = await page.locator('[data-feed-id="1"]').getAttribute('data-feed-url');
  const occupiedURL = await page.locator('[data-feed-id="2"]').getAttribute('data-feed-url');
  await page.locator('[data-feed-id="1"] [data-action="edit-feed"]').click();
  await page.locator('#feed-edit-name').fill('少数派 · 精选');
  await page.locator('#feed-edit-url').fill(occupiedURL);
  await page.locator(action('save-feed-edit')).click();
  await page.locator('#feed-edit-dialog [role="alert"]').waitFor();
  assert.equal(await page.locator('#feed-edit-name').inputValue(), '少数派 · 精选', 'failed save preserves draft');
  assert.equal(await page.locator('[data-feed-id="1"]').getAttribute('data-feed-title'), 'Alpha');
  await page.locator('#feed-edit-url').fill(`${oldURL}?edited=1`);
  await page.locator(action('save-feed-edit')).click();
  await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
  await page.waitForFunction(() => document.querySelector('[data-feed-id="1"]').dataset.feedTitle === '少数派 · 精选');
  await page.locator('[data-feed-id="1"] [data-action="edit-feed"]').click();
  assert.equal(await page.locator('#feed-edit-name').inputValue(), '少数派 · 精选');
  assert.equal(await page.locator('#feed-edit-url').inputValue(), `${oldURL}?edited=1`);
  await page.getByRole('button', { name: '使用原名', exact: true }).click();
  assert.equal(await page.locator('#feed-edit-name').inputValue(), '');
  await page.locator('#feed-edit-name').fill('少数派 · 精选');
  await page.keyboard.press('Escape');
  await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
  await page.locator('[data-action="activate-home"]').first().click(); await ready(page);
  // Home may briefly present cached rows before reloading the edited URL scope.
  await expect(page.locator('.source-filter-trigger span')).toHaveText('…');
  await expect(page.locator('[data-position-entry="72"]')).toHaveAttribute('data-read', 'true');
  await expect(page.locator('.entries-count'), 'URL edit keeps selected source scope').toHaveText('12 篇');
  await expect(page.locator(`${rows} [data-slot="entry-card-meta"]`).first()).toHaveText('少数派 · 精选 · 2026-09-02');
}

async function responsive(page) {
  for (const [width, height] of [[1920, 1080], [1536, 864], [760, 1000], [390, 844], [360, 800]]) {
    await page.setViewportSize({ width, height });
    await page.locator(action('select-current-page')).scrollIntoViewIfNeeded();
    const metrics = await page.evaluate(() => {
      const r = s => document.querySelector(s).getBoundingClientRect().toJSON();
      return { width: innerWidth, scrollWidth: document.documentElement.scrollWidth,
        box: r('.entry-select-box[type="checkbox"]'), hit: r('.entry-select-hit'), pager: r('[data-layout="entry-pagination"]'),
        previous: r('[data-action="entry-page-previous"]'), next: r('[data-action="entry-page-next"]'),
        railVisible: !!document.querySelector('[data-layout="entry-directory-rail"]').getClientRects().length };
    });
    assert.ok(metrics.scrollWidth <= width + 1, `${width}px has no horizontal overflow`);
    assert.equal(metrics.box.width, 16); assert.equal(metrics.hit.width, 44);
    assert.ok(metrics.pager.bottom <= height - 10 && metrics.pager.bottom >= height - 20);
    assert.ok(metrics.next.x - metrics.previous.x < 140, 'paging buttons stay close');
    assert.equal(metrics.railVisible, width > 1000);
    if (width <= 1000) {
      const toolbar = await page.evaluate(() => {
        const r = selector => document.querySelector(selector).getBoundingClientRect().toJSON();
        return {title:r('.entries-toolbar-title'), panel:r('.entries-toolbar'), buttons:[
          '[data-action="mark-selected-read"]', '[data-action="archive-selected"]',
          '.entries-view-options summary', '[data-action="open-directory"]',
        ].map(r)};
      });
      for (const [index, button] of toolbar.buttons.entries()) {
        assert.ok(button.width >= 44 && button.height >= 44, `${width}px toolbar hit target`);
        assert.ok(Math.abs(button.y - toolbar.title.y) <= 1, `${width}px actions stay in the title row`);
        if (index) assert.ok(button.x >= toolbar.buttons[index-1].right, `${width}px toolbar actions do not overlap`);
      }
      assert.ok(Math.abs(toolbar.buttons[3].right - toolbar.panel.right) <= 1, `${width}px directory opens from the far right`);
      assert.ok(toolbar.buttons[0].x >= toolbar.title.right, `${width}px title does not overlap actions`);
      await page.locator(action('open-directory')).click();
      assert.equal(await page.locator(rail).isVisible(), true);
      await page.locator(action('close-directory')).click();
      assert.equal(await page.locator(rail).isVisible(), false);
    }
  }
  await page.locator(action('select-entry')).first().focus();
  await page.keyboard.press('Space');
  assert.equal(await page.locator(selected).count(), 1, 'keyboard can select an article');
  await page.keyboard.press('Space');
  assert.equal(await page.locator(selected).count(), 0);
  await page.locator(action('open-source-filter')).click();
  assert.equal(await page.locator('#source-filter-dialog').isVisible(), true);
  await page.setViewportSize({ width: 1536, height: 864 });
  await page.setViewportSize({ width: 360, height: 800 });
  // setViewportSize acknowledges emulation before resize callbacks/layout finish.
  // The anchored dialog repositions on resize; observe the presented frame.
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const box = await page.locator('#source-filter-dialog').boundingBox();
  assert.ok(box.x >= 0 && box.x + box.width <= 361 && box.y >= 0 && box.y + box.height <= 801, JSON.stringify(box));
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('#source-filter-dialog').count(), 0);
}

async function toolbarThemes(page) {
  for (const preset of [null, 'atlas-sidebar', 'newsprint', 'amethyst-glass', 'midnight-ledger']) {
    await page.goto('about:blank');
    const session = await fixture.get(page).server.useSeed('directory-contract', {preset});
    const split = session.cookie.indexOf('=');
    await page.context().addCookies([{name:session.cookie.slice(0,split), value:session.cookie.slice(split+1), url:fixture.origin, httpOnly:true, sameSite:'Strict'}]);
    await page.goto(fixture.origin + '/entries'); await ready(page);
    for (const width of [360, 600, 601, 760, 1000, 1001, 1200]) {
      await page.setViewportSize({width, height:900});
      if (await page.locator(rail).isVisible()) await page.locator(action('close-directory')).click();
      const geometry = await page.evaluate(() => {
        const r = selector => document.querySelector(selector).getBoundingClientRect().toJSON();
        return {title:r('.entries-toolbar-title'), toolbar:r('.entries-toolbar'), overflow:document.documentElement.scrollWidth>innerWidth,
          buttons:['[data-action="mark-selected-read"]','[data-action="archive-selected"]','.entries-view-options summary','[data-action="open-directory"]'].map(r)};
      });
      const label = `${preset || 'default'}/${width}`;
      assert.equal(geometry.overflow, false, label);
      geometry.buttons.forEach((button, index) => {
        assert.ok(button.width>=44 && button.height>=44, `${label} hit area`);
        assert.ok(Math.abs(button.y-geometry.title.y)<1, `${label} title row`);
        assert.ok(button.x>=(index?geometry.buttons[index-1].right:geometry.title.right)-1, `${label} no overlap`);
      });
      assert.ok(Math.abs(geometry.buttons[3].right-geometry.toolbar.right)<1, `${label} right edge`);
      await page.locator(action('open-directory')).click();
      const alpha = await page.locator('.directory-heading-row').evaluate(e => {
        const canvas=document.createElement('canvas'), ctx=canvas.getContext('2d');
        ctx.fillStyle=getComputedStyle(e).backgroundColor; ctx.fillRect(0,0,1,1);
        return ctx.getImageData(0,0,1,1).data[3];
      });
      assert.equal(alpha, 255, `${label} scrolling text cannot show through the heading`);
      // Stress only the label's layout; keep the fixture's selection state intact.
      const longCount = await page.locator('.entries-count').evaluate(e => {
        const original=e.textContent;
        e.textContent='已选 12345678901234567890';
        const title=e.parentElement.getBoundingClientRect(), actions=document.querySelector('.entries-toolbar-actions').getBoundingClientRect();
        const result={sameRow:Math.abs(title.y-actions.y)<1, fits:title.right<=actions.left+1, ellipsis:getComputedStyle(e).textOverflow};
        e.textContent=original;
        return result;
      });
      assert.ok(longCount.sameRow && longCount.fits && longCount.ellipsis==='ellipsis', `${label} long count stays inside the title`);
    }
  }
}

async function editEdges(page) {
  await page.locator('[data-nav="feeds"]').first().click();
  const feed = page.locator('[data-feed-id="1"]');
  await feed.locator(action('remove-feed')).click();
  assert.equal(await feed.getAttribute('data-state'), 'confirm');
  await feed.locator(action('edit-feed')).click();
  await page.locator('#feed-edit-name').waitFor();
  await page.keyboard.press('Escape');
  assert.equal(await feed.getAttribute('data-state'), 'idle', 'editing cancels the previous delete confirmation');
  await feed.locator(action('remove-feed')).click();
  assert.equal(await feed.getAttribute('data-state'), 'confirm', 'the next delete click only asks for confirmation');
  await feed.locator(action('edit-feed')).click();
  await page.locator('#feed-edit-name').waitFor();
  await page.keyboard.press('Escape');
  await page.goto(`${fixture.origin}/__codex/setup-local-auth?seed=directory-contract&long-title=true&next=/feeds`);
  await feed.locator(action('edit-feed')).click();
  await page.locator('#feed-edit-name').waitFor();
  for (const width of [1536, 360]) {
    await page.setViewportSize({ width, height: 800 });
    const metrics = await page.locator('#feed-edit-dialog').evaluate(dialog => ({
      width: dialog.clientWidth, scrollWidth: dialog.scrollWidth,
      right: dialog.getBoundingClientRect().right,
      resetRight: dialog.querySelector('.text-button').getBoundingClientRect().right,
    }));
    assert.ok(metrics.scrollWidth <= metrics.width + 1, `long original name fits ${width}px dialog`);
    assert.ok(metrics.resetRight <= metrics.right, 'reset button remains within the dialog');
  }
  await page.getByRole('button', { name: '使用原名', exact: true }).click();
  assert.equal(await page.locator('#feed-edit-name').inputValue(), '');
  await page.keyboard.press('Escape');
}

async function queryStability(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  await seed(page, new URL(page.url()).origin);
  const first = page.locator(`${rows}[data-position-entry="72"]`);
  const original = await first.innerText();
  let queries = 0;
  const countQuery = request => { if (new URL(request.url()).pathname === '/api/v1/entries') queries++; };
  page.on('request', countQuery);
  // Intentionally cross the archive cutoff clock's second boundary. This
  // local operations must not issue another list query.
  await page.waitForTimeout(1100);
  await first.locator(action('select-entry')).check();
  await page.waitForTimeout(250);
  assert.equal(await first.innerText(), original, 'selection must not reload the list');
  await page.waitForTimeout(1100);
  await page.locator(action('close-directory')).click();
  await page.locator(action('open-directory')).click();
  await page.waitForTimeout(250);
  assert.equal(await first.innerText(), original, 'directory visibility must not reload the list');
  assert.equal(await page.locator(selected).count(), 1, 'directory visibility preserves selection');
  await page.waitForTimeout(1100);
  await page.locator(action('entry-page-next')).click();
  await page.waitForFunction(() => document.querySelector('[data-page="entries"]').dataset.positionPage === '2');
  await page.locator(action('entry-page-previous')).click();
  await first.waitFor();
  await page.waitForTimeout(250);
  assert.equal(await first.innerText(), original, 'local pagination must not reload the list');
  assert.equal(await page.locator(selected).count(), 0, 'paging still clears selection');
  await first.locator(action('select-entry')).check();
  await Promise.all([
    page.waitForResponse(response => new URL(response.url()).pathname === '/api/v1/entries'),
    page.getByRole('button', { name: '仅未读', exact: true }).click(),
  ]);
  assert.equal(queries, 1, 'only the filter change queries the server');
  page.off('request', countQuery);
  assert.equal(await page.locator(selected).count(), 0, 'a filter change still queries and clears selection');
}

async function importNames(page) {
  for (const format of ['config', 'opml']) {
    await seed(page, new URL(page.url()).origin);
    await page.locator('[data-nav="feeds"]').first().click();
    const feed = page.locator('[data-feed-id="1"]');
    await feed.locator(action('edit-feed')).click();
    await page.locator('#feed-edit-name').fill('Imported alias');
    await page.locator(action('save-feed-edit')).click();
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
    await page.locator(action(`export-${format}`)).click();
    await page.waitForFunction(format => document.querySelector(`#${format}-text`).value.includes('Imported alias'), format);
    await page.locator(action(`import-${format}`)).click();
    if (format === 'config') await page.locator(action('import-config')).click();
    await page.getByRole('status').filter({ hasText: format === 'config' ? '配置包已导入' : 'OPML 已导入' }).waitFor();
    assert.equal(await feed.getAttribute('data-feed-title'), 'Imported alias');
    await feed.locator(action('edit-feed')).click();
    await page.locator('#feed-edit-name').waitFor();
    assert.equal(await page.locator('#feed-edit-name').inputValue(), 'Imported alias');
    assert.equal(await page.locator('.feed-edit-original small').innerText(), '原始名称：Alpha', `${format} preserves the remote title`);
    await page.getByRole('button', { name: '使用原名', exact: true }).click();
    await page.locator(action('save-feed-edit')).click();
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
    await page.waitForFunction(() => document.querySelector('[data-feed-id="1"]').dataset.feedTitle === 'Alpha');
  }
}

async function sourceIdentity(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  await seed(page, new URL(page.url()).origin);
  await page.locator('[data-nav="feeds"]').first().click();
  const secondURL = await page.locator('[data-feed-id="2"]').getAttribute('data-feed-url');
  for (const id of [1, 2]) {
    await page.locator(`[data-feed-id="${id}"] ${action('edit-feed')}`).click();
    await page.locator('#feed-edit-name').fill('Same name');
    await page.locator(action('save-feed-edit')).click();
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
  }
  await page.locator('[data-action="activate-home"]').first().click(); await ready(page);
  await page.locator(action('group-by-source')).click();
  const groups = page.locator(`${rail} [data-directory-kind="group"]`);
  await groups.filter({ hasText: 'Same name' }).first().waitFor();
  assert.equal(await groups.count(), 6, 'renaming sources must not merge their directory groups');
  assert.equal(await groups.filter({ hasText: 'Same name' }).count(), 2);
  const anchors = await groups.evaluateAll(items => items.map(item => item.dataset.directoryAnchor));
  assert.equal(new Set(anchors).size, 6, 'source anchors are unique even with identical names');
  await page.locator(action('select-current-page')).click();
  assert.equal(await page.locator(selected).count(), 8, 'source grouping still selects exactly this page');
  await page.locator(action('open-source-filter')).click();
  const chooser = page.locator('#source-filter-dialog');
  await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
  await chooser.getByRole('searchbox', { name: '搜索来源' }).fill(secondURL);
  await chooser.getByRole('checkbox', { name: 'Same name', exact: true }).check();
  await page.locator(action('apply-source-filter')).click();
  await page.waitForFunction(() => document.querySelector('.entries-count').textContent === '12 篇');
  assert.deepEqual(await page.locator(rows).evaluateAll(items => items.map(item => Number(item.dataset.positionEntry))), [71, 65, 59, 53, 47, 41, 35, 29], 'the second source remains independently selectable');
}

async function holdWrites(page, interaction) {
  let release, seen = false;
  const gate = new Promise(resolve => { release = resolve; });
  // Start the bounded wait when the interaction asks for it, after its click.
  // A raw Promise here is unaffected by page.setDefaultTimeout().
  const captured = () => expect.poll(() => seen, {
    message: 'the interaction must issue an intercepted API write', timeout: 15000,
  }).toBe(true);
  const pending = [];
  const handler = async route => {
    if (route.request().method() === 'GET') return route.continue();
    seen = true;
    const continued = gate.then(() => route.continue());
    pending.push(continued);
    await continued;
  };
  await page.route('**/api/v1/**', handler);
  try { await interaction(captured, release); }
  finally { release(); await Promise.all(pending); await page.unroute('**/api/v1/**', handler); }
}

async function pendingWrites(page) {
  await seed(page, new URL(page.url()).origin);
  const first = page.locator(`${rows}[data-position-entry="72"]`);
  await first.locator(action('select-entry')).check();
  await holdWrites(page, async captured => {
    await first.locator(action('mark-read')).click();
    await captured();
    assert.equal(await first.locator(action('mark-read')).isDisabled(), true, 'the pending row rejects repeated writes');
    assert.equal(await first.locator(action('toggle-starred')).isDisabled(), true);
    assert.equal(await page.locator(`${rows}[data-position-entry="71"] ${action('mark-read')}`).isDisabled(), false, 'other rows remain usable');
    assert.equal(await page.locator(action('mark-selected-read')).isDisabled(), true, 'batch waits for outstanding row writes');
    assert.equal(await page.locator(action('archive-selected')).isDisabled(), true);
  });
  await page.waitForFunction(() => document.querySelector('[data-position-entry="72"]').dataset.read === 'true' && !document.querySelector('[data-action="mark-selected-read"]').disabled);
  await first.locator(action('mark-read')).click();
  await page.waitForFunction(() => document.querySelector('[data-position-entry="72"]').dataset.read === 'false' && !document.querySelector('[data-position-entry="72"] [data-action="mark-read"]').disabled);
  await page.locator(`${rows}[data-position-entry="71"] ${action('select-entry')}`).check();
  await holdWrites(page, async captured => {
    await page.locator(action('archive-selected')).click();
    await captured();
    assert.equal(await page.locator(`${rows} [data-layout="entry-card-actions"] button`).evaluateAll(buttons => buttons.every(button => button.disabled)), true, 'batch writes block overlapping per-row actions');
  });
  await page.waitForFunction(() => !document.querySelector('[data-position-entry="72"]') && !document.querySelector('[data-position-entry="71"]'));
  await seed(page, new URL(page.url()).origin);
  const refuse = route => route.fulfill({status:503, contentType:'application/json',
    body:JSON.stringify({code:'unavailable',message:'review write failure'})});
  await page.route('**/api/v1/entries/72/read', refuse);
  try {
    await first.locator(action('mark-read')).click();
    await page.locator('[data-layout="status-banner"][data-state="error"]').waitFor({ state: 'attached' });
    assert.equal(await first.locator(action('mark-read')).isDisabled(), false, 'a failed write unlocks retry');
    assert.equal(await first.getAttribute('data-read'), 'false', 'a failed write preserves flags');
  } finally {
    await page.unroute('**/api/v1/entries/72/read', refuse);
  }
  await first.locator(action('mark-read')).click();
  await page.waitForFunction(() => document.querySelector('[data-position-entry="72"]').dataset.read === 'true');
}

async function preferenceOrdering(page) {
  await seed(page, new URL(page.url()).origin);
  for (const labels of [['仅未读', '仅未读'], ['来源', '时间']]) {
    await page.evaluate(installWorkspaceResponseProbe);
    const before = await page.getByRole('button', { name: '仅未读', exact: true }).getAttribute('aria-pressed');
    console.log(`preferenceOrdering: ${labels.join(' -> ')} start`);
    let afterClicks;
    let finalSave;
    const finishedSaves = new Set();
    const requestIds = new Map();
    const events = [];
    const started = Date.now();
    const requestId = request => {
      if (!requestIds.has(request)) requestIds.set(request, requestIds.size + 1);
      return requestIds.get(request);
    };
    const record = (event, request, detail = {}) => {
      if (request.method() !== 'PUT' || new URL(request.url()).pathname !== '/api/v1/workspace') return;
      events.push({ event, id: requestId(request), elapsedMs: Date.now() - started,
        workspace: request.postDataJSON(), ...detail });
    };
    const observeRequest = request => record('request', request);
    const observeSave = response => {
      const request = response.request();
      if (request.method() !== 'PUT' || new URL(request.url()).pathname !== '/api/v1/workspace') return;
      record('response', request, { status: response.status() });
      const workspace = request.postDataJSON();
      if (workspace.read_filter === 'all' && workspace.grouping_mode === 'time') finalSave = response;
    };
    const observeFinished = request => {
      finishedSaves.add(request);
      record('finished', request);
    };
    const observeFailed = request => record('failed', request, { failure: request.failure() });
    page.on('request', observeRequest);
    page.on('response', observeSave);
    page.on('requestfinished', observeFinished);
    page.on('requestfailed', observeFailed);
    try {
      await holdWrites(page, async captured => {
        await page.getByRole('button', { name: labels[0], exact: true }).click();
        await captured();
        console.log('preferenceOrdering: first write intercepted');
        await page.getByRole('button', { name: labels[1], exact: true }).click();
        afterClicks = await page.getByRole('button', { name: '仅未读', exact: true }).getAttribute('aria-pressed');
        assert.equal(afterClicks, 'false', 'the visible filter must already reflect the final All choice');
        // The initial stored value already equals the final choice while the
        // first write is blocked. Polling that value alone cannot prove success.
        const initial = await page.evaluate(async () => {
          const input = encodeURIComponent(JSON.stringify({load_preferences:true,load_feeds:false}));
          return (await (await fetch('/api/v1/entries/bootstrap?input=' + input, {
            signal: AbortSignal.timeout(15000),
          })).json()).workspace;
        });
        assert.equal(initial.read_filter, 'all');
        assert.equal(initial.grouping_mode, 'time');
        assert.equal(finalSave, undefined, 'no final save can complete while writes are held');
      });
      // Continuing the held route only sends it. Wait for the queued final PUT
      // to finish before reloading can cancel the document's remaining tasks.
      await expect.poll(() => Boolean(finalSave), { message: 'the final workspace save must respond', timeout: 15000 }).toBe(true);
      assert.ok(finalSave.ok(), `the final workspace save failed: ${finalSave.status()}`);
      await expect.poll(() => finishedSaves.has(finalSave.request()), {
        message: 'the final workspace response must finish', timeout: 15000,
      }).toBe(true);
      console.log('preferenceOrdering: final save finished');
      console.log('preferenceOrdering response reads:', JSON.stringify(
        await page.evaluate(() => window.__workspaceResponseProbe.records)));
    } catch (error) {
      // Preserve the request lifecycle when CI receives headers but never reports
      // completion. Observe only: do not retry, consume a response, or weaken the wait.
      console.error('preferenceOrdering request events:', JSON.stringify({ labels,
        finalSaveRequestId: finalSave ? requestId(finalSave.request()) : null, events,
        responseReads: await page.evaluate(() => window.__workspaceResponseProbe.records) }));
      throw error;
    } finally {
      page.off('request', observeRequest);
      page.off('response', observeSave);
      page.off('requestfinished', observeFinished);
      page.off('requestfailed', observeFailed);
      await page.evaluate(() => window.__workspaceResponseProbe.dispose());
    }
    // Read the persisted server preference before unloading page tasks.
    await page.waitForFunction(async () => {
      const input = encodeURIComponent(JSON.stringify({load_preferences:true,load_feeds:false}));
      const {workspace} = await (await fetch('/api/v1/entries/bootstrap?input=' + input)).json();
      return workspace?.read_filter === 'all' && workspace.grouping_mode === 'time';
    });
    await page.reload(); await ready(page);
    assert.equal(await page.getByRole('button', { name: '仅未读', exact: true }).getAttribute('aria-pressed'), 'false', `rapidly clearing a filter must persist the final All choice (${labels}; before=${before}; afterClicks=${afterClicks})`);
    assert.equal(await page.locator(action('group-by-time')).getAttribute('aria-pressed'), 'true', 'rapid grouping changes must persist the final Time choice');
  }
}

async function shortSourceDialog(page) {
  for (const [width, height] of [[760, 400], [360, 480]]) {
    await page.setViewportSize({ width, height });
    await seed(page, new URL(page.url()).origin);
    await page.locator(action('open-source-filter')).click();
    const chooser = page.locator('#source-filter-dialog');
    const verifyActions = async () => {
      for (const name of ['取消', '应用']) {
        const box = await chooser.getByRole('button', { name, exact: true }).boundingBox();
        assert.ok(box && box.y >= 0 && box.y + box.height <= height - 10, `${width}x${height}: ${name} must be fully visible without scrolling the dialog`);
      }
      assert.equal(await chooser.evaluate(el => el.scrollHeight > el.clientHeight + 1), false, 'only the source list should need vertical scrolling');
    };
    await verifyActions();
    await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
    await chooser.getByRole('checkbox', { name: 'Foxtrot', exact: true }).check();
    await verifyActions();
    await chooser.getByRole('button', { name: '应用', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('.entries-count').textContent === '12 篇');
    assert.equal(await page.locator(`${rows} [data-slot="entry-card-meta"]`).evaluateAll(items => items.every(el => el.textContent.startsWith('Foxtrot · '))), true);
  }
}

// Hold a real preference write across navigation. The queued final choice must
// finish before a feed URL edit migrates the selected source in the same library.
async function preferencesSurviveNavigation(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  await seed(page, new URL(page.url()).origin);
  let release, seen;
  const gate = new Promise(resolve => { release = resolve; });
  const captured = new Promise(resolve => { seen = resolve; });
  const pending = [];
  const handler = route => {
    seen();
    const continued = gate.then(() => route.continue());
    pending.push(continued);
    return continued;
  };
  await page.route('**/api/v1/workspace', handler);
  try {
    await page.locator(action('open-source-filter')).click();
    const chooser = page.locator('#source-filter-dialog');
    await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
    await chooser.getByRole('checkbox', { name: 'Alpha', exact: true }).check();
    await page.locator(action('apply-source-filter')).click();
    await captured;
    await page.getByRole('button', { name: '仅未读', exact: true }).click();
    await expect(page.getByRole('button', { name: '仅未读', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await page.locator('[data-nav="feeds"]').first().click();
    const feed = page.locator('[data-feed-id="1"]');
    await feed.waitFor();
    const editedURL = `${await feed.getAttribute('data-feed-url')}?navigation=1`;
    await feed.locator(action('edit-feed')).click();
    await page.locator('#feed-edit-url').fill(editedURL);
    await page.locator(action('save-feed-edit')).click();
    release();
    await Promise.all(pending);
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
    await page.locator('[data-action="activate-home"]').first().click();
    await ready(page);
    await expect(page.locator('.source-filter-trigger span')).toHaveText('…');
    await expect(page.getByRole('button', { name: '仅未读', exact: true })).toHaveAttribute('aria-pressed', 'true');
    const stored = await page.evaluate(async () => {
      const input = encodeURIComponent(JSON.stringify({load_preferences:true,load_feeds:false}));
      return (await (await fetch('/api/v1/entries/bootstrap?input=' + input)).json()).workspace;
    });
    assert.deepEqual(stored.selected_feed_urls, [editedURL], 'the final source choice is migrated after pending saves finish');
    assert.equal(stored.read_filter, 'unread_only', 'the queued choice survives leaving its page');
  } finally {
    release();
    await Promise.all(pending);
    await page.unroute('**/api/v1/workspace', handler);
  }
}

async function reopeningDuringSourceEdit(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  await seed(page, new URL(page.url()).origin);
  await page.locator(action('open-source-filter')).click();
  const chooser = page.locator('#source-filter-dialog');
  await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
  await chooser.getByRole('checkbox', { name: 'Alpha', exact: true }).check();
  const selectedSource = page.waitForResponse(response => response.url().endsWith('/api/v1/workspace') && response.request().method() === 'PUT');
  await page.locator(action('apply-source-filter')).click();
  assert.equal((await selectedSource).ok(), true);
  await page.locator('[data-nav="feeds"]').first().click();
  const feed = page.locator('[data-feed-id="1"]');
  await feed.waitFor();
  const editedURL = `${await feed.getAttribute('data-feed-url')}?return-during-save=1`;
  await feed.locator(action('edit-feed')).click();
  await page.locator('#feed-edit-url').fill(editedURL);
  await holdWrites(page, async (captured, release) => {
    await page.locator(action('save-feed-edit')).click();
    await captured();
    // Browser history can leave a native modal even while its save is pending.
    await page.goBack();
    await page.locator('[data-page="entries"]').waitFor();
    assert.equal(await page.locator('[data-page="entries"]').getAttribute('data-position-ready'), 'false', 'new page waits for the source edit before loading its preferences');
    release();
    await ready(page);
  });
  await expect(page.locator('.entries-count')).toHaveText('12 篇');
  const savedFilter = page.waitForResponse(response => response.url().endsWith('/api/v1/workspace') && response.request().method() === 'PUT');
  await page.getByRole('button', { name: '仅未读', exact: true }).click();
  assert.equal((await savedFilter).ok(), true);
  const stored = await page.evaluate(async () => {
    const input = encodeURIComponent(JSON.stringify({load_preferences:true,load_feeds:false}));
    return (await (await fetch('/api/v1/entries/bootstrap?input=' + input)).json()).workspace;
  });
  assert.deepEqual(stored.selected_feed_urls, [editedURL], 'the reopened page must not write the old URL back');
  assert.equal(stored.read_filter, 'unread_only');
}

async function articleWritesSurviveNavigation(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  for (const command of ['mark-read', 'toggle-starred', 'mark-selected-read', 'archive-selected']) {
    await seed(page, new URL(page.url()).origin);
    const countBefore = Number.parseInt(await page.locator('.entries-count').innerText(), 10);
    const first = page.locator(`${rows}[data-position-entry="72"]`);
    const bulk = ['mark-selected-read', 'archive-selected'].includes(command);
    if (bulk) await first.locator(action('select-entry')).check();
    const saved = page.waitForResponse(response => response.url().includes('/api/v1/entries/') && response.request().method() !== 'GET');
    await holdWrites(page, async captured => {
      await (bulk ? page.locator(action(command)) : first.locator(action(command))).click();
      await captured();
      await page.locator('[data-nav="feeds"]').first().click();
      await page.locator('[data-feed-id="1"]').waitFor();
    });
    assert.equal((await saved).ok(), true, `${command}: navigation does not abort an accepted write`);
    await page.locator(action('activate-home')).first().click();
    await ready(page);
    if (command === 'archive-selected') {
      await expect(first).toHaveCount(0);
      await expect(page.locator('.entries-count')).toHaveText(`${countBefore - 1} 篇`);
    } else if (command === 'toggle-starred') {
      await expect(first.locator(action('toggle-starred'))).toHaveAttribute('aria-pressed', 'true');
    } else {
      await expect(first).toHaveAttribute('data-read', 'true');
      assert.equal(await page.locator(`${rows}[data-read="true"]`).count(), 1, 'only the accepted row changes');
    }
  }
}

async function preferencesRecoveredAfterArticleWrites(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  for (const command of ['mark-read', 'mark-selected-read']) {
    let unavailable = true;
    const preferenceWrites = [];
    const track = request => {
      if (new URL(request.url()).pathname === '/api/v1/workspace' && request.method() !== 'GET') {
        preferenceWrites.push(request.postData());
      }
    };
    const pattern = '**/api/v1/entries/bootstrap?*';
    const handler = route => unavailable
      ? route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ code: 'unavailable', message: 'initial preferences unavailable' }) })
      : route.continue();
    page.on('request', track);
    await page.route(pattern, handler);
    try {
      await seed(page, fixture.origin);
      assert.ok(await page.locator(rows).count() > 8, 'initial read uses fallback page size');
      const unread = page.getByRole('button', { name: '仅未读', exact: true });
      await unread.click();
      await expect(unread).toHaveAttribute('aria-pressed', 'true');
      const first = page.locator(`${rows}[data-position-entry="72"]`);
      if (command === 'mark-selected-read') await first.locator(action('select-entry')).check();
      unavailable = false;
      const recovered = page.waitForResponse(response => response.url().includes('/api/v1/entries/bootstrap?') && response.ok());
      await (command === 'mark-read' ? first.locator(action(command)) : page.locator(action(command))).click();
      await recovered;
      // Applying the recovered page size proves the reply reached Rust state;
      // checking the button immediately after HTTP completion could pass too early.
      await expect(page.locator(rows)).toHaveCount(8);
      await expect(unread).toHaveAttribute('aria-pressed', 'true');
      await expect(first).toHaveCount(0);
      assert.deepEqual(preferenceWrites, [], 'recovery keeps the choice without saving it');
    } finally {
      await page.unroute(pattern, handler);
      page.off('request', track);
    }
  }
}

async function readerWritesSurviveNavigation(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  for (const command of ['mark-read', 'toggle-starred']) {
    await seed(page, fixture.origin);
    const first = page.locator(`${rows}[data-position-entry="72"]`);
    await first.locator('[data-slot="entry-card-title"]').click();
    await page.locator('[data-page="reader"][data-position-ready="true"]').waitFor();
    const saved = page.waitForResponse(response => /\/api\/v1\/entries\/72\/(read|starred)$/.test(new URL(response.url()).pathname) && response.request().method() !== 'GET')
      .then(response => ({ response }), error => ({ error }));
    await holdWrites(page, async captured => {
      await page.locator(`[data-layout="reader-bottom-bar"] ${action(command)}`).click();
      await captured();
      await page.locator('[data-nav="back"]').click();
      await ready(page);
    });
    const outcome = await saved;
    assert.ifError(outcome.error);
    assert.equal(outcome.response.ok(), true, `${command}: returning to the list keeps the reader write`);
    if (command === 'mark-read') await expect(first).toHaveAttribute('data-read', 'true');
    else await expect(first.locator(action('toggle-starred'))).toHaveAttribute('aria-pressed', 'true');
  }
}

async function dialogComposition(page) {
  await page.setViewportSize({ width: 1536, height: 864 });
  await seed(page, new URL(page.url()).origin);
  const cdp = await page.context().newCDPSession(page);
  const composingEscape = async (input, dialog) => {
    await input.focus();
    await input.evaluate(el => {
      window.__workspaceEscapeComposing = null;
      el.addEventListener('keydown', event => {
        window.__workspaceEscapeComposing = event.isComposing;
      }, { capture: true, once: true });
    });
    // Chromium's real composition state, followed by ordinary keyboard input;
    // not a synthetic key event or a direct call to the dialog's handler.
    await cdp.send('Input.imeSetComposition', { text: 'zhong', selectionStart: 5, selectionEnd: 5 });
    await page.keyboard.press('Escape');
    assert.equal(await page.evaluate(() => window.__workspaceEscapeComposing), true, 'the Escape event must be delivered during composition');
    assert.equal(await dialog.count(), 1, 'composing Escape must not discard the dialog draft');
    assert.equal(await dialog.evaluate(el => el.open), true, 'the native dialog must stay open too');
    await cdp.send('Input.imeSetComposition', { text: '', selectionStart: 0, selectionEnd: 0 });
  };
  try {
    await page.locator('[data-nav="feeds"]').first().click();
    const feed = page.locator('[data-feed-id="1"]');
    await feed.locator(action('edit-feed')).click();
    await page.locator('#feed-edit-name').fill('尚未保存的名称');
    await page.locator('#feed-edit-url').fill('https://edited.example/feed.xml');
    await composingEscape(page.locator('#feed-edit-name'), page.locator('#feed-edit-dialog'));
    assert.equal(await page.locator('#feed-edit-name').inputValue(), '尚未保存的名称');
    assert.equal(await page.locator('#feed-edit-url').inputValue(), 'https://edited.example/feed.xml');
    await page.locator(action('save-feed-edit')).click();
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });
    await page.waitForFunction(() => document.querySelector('[data-feed-id="1"]').dataset.feedTitle === '尚未保存的名称');
    assert.equal(await feed.getAttribute('data-feed-url'), 'https://edited.example/feed.xml');
    await feed.locator(action('edit-feed')).click();
    await page.locator('#feed-edit-name').waitFor();
    await page.keyboard.press('Escape');
    await page.locator('#feed-edit-dialog').waitFor({ state: 'detached' });

    await page.locator(action('activate-home')).first().click(); await ready(page);
    await page.locator(action('select-entry')).first().check();
    await page.locator(action('open-source-filter')).click();
    const chooser = page.locator('#source-filter-dialog');
    await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).uncheck();
    await chooser.getByRole('checkbox', { name: 'Bravo', exact: true }).check();
    await composingEscape(chooser.getByRole('searchbox', { name: '搜索来源' }), chooser);
    assert.equal(await chooser.getByRole('checkbox', { name: 'Bravo', exact: true }).isChecked(), true, 'source draft survives composing Escape');
    assert.equal(await chooser.getByRole('checkbox', { name: '全部来源', exact: true }).isChecked(), false);
    await page.keyboard.press('Escape');
    await chooser.waitFor({ state: 'detached' });
    assert.equal(await page.locator(selected).count(), 1, 'canceling the source draft preserves article selection');
    assert.equal(await page.locator('.source-filter-trigger').innerText(), '全部来源', 'canceling does not apply the source draft');
  } finally {
    await cdp.detach();
  }
}

module.exports = { seed, selection, sources, editing, responsive, editEdges, queryStability, importNames, sourceIdentity, pendingWrites, preferenceOrdering, shortSourceDialog, dialogComposition, preferencesSurviveNavigation, reopeningDuringSourceEdit, articleWritesSurviveNavigation, preferencesRecoveredAfterArticleWrites, readerWritesSurviveNavigation, toolbarThemes };
if (require.main === module) (async () => {
  const { chromium } = require('playwright');
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
  try {
    const page = await browser.newPage({ viewport: { width: 1536, height: 864 } });
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    page.setDefaultTimeout(15000);
    await fixture.install(page.context(), require('node:path').resolve(__dirname,'../..'));
    await seed(page, fixture.origin);
    for (const [name, run] of Object.entries({ selection, sources, editing, responsive, editEdges, queryStability, importNames, sourceIdentity, pendingWrites, preferenceOrdering, shortSourceDialog, dialogComposition, preferencesSurviveNavigation, reopeningDuringSourceEdit, articleWritesSurviveNavigation, preferencesRecoveredAfterArticleWrites, readerWritesSurviveNavigation, toolbarThemes })) {
      try { console.log(`START ${name}`); await run(page); console.log(`PASS ${name}`); }
      catch (error) {
        const fs = require('node:fs/promises'), path = require('node:path');
        const output = path.resolve(process.env.ARTIFACT_DIR || 'target/workspace-server-regression');
        await fs.mkdir(output, {recursive:true});
        await fs.writeFile(path.join(output, `failure-${name}.html`), await page.content());
        await page.screenshot({path:path.join(output, `failure-${name}.png`)});
        throw error;
      }
    }
    assert.deepEqual(errors, []);
    console.log('PASS no page errors');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
