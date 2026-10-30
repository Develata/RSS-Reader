// Tests an already launched isolated Android host. Does not install/reset packages or manage mappings.
// node scripts/browser/rssr_theme_selector_android.mjs --process-file <run>/process.json
//   --artifact-dir <new directory under target>
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {DatabaseSync} from 'node:sqlite';
import {connect,evaluate,waitFor,selectorExists,sleep} from './cdp_session.mjs';
import {pointerClick} from './server_page.mjs';
const args=new Map();for(let i=2;i<process.argv.length;i+=2)args.set(process.argv[i],process.argv[i+1]);
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const identity=JSON.parse((await fs.readFile(path.resolve(args.get('--process-file')),'utf8')).replace(/^\uFEFF/,''));
assert.equal(identity.platform,'android');
assert.match(identity.source,/^[0-9a-f]{40}$/);
const serial=identity.serial,pkg=identity.package,pid=String(identity.pid);
assert.equal(pkg,'io.github.develata.rssreader.backpolicytest');assert.match(pid,/^\d+$/);
const cdp=new URL(identity.cdp);assert.equal(cdp.protocol,'http:');assert.equal(cdp.hostname,'127.0.0.1');
const port=Number(cdp.port);assert.ok(Number.isInteger(port)&&port>=1024&&port<=65535);
const adb=(...args)=>execFileSync('adb',['-s',serial,...args],{windowsHide:true,maxBuffer:40*1024*1024,timeout:30000});
const text=(...args)=>adb(...args).toString('utf8').trim();
const sha=b=>createHash('sha256').update(b).digest('hex');
const apkHash=sha(await fs.readFile(path.resolve(identity.apk)));assert.equal(apkHash,identity.artifactSha256);
const installed=text('shell','pm','path',pkg).match(/^package:(\/data\/app\/[A-Za-z0-9_+~=/.-]+\/base\.apk)$/)?.[1];assert.ok(installed);
assert.equal(text('shell','sha256sum',installed).split(/\s/)[0],apkHash);
assert.equal(text('shell','run-as',pkg,'pwd'),'/data/user/0/'+pkg);
assert.equal(text('shell','pidof',pkg),pid);
assert.ok(text('forward','--list').split(/\r?\n/).some(line=>line.trim()===`${serial} tcp:${port} localabstract:webview_devtools_remote_${pid}`));
const output=path.resolve(args.get('--artifact-dir')),relative=path.relative(path.join(root,'target'),output);
assert.ok(relative&&relative!=='..'&&!relative.startsWith('..'+path.sep)&&!path.isAbsolute(relative));await fs.mkdir(output);
const remote='/data/local/tmp/rssr-theme-selector-'+pid;adb('shell','mkdir','-p',remote);
const report={status:'running',platform:'android',serial,pkg,apkHash,source:identity.source,sourcePatchSha256:identity.sourcePatchSha256,checks:[],pageErrors:[],nativeInput:'adb input guarded by resumed Activity',scriptSha256:sha(await fs.readFile(new URL(import.meta.url)))};
let client;
const foreground=()=>{
 const resumed=text('shell','dumpsys','activity','activities').split(/\r?\n/).filter(l=>/^\s*(?:mResumedActivity:|topResumedActivity=)/.test(l));
 assert.ok(resumed.some(l=>l.includes(pkg+'/dev.dioxus.main.MainActivity')), 'Test Activity is not resumed: '+resumed.join(';'));
};
const nativeInput=(...args)=>{foreground();adb('shell','input',...args);};
const snapshot=async name=>{
 foreground();
 for(let attempt=0;attempt<2;attempt++) {
  try {adb('shell','uiautomator','dump','--compressed',remote+'/ui.xml');break}
  catch(error) {
   (report.snapshotAttempts??=[]).push({name,attempt,error:String(error.stderr||error.stdout||error.message)});
   if(attempt===1) throw error;await sleep(500);foreground();
  }
 }
 const xml=adb('exec-out','cat',remote+'/ui.xml').toString('utf8');
 await fs.writeFile(path.join(output,name+'.xml'),xml);
 foreground(); // UI-tree collection can take time: check again before the device screenshot.
 await fs.writeFile(path.join(output,name+'.png'),adb('exec-out','screencap','-p'));
 return xml;
};
const stored=async name=>{
 const tar=path.join(output,name+'.tar'),dir=path.join(output,name);await fs.mkdir(dir);
 await fs.writeFile(tar,adb('exec-out','run-as',pkg,'tar','cf','-','files/RSS-Reader'));
 execFileSync('tar',['-xf',tar,'-C',dir]);
 const db=new DatabaseSync(path.join(dir,'files/RSS-Reader/library.sqlite3'));
 assert.equal(db.prepare('SELECT COUNT(*) AS n FROM entries').get().n,100);
 assert.equal(db.prepare('SELECT COUNT(*) AS n FROM feeds').get().n,6);
 assert.ok(db.prepare('SELECT url FROM feeds').all().every(feed=>new URL(feed.url).hostname==='127.0.0.1'), 'synthetic local-only feeds');
 const s=JSON.parse(db.prepare("SELECT value FROM app_settings WHERE key='user_settings'").get().value);db.close();return s;
};
const check=async(name,fn)=>{try{await fn();report.checks.push({name,pass:true});console.log('PASS '+name)}catch(e){report.checks.push({name,pass:false,error:String(e.stack)});throw e}};
try{
 let target;
 for(let i=0;i<100&&!target;i++){try{target=(await(await fetch(`http://127.0.0.1:${port}/json/list`,{signal:AbortSignal.timeout(1000)})).json()).find(t=>t.type==='page'&&/dioxus/.test(t.url))}catch{} if(!target)await sleep(100)}
 assert.ok(target);client=connect(target.webSocketDebuggerUrl);client.on('Runtime.exceptionThrown',e=>report.pageErrors.push(e.exceptionDetails));
 await client.send('Runtime.enable');await client.send('Page.enable');report.webViewVersion=(await client.send('Browser.getVersion')).product;
 await selectorExists(client,'[data-page="entries"][data-position-ready="true"]',20000);
 report.entrySpacing=await evaluate(client,`(()=>{const s=getComputedStyle(document.querySelector('[data-layout="entry-card"][data-variant="reading"]'));return {minHeight:s.minHeight,paddingTop:s.paddingTop}})()`);
 await pointerClick(client,'[data-nav="settings"]');await waitFor(client,`document.querySelector('#settings-theme-mode')?.value==='dark'`);
 await evaluate(client,`window.themeEvents=[];for(const type of ['input','change'])document.querySelector('#settings-theme-mode').addEventListener(type,e=>window.themeEvents.push({type:e.type,value:e.target.value,trusted:e.isTrusted}));`);
 const original=await stored('initial-db');
 assert.equal(original.theme,'dark');assert.equal(original.archive_after_months,6);
 assert.equal(original.refresh_interval_minutes,525600);
 await check('retired-density-uses-default-spacing-without-a-control',async()=>{
  assert.equal(original.list_density,'compact');
  assert.deepEqual(report.entrySpacing,{minHeight:'72px',paddingTop:'10px'});
  assert.ok(await evaluate(client,`document.querySelector('.app-shell').dataset.density==='comfortable'&&!document.querySelector('[data-field="list-density"]')&&!document.body.innerText.includes('列表密度')`));
  await snapshot('density-retired');
 });
 const open=async()=>{await pointerClick(client,'#settings-theme-mode');await sleep(300)};
 await check('android-native-list-popup-and-system-back-cancel',async()=>{
  await open();const xml=await snapshot('native-popup-dark');
  assert.match(xml,/class="android\.widget\.ListView"/);assert.match(xml,/text="跟随系统"/);assert.match(xml,/text="浅色"/);assert.match(xml,/text="深色"/);
  report.nativePopupClasses=[...new Set([...xml.matchAll(/class="([^"]+)"/g)].map(m=>m[1]))];
  nativeInput('keyevent','4');await sleep(300);const closed=await snapshot('back-canceled');assert.ok(!closed.includes('class="android.widget.ListView"'));
  await selectorExists(client,'[data-page="settings"]');assert.equal(await evaluate(client,`document.querySelector('#settings-theme-mode')?.value`),'dark');
  assert.deepEqual(await evaluate(client,'window.themeEvents'),[]);assert.deepEqual(await stored('canceled-db'),original);
 });
 await check('android-native-selection-previews-without-saving',async()=>{
  await open();const xml=await snapshot('choose-light');
  const node=[...xml.matchAll(/<node\b[^>]*>/g)].map(m=>m[0]).find(n=>n.includes('text="浅色"'));assert.ok(node);
  const b=node.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/);assert.ok(b);
  nativeInput('tap',String(Math.floor((+b[1]+ +b[3])/2)),String(Math.floor((+b[2]+ +b[4])/2)));
  await waitFor(client,`document.querySelector('#settings-theme-mode')?.value==='light' && document.querySelector('.app-shell').classList.contains('theme-light')`);
  report.userEvents=await evaluate(client,'window.themeEvents');assert.ok(report.userEvents.some(e=>e.type==='change'&&e.value==='light'&&e.trusted));
  assert.deepEqual(await stored('preview-db'),original);
  await open();await snapshot('pending-light-popup');nativeInput('keyevent','4');await sleep(250);
  assert.deepEqual(await evaluate(client,'window.themeEvents'),report.userEvents);assert.equal(await evaluate(client,`document.querySelector('#settings-theme-mode')?.value`),'light');
 });
 await check('android-explicit-save-and-reopen',async()=>{
  await evaluate(client,`document.querySelector('[data-action="save-settings"]').scrollIntoView({block:'center',behavior:'instant'})`);await sleep(200);
  await pointerClick(client,'[data-action="save-settings"]');
  await waitFor(client,`document.querySelector('[data-layout="settings-save-region"]').dataset.state==='clean'`,10000);
  assert.deepEqual(await stored('saved-db'),{...original,theme:'light'});
  await pointerClick(client,'[data-action="activate-home"]');await selectorExists(client,'[data-page="entries"]');
  await pointerClick(client,'[data-nav="settings"]');await waitFor(client,`document.querySelector('#settings-theme-mode')?.value==='light'`);
  await snapshot('saved-light');
 });
 assert.deepEqual(report.pageErrors,[]);report.status='pass';
}catch(error){report.status='fail';report.error=String(error.stack);process.exitCode=1;
 if(client)try{report.lastPage=await evaluate(client,`document.body.innerText`)}catch{}
}finally{
 client?.close();
 try{await fs.writeFile(path.join(output,'app.log'),adb('logcat','-d','--pid='+pid,'-v','brief'))}
 catch(error){report.logCaptureError=String(error.message)}
 await fs.writeFile(path.join(output,'acceptance.json'),JSON.stringify(report,null,2));
}
