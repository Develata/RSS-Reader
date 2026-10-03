import { evaluate } from './cdp_session.mjs';

// Literal facts from directory_contract_*.json. Do not derive expected selection
// from the tracker, DOM order, or the production grouping/selection algorithm.
export const directoryCases = {
  time: {
    group: 'entry-group-2026-年-09-月', first: 'entry-group-2026-09-02-72',
    next: 'entry-group-2026-09-01-66',
    otherGroup: 'entry-group-2026-年-08-月', other: 'entry-group-2026-08-02-60',
    beforeOther: 'entry-group-2026-09-01-66',
    remoteGroup: 'entry-group-2026-年-04-月', remote: 'entry-group-2026-04-02-12',
  },
  source: {
    group: 'entry-group-alpha', first: 'entry-group-2026-年-09-月-72',
    next: 'entry-group-2026-年-08-月-60',
    otherGroup: 'entry-group-bravo', other: 'entry-group-2026-年-09-月-71',
    beforeOther: 'entry-group-2026-年-05-月-24',
    remoteGroup: 'entry-group-alpha', remote: 'entry-group-2026-年-04-月-12',
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

export async function scrollAnchor(client, id, topPx = 80, tolerance = 0.75) {
  await evaluate(client, `(() => {
    const anchor = document.getElementById(${q(id)});
    if (!anchor) throw new Error('Missing fixture anchor: ' + ${q(id)});
    window.scrollTo({top: scrollY + anchor.getBoundingClientRect().top - ${topPx}, behavior:'instant'});
  })()`);
  await settle(client, `Math.abs(document.getElementById(${q(id)}).getBoundingClientRect().top - ${topPx}) <= ${tolerance}`, `anchor ${id} did not reach ${topPx}px (tolerance ${tolerance})`);
}

export async function snapshot(client) {
  return evaluate(client, `(() => {
    const visible = e => !!e && !!(e.offsetWidth || e.offsetHeight || e.getClientRects().length);
    const state = selector => {
      const root = document.querySelector(selector);
      return {visible:visible(root), scrollTop:root?.scrollTop, scrollLeft:root?.scrollLeft,
        maxScrollTop:root ? root.scrollHeight-root.clientHeight : 0,
        groups:[...(root?.querySelectorAll('[data-directory-kind="group"][data-active="true"]') || [])].map(e=>e.dataset.directoryAnchor),
        items:[...(root?.querySelectorAll('[data-directory-kind="item"][data-active="true"]') || [])].map(e=>e.dataset.directoryAnchor)};
    };
    return {url:location.href, y:scrollY, width:innerWidth, height:innerHeight,
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
    return groups.length === 1 && groups[0].dataset.directoryAnchor === ${q(expectedGroup)} &&
      (${q(consumer)} === 'top' || (items.length === 1 && items[0].dataset.directoryAnchor === ${q(expectedItem)}));
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
    await evaluate(client, `document.querySelector('[data-action="show-entry-controls"]')?.click()`);
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
    await evaluate(client, `(() => { const select = document.querySelector('[data-field="entry-grouping-mode"]');
      if (!select) throw new Error('Grouping control missing');
      if (select.value !== ${q(mode)}) { select.value = ${q(mode)}; select.dispatchEvent(new Event('change',{bubbles:true})); }
    })()`);
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
  const {prepare} = directoryHarness(client, env);
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
  for (const mode of ['time', 'source']) {
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
        check(`${mode}: 95px selects anchor and opens active group`, active.open==='true' && active.body==='true' && active.canToggle==='false' && active.disabled==='true' && active.expanded==='true', s);
      }
      await env.capture(`directory-${mode}-boundary`);
    });
    for (const key of ['Enter','Space']) {
      await scenario(`${mode}-active-${key}`, async () => {
        const c = await prepare(mode);
        await scrollAnchor(client,c.first);
        const before = (await snapshot(client)).groups.find(g=>g.id===c.group);
        await evaluate(client, `document.querySelector(${q(group(c.group))}).focus({preventScroll:true})`);
        await keyPress(client,key); // Exactly ONE activation per isolated scenario.
        await settle(client, `document.querySelector(${q(group(c.group))}).dataset.open === 'true'`, 'active group must remain open');
        const afterKey = await snapshot(client);
        // Crucial: scroll away in the SAME mounted page; a route reset would hide
        // a keyboard-induced mutation of the latent manual preference.
        await scrollAnchor(client,c.other);
        await expectActive(client,c.otherGroup,c.other);
        const after = await snapshot(client), departed = after.groups.find(g=>g.id===c.group);
        check(`${mode}: active ${key} cannot mutate latent manual preference`,
          afterKey.groups.find(g=>g.id===c.group).base===before.base && departed.base===before.base && departed.open===before.base,
          {before,afterKey,after});
      });
      await scenario(`${mode}-inactive-${key}`, async () => {
        const c = await prepare(mode);
        await scrollAnchor(client,c.other);
        await expectActive(client,c.otherGroup,c.other);
        await evaluate(client, `document.querySelector(${q(group(c.group))}).focus({preventScroll:true})`);
        await keyPress(client,key);
        await settle(client, `document.querySelector(${q(group(c.group))}).dataset.openBase === 'false'`, 'inactive key did not collapse base');
        await scrollAnchor(client,c.first);
        await expectActive(client,c.group,c.first);
        const active = (await snapshot(client)).groups.find(g=>g.id===c.group);
        await scrollAnchor(client,c.other);
        await expectActive(client,c.otherGroup,c.other);
        const departed = (await snapshot(client)).groups.find(g=>g.id===c.group);
        check(`${mode}: inactive ${key} preference survives temporary active expansion`, active.base==='false' && active.open==='true' && departed.open==='false' && departed.canToggle==='true', {active,departed});
      });
    }
    await scenario(`${mode}-local-toggle-render`, async () => {
      const c = await prepare(mode);
      await scrollAnchor(client,c.first);
      await expectActive(client,c.group,c.first);
      const distantGroup = mode==='time'?c.remoteGroup:'entry-group-foxtrot';
      if ((await snapshot(client)).groups.find(g=>g.id===distantGroup).open!=='true') {
        await click(client,group(distantGroup));
        await settle(client, `document.querySelector(${q(group(distantGroup))}).dataset.open === 'true'`, 'distant group expansion');
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
      // A real VDOM flag update must reconcile the JS-selected state in place.
      await scrollAnchor(client,c.other);
      await expectActive(client,c.otherGroup,c.other);
      const priorLabel = await evaluate(client, `document.querySelector('[data-position-entry="72"] [data-action="toggle-starred"]').textContent.trim()`);
      const nextLabel = priorLabel === '收藏' ? '取消收藏' : '收藏';
      await evaluate(client, `document.querySelector('[data-position-entry="72"] [data-action="toggle-starred"]').click()`);
      await settle(client, `document.querySelector('[data-position-entry="72"] [data-action="toggle-starred"]').textContent.trim() === ${q(nextLabel)}`, 'flag rerender not completed');
      await expectActive(client,c.otherGroup,c.other);
      check(`${mode}: VDOM flag update preserves viewport highlight`, true, await snapshot(client));
    });
    await scenario(`${mode}-navigation`, async () => {
      const c = await prepare(mode);
      await click(client,item(c.next));
      await settle(client, `decodeURIComponent(location.hash) === ${q('#'+c.next)} && Math.abs(document.getElementById(${q(c.next)}).getBoundingClientRect().top - parseFloat(getComputedStyle(document.getElementById(${q(c.next)})).scrollMarginTop || 0)) <= 2`, 'same-page navigation/hash alignment');
      check(`${mode}: same-page directory navigation reaches target/hash`, true, await snapshot(client));
      if ((await snapshot(client)).groups.find(g=>g.id===c.remoteGroup).open!=='true') await click(client,group(c.remoteGroup));
      await settle(client, `document.querySelector(${q(group(c.remoteGroup))}).dataset.open === 'true' && document.querySelector(${q(item(c.remote))}).getClientRects().length > 0`, 'remote directory item must finish expanding before navigation');
      await click(client,item(c.remote));
      await settle(client, `document.querySelector('[data-slot="entry-pagination-status"]').textContent.trim() === '2 / 2' && decodeURIComponent(location.hash) === ${q('#'+c.remote)} && !!document.getElementById(${q(c.remote)})`, 'cross-page target/hash');
      await settle(client, `Math.abs(document.getElementById(${q(c.remote)}).getBoundingClientRect().top - parseFloat(getComputedStyle(document.getElementById(${q(c.remote)})).scrollMarginTop || 0)) <= 2`, 'cross-page alignment');
      check(`${mode}: cross-page directory navigation reaches target/hash`, true, await snapshot(client));
    });
    if (!env.native) await scenario(`${mode}-consumers`, async () => {
      const c = await prepare(mode);
      await scrollAnchor(client,c.other);
      await expectActive(client,c.otherGroup,c.other);
      for (const width of [721,720,1280,360]) {
        const mobile=width<=720;
        await env.setViewport(width,800,mobile,mobile?3:1);
        // Responsive layout changes anchor positions. Restore the explicit
        // article target before comparing consumers; do not infer it from JS.
        await scrollAnchor(client,c.other);
        await expectActive(client,c.otherGroup,c.other,mobile?'top':'rail');
        const s = await snapshot(client);
        const activeWeight=await evaluate(client, `getComputedStyle(document.querySelector(${q((mobile?top:rail)+' [data-directory-kind="group"][data-active="true"] [data-slot="entry-directory-title"]')})).fontWeight`);
        check(`${mode}: ${width}px uses and emphasizes the visible directory consumer`, s.rail.visible===!mobile && s.top.visible===mobile && Number(activeWeight)>=600, {...s,activeWeight});
      }
      const before = await snapshot(client);
      const target = mode==='source'?'entry-group-2026-年-09-月-67':'entry-group-2026-06-02-36';
      const requestedY = await evaluate(client, `scrollY + document.getElementById(${q(target)}).getBoundingClientRect().top - 80`);
      await scrollAnchor(client,target);
      const expectedGroup = mode==='source'?'entry-group-foxtrot':'entry-group-2026-年-06-月';
      await expectActive(client,expectedGroup,null,'top');
      const aligned = await evaluate(client, `(() => {
        const root=document.querySelector(${q(top)}), active=root.querySelector('[data-active="true"]');
        const r=root.getBoundingClientRect(), a=active.getBoundingClientRect();
        return {y:scrollY,left:root.scrollLeft,fits:a.left>=r.left-1 && a.right<=r.right+1};
      })()`);
      const after = await snapshot(client);
      check(`${mode}: mobile horizontal alignment preserves requested body position`, aligned.left>0 && aligned.fits && Math.abs(after.y-requestedY)<=1, {before,requestedY,aligned,after});
      await env.capture(`directory-${mode}-mobile`);
    });
  }
  await scenario('lifecycle', async () => {
    const c = await prepare('time');
    const installed = await listenerCount(client);
    await evaluate(client, `window.__directoryTestTracker = window.__rssrEntryDirectoryTracker; document.querySelector('[data-nav="settings"]').click()`);
    await settle(client, `!!document.querySelector('[data-page="settings"]')`, 'leave entries');
    await evaluate(client, `window.dispatchEvent(new Event('resize'))`);
    await settle(client, `!window.__rssrEntryDirectoryTracker`, 'tracker must clean up after leaving entries');
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
