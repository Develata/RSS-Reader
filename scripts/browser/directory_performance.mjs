import { readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { evaluate } from './cdp_session.mjs';
import { directoryHarness, rail, group, item, click, keyPress, scrollAnchor, expectActive, settle, userWheel } from './directory_contract.mjs';

const q=JSON.stringify;
const median=v=>{const s=[...v].sort((a,b)=>a-b);return(s[Math.floor((s.length-1)/2)]+s[Math.ceil((s.length-1)/2)])/2;};
const p95=v=>[...v].sort((a,b)=>a-b)[Math.ceil(v.length*.95)-1];
const summarize=v=>({count:v.length,medianMs:median(v),p95Ms:p95(v),minMs:Math.min(...v),maxMs:Math.max(...v)});

// Instrumentation is armed before input. Event.timeStamp predates invocation
// regardless of capture/bubble listener order. Mutation/scroll microtasks see
// the first matching DOM; rAF is ONLY the separately reported two-frame proxy.
async function sample(client, selector, eventName, condition, trigger, native) {
  await evaluate(client, `(() => {
    if (${condition}) throw new Error('Benchmark outcome already true before input');
    const target=${q(selector)}, eventName=${q(eventName)};
    let start=null, completed=false, frame=0, listenerLagMs, triggerTrusted;
    const matches=e=>target==='window' || (e.target instanceof Element && !!e.target.closest(target));
    const focused=()=>document.visibilityState==='visible' && (!${q(native)} || document.hasFocus());
    if(!focused()) throw new Error('Benchmark window is not visible/focused');
    window.__directorySample=new Promise((resolve,reject)=>{
      const cleanup=()=>{clearTimeout(timer);cancelAnimationFrame(frame);observer.disconnect();window.removeEventListener(eventName,begin,true);window.removeEventListener('scroll',changed,true);window.removeEventListener('hashchange',changed,true);};
      window.__directorySampleCancel=cleanup;
      const fail=message=>{cleanup();reject(new Error(message));};
      const timer=setTimeout(()=>fail('directory sample timeout'),8000);
      function inspect(phase){
        if(start===null || completed) return;
        if(!focused()) {completed=true;fail('Benchmark lost visibility/focus');return;}
        if(${condition}) {
          completed=true; const domMs=performance.now()-start;
          frame=requestAnimationFrame(()=>{frame=requestAnimationFrame(()=>{
            if(!focused() || !(${condition})) {fail('Outcome changed before proxy completion');return;}
            const result={domMs,proxyMs:performance.now()-start,listenerLagMs,completionPhase:phase,trusted:triggerTrusted};
            cleanup();resolve(result);
          });});
        }
      }
      function begin(e){
        if(start!==null || !matches(e)) return;
        if(!e.isTrusted) {fail('Untrusted benchmark input');return;}
        start=e.timeStamp;listenerLagMs=performance.now()-start;triggerTrusted=e.isTrusted;
        if(listenerLagMs < -1 || listenerLagMs > 3000) {fail('Event timestamp is not in the performance clock domain');return;}
        queueMicrotask(()=>inspect('event-microtask'));
      }
      function changed(){queueMicrotask(()=>inspect('scroll-or-hash-microtask'));}
      const observer=new MutationObserver(()=>inspect('mutation'));
      observer.observe(document.querySelector('[data-page="entries"]'),{subtree:true,childList:true,attributes:true});
      window.addEventListener(eventName,begin,true);
      window.addEventListener('scroll',changed,{capture:true,passive:true});
      window.addEventListener('hashchange',changed,true);
    });window.__directorySample.catch(()=>{});
  })()`);
  try {await trigger();return await evaluate(client,'window.__directorySample');}
  finally {await evaluate(client,'window.__directorySampleCancel?.();delete window.__directorySampleCancel;delete window.__directorySample');}
}

async function drain(client) {
  // Legacy A has no Rust directory bridge. Positive setup conditions below
  // still apply; B additionally drains its real bridge, outside sample timing.
  await evaluate(client,'window.__rssrEntryDirectoryTracker.whenSettled?.()');
}

export async function measureDirectory(client, env, metadataFile, artifactDir) {
  const metadata=JSON.parse(await readFile(metadataFile,'utf8'));
  for(const key of ['baselineSha','build','platform','artifactSha256','refreshRateHz','toolchain','sampleCount','measureFollow']) {
    if(!(key in metadata)) throw new Error(`Performance metadata missing ${key}`);
  }
  if(!/^[a-f0-9]{40}$/.test(metadata.baselineSha)||metadata.build!=='release') throw new Error('Exact baseline SHA and release build required');
  const batches=metadata.batchCount??3,count=metadata.sampleCount;
  if(!Number.isInteger(batches)||batches<1||batches>10||!Number.isInteger(count)||count<5||count>1000) throw new Error('Invalid batch/sample count');
  const {prepare}=directoryHarness(client,env);
  const names=['scroll-item','expand-mouse','expand-Enter','expand-Space','navigate-align',...(metadata.measureFollow?['resume-same-group','resume-cross-group']:[])];
  const report={status:'running',recordedAt:new Date().toISOString(),metadata,
    host:{platform:os.platform(),release:os.release(),arch:os.arch(),cpu:os.cpus()[0]?.model,memoryBytes:os.totalmem()},browser:await client.send('Browser.getVersion'),
    dataset:'directory-contract: 6 feeds, 72 entries, page size 50; default theme; time grouping',
    methodology:'Trusted browser event.timeStamp -> first matching DOM observed in mutation/scroll/event microtask -> two rAF callbacks (proxy only). Same window-capture instrumentation armed before input in A/B; no rAF condition polling. Scroll starts at a real wheel event. Common scroll changes items within one already-open group; normalize other groups closed. Navigation hash and geometry reset identically. Resume is B-only because legacy A does not implement that contract.',
    unmeasured:['photon/display latency','dropped frames','isolated layout cost','hardware/OS input latency','cold/lazy mount','total or live Rust memory','Android native performance'],batches:[]};
  const flush=()=>writeFile(path.join(artifactDir,'directory-performance.json'),JSON.stringify(report,null,2)+'\n');
  try {
    for(let batch=0;batch<batches;batch++) {
      const c=await prepare('time');
      report.viewport=await evaluate(client,'({width:innerWidth,height:innerHeight,dpr:devicePixelRatio,visibility:document.visibilityState,focused:document.hasFocus()})');
      const result={batch:batch+1,paths:{}};report.batches.push(result);
      for(const anchor of await evaluate(client,`[...document.querySelectorAll(${q(rail+' [data-layout="entry-directory-toggle"]')})].filter(e=>e.dataset.directoryAnchor!==${q(c.group)}&&e.dataset.open==='true').map(e=>e.dataset.directoryAnchor)`)) {
        await click(client,group(anchor));await settle(client,`document.querySelector(${q(group(anchor))}).dataset.open==='false'`,'normalize inactive group closed');
      }
      await scrollAnchor(client,c.first,80,2);await userWheel(client,2);await expectActive(client,c.group,c.first);await drain(client);
      for(const name of names) {
        const values=[];result.paths[name]={warmup:3,samples:values};
        for(let i=-3;i<count;i++) {
          let selector='window',event='wheel',condition,trigger;
          if(name==='scroll-item') {
            const target=i%2===0?c.next:c.first,from=i%2===0?c.first:c.next;
            await scrollAnchor(client,from,80,2);await expectActive(client,c.group,from);await drain(client);
            const delta=await evaluate(client,`document.getElementById(${q(target)}).getBoundingClientRect().top-80`);
            condition=`document.querySelector(${q(item(target))})?.dataset.active==='true'`;
            trigger=()=>userWheel(client,delta);
          } else if(name.startsWith('expand-')) {
            await scrollAnchor(client,c.first,80,2);await expectActive(client,c.group,c.first);
            if(await evaluate(client,`document.querySelector(${q(group(c.otherGroup))}).dataset.open==='true'`)) {
              await click(client,group(c.otherGroup));await settle(client,`document.querySelector(${q(group(c.otherGroup))}).dataset.open==='false'`,'collapse setup');
            }
            await drain(client);selector=group(c.otherGroup);const key=name.slice('expand-'.length);
            await evaluate(client,`document.querySelector(${q(selector)}).focus({preventScroll:true})`);
            event=key==='mouse'?'click':key==='Enter'?'keydown':'keyup';
            condition=`document.querySelector(${q(selector)}).dataset.open==='true' && document.querySelector(${q(selector)}).getAttribute('aria-expanded')==='true'`;
            trigger=()=>key==='mouse'?click(client,selector):keyPress(client,key);
          } else if(name==='navigate-align') {
            if(await evaluate(client,`document.querySelector(${q(group(c.otherGroup))}).dataset.open==='true'`)) {
              await click(client,group(c.otherGroup));await settle(client,`document.querySelector(${q(group(c.otherGroup))}).dataset.open==='false'`,'navigation setup collapse');
            }
            await evaluate(client,`history.replaceState(null,'',${q('#'+encodeURIComponent(c.next))})`);
            await scrollAnchor(client,c.first,80,2);await expectActive(client,c.group,c.first);await drain(client);
            selector=item(c.next);event='click';
            condition=`location.hash===${q('#'+encodeURIComponent(c.next))} && Math.abs(document.getElementById(${q(c.next)}).getBoundingClientRect().top-parseFloat(getComputedStyle(document.getElementById(${q(c.next)})).scrollMarginTop||0))<=2`;
            trigger=()=>click(client,selector);
          } else {
            await scrollAnchor(client,c.first,80,2);await userWheel(client,2);await expectActive(client,c.group,c.first);await drain(client);
            await click(client,group(c.group));await settle(client,`document.querySelector(${q(group(c.group))}).dataset.open==='false'`,'manual close setup');
            await click(client,group(c.otherGroup));await settle(client,`document.querySelector(${q(group(c.otherGroup))}).dataset.open==='true'`,'manual other group setup');await drain(client);
            const target=name==='resume-same-group'?c.first:c.other,expected=name==='resume-same-group'?c.group:c.otherGroup;
            const beforeY=await evaluate(client,'scrollY');
            const delta=name==='resume-same-group'?24:await evaluate(client,`document.getElementById(${q(target)}).getBoundingClientRect().top-80`);
            condition=`document.querySelector(${q(rail)}).dataset.directoryMode==='follow' && Math.abs(scrollY-${beforeY})>1 && document.querySelector(${q(item(target))})?.dataset.active==='true' && [...document.querySelectorAll(${q(rail+' [data-layout="entry-directory-toggle"]')})].every(e=>e.dataset.open===(e.dataset.directoryAnchor===${q(expected)}?'true':'false'))`;
            trigger=()=>userWheel(client,delta);
          }
          const value=await sample(client,selector,event,condition,trigger,env.native);
          if(i===-3) result.paths[name].firstWarmup=value;
          if(i>=0) values.push(value);
          await drain(client);
        }
        result.paths[name].summary={dom:summarize(values.map(v=>v.domMs)),proxy:summarize(values.map(v=>v.proxyMs)),listenerLag:summarize(values.map(v=>v.listenerLagMs))};await flush();
      }
    }
    await env.capture('directory-performance');env.assertThat('directory performance console is clean',env.consoleErrors.length===0,env.consoleErrors);report.status='pass';
  } catch(error) {report.status='fail';report.error=String(error);throw error;}
  finally {await flush();}
}
