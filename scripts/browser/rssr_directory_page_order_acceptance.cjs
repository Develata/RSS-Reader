// A full-day directory must follow the reading order of each page, including
// source boundaries. Inspect every frame: settled-state checks miss visible jumps.
const {chromium} = require('playwright');
const fs = require('node:fs/promises');
const path = require('node:path');
const fixture = require('./directory_fixture.cjs');

(async () => {
  const output = path.resolve(process.env.ARTIFACT_DIR || 'target/directory-page-order');
  await fs.mkdir(output, {recursive:true});
  const browser = await chromium.launch({executablePath:process.env.CHROME_BIN,
    channel:process.env.CHROME_BIN ? undefined : 'chrome', headless:true});
  const context = await browser.newContext();
  await fixture.install(context, path.resolve(__dirname, '../..'));
  const page = await context.newPage(), results = [], errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const check = (name, pass, details) => {
    results.push({name, pass:!!pass, details});
    if (!pass) throw new Error(name + ': ' + JSON.stringify(details));
  };
  const rail = '[data-directory-consumer="rail"]';
  const idle = () => page.evaluate(() => window.__rssrEntryDirectoryTracker.whenSettled());
  const startRecording = () => page.evaluate(() => {
    cancelAnimationFrame(window.__directoryFrame);
    window.__directoryFrames = [];
    const sample = () => {
      const rail = document.querySelector('[data-directory-consumer="rail"]');
      window.__directoryFrames.push({at:performance.now(),mainY:scrollY,top:rail.scrollTop,
        leaf:rail.dataset.directoryCurrentLeaf});
      window.__directoryFrame = requestAnimationFrame(sample);
    };
    sample();
  });
  const finishRecording = () => page.evaluate(() => {
    cancelAnimationFrame(window.__directoryFrame);
    return window.__directoryFrames;
  });
  const state = () => page.locator(rail).evaluate(e => {
    const header = e.querySelector('.directory-heading-row').getBoundingClientRect();
    const current = e.querySelector('[data-directory-level="article"][data-active="true"]');
    const row = current?.getBoundingClientRect(), box = e.getBoundingClientRect();
    return {top:e.scrollTop, headerY:header.y, leaf:e.dataset.directoryCurrentLeaf,
      mode:e.dataset.directoryMode, index:current?.dataset.virtualIndex,
      visible:!!row && row.top >= header.bottom - 1 && row.bottom <= box.bottom + 1,
      mounted:e.querySelectorAll('[data-virtual-index]').length,
      count:Number(e.querySelector('[data-virtual-count]')?.dataset.virtualCount),
      selected:document.querySelectorAll('[data-action="select-entry"]:checked').length};
  });
  try {
    for (const [width, height, theme] of [[1278,894,'light'], [1278,894,'dark'], [760,960,'light'], [390,844,'dark']]) {
      const label = `${width}-${theme}`;
      await page.setViewportSize({width,height});
      await page.goto(fixture.origin + '/__codex/setup-local-auth?seed=directory-page-order&theme=' + theme);
      await page.locator('[data-position-entry]').first().waitFor();
      await page.locator('[data-action="select-entry"]').first().check();
      if (width <= 1000) await page.locator('[data-action="open-directory"]').click();
      await idle();
      await page.locator('#entry-card-5000').evaluate(e => window.scrollTo(0, scrollY + e.getBoundingClientRect().top - 116));
      await idle();
      const before = await state();
      await startRecording();
      await page.mouse.move(width <= 1000 ? 60 : 600, 450);
      const observations = [];
      for (let i = 0; i < 12; i++) {
        const down = i % 2 === 0;
        await page.mouse.wheel(0, down ? 40 : -40);
        const expected = down ? 'entry-card-5000' : 'entry-card-4951';
        await page.waitForFunction(expected => document.querySelector('[data-directory-consumer="rail"]').dataset.directoryCurrentLeaf === expected, expected);
        await idle();
        observations.push(await state());
      }
      const frames = await finishRecording();
      await fs.writeFile(path.join(output, label + '-frames.json'), JSON.stringify({before,observations,frames},null,2));
      await page.screenshot({path:path.join(output, label + '.png')});
      const maxJump = Math.max(0, ...frames.slice(1).map((frame, i) => Math.abs(frame.top - frames[i].top)));
      check(label + ' source-boundary scroll has no large directory jump', maxJump <= 100, {maxJump});
      check(label + ' adjacent main rows remain adjacent in the directory', observations.every((s,i) => Number(s.index) === (i % 2 === 0 ? 9 : 8)), observations);
      check(label + ' follow keeps the target below the fixed heading', observations.every(s => s.visible && s.mode === 'follow' && Math.abs(s.headerY-before.headerY)<1), observations);
      check(label + ' full directory stays virtualized and selection remains', observations.every(s => s.count>1000 && s.mounted>0 && s.mounted<60 && s.selected===1), observations);
      check(label + ' page identity and responsive layout', await page.evaluate(() => location.pathname==='/entries' && document.title==='RSS-Reader' && document.querySelectorAll('[data-position-entry]').length===50 && document.documentElement.scrollWidth<=innerWidth), await page.url());
      // Traverse beyond the initially mounted window, then reverse. This covers
      // measurement/reveal/alignment feedback, not just two already visible rows.
      const walk = [];
      await startRecording();
      let last = 8;
      for (const direction of [1, -1]) {
        for (let i = direction > 0 ? 9 : last - 1; i >= 0 && i < 50; i += direction) {
          const target = await page.locator('[data-position-entry]').nth(i).evaluate(e => {
            const delta = e.getBoundingClientRect().top - 95;
            return {anchor:e.id,delta,reachable:scrollY + delta <= document.documentElement.scrollHeight-innerHeight};
          });
          if (!target.reachable) break;
          await page.mouse.wheel(0,target.delta);
          await page.waitForFunction(anchor => document.querySelector('[data-directory-consumer="rail"]').dataset.directoryCurrentLeaf===anchor,target.anchor);
          await idle();
          walk.push({expected:i,...await state()});
          if (direction>0) last=i;
        }
      }
      const walkFrames = await finishRecording();
      await fs.writeFile(path.join(output,label+'-walk.json'),JSON.stringify({walk,frames:walkFrames},null,2));
      const walkJump = Math.max(0,...walkFrames.slice(1).map((f,i)=>Math.abs(f.top-walkFrames[i].top)));
      check(label + ' following across virtual windows settles without long jumps',
        last>30 && walkJump<=150 && walk.every(s=>Number(s.index)===s.expected && s.visible && s.mode==='follow' && s.selected===1 && s.mounted<60),
        {last,steps:walk.length,maxJump:walkJump});
      if (width === 1278 && theme === 'light') {
        const target = page.locator(rail + ' [data-directory-anchor="entry-card-5000"]');
        await target.focus();
        await page.keyboard.press('End');
        await page.waitForFunction(() => document.activeElement?.dataset.virtualIndex === String(Number(document.activeElement?.parentElement?.dataset.virtualCount)-1));
        await idle();
        const destination = await page.evaluate(() => ({page:document.activeElement.dataset.directoryPage,anchor:document.activeElement.dataset.directoryAnchor}));
        await page.keyboard.press('Enter');
        await page.waitForFunction(d => document.querySelector('[data-page="entries"]').dataset.positionPage === d.page && location.hash === '#' + d.anchor, destination);
        await idle();
        check('last directory article retains global page identity', Number(destination.page)>1 && await page.locator('[data-action="select-entry"]:checked').count()===0, destination);
      }
    }
    check('no unhandled page errors', errors.length === 0, errors);
  } finally {
    await fs.writeFile(path.join(output,'acceptance.json'), JSON.stringify({results,errors},null,2));
    await context.close();
    await browser.close();
  }
  console.log(JSON.stringify(results));
})().catch(error => {console.error(error);process.exitCode=1;});
