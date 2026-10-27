const {chromium} = require('playwright');
const fs = require('node:fs/promises');
const path = require('node:path');
const fixture = require('./directory_fixture.cjs');

async function main() {
  const repo = path.resolve(__dirname, '../..');
  const output = path.resolve(process.env.ARTIFACT_DIR || 'target/directory-window-20261026/web');
  await fs.mkdir(output, {recursive:true});
  const browser = await chromium.launch({executablePath:process.env.CHROME_BIN,channel:process.env.CHROME_BIN?undefined:'chrome',headless:true});
  const context = await browser.newContext({viewport:{width:1440,height:900}});
  await fixture.install(context, repo);
  await context.addInitScript(()=>{
    const original = window.Function;
    window.__windowFault = null;
    window.Function = new Proxy(original,{construct(target,args){
      const compiled = Reflect.construct(target,args);
      if(args[0]!=='dioxus' || !String(args[1]).includes('const windowId =')) return compiled;
      return function(channel){return compiled.call(this,new Proxy(channel,{get(target,key){
        if(key==='send') return fact=>{
          const fault=window.__windowFault;
          if(!fault || fact.kind!=='reveal') return target.send(fact);
          window.__windowFault=null;
          return target.send(fault==='null'?null:fault==='kind'?{...fact,kind:'unknown'}:{...fact,sequence:'invalid'});
        };
        const value=Reflect.get(target,key,target);return typeof value==='function'?value.bind(target):value;
      }}));};
    }});
  });
  const page = await context.newPage();
  const errors = [], results = [];
  page.on('pageerror', e=>errors.push(e.message));
  const check = (name, value, details) => {
    results.push({name, pass:!!value, details});
    if (!value) throw new Error(name + ': ' + JSON.stringify(details));
  };
  const idle = () => page.evaluate(()=>window.__rssrEntryDirectoryTracker.whenSettled());
  const rail = '[data-directory-consumer="rail"]';
  const rows = '[data-virtual-index]';
  const focused = index => page.waitForFunction(i=>Number(document.activeElement?.dataset.virtualIndex) === i,index,{timeout:5000});
  const mounted = () => page.locator(rows).count();
  try {
    await page.goto(fixture.origin + '/__codex/setup-local-auth?seed=directory-window');
    await page.locator('[data-page="entries"]').waitFor({timeout:15000});
    await page.locator('[data-position-entry]').first().waitFor();
    await idle();
    const stats = await page.locator('[data-virtual-articles]').evaluateAll(es=>es.map(e=>({total:Number(e.dataset.virtualCount),mounted:e.querySelectorAll('button').length})));
    check('600 logical articles have a bounded mounted window', stats.length === 1 && stats[0].total === 600 && stats[0].mounted > 0 && stats[0].mounted < 60, stats);
    await page.screenshot({path:path.join(output,'source-initial.png')});
    await page.locator('[data-action="select-entry"]').first().check();
    await page.locator('[data-position-entry]').first().evaluate(e=>window.__windowTestCard=e);
    await page.locator(rows).first().focus();
    await page.keyboard.press('End');
    await focused(599); await idle();
    check('End reaches the unmounted last article without mounting the full branch',await mounted()<60,await mounted());
    check('directory navigation preserves selected article and main list identity',await page.evaluate(()=>document.querySelector('[data-action="select-entry"]').checked && window.__windowTestCard===document.querySelector('[data-position-entry]')));
    await page.keyboard.press('Home'); await focused(0); await idle();
    const initialEnd = await page.locator(rows).evaluateAll(es=>Math.max(...es.map(e=>Number(e.dataset.virtualIndex))));
    for(let i=1;i<=initialEnd+2;i++){await page.keyboard.press('Tab');await focused(i);}
    await idle();
    check('Tab crosses the original mounted boundary in logical order',await page.evaluate(i=>Number(document.activeElement.dataset.virtualIndex)===i,initialEnd+2));
    await page.keyboard.press('Enter');
    await page.waitForFunction(()=>location.hash.startsWith('#entry-card-'));
    check('same-page article jump preserves selection',await page.locator('[data-action="select-entry"]').first().isChecked());
    await page.keyboard.press('End');await focused(599);
    await page.keyboard.press('Enter');
    await page.waitForFunction(()=>document.querySelector('[data-page="entries"]')?.dataset.positionPage==='12' && location.hash==='#entry-card-1');
    await idle();
    check('last article uses global page 12 and clears old page selection',await page.locator('[data-action="select-entry"]:checked').count()===0);
    check('cross-page reveal remains bounded',await mounted()>0 && await mounted()<60,await mounted());
    await page.screenshot({path:path.join(output,'source-last-page.png')});
    await page.setViewportSize({width:1080,height:720}); await idle();
    const sizes = await page.locator(rows).evaluateAll(es=>es.map(e=>e.getBoundingClientRect().height));
    check('width change remeasures unequal article heights',Math.max(...sizes)>Math.min(...sizes),sizes);
    await page.setViewportSize({width:390,height:844});
    await page.locator('[data-action="open-directory"]').click(); await idle();
    check('mobile drawer keeps the full logical article branch and bounded DOM',await page.locator('[data-virtual-articles]').getAttribute('data-virtual-count')==='600' && await mounted()>0 && await mounted()<60,await mounted());
    await page.screenshot({path:path.join(output,'mobile-directory.png')});
    await page.goto(fixture.origin+'/__codex/setup-local-auth?seed=directory-window&group=time');
    await page.locator('[data-page="entries"]').waitFor();
    await page.locator('[data-position-entry]').first().waitFor();
    await page.locator('[data-action="open-directory"]').click();await idle();
    check('time mode retains month date and article levels',await page.locator(rail+' [data-directory-level="date"]').count()===1 && await mounted()>0 && await mounted()<60);
    await page.locator(rows).first().focus();await page.keyboard.press('End');await focused(599);
    await page.keyboard.press('Enter');
    await page.waitForFunction(()=>location.hash==='#entry-card-1' && document.querySelector('[data-page="entries"]')?.dataset.positionPage==='12');await idle();
    check('time mode locates the last article across pages',await mounted()>0 && await mounted()<60);
    await page.setViewportSize({width:1440,height:900});
    await page.goto(fixture.origin+'/__codex/setup-local-auth?seed=directory-window');
    await page.locator(rows).first().waitFor();await idle();
    await page.locator('[data-action="select-entry"]').first().check();
    await page.locator(rows).first().focus();await page.keyboard.press('End');await focused(599);await idle();
    const beforeHide = await page.locator(rail).evaluate(e=>e.scrollTop);
    await page.keyboard.press('Escape');
    await page.waitForFunction(()=>!document.querySelector('[data-virtual-articles]'));
    await page.locator('[data-action="open-directory"]').click();await idle();
    const afterShow = await page.locator(rail).evaluate(e=>e.scrollTop);
    check('hide and reopen restore manual scroll and selection',Math.abs(beforeHide-afterShow)<2 && await page.locator('[data-action="select-entry"]').first().isChecked(),{beforeHide,afterShow});
    const mainTarget = await page.locator('#entry-card-580').boundingBox();
    await page.mouse.move(400,350);await page.mouse.wheel(0,mainTarget.y-80);
    await page.waitForFunction(()=>document.querySelector('[data-directory-consumer="rail"]')?.dataset.directoryCurrentLeaf==='entry-card-580');await idle();
    const following = await page.locator(rail).evaluate(e=>{const t=e.querySelector('[data-directory-anchor="entry-card-580"]'), r=e.getBoundingClientRect(), b=t?.getBoundingClientRect();return {mode:e.dataset.directoryMode,visible:!!b&&b.top>=r.top-1&&b.bottom<=r.bottom+1};});
    check('main-list wheel reveals the previously unmounted article and resumes following',following.mode==='follow'&&following.visible,following);
    check('resuming follow does not clear same-page selection',await page.locator('[data-action="select-entry"]').first().isChecked());
    await page.addStyleTag({content:'#reading-directory [data-directory-level="article"] [data-slot="entry-directory-title"] {font-size:20px; -webkit-line-clamp:unset;}'});await idle();
    const tall = await page.locator(rows).evaluateAll(es=>Math.max(...es.map(e=>e.getBoundingClientRect().height)));
    check('custom typography uses actual variable heights without losing the bounded window',tall>100 && await mounted()<60,{tall,mounted:await mounted()});
    await page.locator(rows).first().focus();
    for(const fault of ['kind','type','null']){
      await page.keyboard.press('Home');await focused(0);await idle();
      await page.evaluate(fault=>{window.__windowFault=fault;},fault);
      await page.keyboard.press('End');await idle();
      check('malformed window '+fault+' is rejected without moving focus',await page.evaluate(()=>document.activeElement.dataset.virtualIndex==='0' && window.__windowFault===null));
      await page.keyboard.press('End');await focused(599);await idle();
      check('window bridge continues after '+fault,await mounted()<60);
    }
    check('no unhandled page errors',errors.length === 0,errors);
  } catch (error) {
    console.log('failure state',await page.evaluate(()=>({focus:{tag:document.activeElement?.tagName,index:document.activeElement?.dataset.virtualIndex}, windows:[...document.querySelectorAll('[data-virtual-articles]')].map(e=>({rev:e.dataset.windowRevision,idle:e.__rssrVirtualWindow?.idle,rows:[...e.querySelectorAll('[data-virtual-index]')].map(r=>r.dataset.virtualIndex)})),top:document.querySelector('[data-directory-consumer="rail"]')?.scrollTop})).catch(()=>null));
    throw error;
  } finally {
    await fs.writeFile(path.join(output,'acceptance.json'),JSON.stringify({results,errors},null,2));
    await browser.close();
  }
  console.log(JSON.stringify(results));
}
main().catch(e=>{console.error(e);process.exitCode=1;});
