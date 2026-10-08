import { evaluate } from './cdp_session.mjs';
import { rail, group, click, settle, directoryIdle } from './directory_contract.mjs';

// Literal facts from directory_scroll_*.json, independent of the selector.
const october = 'entry-group-2026-年-10-月';
const september = 'entry-group-2026-年-09-月';
const march = 'entry-group-2026-年-03-月';
const octoberMiddle = 'entry-group-2026-10-03-1003';
const septemberFirst = 'entry-group-2026-09-16-916';
const septemberMiddle = 'entry-group-2026-09-08-908';
const marchLast = 'entry-group-2026-03-01-301';
const q = JSON.stringify;

async function geometry(client) {
  return evaluate(client, `(() => {
    const rail=document.querySelector(${q(rail)}), rect=rail.getBoundingClientRect(), css=getComputedStyle(rail);
    const buttons=[...rail.querySelectorAll('[data-directory-anchor]')].filter(e=>e.getClientRects().length);
    const bounds=e=>({id:e.dataset.directoryAnchor,rect:e.getBoundingClientRect().toJSON()});
    const hittable=e=>{const r=e.getBoundingClientRect(),x=r.left+r.width/2,y=r.top+r.height/2;
      return y>=Math.max(0,rect.top)&&y<Math.min(innerHeight,rect.bottom)&&e.contains(document.elementFromPoint(x,y));};
    const last=buttons.at(-1), lastRect=last.getBoundingClientRect(), clipBottom=rect.top+rail.clientTop+rail.clientHeight;
    return {y:scrollY,width:innerWidth,height:innerHeight,dpr:devicePixelRatio,
      rail:{scrollHeight:rail.scrollHeight,clientHeight:rail.clientHeight,scrollTop:rail.scrollTop,
        clientWidth:rail.clientWidth,offsetWidth:rail.offsetWidth,clientLeft:rail.clientLeft,rect:rect.toJSON(),
        mode:rail.dataset.directoryMode,current:rail.dataset.directoryCurrentGroup,
        css:{top:css.top,maxHeight:css.maxHeight,overflowY:css.overflowY,overscrollBehavior:css.overscrollBehavior},
        last:{...bounds(last),hittable:hittable(last),fullyVisible:lastRect.top>=rect.top&&lastRect.bottom<=Math.min(clipBottom,innerHeight)+1}},
      groups:[...rail.querySelectorAll('[data-directory-kind="group"]')].map(e=>({id:e.dataset.directoryAnchor,
        open:e.dataset.open,dates:e.parentElement.querySelectorAll('[data-directory-kind="item"]').length,hittable:hittable(e)})),
      visibleItems:buttons.filter(e=>e.dataset.directoryKind==='item').map(bounds),
      activeItems:buttons.filter(e=>e.dataset.directoryKind==='item'&&e.dataset.active==='true').map(e=>({...bounds(e),hittable:hittable(e)}))};
  })()`);
}

// Observe real input and every subsequent frame, including transient snapback
// or outer-page displacement. No dispatchEvent, scrollTop writes or production
// event/rAF interception. These are functional traces, not performance samples.
async function traceInput(client, send) {
  const before=await geometry(client);
  await evaluate(client, `(() => {
    const rail=document.querySelector(${q(rail)}), abort=new AbortController();
    const trace={events:[],frames:[]}; window.__directoryScrollProbe=trace;
    const state=()=>({at:performance.now(),y:scrollY,top:rail.scrollTop,mode:rail.dataset.directoryMode,current:rail.dataset.directoryCurrentGroup});
    const record=e=>trace.events.push({...state(),type:e.type,trusted:e.isTrusted,deltaY:e.deltaY,
      target:e.target===document?'document':e.target?.getAttribute?.('data-layout')||e.target?.tagName,
      inside:!!e.target?.closest?.(${q(rail)})});
    for(const type of ['wheel','pointerdown','pointerup','scroll'])document.addEventListener(type,record,{capture:true,passive:true,signal:abort.signal});
    let frame,timer,count=0;
    trace.done=new Promise((resolve,reject)=>{
      timer=setTimeout(()=>{cancelAnimationFrame(frame);abort.abort();reject(new Error('Directory scroll frame timeout'));},6000);
      const tick=()=>{trace.frames.push(state());if(++count===45){clearTimeout(timer);abort.abort();resolve();}else frame=requestAnimationFrame(tick);};
      frame=requestAnimationFrame(tick);
    });
    trace.done.catch(()=>{});
    trace.cleanup=()=>{clearTimeout(timer);cancelAnimationFrame(frame);abort.abort();};
  })()`);
  try {
    const input=await send(before);
    await evaluate(client, 'window.__directoryScrollProbe.done');
    await directoryIdle(client);
    const trace=await evaluate(client, '({events:window.__directoryScrollProbe.events,frames:window.__directoryScrollProbe.frames})');
    return {before,after:await geometry(client),input,...trace};
  } finally {
    await evaluate(client, 'window.__directoryScrollProbe?.cleanup();delete window.__directoryScrollProbe');
  }
}

async function wheel(client, inside, deltaY) {
  return traceInput(client, async before=>{
    const r=before.rail.rect;
    const point=inside?{x:r.left+r.width/2,y:r.top+60}:{x:300,y:450};
    const hit=await evaluate(client, `(() => {const e=document.elementFromPoint(${point.x},${point.y});
      return {tag:e?.tagName,layout:e?.getAttribute('data-layout'),inside:!!e?.closest(${q(rail)})};})()`);
    if(hit.inside!==inside)throw new Error('Wheel point did not hit the intended scroll region');
    await client.send('Input.dispatchMouseEvent',{type:'mouseMoved',...point});
    await client.send('Input.dispatchMouseEvent',{type:'mouseWheel',...point,deltaX:0,deltaY});
    return {...point,deltaY,hit};
  });
}

export async function checkDirectoryScroll(client, env) {
  const check=(name,ok,evidence)=>env.assertThat('natural directory: '+name,ok,evidence);
  const onlyOpen=(state,id)=>state.rail.mode==='follow' && state.rail.current===id &&
    state.groups.length===8 && state.groups.every(g=>g.open===(g.id===id?'true':'false'));
  const activeItem=(state,id)=>state.activeItems.length===1&&state.activeItems[0].id===id&&state.activeItems[0].hittable;
  const isolated=trace=>Math.abs(trace.after.y-trace.before.y)<=1 && trace.frames.every(f=>Math.abs(f.y-trace.before.y)<=1);
  const trustedWheel=trace=>trace.events.some(e=>e.type==='wheel' && e.trusted && e.inside);
  const manual=trace=>trace.after.rail.mode==='manual' &&
    JSON.stringify(trace.after.groups.map(g=>g.open))===JSON.stringify(trace.before.groups.map(g=>g.open));
  async function mainTo(id,top) {
    const delta=await evaluate(client, `document.getElementById(${q(id)}).getBoundingClientRect().top-${top}`);
    // Chromium accumulates fractional wheel deltas across gestures. Quantize
    // the requested input, keeping the strict 0.75px anchor check unchanged.
    const trace=await wheel(client,false,Math.round(delta));
    try {
      await settle(client, `Math.abs(document.getElementById(${q(id)}).getBoundingClientRect().top-${top})<=0.75`, 'literal month boundary position');
    } catch {
      check('main wheel reaches literal anchor '+id,false,{requestedTop:top,
        actualTop:await evaluate(client,`document.getElementById(${q(id)}).getBoundingClientRect().top`),trace});
    }
    return trace;
  }
  async function shortWheel(label) {
    const trace=await wheel(client,true,160);
    check(label+': content fits; every month and final item remain accessible',
      trace.before.rail.scrollHeight===trace.before.rail.clientHeight && trace.after.rail.scrollHeight===trace.after.rail.clientHeight &&
      trace.after.rail.last.fullyVisible && trace.after.groups.every(g=>g.hittable),trace);
    check(label+': trusted wheel enters manual without moving either scroller',
      trustedWheel(trace)&&manual(trace)&&isolated(trace)&&trace.frames.every(f=>Math.abs(f.top)<=1),trace);
  }
  if(!env.native) {
    await env.setViewport(1280,900,false,1);
    await env.seed('directory-scroll','/entries');
  }
  await settle(client, `document.querySelector('[data-page="entries"][data-position-ready="true"]') &&
    document.querySelectorAll('[data-position-entry]').length===26 && !!window.__rssrEntryDirectoryTracker &&
    document.querySelector('[data-action="activate-home"]')?.getAttribute('aria-busy')==='false'`, 'natural scroll fixture ready');
  await env.manualScroll();
  await wheel(client,false,-100000);
  const initial=await geometry(client);
  check('fixed fixture has eight months, four October dates and sixteen September dates',
    initial.groups.length===8 && initial.groups[0].id===october && initial.groups[0].dates===4 &&
    initial.groups[1].id===september && initial.groups[1].dates===16,initial);
  check('start at true page top with natural single-month following',initial.y===0&&onlyOpen(initial,october)&&
    initial.rail.rect.top>parseFloat(initial.rail.css.top),initial);
  await shortWheel('page top');
  await env.capture('directory-natural-top');

  const sticky=await wheel(client,false,250);
  check('actual same-month main wheel restores follow at sticky position',sticky.after.y>sticky.before.y&&onlyOpen(sticky.after,october)&&
    Math.abs(sticky.after.rail.rect.top-parseFloat(sticky.after.rail.css.top))<=1,sticky);
  await shortWheel('sticky October');
  const middle=await mainTo(octoberMiddle,80);
  check('October middle still has only the natural current month open',onlyOpen(middle.after,october)&&activeItem(middle.after,octoberMiddle),middle);
  await shortWheel('October middle');

  await click(client,group(october));
  await settle(client, `document.querySelector(${q(group(october))}).dataset.open==='false'`, 'current month may collapse');
  await click(client,group(september));
  await settle(client, `document.querySelector(${q(group(september))}).dataset.open==='true'`, 'other month may expand');
  const expanded=await wheel(client,true,200);
  check('manual expansion creates real overflow and wheel moves only the rail',
    expanded.before.rail.scrollHeight-expanded.before.rail.clientHeight>50 && expanded.after.rail.scrollTop>expanded.before.rail.scrollTop+20 &&
    trustedWheel(expanded)&&manual(expanded)&&isolated(expanded),expanded);
  check('current month remains collapsed during independent directory browsing',expanded.after.groups.find(g=>g.id===october).open==='false',expanded.after);
  await wheel(client,true,100000);
  await click(client,group(march));
  await settle(client, `document.querySelector(${q(group(march))}).dataset.open==='true'`, 'last month expands');
  await wheel(client,true,100000);
  const end=await wheel(client,true,100000);
  check('last month date is reachable after scrolling and expanding',end.after.rail.last.id===marchLast&&
    end.after.rail.last.hittable&&end.after.rail.last.fullyVisible&&isolated(end),end);
  const boundary=await wheel(client,true,200);
  check('wheel at the directory end never chains into the main list',trustedWheel(boundary)&&manual(boundary)&&isolated(boundary)&&
    Math.abs(boundary.before.rail.scrollTop-(boundary.before.rail.scrollHeight-boundary.before.rail.clientHeight))<=1,boundary);
  await env.capture('directory-natural-manual-end');

  const resume=await wheel(client,false,24);
  check('same-month actual main scroll clears all manual choices and realigns',resume.after.y>resume.before.y&&
    onlyOpen(resume.after,october)&&resume.after.rail.scrollTop===0,resume);
  const beforeMonth=await mainTo(septemberFirst,97);
  check('97px side of month threshold still follows October',onlyOpen(beforeMonth.after,october),beforeMonth);
  const afterMonth=await mainTo(septemberFirst,95);
  check('95px side follows September and naturally overflows',onlyOpen(afterMonth.after,september)&&activeItem(afterMonth.after,septemberFirst)&&
    afterMonth.after.rail.scrollHeight-afterMonth.after.rail.clientHeight>50,afterMonth);

  const drag=await traceInput(client,async before=>{
    const r=before.rail,gutter=r.offsetWidth-r.clientWidth-r.clientLeft;
    if(gutter<2)throw new Error('No physical scrollbar gutter; cannot claim a drag test');
    const track=r.clientHeight-16, thumb=track*r.clientHeight/r.scrollHeight;
    const x=r.rect.right-gutter/2, y=r.rect.top+8+thumb/2+r.scrollTop/r.scrollHeight*track;
    const deltaY=r.scrollTop>(r.scrollHeight-r.clientHeight)/2?-40:40;
    await client.send('Input.dispatchMouseEvent',{type:'mouseMoved',x,y});
    await client.send('Input.dispatchMouseEvent',{type:'mousePressed',x,y,button:'left',buttons:1,clickCount:1});
    try {
      for(let i=1;i<=8;i++)await client.send('Input.dispatchMouseEvent',{type:'mouseMoved',x,y:y+i*deltaY/8,button:'left',buttons:1});
    } finally {
      await client.send('Input.dispatchMouseEvent',{type:'mouseReleased',x,y:y+deltaY,button:'left',buttons:0,clickCount:1});
    }
    return {x,y,deltaY,gutter};
  });
  check('real scrollbar drag starts manual browsing without a main-list jump',
    Math.abs(drag.after.rail.scrollTop-drag.before.rail.scrollTop)>5 && manual(drag)&&isolated(drag)&&
    drag.events.some(e=>e.type==='pointerdown'&&e.trusted&&e.inside),drag);
  const longWheel=await wheel(client,true,120);
  check('natural September overflow responds to wheel without snapback',trustedWheel(longWheel)&&manual(longWheel)&&isolated(longWheel)&&
    longWheel.after.rail.scrollTop>longWheel.before.rail.scrollTop+20 &&
    longWheel.frames.every((f,i,a)=>i===0||f.top>=a[i-1].top-1),longWheel);
  const septemberMid=await mainTo(septemberMiddle,80);
  check('September middle main scroll resumes single-month following',onlyOpen(septemberMid.after,september)&&activeItem(septemberMid.after,septemberMiddle),septemberMid);
  const back=await mainTo(octoberMiddle,80);
  check('return to October restores the short accessible directory',onlyOpen(back.after,october)&&activeItem(back.after,octoberMiddle)&&
    back.after.rail.scrollHeight===back.after.rail.clientHeight&&back.after.rail.last.fullyVisible,back);
  await env.capture('directory-natural-return-october');
}
