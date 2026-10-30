// Actual Android DocumentsUI + isolated test host. Does not install/reset apps or own adb mappings.
// Requires scripts/android/theme-import/ThemeImportDocuments.kt in the test APK only.
// node scripts/browser/rssr_theme_import_android.mjs --process-file <run>/process.json
//   --artifact-dir <fresh directory under target>
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
const {serial}=identity,pkg=identity.package,pid=String(identity.pid);
assert.equal(identity.platform,'android');assert.equal(pkg,'io.github.develata.rssreader.backpolicytest');
assert.match(pid,/^\d+$/);assert.match(identity.source,/^[0-9a-f]{40}$/);
const cdp=new URL(identity.cdp);assert.equal(cdp.protocol,'http:');assert.equal(cdp.hostname,'127.0.0.1');
const port=Number(cdp.port);assert.ok(Number.isInteger(port)&&port>=1024&&port<=65535);
const adb=(...args)=>execFileSync('adb',['-s',serial,...args],{windowsHide:true,maxBuffer:40*1024*1024,timeout:30000});
const text=(...args)=>adb(...args).toString('utf8').trim(),sha=b=>createHash('sha256').update(b).digest('hex');
const apkHash=sha(await fs.readFile(identity.apk));assert.equal(apkHash,identity.artifactSha256);
const installed=text('shell','pm','path',pkg).match(/^package:(\/data\/app\/[A-Za-z0-9_+~=/.-]+\/base\.apk)$/)?.[1];assert.ok(installed);
assert.equal(text('shell','sha256sum',installed).split(/\s/)[0],apkHash);
assert.equal(text('shell','run-as',pkg,'pwd'),'/data/user/0/'+pkg);assert.equal(text('shell','pidof',pkg),pid);
assert.ok(text('forward','--list').split(/\r?\n/).some(line=>line.trim()===`${serial} tcp:${port} localabstract:webview_devtools_remote_${pid}`));
const output=path.resolve(args.get('--artifact-dir')),relative=path.relative(path.join(root,'target'),output);
assert.ok(relative&&!relative.startsWith('..')&&!path.isAbsolute(relative));await fs.mkdir(output);
const picker='com.google.android.documentsui/com.android.documentsui.picker.PickActivity';
const activity=pkg+'/dev.dioxus.main.MainActivity';
assert.ok(text('shell','cmd','package','resolve-activity','--brief','-a','android.intent.action.OPEN_DOCUMENT','-c','android.intent.category.OPENABLE','-t','text/css').includes(picker),'picker identity must be reviewed for this device');
const remote='/data/local/tmp/rssr-theme-import-'+pid;adb('shell','mkdir','-p',remote);
const report={status:'running',platform:'android',serial,pkg,apkHash,source:identity.source,sourcePatchSha256:identity.sourcePatchSha256,
 checks:[],pageErrors:[],picker,fixture:'test-only DocumentsProvider; actual system picker and ContentResolver; injected read failure/delay',
 scriptSha256:sha(await fs.readFile(new URL(import.meta.url)))};
let client;
const resumed=()=>text('shell','dumpsys','activity','activities').split(/\r?\n/).filter(l=>/^\s*(?:mResumedActivity:|topResumedActivity=)/.test(l));
const foreground=component=>assert.ok(resumed().some(l=>l.includes(component)),'expected foreground: '+component);
const waitForeground=async component=>{for(let i=0;i<60;i++){if(resumed().some(l=>l.includes(component)))return;await sleep(100)}foreground(component)};
const nativeInput=(component,...args)=>{foreground(component);adb('shell','input',...args)};
const snapshot=async(name,component)=>{
 foreground(component);
 for(let attempt=0;attempt<2;attempt++){
  try{adb('shell','uiautomator','dump','--compressed',remote+'/ui.xml');break}
  catch(error){(report.snapshotAttempts??=[]).push({name,attempt,error:String(error.stderr||error.message)});if(attempt===1)throw error;await sleep(500);foreground(component)}
 }
 const xml=adb('exec-out','cat',remote+'/ui.xml').toString('utf8');await fs.writeFile(path.join(output,name+'.xml'),xml);
 foreground(component);await fs.writeFile(path.join(output,name+'.png'),adb('exec-out','screencap','-p'));return xml;
};
const tapNode=(xml,predicate)=>{
 const node=[...xml.matchAll(/<node\b[^>]*>/g)].map(m=>m[0]).find(predicate);assert.ok(node,'expected picker node');
 const b=node.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/);assert.ok(b);
 nativeInput(picker,'tap',String(Math.floor((+b[1]+ +b[3])/2)),String(Math.floor((+b[2]+ +b[4])/2)));
};
const click=async selector=>{await evaluate(client,`document.querySelector(${JSON.stringify(selector)}).scrollIntoView({block:'center',behavior:'instant'})`);await sleep(100);await pointerClick(client,selector)};
const status=()=>evaluate(client,`document.querySelector('[data-layout="settings-save-region"]')?.innerText||''`);
const form=()=>evaluate(client,`({draft:document.querySelector('#settings-custom-css')?.value,preview:getComputedStyle(document.documentElement).getPropertyValue('--rssr-import-test').trim(),status:document.querySelector('[data-layout="settings-save-region"]')?.innerText})`);
const setCss=async raw=>{await evaluate(client,`(()=>{const e=document.querySelector('#settings-custom-css');e.value=${JSON.stringify(raw)};e.dispatchEvent(new Event('input',{bubbles:true}));})()`);await sleep(150)};
const open=async()=>{foreground(activity);await click('[data-action="import-custom-css-file"]');await waitForeground(picker)};
const choose=async(name,label)=>{
 await open();let xml=await snapshot(label+'-picker',picker);
 if(!xml.includes(`text="${name}.css"`)){
  tapNode(xml,n=>/content-desc="(?:显示根目录|显示根目录列表|Show roots|打开导航抽屉|Open navigation drawer)"/.test(n));
  await sleep(250);xml=await snapshot(label+'-roots',picker);
  tapNode(xml,n=>n.includes('text="RSSR theme import test"'));await sleep(250);
  xml=await snapshot(label+'-documents',picker);
 }
 // DocumentsUI may remember its large grid layout, where the last fixture is off-screen.
 if(xml.includes('resource-id="com.google.android.documentsui:id/sub_menu_list"')){
  tapNode(xml,n=>n.includes('resource-id="com.google.android.documentsui:id/sub_menu_list"'));
  await sleep(200);xml=await snapshot(label+'-list',picker);
 }
 assert.ok(xml.includes('package="com.google.android.documentsui"'));
 tapNode(xml,n=>n.includes(`text="${name}.css"`));await waitForeground(activity);
};
const stored=async name=>{
 const dir=path.join(output,name);await fs.mkdir(dir);
 const archive=path.join(output,name+'.tar');await fs.writeFile(archive,adb('exec-out','run-as',pkg,'tar','cf','-','files/RSS-Reader'));
 execFileSync('tar',['-xf',archive,'-C',dir]);
 const db=new DatabaseSync(path.join(dir,'files/RSS-Reader/library.sqlite3'));
 assert.equal(db.prepare('SELECT COUNT(*) n FROM entries').get().n,100);assert.equal(db.prepare('SELECT COUNT(*) n FROM feeds').get().n,6);
 assert.ok(db.prepare('SELECT url FROM feeds').all().every(f=>new URL(f.url).hostname==='127.0.0.1'));
 const value=JSON.parse(db.prepare("SELECT value FROM app_settings WHERE key='user_settings'").get().value);db.close();return value;
};
const advanced=async()=>{if(!await evaluate(client,`document.querySelector('[data-layout="theme-advanced"]').open`))await click('[data-slot="theme-advanced-toggle"]')};
const pending=async()=>{for(let i=0;i<100;i++){try{if(text('shell','run-as',pkg,'cat','files/theme-import-fixture/opened')==='delayed')return}catch{}await sleep(100)}throw Error('provider did not open delayed stream')};
const resetDelay=()=>{adb('shell','run-as',pkg,'mkdir','-p','files/theme-import-fixture');adb('shell','run-as',pkg,'rm','-f','files/theme-import-fixture/release','files/theme-import-fixture/opened')};
const release=()=>adb('shell','run-as',pkg,'touch','files/theme-import-fixture/release');
const check=async(name,run)=>{try{await run();report.checks.push({name,pass:true});console.log('PASS '+name)}catch(error){report.checks.push({name,pass:false,error:String(error.stack)});throw error}};
try{
 let target;for(let i=0;i<100&&!target;i++){try{target=(await(await fetch(cdp+'json/list',{signal:AbortSignal.timeout(1000)})).json()).find(t=>t.type==='page'&&/dioxus/.test(t.url))}catch{}if(!target)await sleep(100)}
 assert.ok(target);client=connect(target.webSocketDebuggerUrl);client.on('Runtime.exceptionThrown',e=>report.pageErrors.push(e.exceptionDetails));
 await client.send('Runtime.enable');await client.send('Page.enable');report.webViewVersion=(await client.send('Browser.getVersion')).product;
 report.androidVersion=text('shell','getprop','ro.build.version.release');report.sdk=text('shell','getprop','ro.build.version.sdk');
 await selectorExists(client,'[data-page="entries"][data-position-ready="true"]',20000);
 await click('[data-nav="settings"]');await selectorExists(client,'#settings-theme-mode');await advanced();
 const original=await stored('initial-db');assert.equal(original.archive_after_months,6);assert.equal(original.refresh_interval_minutes,525600);
 const valid='\uFEFF/* Android 导入 */\r\n:root { --rssr-import-test: 37; }\r\n';
 const displayedValid=valid.replaceAll('\r\n','\n'); // HTML textarea normalizes line endings; SQLite must retain the imported bytes.
 if(args.has('--bom-mode')){
  assert.ok(['baseline','fixed'].includes(args.get('--bom-mode')));report.bomCases=[];
  await check('leading-bom-render-'+args.get('--bom-mode'),async()=>{
   for(const raw of [displayedValid.slice(1),displayedValid]){
    await setCss(raw);await click('[data-action="apply-custom-css"]');await sleep(300);
    const actual=await evaluate(client,`({draft:document.querySelector('#settings-custom-css').value,style:document.querySelector('#user-custom-css').textContent,rules:Array.from(document.querySelector('#user-custom-css').sheet.cssRules,r=>r.cssText),preview:getComputedStyle(document.documentElement).getPropertyValue('--rssr-import-test').trim()})`);
    report.bomCases.push(actual);assert.equal(actual.draft,raw);
    assert.equal(actual.style,args.get('--bom-mode')==='fixed'?raw.replace(/^\uFEFF/,''):raw);
    assert.equal(actual.preview,raw.startsWith('\uFEFF')&&args.get('--bom-mode')==='baseline'?'':'37');
   }
   assert.deepEqual(await stored('bom-preview-db'),original);await snapshot('bom-'+args.get('--bom-mode'),activity);
  });
 }else{
 await check('system-picker-cancel-preserves-draft-preview-and-storage',async()=>{
  const before=await form();await open();await snapshot('cancel-picker',picker);nativeInput(picker,'keyevent','4');await waitForeground(activity);
  await waitFor(client,`document.body.innerText.includes('已取消载入 CSS 文件')`);
  const after=await form();assert.equal(after.draft,before.draft);assert.equal(after.preview,before.preview);assert.deepEqual(await stored('cancel-db'),original);
 });
 await check('utf8-bom-crlf-import-previews-exact-text-without-saving',async()=>{
  await choose('valid','valid');await waitFor(client,`document.querySelector('#settings-custom-css').value===${JSON.stringify(displayedValid)}`);
  await waitFor(client,`getComputedStyle(document.documentElement).getPropertyValue('--rssr-import-test').trim()==='37'`);
  assert.equal((await form()).preview,'37');assert.match(await status(),/尚未保存/);assert.deepEqual(await stored('preview-db'),original);
  await snapshot('valid-preview',activity);
 });
 for(const [name,message] of [['invalid-utf8','不是有效 UTF-8'],['oversize','不能超过 65536 字节'],['invalid-css','自定义 CSS 格式无效'],['read-failure','RSSR injected provider read failure']]){
  await check(name+'-keeps-current-draft-preview-and-storage',async()=>{
   const before=await form();await choose(name,name);await waitFor(client,`document.body.innerText.includes(${JSON.stringify(message)})`);
   const after=await form();assert.equal(after.draft,before.draft);assert.equal(after.preview,before.preview);assert.deepEqual(await stored(name+'-db'),original);
  });
 }
 await check('exact-65536-byte-file-is-admitted',async()=>{
  await choose('boundary','boundary');await waitFor(client,`document.querySelector('#settings-custom-css').value.length===65536`);
  assert.match(await status(),/已从文件载入并预览/);assert.deepEqual(await stored('boundary-db'),original);
 });
 await check('later-edit-rejects-delayed-file-and-feedback',async()=>{
  resetDelay();await choose('delayed','delayed-edit');await pending();await setCss('/* later edit */');const before=await form();release();await sleep(700);
  assert.deepEqual(await form(),before);assert.deepEqual(await stored('delayed-edit-db'),original);
 });
 await check('page-disposal-releases-waiter-and-old-provider-cannot-revive',async()=>{
  resetDelay();await choose('delayed','delayed-leave');await pending();await click('[data-action="activate-home"]');await selectorExists(client,'[data-page="entries"]');
  await click('[data-nav="settings"]');await selectorExists(client,'#settings-theme-mode');await advanced();
  // Physical provider still owns the old read. New caller must fail without stealing it.
  await click('[data-action="import-custom-css-file"]');await waitFor(client,`document.body.innerText.includes('已有主题文件正在载入')`);
  const before=await form();release();await sleep(700);assert.deepEqual(await form(),before);
  await choose('valid','retry-after-disposal');await waitFor(client,`document.querySelector('#settings-custom-css').value===${JSON.stringify(displayedValid)}`);
  assert.equal((await form()).preview,'37');assert.deepEqual(await stored('retry-db'),original);
 });
 await check('explicit-save-persists-only-current-import-and-reopens',async()=>{
  await click('[data-action="save-settings"]');await waitFor(client,`document.querySelector('[data-layout="settings-save-region"]').dataset.state==='clean'`,10000);
  assert.deepEqual(await stored('saved-db'),{...original,custom_css:valid});
  await click('[data-action="activate-home"]');await selectorExists(client,'[data-page="entries"]');await click('[data-nav="settings"]');await selectorExists(client,'#settings-theme-mode');await advanced();
  await waitFor(client,`document.querySelector('#settings-custom-css').value===${JSON.stringify(displayedValid)}`);assert.equal((await form()).preview,'37');await snapshot('saved-reopened',activity);
 });
 }
 assert.deepEqual(report.pageErrors,[]);report.status='pass';
}catch(error){report.status='fail';report.error=String(error.stack);process.exitCode=1;
 if(client)try{report.lastPage=await evaluate(client,'document.body.innerText')}catch{}
 if(client)try{report.themeDebug=await evaluate(client,`({draft:document.querySelector('#settings-custom-css')?.value,style:document.querySelector('#user-custom-css')?.textContent,rules:Array.from(document.querySelector('#user-custom-css')?.sheet?.cssRules||[],r=>r.cssText),preview:getComputedStyle(document.documentElement).getPropertyValue('--rssr-import-test')})`)}catch{}
}finally{
 // Only this test provider's gate; never touch documents selected from another provider.
 if(!args.has('--bom-mode'))try{release()}catch{}
 try{
  const state=text('shell','dumpsys','activity','activities');
  const top=state.split(/\* Hist\s+#\d+:/)[1]?.split(/\* Hist\s+#\d+:/)[0]||'';
  if(resumed().some(l=>l.includes(picker))&&top.includes('launchedFromPackage='+pkg+' ')){
   nativeInput(picker,'keyevent','4');await waitForeground(activity);report.cancelledOwnedPickerOnExit=true;
  }
 }catch(error){report.pickerCleanupError=String(error.message)}
 client?.close();
 try{await fs.writeFile(path.join(output,'app.log'),adb('logcat','-d','--pid='+pid,'-v','brief'))}catch(error){report.logCaptureError=String(error.message)}
 await fs.writeFile(path.join(output,'acceptance.json'),JSON.stringify(report,null,2));
}
