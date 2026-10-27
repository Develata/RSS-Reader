// Lifecycle and navigation fences around the real Rust/WASM directory windows.
const {chromium}=require('playwright');
const fs=require('node:fs/promises');
const path=require('node:path');
const fixture=require('./directory_fixture.cjs');

(async()=>{
  const output=path.resolve(process.env.ARTIFACT_DIR||'target/directory-window-boundaries-20261027');
  await fs.mkdir(output,{recursive:true});
  const browser=await chromium.launch({executablePath:process.env.CHROME_BIN,channel:process.env.CHROME_BIN?undefined:'chrome',headless:true});
  const context=await browser.newContext({viewport:{width:1440,height:900}});
  await fixture.install(context,path.resolve(__dirname,'../..'));
  await context.addInitScript(()=>{
    window.__boundary={hold:null,held:null};
    const Original=window.Function;
    window.Function=new Proxy(Original,{construct(target,args){
      const compiled=Reflect.construct(target,args),script=String(args[1]);
      const type=script.includes('const windowId =')?'window':script.includes("const trackerKey = '__rssrEntryDirectoryTracker'")?'directory':null;
      if(args[0]!=='dioxus'||!type)return compiled;
      return function(channel){return compiled.call(this,new Proxy(channel,{get(target,key){
        if(key==='recv')return async()=>{
          const value=await target.recv(),probe=window.__boundary;
          if((probe.hold==='focus'&&type==='window'&&value?.focus!=null)||(probe.hold==='navigate'&&type==='directory'&&value?.navigate)){
            probe.hold=null;probe.held={type,value};
            await new Promise(resolve=>probe.release=resolve);
          }
          return value;
        };
        const value=Reflect.get(target,key,target);return typeof value==='function'?value.bind(target):value;
      }}));};
    }});
  });
  const page=await context.newPage(),results=[],errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  const rail='[data-directory-consumer="rail"]',rows='[data-virtual-index]';
  const wait=(fn,arg)=>page.waitForFunction(fn,arg,{timeout:5000});
  const idle=()=>page.evaluate(()=>window.__rssrEntryDirectoryTracker.whenSettled());
  const snapshot=()=>page.evaluate(()=>{
    const r=document.querySelector('[data-directory-consumer="rail"]');
    return {page:document.querySelector('[data-page="entries"]')?.dataset.positionPage,epoch:r?.dataset.directoryEpoch,
      railTop:r?.scrollTop,y:scrollY,hash:location.hash,mode:r?.dataset.directoryMode,leaf:r?.dataset.directoryCurrentLeaf,
      count:r?.querySelector('[data-virtual-articles]')?.dataset.virtualCount,
      selected:document.querySelectorAll('[data-action="select-entry"]:checked').length,
      focus:document.activeElement?.dataset.field||document.activeElement?.dataset.virtualIndex||document.activeElement?.tagName};
  });
  const reset=async(suffix='',width=1440)=>{
    await page.setViewportSize({width,height:900});
    await page.goto(fixture.origin+'/__codex/setup-local-auth?seed=directory-window'+suffix);
    await page.locator('[data-position-entry]').first().waitFor();
    if(!await page.locator(rail).isVisible())await page.locator('[data-action="open-directory"]').click();
    await page.locator(rows).first().waitFor();await idle();
  };
  const scenario=async(name,run)=>{
    try{const evidence=await run();results.push({name,pass:true,evidence});}
    catch(error){
      results.push({name,pass:false,error:String(error),state:await snapshot().catch(()=>null)});
      const artifact=path.join(output,`failure-${results.length}`);
      await fs.writeFile(artifact+'.html',await page.content());
      await page.screenshot({path:artifact+'.png'});
    }
  };
  try{
    for(const change of ['search','source-filter','page','group','empty-and-restore','reader-return'])await scenario('Retired focus reply after '+change,async()=>{
      await reset(change==='source-filter'?'&branches=2':'');await page.locator('[data-action="select-entry"]').first().check();
      await page.locator(rows).first().focus();await page.evaluate(()=>window.__boundary.hold='focus');
      await page.keyboard.press('End');await wait(()=>window.__boundary.held);
      if(change==='source-filter'){
        await page.locator('[data-action="open-source-filter"]').click();
        await page.getByRole('checkbox',{name:'Alpha',exact:true}).uncheck();
        await page.locator('[data-action="apply-source-filter"]').click();
        await wait(()=>document.querySelector('[data-position-entry]')?.id==='entry-card-300');
      }else if(change==='reader-return'){
        await page.locator('#entry-card-600 [data-slot="entry-card-title"]').click();
        await page.locator('[data-page="reader"][data-position-ready="true"]').waitFor();await page.goBack();
        await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
      }else if(change==='page'){
        await page.locator('[data-action="entry-page-next"]').click();await wait(()=>document.querySelector('[data-page="entries"]').dataset.positionPage==='2');
      }else if(change==='group'){
        await page.locator('[data-action="group-by-time"]').click();await wait(()=>document.querySelector('[data-layout="entry-groups"]').dataset.groupingMode==='time');
      }else{
        await page.locator('[data-action="toggle-search"]').click();
        const input=page.locator('[data-field="entry-search"]');
        await input.fill(change==='search'?'Window 5':'nothing-matches');
        if(change==='search')await wait(()=>document.querySelector('[data-virtual-articles]')?.dataset.virtualCount==='111');
        else{await wait(()=>!document.querySelector('[data-position-entry]'));await input.fill('');await wait(()=>document.querySelector('[data-virtual-articles]')?.dataset.virtualCount==='600');}
      }
      await idle();const before=await snapshot();await page.evaluate(()=>window.__boundary.release());await idle();const after=await snapshot();
      if(Math.abs(before.railTop-after.railTop)>2||after.selected!==0||before.focus!==after.focus||before.page!==after.page)throw Error(JSON.stringify({before,after}));
      return {before,after};
    });
    for(const roundTrip of [false,true])await scenario('A newer page choice cancels the pending directory jump'+(roundTrip?' even after returning to its page':''),async()=>{
      await reset();await page.locator(rows).first().focus();
      for(let i=1;i<=55;i++){await page.keyboard.press('ArrowDown');await wait(i=>document.activeElement.dataset.virtualIndex===String(i),i);}
      await idle();await page.evaluate(()=>window.__boundary.hold='navigate');await page.keyboard.press('Enter');
      await wait(()=>window.__boundary.held&&document.querySelector('[data-page="entries"]').dataset.positionPage==='2');
      await page.locator('[data-action="entry-page-next"]').click();await wait(()=>document.querySelector('[data-page="entries"]').dataset.positionPage==='3');
      if(roundTrip){await page.locator('[data-action="entry-page-previous"]').click();await wait(()=>document.querySelector('[data-page="entries"]').dataset.positionPage==='2');}
      await page.evaluate(()=>window.__boundary.release());
      const settled=await idle().then(()=>true).catch(()=>false),before=await snapshot();
      if(!roundTrip){await page.locator('[data-action="entry-page-previous"]').click();await wait(()=>document.querySelector('[data-page="entries"]').dataset.positionPage==='2');}
      await idle();const after=await snapshot();
      if(!settled||after.hash!==''||after.hash!==before.hash)throw Error(JSON.stringify({settled,before,after}));
      return {settled,before,after};
    });
    await scenario('A new search cancels the pending directory jump',async()=>{
      await reset();await page.locator(rows).first().focus();await page.keyboard.press('End');
      await wait(()=>document.activeElement.dataset.virtualIndex==='599');await idle();
      await page.evaluate(()=>window.__boundary.hold='navigate');await page.keyboard.press('Enter');
      await wait(()=>window.__boundary.held&&document.querySelector('[data-page="entries"]').dataset.positionPage==='12');
      await page.locator('[data-action="toggle-search"]').click();await page.locator('[data-field="entry-search"]').fill('Window 5');
      await wait(()=>document.querySelector('[data-virtual-articles]')?.dataset.virtualCount==='111');
      await page.evaluate(()=>window.__boundary.release());await idle();const after=await snapshot();
      if(after.page!=='1'||after.hash!==''||after.count!=='111'||after.focus!=='entry-search')throw Error(JSON.stringify(after));
      return after;
    });
    for(const mode of ['source','time'])await scenario(mode+' filtered last-page reader return',async()=>{
      await reset('&branches=2&group='+mode);
      await page.locator('[data-action="open-source-filter"]').click();
      await page.getByRole('dialog').getByRole('checkbox',{name:'Alpha',exact:true}).uncheck();
      await page.locator('[data-action="apply-source-filter"]').click();
      await wait(()=>document.querySelector('[data-virtual-articles]')?.dataset.virtualCount==='300');await idle();
      await page.locator(rows).first().focus();await page.keyboard.press('End');await wait(()=>document.activeElement.dataset.virtualIndex==='299');
      await page.keyboard.press('Enter');await wait(()=>document.querySelector('[data-page="entries"]').dataset.positionPage==='6'&&location.hash==='#entry-card-1');await idle();
      const title=page.locator('#entry-card-25 [data-slot="entry-card-title"]');
      await title.scrollIntoViewIfNeeded();
      // Record the actual click boundary, not a position before a queued wheel
      // or smooth directory scroll. This is the same departure the user sees.
      await page.evaluate(()=>{const capture=event=>{if(!event.target.closest('#entry-card-25 [data-slot="entry-card-title"]'))return;window.__listDeparture={y:scrollY,top:document.querySelector('#entry-card-25').getBoundingClientRect().top};document.removeEventListener('click',capture,true);};document.addEventListener('click',capture,true);});
      await title.click();const before=await page.evaluate(()=>window.__listDeparture);
      await page.locator('[data-page="reader"][data-position-ready="true"]').waitFor();await page.goBack();
      await page.locator('[data-page="entries"][data-position-page="6"][data-position-ready="true"]').waitFor();
      await wait(top=>Math.abs(document.querySelector('#entry-card-25')?.getBoundingClientRect().top-top)<3,before.top);await idle();
      const after={...await snapshot(),top:await page.locator('#entry-card-25').evaluate(e=>e.getBoundingClientRect().top)};
      if(after.count!=='300'||after.selected!==0||Math.abs(after.top-before.top)>3)throw Error(JSON.stringify({before,after}));
      await page.locator('[data-action="open-source-filter"]').click();
      const filters={alpha:await page.getByRole('checkbox',{name:'Alpha',exact:true}).isChecked(),bravo:await page.getByRole('checkbox',{name:'Bravo',exact:true}).isChecked()};
      if(filters.alpha||!filters.bravo)throw Error(JSON.stringify(filters));
      await page.getByRole('button',{name:'取消',exact:true}).click();
      return {before,after,filters};
    });
    if(errors.length)results.push({name:'No page errors',pass:false,errors});
  }finally{
    await fs.writeFile(path.join(output,'acceptance.json'),JSON.stringify({results,errors},null,2));await browser.close();
  }
  console.log(JSON.stringify(results));if(results.some(r=>!r.pass))process.exitCode=1;
})().catch(error=>{console.error(error);process.exitCode=1;});
