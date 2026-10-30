// The same Reader host lifetime contract in the real same-origin Web app.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require('playwright/test');
const { readerHostLifetime } = require('./reader_host_scenarios.cjs');

(async () => {
  const root = path.resolve(__dirname, '../..');
  const output = path.resolve(process.env.ARTIFACT_DIR || 'target/reader-host-web');
  assert.ok(output.startsWith(path.join(root, 'target') + path.sep));
  await fs.mkdir(output, {recursive:true});
  assert.ok((await fs.realpath(output)).startsWith(await fs.realpath(path.join(root,'target')) + path.sep));
  const workDir = path.join(output, 'server-work');
  assert.equal(await fs.stat(workDir).then(()=>true,()=>false), false, 'use a fresh isolated server work directory');
  const report = {status:'running',checks:[],pageErrors:[],consoleErrors:[]};
  let browser, harness, images, page;
  try {
    const png = await fs.readFile(path.join(root,'icons/icon.png'));
    images = http.createServer((_request,response)=>response.writeHead(200,{'Content-Type':'image/png'}).end(png));
    await new Promise(resolve=>images.listen(0,'127.0.0.1',resolve));
    const imageURL = `http://127.0.0.1:${images.address().port}/reader.png`;
    const { startHarness } = await import('./server_harness.mjs');
    const suffix = process.platform==='win32' ? '.exe' : '';
    const target = process.env.CARGO_TARGET_DIR || path.join(root,'target');
    harness = await startHarness({
      serverBin:process.env.RSSR_WEB_BIN || path.join(target,'debug/rssr-web'+suffix),
      fixtureBin:process.env.RSSR_FIXTURE_BIN || path.join(target,'debug/examples/browser_fixture_library'+suffix),
      staticDir:path.resolve(root,process.env.PUBLIC_DIR || 'target/dx/rssr-app/release/web/public'),
      seedsDir:path.join(root,'tests/fixtures/browser_state'),themesDir:path.join(root,'assets/themes'),
      workDir,artifactDir:output,port:8119,username:'smoke',password:'smoke-pass-123',
    });
    const session = await harness.server.useSeed('directory-contract',{edit:({core,entry_content:content,app_state:app})=>{
      core.settings.entries_page_size=8; core.settings.archive_after_months=120;
      app.entries_workspace.show_archived=false;
      for(const entry of content.entries) entry.content_html = '<p>Reader host lifetime fixture.</p>'.repeat(30)
        + `<img src="${imageURL}" alt="Reader host fixture">` + '<p>After the image.</p>'.repeat(15);
    }});
    browser = await chromium.launch({headless:true,...(process.env.CHROME_BIN ? {executablePath:process.env.CHROME_BIN} : {channel:'chrome'})});
    report.browser=browser.version();report.serverBuild=harness.server.buildId;
    const context = await browser.newContext();
    const split=session.cookie.indexOf('=');
    await context.addCookies([{name:session.cookie.slice(0,split),value:session.cookie.slice(split+1),url:harness.server.origin,httpOnly:true,sameSite:'Strict'}]);
    page=await context.newPage();page.setDefaultTimeout(15000);
    page.on('pageerror',e=>report.pageErrors.push(String(e)));
    page.on('console',m=>{if(m.type()==='error')report.consoleErrors.push(m.text());});
    await page.goto(harness.server.origin+'/entries');
    const entries=()=>page.locator('[data-page="entries"][data-position-ready="true"]').waitFor();
    await entries();
    for(const width of [1280,390]) {
      await page.setViewportSize({width,height:844});
      const facts=await readerHostLifetime({page,
        openReader:()=>page.locator('[data-slot="entry-card-title"]').first().click(),
        ready:()=>page.locator('[data-page="reader"][data-state="loaded"][data-position-ready="true"]').waitFor(),
        entries,
        screenshot:()=>page.screenshot({path:path.join(output,`${width}-cleanup.png`),animations:'disabled'}),
      });
      report.checks.push({width,pass:true,facts});
    }
    assert.deepEqual(report.pageErrors,[]);assert.deepEqual(report.consoleErrors,[]);
    report.status='passed';
  } catch(error) {
    report.status='failed';report.error=String(error.stack);process.exitCode=1;
    await page?.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});
  } finally {
    await browser?.close();await harness?.close();
    images?.closeAllConnections();await new Promise(resolve=>images ? images.close(resolve) : resolve());
    await fs.writeFile(path.join(output,'acceptance.json'),JSON.stringify(report,null,2));
    console.log(JSON.stringify(report));
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
