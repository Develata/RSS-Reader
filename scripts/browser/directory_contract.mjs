import { evaluate, sleep } from './cdp_session.mjs';

// Literal facts from directory_contract_*.json. Do not derive expected selection
// from the tracker, DOM order, or the production grouping/selection algorithm.
export const directoryCases = {
  time: {
    group: 'entry-group-2026-年-09-月', first: 'entry-group-2026-09-02',
    next: 'entry-group-2026-09-01',
    otherGroup: 'entry-group-2026-年-08-月', other: 'entry-group-2026-08-02',
    beforeOther: 'entry-group-2026-09-01',
    remoteGroup: 'entry-group-2026-年-04-月', remote: 'entry-group-2026-04-02',
  },
  source: {
    group: 'entry-source-1', first: 'entry-source-1-month-2026-09',
    next: 'entry-source-1-month-2026-08',
    otherGroup: 'entry-source-2', other: 'entry-source-2-month-2026-09',
    beforeOther: 'entry-source-1-month-2026-05',
    remoteGroup: 'entry-source-1', remote: 'entry-source-1-month-2026-04',
  },
};
export const rail = '[data-layout="entry-directory-rail"]';
export const top = '[data-layout="entry-top-directory"]';
export const group = id => `${rail} [data-layout="entry-directory-toggle"][data-directory-anchor="${id}"]`;
export const item = id => `${rail} [data-directory-kind="item"][data-directory-anchor="${id}"]`;
const q = JSON.stringify;

// A condition must hold on successive frames; a bounded timer still rejects if
// the page stops producing frames. No global rAF/event monkeypatches are used.
export async function settle(client, expression, label, timeoutMs = 6000) {
  return evaluate(client, `new Promise((resolve, reject) => {
    let frame, consecutive = 0;
    const timeout = setTimeout(() => { cancelAnimationFrame(frame); reject(new Error(${q(label)})); }, ${timeoutMs});
    const tick = () => {
      try {
        const value = (${expression});
        consecutive = value ? consecutive + 1 : 0;
        if (consecutive >= 3) { clearTimeout(timeout); resolve(value); return; }
        frame = requestAnimationFrame(tick);
      } catch (error) { clearTimeout(timeout); reject(error); }
    };
    frame = requestAnimationFrame(tick);
  })`);
}

export async function keyPress(client, name) {
  const code = name === 'Enter' ? 'Enter' : 'Space';
  const key = name === 'Enter' ? 'Enter' : ' ';
  const virtualKey = name === 'Enter' ? 13 : 32;
  for (const type of ['keyDown', 'keyUp']) {
    await client.send('Input.dispatchKeyEvent', { type, key, code, windowsVirtualKeyCode: virtualKey, nativeVirtualKeyCode: virtualKey,
      ...(name === 'Enter' && type === 'keyDown' ? {text:'\r',unmodifiedText:'\r'} : {}) });
  }
}

export async function click(client, selector) {
  const point = await evaluate(client, `(() => {
    const element = document.querySelector(${q(selector)});
    if (!element) throw new Error('Missing click target: ' + ${q(selector)});
    const parent = element.closest(${q(rail)});
    if (parent) {
      const r = element.getBoundingClientRect(), p = parent.getBoundingClientRect();
      if (r.top < p.top) parent.scrollTop += r.top - p.top;
      if (r.bottom > p.bottom) parent.scrollTop += r.bottom - p.bottom;
    }
    const r = element.getBoundingClientRect();
    const x = r.left + r.width / 2, y = r.top + r.height / 2;
    if (!r.width || !r.height || y < 0 || y >= innerHeight || !element.contains(document.elementFromPoint(x, y)))
      throw new Error('Click target not visible/hittable: ' + ${q(selector)});
    return {x, y};
  })()`);
  await client.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
  await client.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
}

// Trusted CDP input, with both hit targets resolved before any render. Record
// what was actually clicked and the presented epoch; do not infer intent from
// the eventual location or call HTMLElement.click() as a rapid-input proxy.
export async function clickBurst(client, selectors, gapMs = 0, keyboard = false) {
  const points = await evaluate(client, `(() => {
    const elements=${q(selectors)}.map(s=>document.querySelector(s));
    const parent=elements.at(-1).closest(${q(rail)}), last=elements.at(-1).getBoundingClientRect(), bounds=parent.getBoundingClientRect();
    if(last.bottom>bounds.bottom) parent.scrollTop+=last.bottom-bounds.bottom;
    window.__directoryClicks=[];
    window.__directoryClickProbe=e=>{const b=e.target.closest?.('[data-directory-anchor]');if(b)window.__directoryClicks.push({anchor:b.dataset.directoryAnchor,open:b.dataset.open,trusted:e.isTrusted,at:e.timeStamp,epoch:b.closest('[data-directory-consumer]').dataset.directoryEpoch,page:document.querySelector('[data-page="entries"]').dataset.positionPage});};
    window.addEventListener('click',window.__directoryClickProbe,true);
    return elements.map(e=>{const r=e.getBoundingClientRect(),x=r.left+r.width/2,y=r.top+r.height/2;if(!e.contains(document.elementFromPoint(x,y)))throw new Error('Rapid input target is not hittable');return {x,y};});
  })()`);
  try {
    for (const [index,point] of points.entries()) {
      if (index && gapMs) await sleep(gapMs);
      if (keyboard) {
        // Focus setup is independent of the first jump's geometry; both
        // activations are still trusted browser input, never element.click().
        // A page switch can briefly hide the second control. Wait for a real
        // focusable target; gapMs is a minimum, actual event times are recorded.
        await settle(client,`!!document.querySelector(${q(selectors[index])})?.getClientRects().length`,'rapid navigation target must be displayed');
        await evaluate(client,`document.querySelector(${q(selectors[index])}).focus({preventScroll:true})`);
        await keyPress(client,'Enter');
      } else {
        for (const type of ['mousePressed','mouseReleased']) await client.send('Input.dispatchMouseEvent',{type,...point,button:'left',clickCount:1});
      }
    }
    return await evaluate(client,'window.__directoryClicks');
  } finally {
    await evaluate(client,"window.removeEventListener('click',window.__directoryClickProbe,true);delete window.__directoryClickProbe;delete window.__directoryClicks");
  }
}

export async function directoryIdle(client) {
  await evaluate(client, 'window.__rssrEntryDirectoryTracker.whenSettled()');
}

export async function scrollAnchor(client, id, topPx = 80, tolerance = 0.75) {
  await evaluate(client, `(() => {
    const anchor = document.getElementById(${q(id)});
    if (!anchor) throw new Error('Missing fixture anchor: ' + ${q(id)});
    window.scrollTo({top: scrollY + anchor.getBoundingClientRect().top - ${topPx}, behavior:'instant'});
  })()`);
  await settle(client, `Math.abs(document.getElementById(${q(id)}).getBoundingClientRect().top - ${topPx}) <= ${tolerance}`, `anchor ${id} did not reach ${topPx}px (tolerance ${tolerance})`);
}

export async function userWheel(client, deltaY) {
  await client.send('Input.dispatchMouseEvent', {type:'mouseMoved',x:100,y:400});
  await client.send('Input.dispatchMouseEvent', {type:'mouseWheel',x:100,y:400,deltaX:0,deltaY});
}
async function userScrollAnchor(client, id) {
  // Alignment allows a 2px tolerance, while a smooth jump can still be on its
  // last frame. Finish that motion before measuring a new wheel delta; otherwise
  // Chromium can consume the wheel as cancellation of the preceding animation.
  await evaluate(client, `new Promise((resolve, reject) => {
    let previous = scrollY, stable = 0, frame;
    const timeout = setTimeout(() => { cancelAnimationFrame(frame); reject(new Error('main scroll did not settle')); }, 6000);
    const tick = () => {
      const current = scrollY;
      stable = current === previous ? stable + 1 : 0;
      previous = current;
      if (stable >= 6) { clearTimeout(timeout); resolve(); }
      else frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
  })`);
  const delta = await evaluate(client, `document.getElementById(${q(id)}).getBoundingClientRect().top - 80`);
  await userWheel(client,delta);
  await settle(client, `Math.abs(document.getElementById(${q(id)}).getBoundingClientRect().top - 80) <= 2`, 'user wheel anchor positioning');
}
async function expectTargetAligned(client, id) {
  // Compact rows can make the last group shorter than one viewport. In that
  // case scrolling clamps at the document end; the target must still be visible.
  await settle(client, `(() => {
    const anchor=document.getElementById(${q(id)}), scroller=document.scrollingElement;
    if (!anchor) return false;
    const top=anchor.getBoundingClientRect().top;
    const inset=parseFloat(getComputedStyle(anchor).scrollMarginTop || 0);
    const max=Math.max(0,scroller.scrollHeight-scroller.clientHeight);
    return Math.abs(top-inset)<=2 || (Math.abs(scrollY-max)<=2 && top>=inset && top<innerHeight-80);
  })()`, 'directory target must align or remain visible at the clamped document end');
}
async function expectOnlyOpen(client, id) {
  await settle(client, `(() => { const groups=[...document.querySelectorAll(${q(rail+' [data-layout="entry-directory-toggle"]')})];
    return groups.length===6 && groups.every(e=>e.dataset.open === (e.dataset.directoryAnchor===${q(id)}?'true':'false'));
  })()`, 'only the literal current path may be open');
}

export async function snapshot(client) {
  return evaluate(client, `(() => {
    const visible = e => !!e && !!(e.offsetWidth || e.offsetHeight || e.getClientRects().length);
    const state = selector => {
      const root = document.querySelector(selector);
      return {visible:visible(root), scrollTop:root?.scrollTop ?? 0, scrollLeft:root?.scrollLeft ?? 0,
        currentGroup:root?.dataset.directoryCurrentGroup, currentItem:root?.dataset.directoryCurrentItem,
        maxScrollTop:root ? root.scrollHeight-root.clientHeight : 0,
        groups:[...(root?.querySelectorAll('[data-directory-kind="group"][data-active="true"]') || [])].map(e=>e.dataset.directoryAnchor),
        items:[...(root?.querySelectorAll('[data-directory-kind="item"][data-active="true"]') || [])].map(e=>e.dataset.directoryAnchor)};
    };
    return {url:location.href, y:scrollY, width:innerWidth, height:innerHeight,
      mode:document.querySelector(${q(rail)})?.dataset.directoryMode,
      rail:state(${q(rail)}), top:state(${q(top)}),
      groups:[...document.querySelectorAll(${q(rail + ' [data-layout="entry-directory-toggle"]')})].map(e=>({
        id:e.dataset.directoryAnchor, active:e.dataset.active, base:e.dataset.openBase,
        open:e.dataset.open, canToggle:e.dataset.canToggle, disabled:e.getAttribute('aria-disabled'),
        expanded:e.getAttribute('aria-expanded'), body:e.parentElement.querySelector('[data-directory-section-body]')?.dataset.open})),
      anchors:[...document.querySelectorAll('[data-entry-scroll-anchor]')].map(e=>({id:e.id,top:e.getBoundingClientRect().top})),
      page:document.querySelector('[data-slot="entry-pagination-status"]')?.textContent.trim()};
  })()`);
}

export async function expectActive(client, expectedGroup, expectedItem, consumer = 'rail') {
  const selector = consumer === 'rail' ? rail : top;
  await settle(client, `(() => {
    const root = document.querySelector(${q(selector)});
    if (!root || !root.getClientRects().length) return false;
    const groups = root.querySelectorAll('[data-directory-kind="group"][data-active="true"]');
    const items = root.querySelectorAll('[data-directory-kind="item"][data-active="true"]');
    if (groups.length !== 1 || groups[0].dataset.directoryAnchor !== ${q(expectedGroup)}) return false;
    if (${q(consumer)} === 'top') return true;
    // A manually closed branch no longer keeps invisible child DOM nodes.
    // Check the retained Rust path, then require the exact highlight when open.
    return root.dataset.directoryCurrentGroup === ${q(expectedGroup)} &&
      root.dataset.directoryCurrentItem === ${q(expectedItem)} &&
      (groups[0].dataset.open === 'true'
        ? items.length === 1 && items[0].dataset.directoryAnchor === ${q(expectedItem)}
        : items.length === 0);
  })()`, `visible ${consumer} must select ${expectedGroup} / ${expectedItem}`);
}

export function directoryHarness(client, env) {
  async function prepare(mode) {
    if (!env.native) {
      await env.setViewport(1280, 800, false, 1);
      await env.seed('directory-contract', '/entries');
    } else {
      // The caller owns an isolated SQLite fixture. Switch grouping to reset
      // component-local preferences BETWEEN scenarios, never inside key checks.
      await evaluate(client, `if (!document.querySelector('[data-page="entries"]')) document.querySelector('[data-action="activate-home"]').click()`);
    }
    await settle(client, `document.querySelector('[data-page="entries"][data-position-ready="true"]') && document.querySelectorAll('[data-position-entry]').length > 0`, 'directory fixture not ready');
    if (env.native) await setMode(mode === 'time' ? 'source' : 'time');
    await setMode(mode);
    await env.manualScroll();
    await evaluate(client, `window.scrollTo({top:0,behavior:'instant'})`);
    const c = directoryCases[mode];
    await settle(client, `document.getElementById(${q(c.first)}) && !!window.__rssrEntryDirectoryTracker`, 'fixture first group / tracker not ready');
    await expectActive(client, c.group, c.first);
    await settle(client, `document.querySelector('[data-action="activate-home"]')?.getAttribute('aria-busy') === 'false'`, 'fixture startup refresh must finish before interaction');
    const s = await snapshot(client);
    env.assertThat(`${mode}: fixed fixture has page 1 / 2 and 50 entries`, s.page === '1 / 2' && await evaluate(client, `document.querySelectorAll('[data-position-entry]').length === 50`), s);
    return c;
  }
  async function setMode(mode) {
    await evaluate(client, `document.querySelector('[data-action="group-by-${mode}"]').scrollIntoView({block:'center',behavior:'instant'})`);
    await click(client, `[data-action="group-by-${mode}"]`);
    await settle(client, `document.querySelector('[data-entry-scroll-anchor][data-grouping-mode="${mode}"]')`, `grouping ${mode} not rendered`);
  }
  return {prepare, setMode};
}

async function listenerCount(client) {
  const {result} = await client.send('Runtime.evaluate', {expression:'window', objectGroup:'directory-listeners'});
  try {
    const {listeners} = await client.send('DOMDebugger.getEventListeners', {objectId:result.objectId});
    return Object.fromEntries(['scroll','resize'].map(type=>[type,listeners.filter(x=>x.type===type && x.handler?.description.includes('scheduleUpdate(')).length]));
  } finally { await client.send('Runtime.releaseObjectGroup', {objectGroup:'directory-listeners'}); }
}

export async function checkDirectoryContracts(client, env) {
  const {prepare, setMode} = directoryHarness(client, env);
  const failures = [];
  const check = env.assertThat;
  async function scenario(name, run) {
    try { await run(); }
    catch (error) {
      const evidence = await snapshot(client).catch(e => ({captureError:String(e)}));
      failures.push({name, error:String(error), evidence});
      await env.capture(`directory-failure-${name}`).catch(e => { failures.at(-1).captureError = String(e); });
    }
  }
  for (const gap of [0,4,12,32]) await scenario(`rapid-cross-page-${gap}ms`, async () => {
    const c = await prepare('time'), last='entry-group-2026-04-01';
    await click(client,group(c.remoteGroup));
    await settle(client,`document.querySelector(${q(group(c.remoteGroup))}).dataset.open === 'true'`,'expand cross-page targets');
    const hits=await clickBurst(client,[item(c.remote),item(last)],gap,true);
    check(`rapid navigation ${gap}ms: both distinct targets receive trusted clicks`,
      hits.length===2 && hits[0].anchor===c.remote && hits[1].anchor===last && hits.every(h=>h.trusted),hits);
    await settle(client,`location.hash === ${q('#'+last)} && document.querySelector('[data-slot="entry-pagination-status"]').textContent.trim()==='2 / 2' && !!document.getElementById(${q(last)})`,'latest real cross-page click must win');
    await expectTargetAligned(client,last);
    check(`rapid navigation ${gap}ms: latest user target wins`,true,{hits,after:await snapshot(client)});
  });
  for (const mode of ['time', 'source']) {
    await scenario(`${mode}-hidden-tree-preserves-state`, async () => {
      const c = await prepare(mode);
      await click(client, '[data-position-entry="72"] [data-action="select-entry"]');
      await click(client, group(c.group));
      await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === 'false'`, 'close current branch');
      check(`${mode}: closed branch releases its child nodes`, await evaluate(client,
        `document.querySelector(${q(group(c.group))}).nextElementSibling.childElementCount === 0`));
      for (const entry of (await snapshot(client)).groups.filter(g => g.open !== 'true')) {
        await click(client, group(entry.id));
        await settle(client, `document.querySelector(${q(group(entry.id))}).dataset.open === 'true'`, 'open branches before hiding');
      }
      await evaluate(client, `document.querySelector(${q(group(c.group))}).focus({preventScroll:true});
        window.__directoryListIdentity = [...document.querySelectorAll('[data-position-entry]')];`);
      const point = await evaluate(client, `(() => {const r=document.querySelector(${q(rail)}).getBoundingClientRect();
        return {x:r.left+r.width/2,y:r.top+r.height/2};})()`);
      await client.send('Input.dispatchMouseEvent', {type:'mouseWheel', ...point, deltaX:0, deltaY:150});
      await settle(client, `document.querySelector(${q(rail)}).scrollTop > 30`, 'scroll the manual directory');
      await directoryIdle(client);
      const before = await snapshot(client);
      for (const type of ['keyDown','keyUp']) await client.send('Input.dispatchKeyEvent',
        {type,key:'Escape',code:'Escape',windowsVirtualKeyCode:27});
      await settle(client, `document.querySelector(${q(rail)}).dataset.directoryRendered === 'false' &&
        !document.querySelector(${q(rail + ' [data-layout="entry-directory-nav"]')})`, 'hidden directory must unmount its tree');
      await click(client, '[data-action="open-directory"]');
      await settle(client, `document.querySelector(${q(rail)}).dataset.directoryRendered === 'true'`, 'reopen directory');
      await directoryIdle(client);
      const after = await snapshot(client);
      check(`${mode}: reopening retains manual expansion, scroll and selection`,
        after.mode === 'manual' && Math.abs(after.rail.scrollTop-before.rail.scrollTop)<=1 &&
        JSON.stringify(after.groups.map(g=>[g.id,g.open]))===JSON.stringify(before.groups.map(g=>[g.id,g.open])) &&
        await evaluate(client, `document.querySelector('[data-position-entry="72"] [data-action="select-entry"]').checked`),
        {before,after});
      check(`${mode}: directory visibility does not remount article rows`, await evaluate(client,
        `[...document.querySelectorAll('[data-position-entry]')].every((row,i)=>row===window.__directoryListIdentity[i])`));
      await evaluate(client, 'delete window.__directoryListIdentity');
      await userWheel(client, 24);
      await settle(client, `document.querySelector(${q(rail)}).dataset.directoryMode === 'follow'`, 'main scrolling resumes following after reopening');
    });
    await scenario(`${mode}-read-summary-preserves-manual`, async () => {
      const c = await prepare(mode);
      check(`${mode}: read regression uses All feeds and ReadFilter::All`,
        await evaluate(client, `document.querySelector('.source-filter-trigger span').textContent === '全部来源' && document.querySelector('[aria-label="仅未读"]').getAttribute('aria-pressed') === 'false' && document.querySelector('[aria-label="仅已读"]').getAttribute('aria-pressed') === 'false'`));
      await scrollAnchor(client,c.first);
      await click(client,group(c.group));
      await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === 'false'`, 'close current group before read update');
      await click(client,group(c.otherGroup));
      await settle(client, `document.querySelector(${q(group(c.otherGroup))}).dataset.open === 'true'`, 'open other group before read update');
      for (const action of ['toggle-starred','mark-read','mark-read']) {
        const selector = `[data-position-entry="72"] [data-action="${action}"]`;
        const before = await snapshot(client);
        const pressed = await evaluate(client, `document.querySelector(${q(selector)}).getAttribute('aria-pressed')`);
        await click(client,selector);
        await settle(client, `document.querySelector(${q(selector)}).getAttribute('aria-pressed') !== ${q(pressed)}`, 'entry flag write must complete');
        // Unread totals moved to Subscribe; rssr_workspace_acceptance checks those values.
        await directoryIdle(client);
        const after = await snapshot(client);
        check(`${mode}: ${action} preserves manual/open state and both scroll positions`,
          after.mode==='manual' && JSON.stringify(after.groups.map(g=>[g.id,g.open]))===JSON.stringify(before.groups.map(g=>[g.id,g.open])) &&
          Math.abs(after.y-before.y)<=1 && Math.abs(after.rail.scrollTop-before.rail.scrollTop)<=1 && Math.abs(after.top.scrollLeft-before.top.scrollLeft)<=1,
          {before,after});
      }
      const epoch=await evaluate(client,`document.querySelector(${q(rail)}).dataset.directoryEpoch`);
      await setMode(mode==='time'?'source':'time');
      await directoryIdle(client);
      check(`${mode}: a real grouping change still resets directory context`,await evaluate(client,`document.querySelector(${q(rail)}).dataset.directoryEpoch !== ${q(epoch)} && document.querySelector(${q(rail)}).dataset.directoryMode === 'follow'`));
      await expectOnlyOpen(client,directoryCases[mode==='time'?'source':'time'].group);
    });
    await scenario(`${mode}-boundary`, async () => {
      const c = await prepare(mode);
      for (const [anchor, beforeGroup, beforeItem, afterGroup] of [
        [c.next,c.group,c.first,c.group], [c.other,c.group,c.beforeOther,c.otherGroup],
      ]) {
        await scrollAnchor(client, anchor, 97);
        await expectActive(client, beforeGroup, beforeItem);
        check(`${mode}: 97px retains preceding anchor`, true, await snapshot(client));
        await scrollAnchor(client, anchor, 95);
        await expectActive(client, afterGroup, anchor);
        const s = await snapshot(client), active = s.groups.find(g=>g.id===afterGroup);
        check(`${mode}: 95px selects anchor and opens active group`, active.open==='true' && active.body==='true' && active.canToggle==='true' && active.disabled==='false' && active.expanded==='true', s);
      }
      await env.capture(`directory-${mode}-boundary`);
    });
    // These replace PR19's four OLD 'active cannot toggle' expectations.
    // A literal product oracle now requires actual collapse, then same-group
    // USER scrolling to reset it. Baseline evidence remains in baselines/.
    for (const key of ['mouse','Enter','Space']) {
      await scenario(`${mode}-active-${key}`, async () => {
        const c = await prepare(mode);
        await scrollAnchor(client,c.first);
        if (key==='mouse') await click(client,group(c.group));
        else {
          await evaluate(client, `document.querySelector(${q(group(c.group))}).focus({preventScroll:true})`);
          await keyPress(client,key);
        }
        await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === 'false'`, 'current group must actually collapse');
        const closed = await snapshot(client);
        check(`${mode}: current group ${key} collapses immediately`, closed.groups.find(g=>g.id===c.group).body==='false', closed);
        await click(client,group(c.otherGroup));
        await settle(client, `document.querySelector(${q(group(c.otherGroup))}).dataset.open === 'true'`, 'other group explicitly opens');
        // A programmatic main scroll observes the position but must not reset
        // manual choices. This also detects feedback from directory alignment.
        await scrollAnchor(client,c.other);
        await expectActive(client,c.otherGroup,c.other);
        await directoryIdle(client);
        await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === 'false' && document.querySelector(${q(group(c.otherGroup))}).dataset.open === 'true'`, 'programmatic scroll interrupted manual state');
        check(`${mode}: cross-group programmatic observation preserves manual state after ${key}`,(await snapshot(client)).mode==='manual');
        await scrollAnchor(client,c.first);
        await expectActive(client,c.group,c.first);
        await directoryIdle(client);
        const beforeY = (await snapshot(client)).y;
        await userWheel(client, 24);
        await settle(client, `scrollY > ${beforeY + 1}`, 'same-group wheel must really scroll');
        await expectOnlyOpen(client,c.group);
        await expectActive(client,c.group,c.first);
        check(`${mode}: same-group user wheel restores following and clears other groups after ${key}`, true, await snapshot(client));
        await click(client,group(c.group));
        await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === 'false'`, 'manual collapse after follow');
        await userScrollAnchor(client,c.other);
        await expectOnlyOpen(client,c.otherGroup);
        await expectActive(client,c.otherGroup,c.other);
        check(`${mode}: cross-group user scroll resets the previously collapsed path`, true, await snapshot(client));
      });
      await scenario(`${mode}-inactive-${key}`, async () => {
        const c = await prepare(mode);
        for (const wanted of ['true','false']) {
          if (key==='mouse') await click(client,group(c.otherGroup));
          else {
            await evaluate(client, `document.querySelector(${q(group(c.otherGroup))}).focus({preventScroll:true})`);
            await keyPress(client,key);
          }
          await settle(client, `document.querySelector(${q(group(c.otherGroup))}).dataset.open === ${q(wanted)}`, 'non-current input must change displayed state');
        }
        check(`${mode}: non-current ${key} opens and closes`, true, await snapshot(client));
      });
    }
    await scenario(`${mode}-internal-scroll-and-keyboard`, async () => {
      const c = await prepare(mode);
      await scrollAnchor(client,c.first);
      const distant = mode==='time'?c.remoteGroup:'entry-source-6';
      await click(client,group(distant));
      await settle(client, `document.querySelector(${q(group(distant))}).dataset.open === 'true'`, 'manual distant group');
      for (const entry of (await snapshot(client)).groups.filter(g=>g.open!=='true')) {
        await click(client,group(entry.id));
        await settle(client, `document.querySelector(${q(group(entry.id))}).dataset.open === 'true'`, 'expand fixture for a scrollable rail');
      }
      await evaluate(client, `document.querySelector(${q(rail)}).scrollTop=0`);
      const before = await snapshot(client);
      check(`${mode}: internal scroll fixture has real overflow`, before.rail.maxScrollTop > 50, before);
      const point = await evaluate(client, `(() => { const r=document.querySelector(${q(rail)}).getBoundingClientRect(); return {x:r.left+r.width/2,y:r.top+30}; })()`);
      await client.send('Input.dispatchMouseEvent',{type:'mouseWheel',...point,deltaX:0,deltaY:50});
      await settle(client, `document.querySelector(${q(group(distant))}).dataset.open === 'true' && document.querySelector(${q(rail)}).dataset.directoryMode === 'manual'`, 'directory wheel must remain manual');
      await settle(client, `document.querySelector(${q(rail)}).scrollTop > 1`, 'directory must actually scroll');
      const afterInternal = await snapshot(client);
      check(`${mode}: internal wheel preserves manual groups and main position`, Math.abs(afterInternal.y-before.y)<=1, {before,afterInternal});
      // A nested scroll event whose target does not contain the article groups
      // must not be interpreted as the main list, even if it bubbles.
      await evaluate(client, `document.querySelector('[data-position-entry]').dispatchEvent(new Event('scroll',{bubbles:true}))`);
      await directoryIdle(client);
      await settle(client, `document.querySelector(${q(group(distant))}).dataset.open === 'true'`, 'nested event reset manual state');
      const nestedPoint = await evaluate(client, `(() => {
        const main=document.querySelector('[data-layout="entries-main"]');
        const nested=document.createElement('div'); nested.id='directory-test-nested';
        nested.style.cssText='position:fixed;left:80px;top:360px;width:160px;height:100px;overflow:auto;z-index:999;background:white';
        const body=document.createElement('div');body.style.height='800px';nested.append(body);main.append(nested);
        // The app shell establishes a containing block. Verify the actual hit
        // target instead of assuming fixed coordinates are viewport-relative.
        const r=nested.getBoundingClientRect(); nested.style.left=(160-r.left)+'px';nested.style.top=(720-r.top)+'px';
        const placed=nested.getBoundingClientRect(),x=placed.left+30,y=placed.top+30;
        if (!nested.contains(document.elementFromPoint(x,y))) throw new Error('Nested test scroll target is not hittable');
        return {x,y};
      })()`);
      await client.send('Input.dispatchMouseEvent',{type:'mouseMoved',...nestedPoint});
      await client.send('Input.dispatchMouseEvent',{type:'mouseWheel',...nestedPoint,deltaX:0,deltaY:40});
      await settle(client, `document.getElementById('directory-test-nested').scrollTop > 1`, 'nested fixture must actually scroll');
      check(`${mode}: real nested scroll retains manual state`, (await snapshot(client)).mode==='manual', await snapshot(client));
      await evaluate(client, `(() => { document.getElementById('directory-test-nested').remove(); const main=document.querySelector('[data-layout="entries-main"]');main.tabIndex=-1; main.focus({preventScroll:true}); })()`);
      const y = (await snapshot(client)).y;
      for (const type of ['keyDown','keyUp']) await client.send('Input.dispatchKeyEvent',{type,key:'ArrowDown',code:'ArrowDown',windowsVirtualKeyCode:40,nativeVirtualKeyCode:40});
      await settle(client, `scrollY > ${y+1}`, 'main keyboard scroll must move content');
      await expectOnlyOpen(client,c.group);
      check(`${mode}: main keyboard scroll resumes within the current group`, true, await snapshot(client));
    });
    await scenario(`${mode}-local-toggle-render`, async () => {
      const c = await prepare(mode);
      await scrollAnchor(client,c.first);
      await expectActive(client,c.group,c.first);
      const distantGroup = mode==='time'?c.remoteGroup:'entry-source-6';
      if ((await snapshot(client)).groups.find(g=>g.id===distantGroup).open!=='true') {
        await click(client,group(distantGroup));
        await settle(client, `document.querySelector(${q(group(distantGroup))}).dataset.open === 'true'`, 'distant group expansion');
      }
      // Following now opens one group, so explicitly expand the fixture before
      // asserting a meaningful rail displacement (the old defaults overflowed).
      for (const entry of (await snapshot(client)).groups.filter(g=>g.open!=='true')) {
        await click(client,group(entry.id));
        await settle(client, `document.querySelector(${q(group(entry.id))}).dataset.open === 'true'`, 'make local-collapse fixture scrollable');
      }
      // Put the active article's directory entry outside the rail viewport.
      // Otherwise an accidental align call can be a no-op and falsely pass.
      await evaluate(client, `(() => { const root=document.querySelector(${q(rail)}), button=document.querySelector(${q(group(distantGroup))});
        root.scrollTop += button.getBoundingClientRect().top - root.getBoundingClientRect().top - 20;
      })()`);
      const before = await snapshot(client);
      check(`${mode}: local-collapse fixture actually scrolls the rail away from active item`, before.rail.scrollTop>50, before);
      await click(client,group(distantGroup));
      await settle(client, `document.querySelector(${q(group(distantGroup))}).dataset.openBase === 'false'`, 'mouse did not collapse base');
      const after = await snapshot(client);
      // Collapsing content may legitimately clamp scrollTop to the new maximum.
      const expectedRailTop=Math.min(before.rail.scrollTop,after.rail.maxScrollTop);
      check(`${mode}: local collapse does not rewind body or rail`, Math.abs(after.y-before.y)<=1 && Math.abs(after.rail.scrollTop-expectedRailTop)<=1, {before,after,expectedRailTop});
      // A real VDOM flag update must preserve the observed position in place.
      await scrollAnchor(client,c.other);
      await expectActive(client,c.otherGroup,c.other);
      const beforeFlag = await snapshot(client);
      const priorPressed = await evaluate(client, `document.querySelector('[data-position-entry="72"] [data-action="toggle-starred"]').getAttribute('aria-pressed')`);
      await evaluate(client, `document.querySelector('[data-position-entry="72"] [data-action="toggle-starred"]').click()`);
      await settle(client, `document.querySelector('[data-position-entry="72"] [data-action="toggle-starred"]').getAttribute('aria-pressed') !== ${q(priorPressed)}`, 'flag rerender not completed');
      await directoryIdle(client);
      await expectActive(client,c.otherGroup,c.other);
      const afterFlag = await snapshot(client);
      check(`${mode}: VDOM flag update preserves highlight, manual/open state and positions`, afterFlag.mode===beforeFlag.mode &&
        JSON.stringify(afterFlag.groups.map(g=>[g.id,g.open]))===JSON.stringify(beforeFlag.groups.map(g=>[g.id,g.open])) &&
        Math.abs(afterFlag.y-beforeFlag.y)<=1 && Math.abs(afterFlag.rail.scrollTop-beforeFlag.rail.scrollTop)<=1 && Math.abs(afterFlag.top.scrollLeft-beforeFlag.top.scrollLeft)<=1,
        {before:beforeFlag,after:afterFlag});
    });
    await scenario(`${mode}-rapid-interleave`, async () => {
      const c = await prepare(mode);
      await scrollAnchor(client,c.first);
      const hits=await clickBurst(client,[group(c.group),group(c.group)]);
      check(`${mode}: rapid toggles receive two trusted inputs`,hits.length===2 && hits.every(h=>h.trusted && h.anchor===c.group),hits);
      // The second click may arrive before OR after the first DOM commit. Its
      // explicit intent is the inverse of what the user actually saw then.
      const desired=hits[1].open==='true'?'false':'true';
      await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === ${q(desired)}`, 'latest trusted toggle must honor its presented state');
      await directoryIdle(client);
      await userWheel(client,30);
      await expectOnlyOpen(client,c.group);
      await click(client,group(c.group));
      await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === 'false'`, 'manual intent after a user scroll must win');
      // Old main-scroll inertia (without a new main input) must not steal it.
      await scrollAnchor(client,c.other);
      await expectActive(client,c.otherGroup,c.other);
      await directoryIdle(client);
      await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === 'false'`, 'late main observation stole a new manual intent');
      check(`${mode}: quick explicit intents and late scroll preserve latest manual intent`, true, await snapshot(client));
    });
    await scenario(`${mode}-navigation`, async () => {
      const c = await prepare(mode);
      await evaluate(client, "history.replaceState(null, '', '#%')");
      await click(client,item(c.next));
      await settle(client, `location.hash === ${q('#'+encodeURIComponent(c.next))} && Math.abs(document.getElementById(${q(c.next)}).getBoundingClientRect().top - parseFloat(getComputedStyle(document.getElementById(${q(c.next)})).scrollMarginTop || 0)) <= 2`, 'same-page navigation/hash alignment');
      check(`${mode}: same-page directory navigation reaches target/hash`, true, await snapshot(client));
      if ((await snapshot(client)).groups.find(g=>g.id===c.remoteGroup).open!=='true') await click(client,group(c.remoteGroup));
      await settle(client, `document.querySelector(${q(group(c.remoteGroup))}).dataset.open === 'true' && document.querySelector(${q(item(c.remote))}).getClientRects().length > 0`, 'remote directory item must finish expanding before navigation');
      await click(client,item(c.remote));
      await settle(client, `document.querySelector('[data-slot="entry-pagination-status"]').textContent.trim() === '2 / 2' && location.hash === ${q('#'+encodeURIComponent(c.remote))} && !!document.getElementById(${q(c.remote)})`, 'cross-page target/hash');
      await expectTargetAligned(client,c.remote);
      check(`${mode}: cross-page directory navigation reaches target/hash`, true, await snapshot(client));
    });
    if (!env.native) await scenario(`${mode}-consumers`, async () => {
      const c = await prepare(mode);
      await scrollAnchor(client,c.other);
      await expectActive(client,c.otherGroup,c.other);
      for (const width of [1001,1000,1280,360]) {
        const mobile=width<=1000;
        await env.setViewport(width,800,mobile,mobile?3:1);
        await evaluate(client, "window.scrollTo({top:0,behavior:'instant'})");
        if (!await evaluate(client, `!!document.querySelector(${q(rail)}).getClientRects().length`)) {
          await settle(client, `document.querySelector(${q(rail)}).dataset.directoryRendered === 'false' && !document.querySelector(${q(rail+' nav')})`, 'responsive hiding releases the directory tree');
          check(`${mode}: ${width}px hidden directory has no navigation tree`, true);
          await click(client,'[data-action="open-directory"]');
          await settle(client, `!!document.querySelector(${q(rail)}).getClientRects().length && document.querySelector(${q(rail)}).dataset.directoryRendered === 'true'`, 'directory opens from collapsed entry');
        }
        // Responsive layout changes anchor positions. Restore the explicit
        // article target before comparing consumers; do not infer it from JS.
        await scrollAnchor(client,c.other);
        await expectActive(client,c.otherGroup,c.other);
        const s = await snapshot(client);
        const activeWeight=await evaluate(client, `getComputedStyle(document.querySelector(${q(rail+' [data-directory-kind="group"][data-active="true"] [data-slot="entry-directory-title"]')})).fontWeight`);
        check(`${mode}: ${width}px visible directory preserves active path`, s.rail.visible && !s.top.visible && Number(activeWeight)>=600, {...s,activeWeight});
        if (mobile && width!==360) {
          await click(client,'[data-action="close-directory"]');
          const closed = await snapshot(client);
          check(`${mode}: narrow directory closes without moving articles`, !closed.rail.visible && Math.abs(closed.y-s.y)<=1, {before:s,after:closed});
        }
      }
      const before = await snapshot(client);
      const target = mode==='source'?'entry-source-6-month-2026-09':'entry-group-2026-06-02';
      const requestedY = await evaluate(client, `scrollY + document.getElementById(${q(target)}).getBoundingClientRect().top - 80`);
      await userScrollAnchor(client,target);
      const expectedGroup = mode==='source'?'entry-source-6':'entry-group-2026-年-06-月';
      await expectActive(client,expectedGroup,target);
      const aligned = await evaluate(client, `(() => {
        const root=document.querySelector(${q(rail)}), active=root.querySelector('[data-directory-kind="item"][data-active="true"]');
        const r=root.getBoundingClientRect(), a=active.getBoundingClientRect();
        return {y:scrollY,top:root.scrollTop,fits:a.top>=r.top-1 && a.bottom<=r.bottom+1};
      })()`);
      const after = await snapshot(client);
      check(`${mode}: mobile directory alignment preserves requested body position`, aligned.fits && Math.abs(after.y-requestedY)<=1, {before,requestedY,aligned,after});
      await env.capture(`directory-${mode}-mobile`);
    });
  }
  await scenario('source-article-navigation', async () => {
    const c = await prepare('source');
    const navigationHistory = await evaluate(client, `({length:history.length,state:JSON.stringify(history.state)})`);
    const branch = id => `${rail} [data-layout="entry-directory-branch-toggle"][data-directory-anchor="${id}"]`;
    const article = id => `${rail} [data-directory-kind="entry"][data-directory-anchor="entry-card-${id}"]`;
    await settle(client, `!!document.querySelector(${q(article(66))})`, 'current source/month must expose article titles');
    await click(client, '[data-position-entry="72"] [data-action="select-entry"]');
    await click(client, branch(c.first));
    await settle(client, `!document.querySelector(${q(article(66))})`, 'closing month unmounts its article nodes');
    check('source: collapsing the month preserves article selection', await evaluate(client,
      `document.querySelector('[data-position-entry="72"] [data-action="select-entry"]').checked`));
    await evaluate(client, `document.querySelector(${q(branch(c.first))}).focus({preventScroll:true})`);
    await keyPress(client, 'Enter');
    await settle(client, `!!document.querySelector(${q(article(66))})`, 'keyboard expands article titles');
    await click(client, article(66));
    await settle(client, `location.hash === '#entry-card-66'`, 'article link must navigate to the exact article');
    await expectTargetAligned(client, 'entry-card-66');
    check('source: same-page directory jump preserves route history', await evaluate(client,
      `history.length === ${navigationHistory.length} && JSON.stringify(history.state) === ${q(navigationHistory.state)}`));
    check('source: same-page article navigation keeps selection and manual browsing', await evaluate(client,
      `document.querySelector('[data-position-entry="72"] [data-action="select-entry"]').checked && document.querySelector(${q(rail)}).dataset.directoryMode === 'manual'`));
    await click(client, branch(c.remote));
    await settle(client, `!!document.querySelector(${q(article(12))})`, 'remote month must expose off-page articles');
    check('source: article target uses its global page, not the current-page index', await evaluate(client,
      `document.querySelector(${q(article(12))}).dataset.directoryPage === '2'`));
    await click(client, article(12));
    await settle(client, `location.hash === '#entry-card-12' && document.querySelector('[data-slot="entry-pagination-status"]').textContent.trim() === '2 / 2' && !!document.getElementById('entry-card-12')`, 'cross-page article target');
    await expectTargetAligned(client, 'entry-card-12');
    check('source: cross-page directory jump preserves route history', await evaluate(client,
      `history.length === ${navigationHistory.length} && JSON.stringify(history.state) === ${q(navigationHistory.state)}`));
    check('source: cross-page article navigation clears the old selection', await evaluate(client,
      `[...document.querySelectorAll('[data-action="select-entry"]')].every(e=>!e.checked)`));
    // Entry 12 is near the document end, so its jump can legitimately clamp.
    // Entry 22 is the first article on page 2 and can reach the 80px probe line.
    await userScrollAnchor(client, 'entry-card-22');
    await settle(client, `document.querySelector(${q(article(22))})?.getAttribute('aria-current') === 'location'`, 'directory follows the exact visible article');
    await env.capture('directory-source-articles');
  });
  await scenario('lifecycle', async () => {
    const c = await prepare('time');
    const installed = await listenerCount(client);
    await evaluate(client, `window.__directoryTestTracker = window.__rssrEntryDirectoryTracker; document.querySelector('[data-nav="settings"]').click()`);
    await settle(client, `!!document.querySelector('[data-page="settings"]')`, 'leave entries');
    await settle(client, `!window.__rssrEntryDirectoryTracker`, 'tracker must clean up on unmount without another scroll/resize');
    await evaluate(client, 'window.__directoryTestTracker.scheduleUpdate(true)');
    const removed = await listenerCount(client);
    check('directory lifecycle removes its scroll/resize listeners', removed.scroll===installed.scroll-1 && removed.resize===installed.resize-1, {installed,removed});
    await evaluate(client, `document.querySelector('[data-action="activate-home"]').click()`);
    await settle(client, `!!window.__rssrEntryDirectoryTracker && window.__rssrEntryDirectoryTracker !== window.__directoryTestTracker`, 'return must create a new tracker');
    await settle(client, `!!document.querySelector('[data-page="entries"][data-position-ready="true"]')`, 'return position initialization');
    await env.manualScroll();
    await scrollAnchor(client,c.other);
    await expectActive(client,c.otherGroup,c.other);
    const returned = await listenerCount(client);
    check('return installs exactly one listener pair', JSON.stringify(returned)===JSON.stringify(installed), {installed,returned});
    // Direct lifecycle probe: queued rAF must not resurrect listeners/state after
    // cleanup. This is separate from the route-driven cleanup check above.
    await evaluate(client, `window.__rssrEntryDirectoryTracker.scheduleUpdate(true); window.__rssrEntryDirectoryTracker.cleanup(); delete window.__directoryTestTracker`);
    await settle(client, `!window.__rssrEntryDirectoryTracker`, 'pending rAF resurrected disposed tracker');
    const cancelled = await listenerCount(client);
    check('explicit cleanup cancels queued rAF and removes listeners', JSON.stringify(cancelled)===JSON.stringify(removed), {removed,cancelled});
  });
  if (failures.length) {
    check('directory contract scenarios', false, failures);
  }
}
