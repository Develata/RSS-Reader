// Run against scripts/run_web_spa_regression_server.sh with an installed Playwright.
// NODE_PATH may point to the existing Playwright installation; no product dependency is added.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const base = process.env.STATIC_BASE || 'http://127.0.0.1:8097';
const artifacts = process.env.ARTIFACT_DIR || 'target/task1-validation/browser';
const results = [];
const check = (name, data = {}) => { results.push({ name, ...data }); console.log(name); };
(async () => {
  await fs.mkdir(artifacts, { recursive: true });
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
  try {
    const context = await browser.newContext({ viewport: { width: 360, height: 800 }, timezoneId: 'America/New_York' });
    const page = await context.newPage();
    page.setDefaultTimeout(15000);
    await page.goto(`${base}/__codex/setup-local-auth?seed=reader-demo&next=/entries/2`);
    await page.locator('[data-slot="reader-title"]').filter({ hasText: 'Demo Entry Two' }).waitFor();
    const long = '长来源LongUnbrokenSource'.repeat(25);
    await page.evaluate(({long}) => {
      const key='rssr-web-state-v1', state=JSON.parse(localStorage.getItem(key));
      state.settings.archive_after_months=0;
      state.settings.refresh_interval_minutes=10080;
      state.feeds[0].title=long;
      state.feeds[0].last_success_at='2026-03-08 07:00:00.0 +00:00:00';
      state.entries[0].published_at='2026-03-08 06:59:59.0 +00:00:00';
      state.entries[1].published_at='2026-03-08 07:00:00.0 +00:00:00';
      state.entries[1].author='长作者Author'.repeat(35);
      state.entries[1].url='https://example.com/'+ 'long-url'.repeat(80);
      localStorage.setItem(key,JSON.stringify(state));
    }, {long});
    await page.reload();
    await page.getByText('发布时间：2026-03-08 03:00 UTC-04:00', {exact:true}).waitFor();
    assert.equal(await page.locator('[data-slot="reader-meta"]').first().textContent(), `来源：${long}`);
    assert.match(await page.locator('[data-slot="reader-meta"]').nth(1).textContent(), /^作者：长作者/);
    check('直接进入阅读页独立加载来源和作者；DST 后目标时间偏移');
    for (const width of [360,1280]) {
      await page.setViewportSize({width,height:800});
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth), 'reader overflow');
      assert((await page.locator('[data-action="open-original"]').boundingBox()).height>=44);
      await page.screenshot({path:`${artifacts}/reader-${width}.png`,fullPage:true});
    }
    check('360×800 / 1280×800 长来源、作者、URL 无横向溢出；原文入口高度至少 44px');
    const original = page.url();
    const popupPromise=context.waitForEvent('page');
    await page.locator('[data-action="open-original"]').click();
    const popup=await popupPromise;
    await popup.waitForLoadState('domcontentloaded').catch(()=>{});
    assert.equal(await popup.evaluate(()=>window.opener),null);
    assert.equal(page.url(),original);
    await popup.close();
    check('真实点击打开 Web 新标签，opener 隔离，阅读页保留');
    await page.locator('[data-action="mark-read"]').click();
    await page.locator('[data-action="mark-read"][data-state="read"]').waitFor();
    await page.locator('[data-action="activate-home"]').click();
    async function showFilters() {
      const count=page.locator('[data-slot="entry-filters-source-unread-count"]').first();
      if (!await count.isVisible()) await page.locator('[data-action="show-entry-controls"]').click();
      await count.waitFor();
      return count;
    }
    let count=await showFilters();
    await page.waitForFunction(()=>document.querySelector('[data-slot="entry-filters-source-unread-count"]')?.textContent.trim()==='· 0');
    assert.match(await page.locator('[data-field="entry-source-filter"]').first().getAttribute('aria-describedby'),/^source-unread-/);
    for (const width of [360,1280]) {
      await page.setViewportSize({width,height:800});
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth), 'source overflow');
      assert((await page.locator('[data-layout="entry-filters-source-chip"]').first().boundingBox()).height>=44);
      await page.screenshot({path:`${artifacts}/sources-${width}.png`,fullPage:true});
    }
    check('阅读页标已读后返回列表计数为 0，来源长文本无溢出，辅助技术通过描述获得未读篇数');
    await page.locator('[data-action="mark-read"]').first().click();
    await page.waitForFunction(()=>document.querySelector('[data-slot="entry-filters-source-unread-count"]')?.textContent.trim()==='· 1');
    await page.locator('[data-field="search-title"]').fill('does not match');
    await page.waitForTimeout(400);
    assert.equal((await count.textContent()).trim(),'· 1');
    await page.locator('[data-field="search-title"]').fill('');
    await page.locator('[data-field="show-archived"]').click();
    assert.equal((await count.textContent()).trim(),'· 1');
    check('列表标未读更新权威计数，搜索及归档筛选不改变计数');
    await page.evaluate(()=>{globalThis.savedSetItem=Storage.prototype.setItem;Storage.prototype.setItem=function(key,value){if(key==='rssr-web-entry-flags-v1')throw new DOMException('test quota','QuotaExceededError');return savedSetItem.call(this,key,value);};});
    const unreadButton=page.locator('[data-action="mark-read"]').filter({hasText:'标已读'}).first();
    await unreadButton.click();
    await page.locator('[data-state="error"]').first().waitFor();
    assert.equal((await count.textContent()).trim(),'· 1');
    await page.evaluate(()=>{Storage.prototype.setItem=globalThis.savedSetItem;delete globalThis.savedSetItem;});
    await page.locator('[data-nav="feeds"]').click();
    await page.getByText('本地文章 2 · 未读 1',{exact:true}).waitFor();
    await page.getByText(/最近成功：2026-03-08 03:00 UTC-04:00/).waitFor();
    check('存储失败后列表与订阅页计数一致；订阅刷新时间跟随本地偏移');
    await page.goto(`${base}/entries/1`);
    await page.getByText('发布时间：2026-03-08 01:59 UTC-05:00',{exact:true}).waitFor();
    check('同一会话展示 DST 前后两个不同偏移');
    // Re-seed missing metadata in storage, reload the app to exercise direct reader loading.
    await page.evaluate(()=>{
      const key='rssr-web-state-v1',state=JSON.parse(localStorage.getItem(key));
      state.feeds[0].title=null;
      state.entries[0].author='   ';state.entries[0].published_at=null;state.entries[0].url=null;
      localStorage.setItem(key,JSON.stringify(state));
    });
    await page.reload();
    await page.getByText('来源：https://example.com/feed.xml',{exact:true}).waitFor();
    await page.getByText('发布时间：未知发布时间',{exact:true}).waitFor();
    assert.equal(await page.locator('[data-action="open-original"]').count(),0);
    assert.equal(await page.locator('[data-slot="reader-meta"]').count(),2);
    check('缺标题回退订阅 URL；空作者和缺原文隐藏；缺时间显示未知');
    // An orphan can still be read when feed metadata is not obtainable.
    await page.evaluate(()=>{const key='rssr-web-state-v1',state=JSON.parse(localStorage.getItem(key));state.feeds=[];localStorage.setItem(key,JSON.stringify(state));});
    await page.reload();
    await page.getByText('来源：未知来源',{exact:true}).waitFor();
    await page.getByText('Demo Entry One body.',{exact:true}).waitFor();
    check('订阅不可取得时显示未知来源，正文仍可阅读');
    await context.close();
    for (const [timezoneId, date, full, fail] of [
      ['America/New_York','2026-02-28','2026-02-28 19:15 UTC-05:00',false],
      ['Asia/Kathmandu','2026-03-01','2026-03-01 06:00 UTC+05:45',false],
      ['Pacific/Chatham','2026-03-01','2026-03-01 14:00 UTC+13:45',false],
      ['America/New_York','2026-03-01','2026-03-01 00:15 UTC',true],
    ]) {
      const ctx=await browser.newContext({viewport:{width:360,height:800},timezoneId});
      if(fail) await ctx.addInitScript(()=>{Date.prototype.getTimezoneOffset=()=>NaN;});
      const pg=await ctx.newPage();pg.setDefaultTimeout(15000);
      await pg.goto(`${base}/__codex/setup-local-auth?seed=reader-demo&next=/entries/2`);
      await pg.getByText('Demo Entry Two',{exact:true}).waitFor();
      await pg.evaluate(()=>{
        const key='rssr-web-state-v1',state=JSON.parse(localStorage.getItem(key));
        state.settings.archive_after_months=0;
        for(const entry of state.entries)entry.published_at='2026-03-01 00:15:00.0 +00:00:00';
        localStorage.setItem(key,JSON.stringify(state));
      });
      await pg.reload();
      await pg.getByText(`发布时间：${full}`,{exact:true}).waitFor();
      await pg.locator('[data-action="activate-home"]').click();
      await pg.locator('[data-slot="entry-card-meta"]').first().waitFor();
      assert((await pg.locator('[data-slot="entry-card-meta"]').allTextContents()).every(text=>text.includes(date)));
      assert((await pg.locator('[data-slot="entry-group-title"]').allTextContents()).some(text=>text.includes(date)), 'date bucket agrees with card');
      check(`卡片与日期分桶一致：${timezoneId}, ${full}, UTC fallback=${fail}`);
      await ctx.close();
    }
  } finally {
    for (const ctx of browser.contexts()) for (const pg of ctx.pages()) {
      await fs.writeFile(`${artifacts}/last-page.html`,await pg.content()).catch(()=>{});
      await pg.screenshot({path:`${artifacts}/last-page.png`,fullPage:true}).catch(()=>{});
    }
    await browser.close();
    await fs.writeFile(`${artifacts}/results.json`,JSON.stringify(results,null,2));
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
