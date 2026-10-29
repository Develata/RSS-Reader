// Run the small-directory contracts against the isolated real-server fixture.
// Historical filename retained for callers; no BrowserStore or mocked API is used.
const {chromium} = require('playwright');
const fs = require('node:fs/promises');
const path = require('node:path');
const fixture = require('./directory_fixture.cjs');
(async()=>{
  const repo = path.resolve(__dirname,'../..');
  const output = path.resolve(process.env.ARTIFACT_DIR || 'target/directory-window-20261026/regression');
  await fs.mkdir(output,{recursive:true});
  const browser = await chromium.launch({executablePath:process.env.CHROME_BIN,channel:process.env.CHROME_BIN?undefined:'chrome',headless:true});
  const context = await browser.newContext();
  await fixture.install(context,repo);
  const page = await context.newPage(), client = await context.newCDPSession(page);
  const assertions = [], consoleErrors = [];
  page.on('pageerror',error=>consoleErrors.push(error.message));
  const {checkDirectoryContracts} = await import('./directory_contract.mjs');
  const {capturePage} = await import('./server_page.mjs');
  try {
    await checkDirectoryContracts(client,{
      native:false, consoleErrors,
      assertThat(name,condition,details){assertions.push({name,pass:!!condition,details});if(!condition)throw Error(name);},
      async setViewport(width,height,mobile,deviceScaleFactor){
        await client.send('Emulation.setDeviceMetricsOverride',{width,height,mobile,deviceScaleFactor});
        await client.send('Emulation.setTouchEmulationEnabled',{enabled:mobile,maxTouchPoints:5});
      },
      async seed(seed,route){
        await page.goto(fixture.origin+'/__codex/setup-local-auth?seed='+seed+'&next='+encodeURIComponent(route));
        await page.locator('[data-layout="entry-groups"][data-state="populated"]').waitFor();
      },
      // CDP owns the viewport/DPR here. Playwright's cached default viewport
      // clips mobile screenshots incorrectly, so capture through the same session.
      async capture(name){await capturePage(client,output,name);},
      async manualScroll(){
        await page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
        const point=await page.evaluate(()=>({x:innerWidth/2,y:innerHeight/2}));
        await client.send('Input.dispatchMouseEvent',{type:'mouseWheel',...point,deltaX:0,deltaY:1});
        await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      },
    });
    console.log(JSON.stringify({passed:assertions.length,consoleErrors}));
  } finally {
    await fs.writeFile(path.join(output,'acceptance.json'),JSON.stringify({assertions,consoleErrors},null,2));
    await browser.close();
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
