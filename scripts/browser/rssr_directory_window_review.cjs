// Interleavings omitted by the normal large-directory acceptance: real input,
// delayed follow replies, and selection while geometry changes.
const {chromium}=require('playwright');
const fs=require('node:fs/promises');
const path=require('node:path');
const fixture=require('./directory_fixture.cjs');

(async()=>{
  const output=path.resolve(process.env.ARTIFACT_DIR||'target/directory-window-review-20261026');
  await fs.mkdir(output,{recursive:true});
  const browser=await chromium.launch({executablePath:process.env.CHROME_BIN,channel:process.env.CHROME_BIN?undefined:'chrome',headless:true});
  const context=await browser.newContext({viewport:{width:1440,height:900}});
  await fixture.install(context,path.resolve(__dirname,'../..'));
  await context.addInitScript(()=>{
    window.__windowReview={hold:false,held:false};
    const original=window.Function;
    window.Function=new Proxy(original,{construct(target,args){
      const compiled=Reflect.construct(target,args);
      if(args[0]!=='dioxus'||!String(args[1]).includes('const windowId ='))return compiled;
      return function(channel){return compiled.call(this,new Proxy(channel,{get(target,key){
        if(key==='recv')return async()=>{
          const value=await target.recv();
          if(value?.top!=null && value.focus==null && window.__windowReview.hold){
            window.__windowReview.held=true;
            await new Promise(resolve=>window.__windowReview.release=resolve);
          }
          return value;
        };
        const value=Reflect.get(target,key,target);return typeof value==='function'?value.bind(target):value;
      }}));};
    }});
  });
  const page=await context.newPage(),results=[],errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  const rail='[data-directory-consumer="rail"]';
  const idle=()=>page.evaluate(()=>window.__rssrEntryDirectoryTracker.whenSettled());
  const wait=(fn,arg)=>page.waitForFunction(fn,arg,{timeout:5000});
  const state=()=>page.locator(rail).evaluate(e=>{
    const b=e.getBoundingClientRect();
    // Ignore a <=2px clipped sliver at the viewport edge when comparing reflow.
    const first=[...e.querySelectorAll('[data-virtual-index]')].find(r=>r.getBoundingClientRect().bottom>b.top+2);
    return {top:e.scrollTop,mode:e.dataset.directoryMode,first:first?.dataset.directoryAnchor,firstOffset:first?.getBoundingClientRect().top-b.top,
      selected:[...document.querySelectorAll('[data-action="select-entry"]:checked')].map(x=>x.closest('[data-position-entry]').dataset.positionEntry)};
  });
  const reset=async(suffix='')=>{
    await page.setViewportSize({width:1440,height:900});
    await page.goto(fixture.origin+'/__codex/setup-local-auth?seed=directory-window'+suffix);
    await page.locator('[data-virtual-index]').first().waitFor();await idle();
    await page.locator('[data-action="select-entry"]').first().check();
    await page.locator('[data-virtual-index]').first().focus();
  };
  const scenario=async(name,run)=>{
    try{await reset();const evidence=await run();results.push({name,pass:true,evidence});}
    catch(e){results.push({name,pass:false,error:String(e),state:await state().catch(()=>null)});}
  };
  try{
    await scenario('Tab browsing survives hide/reopen without clearing selection',async()=>{
      for(let i=1;i<=30;i++){await page.keyboard.press('Tab');await wait(i=>document.activeElement.dataset.virtualIndex===String(i),i);}
      await idle();const before=await state();
      await page.keyboard.press('Escape');await wait(()=>!document.querySelector('[data-virtual-articles]'));
      await page.locator('[data-action="open-directory"]').click();await idle();const after=await state();
      if(before.mode!=='manual'||Math.abs(before.top-after.top)>2||after.selected.join()!=='600')throw Error(JSON.stringify({before,after}));
      return {before,after};
    });
    await scenario('A delayed automatic reveal cannot overwrite a newer manual scroll',async()=>{
      await page.keyboard.press('End');await wait(()=>document.activeElement.dataset.virtualIndex==='599');await idle();
      await page.evaluate(()=>{window.__windowReview.hold=true;document.querySelector('[data-action="select-entry"]').focus({preventScroll:true});});
      const main=await page.locator('#entry-card-580').boundingBox();
      await page.mouse.move(400,350);await page.mouse.wheel(0,main.y-80);
      await wait(()=>window.__windowReview.held);
      const bounds=await page.locator(rail).boundingBox();
      await page.mouse.move(bounds.x+40,bounds.y+200);await page.mouse.wheel(0,-500);
      await wait(()=>document.querySelector('[data-directory-consumer="rail"]').dataset.directoryMode==='manual');
      // Wait for the actual wheel scroll, not only the input event acknowledgement.
      await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      const before=await state();
      await page.evaluate(()=>{window.__windowReview.hold=false;window.__windowReview.release();});await idle();
      const after=await state();
      if(Math.abs(after.top-before.top)>2||after.mode!=='manual'||after.selected.join()!=='600')throw Error(JSON.stringify({before,after}));
      return {before,after};
    });
    await scenario('Hidden resize restores the same logical row and selection',async()=>{
      await page.keyboard.press('End');await wait(()=>document.activeElement.dataset.virtualIndex==='599');await idle();
      const before=await state();await page.keyboard.press('Escape');
      await page.setViewportSize({width:390,height:844});await page.locator('[data-action="open-directory"]').click();await idle();
      const after=await state();
      if(before.first!==after.first||after.mode!=='manual'||after.selected.join()!=='600')throw Error(JSON.stringify({before,after}));
      return {before,after};
    });
    await scenario('Shift+Tab traverses backwards and outside Tab does not resume follow',async()=>{
      await page.keyboard.press('End');await wait(()=>document.activeElement.dataset.virtualIndex==='599');
      for(let i=598;i>=570;i--){await page.keyboard.press('Shift+Tab');await wait(i=>document.activeElement.dataset.virtualIndex===String(i),i);}
      await idle();const before=await state();
      await page.locator('[data-action="select-entry"]').first().evaluate(e=>e.focus({preventScroll:true}));
      await page.keyboard.press('Tab');await idle();const after=await state();
      if(before.mode!=='manual'||after.mode!=='manual'||Math.abs(before.top-after.top)>2||after.selected.join()!=='600')throw Error(JSON.stringify({before,after}));
      return {before,after};
    });
    await scenario('Read and star updates preserve a distant manual window and selection',async()=>{
      await page.keyboard.press('End');await wait(()=>document.activeElement.dataset.virtualIndex==='599');await idle();
      const before=await state();
      await page.locator('#entry-card-600 [data-action="mark-read"]').evaluate(e=>e.click());
      await wait(()=>document.querySelector('#entry-card-600').dataset.read==='true');
      await page.locator('#entry-card-600 [data-action="toggle-starred"]').evaluate(e=>e.click());
      await wait(()=>document.querySelector('#entry-card-600 [data-action="toggle-starred"]').getAttribute('aria-pressed')==='true');
      await idle();const after=await state();
      if(after.mode!=='manual'||Math.abs(before.top-after.top)>2||after.selected.join()!=='600')throw Error(JSON.stringify({before,after}));
      return {before,after};
    });
    for(const action of ['mark-selected-read','archive-selected'])await scenario(action+' still processes only the selected main-list rows',async()=>{
      if(action==='archive-selected')await reset('&archived=false');
      await page.locator('[data-action="select-entry"]').nth(1).check();
      await page.locator('[data-virtual-index]').first().focus();
      await page.keyboard.press('End');await wait(()=>document.activeElement.dataset.virtualIndex==='599');await idle();
      const before=await state();
      // Dispatch the ordinary control without scrolling the main list to it.
      await page.locator('[data-action="'+action+'"]').evaluate(e=>e.click());
      await wait(()=>!document.querySelector('[data-action="select-entry"]:checked'));
      if(action==='mark-selected-read'){
        await wait(()=>document.querySelector('#entry-card-600')?.dataset.read==='true'&&document.querySelector('#entry-card-599')?.dataset.read==='true');
        const read=await page.locator('[data-position-entry][data-read="true"]').evaluateAll(es=>es.map(e=>e.dataset.positionEntry));
        if(read.join()!=='600,599')throw Error('Unexpected read rows: '+read);
        await idle();const after=await state();
        if(after.mode!=='manual'||Math.abs(before.top-after.top)>2)throw Error(JSON.stringify({before,after}));
      }else{
        await wait(()=>!document.querySelector('#entry-card-600')&&!document.querySelector('#entry-card-599'));
        await wait(()=>document.querySelector('[data-virtual-articles]')?.dataset.virtualCount==='598');
        if(await page.locator('[data-position-entry]').count()!==50)throw Error('Current page no longer full');
      }
      await idle();
      if(!await page.locator('[data-action="mark-selected-read"]').isDisabled()||!await page.locator('[data-action="archive-selected"]').isDisabled())throw Error('Empty-selection actions remain enabled');
      return {before,after:await state(),logical:await page.locator('[data-virtual-articles]').getAttribute('data-virtual-count')};
    });
    await scenario('Multiple expanded windows preserve the middle article across hidden resize',async()=>{
      await reset('&branches=2');
      await page.keyboard.press('End');await wait(()=>document.activeElement.dataset.virtualIndex==='299');
      await page.locator(rail+' [data-directory-anchor="entry-source-2"]').click();
      await page.locator(rail+' [data-directory-anchor="entry-source-2-month-2026-09"][data-can-toggle="true"]').click();await idle();
      await page.locator('#directory-articles-entry-source-2-month-2026-09 [data-virtual-index]').first().focus();
      await page.keyboard.press('Home');await wait(()=>document.activeElement.dataset.virtualIndex==='0');
      for(let i=1;i<=45;i++){await page.keyboard.press('ArrowDown');await wait(i=>document.activeElement.dataset.virtualIndex===String(i),i);}
      await idle();const before=await state();
      await page.keyboard.press('Escape');await page.setViewportSize({width:390,height:844});
      await page.locator('[data-action="open-directory"]').click();await idle();const after=await state();
      if(before.first!==after.first||after.mode!=='manual'||after.selected.join()!=='600'||await page.locator('[data-virtual-articles]').count()!==2)throw Error(JSON.stringify({before,after}));
      return {before,after};
    });
    if(errors.length)results.push({name:'No page errors',pass:false,errors});
  }finally{
    await fs.writeFile(path.join(output,'review.json'),JSON.stringify({results,errors},null,2));
    await browser.close();
  }
  console.log(JSON.stringify(results));if(results.some(r=>!r.pass))process.exitCode=1;
})().catch(e=>{console.error(e);process.exitCode=1;});
