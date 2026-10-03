import { readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { evaluate } from './cdp_session.mjs';
import { directoryHarness, directoryCases, rail, group, item, click, keyPress, scrollAnchor, expectActive, settle } from './directory_contract.mjs';

const q = JSON.stringify;
const median = values => { const s=[...values].sort((a,b)=>a-b); return (s[Math.floor((s.length-1)/2)]+s[Math.ceil((s.length-1)/2)])/2; };

// Only installed in an explicitly requested benchmark. No production counters,
// global API replacement, or instrumentation in the release application's path.
async function sample(client, selector, eventName, condition, trigger) {
  await evaluate(client, `(() => {
    const target = ${selector === 'window' ? 'window' : `document.querySelector(${q(selector)})`};
    if (!target) throw new Error('Missing benchmark target');
    window.__directorySample = new Promise((resolve, reject) => {
      let frame = 0, start;
      const cleanup = () => { clearTimeout(timer); cancelAnimationFrame(frame); target.removeEventListener(${q(eventName)}, begin, true); };
      window.__directorySampleCancel = cleanup;
      const timer = setTimeout(() => { cleanup(); reject(new Error('directory sample timeout')); }, 6000);
      function begin(event) {
        if (!event.isTrusted) { cleanup(); reject(new Error('untrusted benchmark event')); return; }
        start = performance.now();
        target.removeEventListener(${q(eventName)}, begin, true);
        function inspect() {
          if (${condition}) {
            const domMs = performance.now() - start;
            frame = requestAnimationFrame(() => { frame = requestAnimationFrame(() => {
              const result = {domMs, proxyMs:performance.now()-start, trusted:event.isTrusted};
              cleanup(); resolve(result);
            }); });
          } else frame = requestAnimationFrame(inspect);
        }
        frame = requestAnimationFrame(inspect);
      }
      target.addEventListener(${q(eventName)}, begin, true);
    });
    window.__directorySample.catch(()=>{});
  })()`);
  let primaryError;
  try {
    await trigger();
    return await evaluate(client, 'window.__directorySample');
  } catch (error) {
    primaryError = error;
    throw error;
  } finally {
    try {
      await evaluate(client, 'window.__directorySampleCancel?.(); delete window.__directorySampleCancel; delete window.__directorySample');
    } catch (error) { if (!primaryError) throw error; }
  }
}

export async function measureDirectory(client, env, metadataFile, artifactDir) {
  const metadata = JSON.parse(await readFile(metadataFile, 'utf8'));
  for (const key of ['baselineSha','build','platform','artifactSha256','refreshRateHz']) {
    if (!(key in metadata)) throw new Error(`Performance metadata must explicitly supply ${key} (null if unmeasured)`);
  }
  if (!/^[a-f0-9]{40}$/.test(metadata.baselineSha) || metadata.build !== 'release') throw new Error('Performance baseline requires an exact SHA and release build');
  const batches = metadata.batchCount ?? 3;
  if (!Number.isInteger(batches) || batches < 1 || batches > 10) throw new Error('batchCount must be 1..10');
  const {prepare} = directoryHarness(client,env);
  const report = {
    status:'running', recordedAt:new Date().toISOString(), metadata,
    host:{platform:os.platform(),release:os.release(),arch:os.arch(),cpu:os.cpus()[0]?.model,memoryBytes:os.totalmem()},
    browser:await client.send('Browser.getVersion'),
    dataset:'directory-contract: 6 feeds, 72 entries, page size 50; default theme; time grouping',
    methodology:'trusted scroll/click/key activation event -> expected DOM condition -> two rAF callbacks; excludes CDP transport, input dispatch before event, physical display, and pre-action positioning. DOM polling is frame-quantized. Scroll is programmatically positioned and measured from its browser-generated scroll event.',
    unmeasured:['physical display latency','dropped frames','layout cost','p95','Android WebView','migration candidate comparison'],
    batches:[],
  };
  const output = path.join(artifactDir,'directory-performance.json');
  const flush = () => writeFile(output,JSON.stringify(report,null,2)+'\n');
  try {
    for (let batch=0; batch<batches; batch++) {
      const c = await prepare('time');
      const viewport = await evaluate(client, '({width:innerWidth,height:innerHeight,dpr:devicePixelRatio,visibility:document.visibilityState,focused:document.hasFocus(),screen:{width:screen.width,height:screen.height}})');
      if (viewport.visibility!=='visible') throw new Error('Benchmark page must be visible to the browser');
      if (env.native && !viewport.focused) throw new Error('Native benchmark window must have focus');
      report.viewport = viewport;
      const result = {batch:batch+1,paths:{}, memoryBefore: {
        heap: await client.send('Runtime.getHeapUsage'), dom: await client.send('Memory.getDOMCounters'),
        wasmLinearBytes: await evaluate(client, 'globalThis.__dx_mainWasm?.memory?.buffer?.byteLength ?? null'),
      }};
      report.batches.push(result);
      for (const name of ['scroll-highlight','expand-mouse','expand-Enter','expand-Space','navigate-align']) {
        const samples=[];
        result.paths[name]={warmup:3,samples};
        for (let i=-3; i<20; i++) {
          let value;
          if (name==='scroll-highlight') {
            const forward=i%2===0;
            const target=forward?c.other:c.first, expected=forward?c.otherGroup:c.group;
            await scrollAnchor(client,forward?c.first:c.other,80,2);
            await expectActive(client,forward?c.group:c.otherGroup,forward?c.first:c.other);
            value=await sample(client,'window','scroll',`document.querySelector(${q(group(expected))})?.dataset.active === 'true'`,
              () => evaluate(client, `window.scrollTo({top:scrollY+document.getElementById(${q(target)}).getBoundingClientRect().top-80,behavior:'instant'})`));
          } else if (name.startsWith('expand-')) {
            await scrollAnchor(client,c.other,80,2);
            await expectActive(client,c.otherGroup,c.other);
            if (await evaluate(client,`document.querySelector(${q(group(c.group))}).dataset.openBase === 'true'`)) {
              await click(client,group(c.group));
              await settle(client,`document.querySelector(${q(group(c.group))}).dataset.openBase === 'false'`,'benchmark collapse setup');
            }
            await evaluate(client,`document.querySelector(${q(rail)}).scrollTop=0; document.querySelector(${q(group(c.group))}).focus({preventScroll:true})`);
            const key=name.replace('expand-','');
            value=await sample(client,group(c.group),key==='mouse'?'click':key==='Enter'?'keydown':'keyup',
              `document.querySelector(${q(group(c.group))}).dataset.open === 'true' && document.querySelector(${q(group(c.group))}).dataset.openBase === 'true'`,
              () => key==='mouse'?click(client,group(c.group)):keyPress(client,key));
          } else {
            await scrollAnchor(client,c.first,80,2);
            await expectActive(client,c.group,c.first);
            value=await sample(client,item(c.next),'click',
              `location.hash === ${q('#'+c.next)} && Math.abs(document.getElementById(${q(c.next)}).getBoundingClientRect().top - parseFloat(getComputedStyle(document.getElementById(${q(c.next)})).scrollMarginTop || 0))<=2`,
              () => click(client,item(c.next)));
          }
          if (i===-3) result.paths[name].firstSample = value;
          if (i>=0) samples.push(value);
        }
        const times=samples.map(s=>s.proxyMs);
        result.paths[name].summary={count:times.length,medianMs:median(times),minMs:Math.min(...times),maxMs:Math.max(...times)};
        await flush();
      }
      result.memoryAfter = {heap: await client.send('Runtime.getHeapUsage'), dom: await client.send('Memory.getDOMCounters'),
        wasmLinearBytes: await evaluate(client, 'globalThis.__dx_mainWasm?.memory?.buffer?.byteLength ?? null')};
    }
    report.repeatNoise = Object.fromEntries(Object.keys(report.batches[0].paths).map(name => {
      const medians=report.batches.map(b=>b.paths[name].summary.medianMs);
      return [name,{batchMediansMs:medians,medianRangeMs:Math.max(...medians)-Math.min(...medians)}];
    }));
    await env.capture('directory-performance');
    env.assertThat('directory performance console is clean',env.consoleErrors.length===0,env.consoleErrors);
    report.status='pass';
  } catch (error) {
    report.status='fail'; report.error=String(error); throw error;
  } finally { await flush(); }
}
