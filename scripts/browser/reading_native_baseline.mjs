// Bounded native-host measurements. Requires an explicitly selected, seeded
// test window; never seeds storage, installs an APK, or closes a user process.
import { mkdir, writeFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { connect, evaluate, waitFor, sleep } from './cdp_session.mjs';

const run = promisify(execFile);
const opts = Object.fromEntries(process.argv.slice(2).reduce((pairs, arg, i, all) => {
  if (i % 2 === 0) pairs.push([arg.replace(/^--/, ''), all[i + 1]]);
  return pairs;
}, []));
for (const name of ['cdp-base', 'artifact-dir', 'source', 'artifact-sha', 'platform', 'app-pid']) {
  if (!opts[name]) throw new Error(`Missing --${name}`);
}
if (!/^[a-f0-9]{40}$/i.test(opts.source) || !/^[a-f0-9]{64}$/i.test(opts['artifact-sha'])) {
  throw new Error('Exact source and artifact identities are required');
}
const android = opts.platform === 'android';
if (android ? !opts.serial : !opts.exe) throw new Error('Supply --serial or --exe');
const samples = Number(opts.samples ?? 5);
if (!Number.isInteger(samples) || samples < 1 || samples > 30) throw new Error('Invalid sample count');
const readerSamples = Number(opts['reader-samples'] ?? samples);
if (!Number.isInteger(readerSamples) || readerSamples < 1 || readerSamples > 30) throw new Error('Invalid reader sample count');
const out = path.resolve(opts['artifact-dir']);
await mkdir(out, { recursive: true });
const targets = await (await fetch(`${opts['cdp-base']}/json/list`)).json();
const target = targets.find(t => t.type === 'page' && /dioxus\.index\.html|https?:\/\/(?:dioxus|tauri)\.localhost/.test(t.url));
if (!target) throw new Error('No native Dioxus target at the explicit endpoint');
const c = connect(target.webSocketDebuggerUrl, 30000);
const q = JSON.stringify;
const report = { metadata: opts, target: { id: target.id, url: target.url }, recordedAt: new Date().toISOString(),
  notes: ['Two-rAF completion is a presentation proxy, not an actual displayed-frame timestamp.',
    'Scroll rAF gaps are callback cadence, not compositor jank or FPS.',
    'Memory is sampled after settling, outside timed interactions. No forced GC.'],
  timings: [], memory: [], scroll: [], errors: [] };
c.on('Runtime.exceptionThrown', e => report.errors.push(e));
await c.send('Runtime.enable');
await c.send('Page.enable');
await c.send('Page.bringToFront');
const ev = expr => evaluate(c, expr);
const row = '[data-slot="entry-card-title"]';
const search = '[data-field="entry-search"]';
let firstTitle;
const listReady = `document.querySelector('[data-page="entries"][data-position-ready="true"]')`;
const readerReady = `document.querySelector('[data-page="reader"][data-state="loaded"]') && document.querySelectorAll('article p').length >= 120`;
const directory = '[data-directory-consumer="rail"]';
let rail = directory;
let openedDirectory = false;

async function click(selector) {
  const point = await ev(`(() => {const e=document.querySelector(${q(selector)});if(!e)throw Error('Missing click target');
    e.scrollIntoView({block:'nearest'});const r=e.getBoundingClientRect();if(!r.width||!r.height)throw Error('Hidden click target');
    return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
  if (android) {
    await c.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ ...point, radiusX: 1, radiusY: 1 }] });
    await c.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  } else {
    await c.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
    await c.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
  }
}
async function key(key, code, vk, modifiers = 0) {
  await c.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key, code, windowsVirtualKeyCode: vk, modifiers });
  await c.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode: vk, modifiers });
}
async function selectSearchText() {
  await settle();
  await click(search);
  await waitFor(c, `document.activeElement?.matches(${q(search)})`);
  await key('a', 'KeyA', 65, 2);
}
async function settle() {
  await ev('window.__rssrEntryDirectoryTracker?.whenSettled?.()');
  await sleep(350);
}
async function timed(name, selector, event, condition, trigger) {
  await ev(`(() => {if(${condition})throw Error('Outcome already true before '+${q(name)});
    let start=null,done=false,observer,frame,timer;
    const cleanup=()=>{clearTimeout(timer);cancelAnimationFrame(frame);observer.disconnect();window.removeEventListener(${q(event)},begin,true)};
    function begin(e){if(start!==null||!(e.target instanceof Element)||!e.target.closest(${q(selector)}))return;
      if(!e.isTrusted)throw Error('Untrusted input');start=e.timeStamp;queueMicrotask(check);}
    function check(){if(start===null||done||!(${condition}))return;done=true;
      const domMs=performance.now()-start;frame=requestAnimationFrame(()=>frame=requestAnimationFrame(()=>{
        const result={domMs,proxyMs:performance.now()-start,visible:document.visibilityState,focused:document.hasFocus()};cleanup();window.__readingSampleResolve(result)}));}
    window.__readingSample=new Promise((resolve,reject)=>{window.__readingSampleResolve=resolve;
      timer=setTimeout(()=>{cleanup();reject(Error('Timed out: '+${q(name)}))},12000);
      observer=new MutationObserver(check);observer.observe(document.body,{subtree:true,attributes:true,childList:true,characterData:true});
      window.addEventListener(${q(event)},begin,true);});window.__readingSample.catch(()=>{});
    window.__readingSampleCancel=cleanup;})()`);
  try {
    await trigger();
    const result = await ev('window.__readingSample');
    if (result.visible !== 'visible' || (!android && !result.focused) || result.domMs < 0) throw new Error('Invalid visibility or clock');
    report.timings.push({ name, ...result });
  } finally { await ev('window.__readingSampleCancel?.();delete window.__readingSample;delete window.__readingSampleResolve;delete window.__readingSampleCancel'); }
  await settle();
}
async function adb(...args) {
  return (await run('adb', ['-s', opts.serial, ...args], { timeout: 30000, maxBuffer: 8e6 })).stdout;
}
async function memory(state) {
  await sleep(4000);
  for (let i = 0; i < 3; i++) {
    let snapshot;
    if (android) {
      const processes = await adb('shell', 'dumpsys', 'activity', 'processes');
      const blocks = processes.split(/(?=\s+\*APP\* UID)/);
      const owned = blocks.filter(b => /packageList=\{[^}]*\bio\.github\.develata\.rssreader\b/.test(b));
      const pids = [...new Set(owned.map(b => b.match(/ProcessRecord\{\S+ (\d+):/)?.[1]).filter(Boolean))];
      if (!pids.includes(opts['app-pid']) || pids.length < 2) throw new Error('Cannot establish app plus renderer ownership');
      const rows = [];
      for (const pid of pids) {
        const raw = await adb('shell', 'dumpsys', 'meminfo', pid);
        await writeFile(path.join(out, `${state}-${i}-${pid}.meminfo.txt`), raw);
        const pss = raw.match(/TOTAL PSS:\s*(\d+)/)?.[1] ?? raw.match(/^\s*TOTAL\s+(\d+)/m)?.[1];
        if (!pss) throw new Error(`Missing PSS for ${pid}`);
        rows.push({ pid: Number(pid), pssKiB: Number(pss) });
      }
      snapshot = { rows, totalPssKiB: rows.reduce((sum, p) => sum + p.pssKiB, 0) };
      if (i === 0) await writeFile(path.join(out, `${state}-processes.txt`), processes);
    } else {
      for (let attempt = 0; attempt < 3; attempt++) {
        const raw = await run('pwsh', ['-NoProfile', '-File', path.resolve('scripts/measure_windows_process_tree.ps1'),
          '-AppProcessId', opts['app-pid'], '-ExpectedExecutable', opts.exe], { timeout: 30000, maxBuffer: 3e6 });
        snapshot = JSON.parse(raw.stdout);
        if (snapshot.complete && snapshot.rows.length >= 2) break;
        (report.memoryRetries ??= []).push({state, sample:i, attempt, snapshot});
        await sleep(300);
      }
      if (!snapshot.complete || snapshot.rows.length < 2) throw new Error('Incomplete process-tree memory');
    }
    report.memory.push({ state, sample: i, ...snapshot });
    await sleep(300);
  }
}
async function screenshot(name) {
  const shot = await c.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  await writeFile(path.join(out, name), Buffer.from(shot.data, 'base64'));
}
async function openDirectory() {
  const visible = await ev(`!!document.querySelector(${q(directory)})?.getBoundingClientRect().width`);
  if (visible) { rail = directory; return; }
  await click('[data-action="open-directory"]');
  openedDirectory = true;
  await waitFor(c, `document.querySelector(${q(rail)})?.getBoundingClientRect().width > 0`);
  await settle();
}
async function closeDirectory() {
  if (openedDirectory) {
    await click(`${rail} [data-action="close-directory"]`);
    await settle();
  }
}

async function measure() {
  if (await ev(`!!document.querySelector('[data-page="reader"]')`)) await click('[data-nav="back"]');
  await waitFor(c, listReady);
  if (opts['launched-at']) report.startupObservedMs = Date.now() - Number(opts['launched-at']);
  if (!await ev(`!!document.querySelector(${q(search)})`)) await click('[data-action="toggle-search"]');
  await waitFor(c, `!!document.querySelector(${q(search)})`);
  await selectSearchText(); await key('Backspace', 'Backspace', 8);
  await waitFor(c, `document.querySelectorAll(${q(row)}).length === 50`);
  const href = await ev(`[...document.querySelectorAll(${q(row)})].find(e=>e.textContent.includes('05000'))?.getAttribute('href')`);
  if (!/^\/entries\/\d+$/.test(href)) throw new Error('Missing known fixture article');
  firstTitle = `${row}[href="${href}"]`;
  await click('[data-action="group-by-source"]');
  await settle();
  report.environment = await ev(`({viewport:[innerWidth,innerHeight,devicePixelRatio],userAgent:navigator.userAgent,
    listKey:document.querySelector('[data-page="entries"]').getAttribute('data-position-key'),
    title:document.querySelector(${q(firstTitle)}).textContent,domNodes:document.querySelectorAll('*').length})`);
  if (!report.environment.title.startsWith('阅读基线文章')) throw new Error('Not the isolated reading fixture');
  await click('[data-action="toggle-search"]'); await settle();
  if (opts['prepare-only'] === 'true') { report.status = 'prepared'; return; }
  await memory('list');
  await screenshot('list.png');
  await click('[data-action="toggle-search"]');
  await waitFor(c, `!!document.querySelector(${q(search)})`);
  for (let i = 0; i < samples; i++) {
    await selectSearchText();
    await timed('search-one', search, 'input', `document.querySelectorAll(${q(row)}).length === 1`,
      () => c.send('Input.insertText', { text: '05000' }));
    await selectSearchText();
    await timed('search-clear', search, 'input', `document.querySelectorAll(${q(row)}).length === 50`,
      () => key('Backspace', 'Backspace', 8));
  }
  // Hide search (and the mobile keyboard) before list/directory/reader work.
  await click('[data-action="toggle-search"]'); await settle();
  await openDirectory();
  const source = `${rail} [data-directory-level="source"][data-directory-anchor="entry-source-5"]`;
  if (await ev(`document.querySelector(${q(source)}).getAttribute('aria-expanded') === 'true'`)) { await click(source); await settle(); }
  for (let i = 0; i < samples; i++) {
    await timed('directory-source-open', source, 'click', `document.querySelector(${q(source)})?.getAttribute('aria-expanded') === 'true'`, () => click(source));
    await click(source); await settle();
  }
  await click(source); await settle();
  report.directory = await ev(`({nodes:document.querySelector(${q(rail)}).querySelectorAll('*').length,
    articleLeaves:document.querySelector(${q(rail)}).querySelectorAll('[data-directory-level="article"]').length})`);
  await memory('directory');
  await screenshot('directory.png');
  await closeDirectory();
  for (let i = 0; i < readerSamples; i++) {
    await timed('reader-open', firstTitle, 'click', readerReady, () => click(firstTitle));
    await waitFor(c, `[...document.querySelectorAll('article img')].every(e=>e.complete && e.naturalWidth>0)`);
    await settle();
    if (i === 0) { await memory('reader'); await screenshot('reader.png'); }
    if (android) await adb('shell', 'dumpsys', 'gfxinfo', 'io.github.develata.rssreader', 'reset');
    await ev(`window.__readingFrames=[];window.__readingFrameStop=false;
      requestAnimationFrame(function tick(t){window.__readingFrames.push(t);if(!window.__readingFrameStop)requestAnimationFrame(tick)})`);
    const viewport = await ev('({width:innerWidth,height:innerHeight})');
    await c.send('Input.synthesizeScrollGesture', { x: Math.round(viewport.width * .5), y: Math.round(viewport.height * .65),
      yDistance: -1800, speed: 1100, gestureSourceType: android ? 'touch' : 'mouse' });
    const scroll = await ev(`window.__readingFrameStop=true;({frames:window.__readingFrames,scrollY,bodyLength:document.querySelector('article').innerText.length})`);
    if (scroll.scrollY < 100) throw new Error('Scroll gesture did not move the reader');
    report.scroll.push(scroll); await settle();
    if (android) await writeFile(path.join(out, `scroll-${i}-gfxinfo.txt`),
      await adb('shell', 'dumpsys', 'gfxinfo', 'io.github.develata.rssreader', 'framestats'));
    // Returning is timed separately, with a real click, after the scroll has settled.
    await timed('reader-return', '[data-nav="back"]', 'click', `${listReady} && document.querySelectorAll(${q(row)}).length===50`, () => click('[data-nav="back"]'));
  }
  await memory('returned');
  report.status = 'passed';
}
try {
  await measure();
} catch (error) {
  report.status = 'failed'; report.failure = String(error.stack ?? error); process.exitCode = 1;
  report.failureState = await ev(`({text:document.body.innerText.slice(0,1500),focus:document.activeElement?.outerHTML.slice(0,500),
    search:document.querySelector('[data-field="entry-search"]')?.value,visible:document.visibilityState,focused:document.hasFocus()})`).catch(()=>null);
  await screenshot('failure.png').catch(()=>{});
} finally {
  await writeFile(path.join(out, 'measurements.json'), JSON.stringify(report, null, 2));
  c.close();
}
console.log(JSON.stringify({ status: report.status, timings: report.timings.length, memory: report.memory.length, error: report.failure, out }));
