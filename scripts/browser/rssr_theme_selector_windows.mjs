// Theme select in a real Windows host, driven by CDP and observed with Windows UI Automation.
// CDP keys are not a physical keyboard/IME test. Does not activate windows or send global input.
// node scripts/browser/rssr_theme_selector_windows.mjs --app-bin <exe> --seed-dir <seed>
//   --artifact-dir <new directory under target> [--port 9495]
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
import { connect, evaluate, selectorExists, sleep, waitFor } from './cdp_session.mjs';
import { pointerClick } from './server_page.mjs';

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) args.set(process.argv[i], process.argv[i + 1]);
assert.equal(process.platform, 'win32');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const output = path.resolve(args.get('--artifact-dir'));
const relative = path.relative(path.join(root, 'target'), output);
assert.ok(relative && relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative));
await fs.mkdir(output); // A new directory: never overwrite an earlier run or its database.
const exe = path.join(output, 'rssr-app.exe');
await fs.copyFile(path.resolve(args.get('--app-bin')), exe);
const library = path.join(output, 'RSS-Reader');
await fs.mkdir(library);
await fs.copyFile(path.resolve(args.get('--seed-dir'), 'library.sqlite3'), path.join(library, 'library.sqlite3'));
const database = new DatabaseSync(path.join(library, 'library.sqlite3'));
database.exec('PRAGMA busy_timeout=5000');
assert.equal(database.prepare('SELECT COUNT(*) AS n FROM entries').get().n, 100);
assert.equal(database.prepare('SELECT COUNT(*) AS n FROM feeds').get().n, 6);
assert.ok(database.prepare('SELECT url FROM feeds').all().every(feed => new URL(feed.url).hostname === '127.0.0.1'), 'synthetic local-only feeds');
const stored = () => JSON.parse(database.prepare("SELECT value FROM app_settings WHERE key='user_settings'").get().value);
const replace = value => database.prepare("UPDATE app_settings SET value=?, updated_at=? WHERE key='user_settings'")
  .run(JSON.stringify(value), new Date().toISOString());
const patch = value => replace({ ...stored(), ...value });
patch({ theme: 'dark', list_density: 'compact', startup_view: 'all', refresh_interval_minutes: 525600, archive_after_months: 6 });
const original = stored();
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const report = {
  status: 'running', platform: 'windows', exe, artifactSha256: sha(await fs.readFile(exe)),
  scriptSha256: sha(await fs.readFile(fileURLToPath(import.meta.url))), checks: [], pageErrors: [], consoleErrors: [],
  injected: ['external SQLite settings change', 'DOM focus / visibilitychange / online'], actualInput: ['CDP pointer into native host', 'CDP keyboard into WebView2 popup'],
};
const port = Number(args.get('--port') || 9495);
assert.ok(Number.isInteger(port) && port >= 1024 && port <= 65535);
const base = `http://127.0.0.1:${port}`;
const occupied = execFileSync('powershell.exe', ['-NoProfile', '-Command',
  `(Get-NetTCPConnection -LocalPort ${port} -State Listen -ErrorAction SilentlyContinue).Count`
], { encoding: 'utf8', windowsHide: true }).trim();
assert.equal(Number(occupied), 0, 'CDP port must be unused');
const stdout = await fs.open(path.join(output, 'app.stdout.log'), 'w');
const stderr = await fs.open(path.join(output, 'app.stderr.log'), 'w');
let app, client;
const check = async (name, body) => {
  try { await body(); report.checks.push({ name, pass: true }); console.log('PASS ' + name); }
  catch (error) { report.checks.push({ name, pass: false, error: String(error.stack) }); throw error; }
};
try {
  app = spawn(exe, [], { cwd: output, windowsHide: true, stdio: ['ignore', stdout.fd, stderr.fd], env: {
    ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    WEBVIEW2_USER_DATA_FOLDER: path.join(output, 'webview'),
  } });
  report.pid = app.pid;
  let target;
  for (let tries = 0; tries < 150 && !target; tries++) {
    assert.equal(app.exitCode, null, 'native app exited during startup');
    try {
      const response = await fetch(base + '/json/list', { signal: AbortSignal.timeout(1000) });
      target = (await response.json()).find(item => item.type === 'page' && /dioxus/.test(item.url));
    } catch {}
    if (!target) await sleep(100);
  }
  assert.ok(target, 'isolated native CDP page');
  const owners = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-Command',
    `$owner = (Get-NetTCPConnection -LocalPort ${port} -State Listen | Select-Object -First 1).OwningProcess; $chain = @(); for ($i = 0; $i -lt 12 -and $owner; $i++) { $p = Get-CimInstance Win32_Process -Filter "ProcessId = $owner"; if (!$p) { break }; $chain += [int]$p.ProcessId; $owner = $p.ParentProcessId }; ConvertTo-Json -InputObject $chain -Compress`
  ], { encoding: 'utf8', windowsHide: true }).trim());
  assert.ok(owners.includes(app.pid), 'CDP belongs to this test process');
  client = connect(target.webSocketDebuggerUrl);
  client.on('Runtime.exceptionThrown', event => report.pageErrors.push(event.exceptionDetails));
  client.on('Runtime.consoleAPICalled', event => {
    if (event.type === 'error') report.consoleErrors.push(event.args.map(arg => arg.value ?? arg.description));
  });
  await client.send('Runtime.enable');
  await client.send('Page.enable');
  report.webViewVersion = (await client.send('Browser.getVersion')).product;
  const hasTheme = mode => `document.querySelector('.app-shell')?.classList.contains('theme-${mode}')`;
  const field = name => `[data-field="${name}"]`;
  const fieldValue = (name, value) => `document.querySelector('${field(name)}')?.value === ${JSON.stringify(value)}`;
  const triggers = () => evaluate(client, `(() => {
    window.dispatchEvent(new Event('focus'));
    document.dispatchEvent(new Event('visibilitychange'));
    window.dispatchEvent(new Event('online'));
  })()`);
  const snapshot = async (name, keys = '') => {
    const prefix = path.join(output, name);
    for(const token of keys.matchAll(/\{(ESC|HOME|DOWN|ENTER)\}/g)) {
      const [key,code,windowsVirtualKeyCode]=({ESC:['Escape','Escape',27],HOME:['Home','Home',36],DOWN:['ArrowDown','ArrowDown',40],ENTER:['Enter','Enter',13]})[token[1]];
      for(const type of ['keyDown','keyUp']) await client.send('Input.dispatchKeyEvent',{type,key,code,windowsVirtualKeyCode});
    }
    if(keys) await sleep(250);
    execFileSync('powershell.exe', ['-NoProfile', '-File', path.join(root, 'scripts/browser/rssr_theme_selector_windows.ps1'),
      '-RootProcessId', String(app.pid), '-OutputPrefix', prefix, ...(name === 'closed-initial' ? ['-ShowWithoutActivating'] : [])], {windowsHide:true, timeout:30000});
    return JSON.parse(await fs.readFile(prefix + '.json', 'utf8'));
  };
  await selectorExists(client, '[data-page="entries"][data-position-ready="true"]', 20000);
  await waitFor(client, hasTheme('dark'));
  report.entrySpacing = await evaluate(client, `(() => { const s = getComputedStyle(document.querySelector('[data-layout="entry-card"][data-variant="reading"]')); return { minHeight: s.minHeight, paddingTop: s.paddingTop }; })()`);
  await pointerClick(client, '[data-nav="settings"]');
  await waitFor(client, fieldValue('theme-mode', 'dark'));
  await evaluate(client, `window.themeEvents=[]; for(const type of ['input','change']) document.querySelector('#settings-theme-mode').addEventListener(type,e=>window.themeEvents.push({type:e.type,value:e.target.value,trusted:e.isTrusted}));`);
  await snapshot('closed-initial');
  await check('retired-density-uses-default-spacing-without-a-control', async () => {
    assert.equal(original.list_density, 'compact');
    assert.deepEqual(report.entrySpacing, { minHeight: '72px', paddingTop: '10px' });
    assert.ok(await evaluate(client, `document.querySelector('.app-shell').dataset.density === 'comfortable' && !document.querySelector('[data-field="list-density"]') && !document.body.innerText.includes('列表密度')`));
  });
  await check('real-popup-cancel-preserves-draft-and-storage', async () => {
    await pointerClick(client, '#settings-theme-mode');
    await sleep(300);
    report.openPopup = await snapshot('popup-dark');
    const controls = report.openPopup.windows.flatMap(window => window.controls);
    const select = controls.find(control => control.type === 'ControlType.ComboBox' && control.name === '主题');
    assert.equal(select?.framework, 'Chrome');
    assert.equal(select.nativeHandle, 0, 'the HTML select is not a separate Win32 ComboBox');
    assert.deepEqual(controls.filter(control => control.type === 'ControlType.ListItem').map(control => control.name), ['跟随系统','浅色','深色']);
    const canceled = await snapshot('after-escape', '{ESC}');
    assert.ok(!canceled.windows.flatMap(window => window.controls).some(control => control.type === 'ControlType.ListItem'));
    await waitFor(client, fieldValue('theme-mode', 'dark'));
    assert.equal(await evaluate(client, 'document.activeElement?.id'), 'settings-theme-mode');
    assert.deepEqual(await evaluate(client, 'window.themeEvents'), []);
    assert.deepEqual(stored(), original);
  });
  await check('real-keyboard-selection-previews-without-writing', async () => {
    await pointerClick(client, '#settings-theme-mode');
    await sleep(200);
    await snapshot('selected-light', '{HOME}{DOWN}{ENTER}');
    await waitFor(client, fieldValue('theme-mode', 'light'));
    await waitFor(client, hasTheme('light'));
    report.userEvents = await evaluate(client, 'window.themeEvents');
    assert.ok(report.userEvents.some(e=>e.type==='change' && e.value==='light' && e.trusted));
    assert.deepEqual(stored(), original);
    await pointerClick(client, '#settings-theme-mode');
    await sleep(200);
    await snapshot('pending-light-popup');
    await snapshot('pending-light-canceled', '{ESC}');
    await waitFor(client, fieldValue('theme-mode', 'light'));
    assert.deepEqual(await evaluate(client, 'window.themeEvents'), report.userEvents);
  });
  await check('reread-preserves-pending-and-does-not-emit-input', async () => {
    patch({theme:'system',startup_view:'last_feed'});
    const external=stored();
    await triggers();
    await waitFor(client, fieldValue('startup-view','last_feed'));
    assert.ok(await evaluate(client, fieldValue('theme-mode','light')));
    assert.deepEqual(await evaluate(client, 'window.themeEvents'), report.userEvents);
    assert.deepEqual(stored(),external);
  });
  await check('explicit-save-persists-only-selected-theme', async () => {
    const before=stored();
    await evaluate(client, `document.querySelector('[data-action="save-settings"]').scrollIntoView({block:'center',behavior:'instant'})`);
    await sleep(300);
    report.beforeSave=await evaluate(client, `(()=>{const e=document.querySelector('[data-action="save-settings"]'),r=e.getBoundingClientRect();return {state:document.querySelector('[data-layout="settings-save-region"]').dataset.state,rect:r.toJSON(),hit:document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)?.outerHTML}})()`);
    await pointerClick(client, '[data-action="save-settings"]');
    for(let i=0;i<100 && stored().theme!=='light';i++) await sleep(100);
    report.afterSave=await evaluate(client, `document.querySelector('[data-layout="settings-save-region"]').innerText`);
    assert.deepEqual(stored(), {...before,theme:'light'});
    await pointerClick(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-page="entries"][data-position-ready="true"]');
    await waitFor(client, hasTheme('light'));
    await pointerClick(client, '[data-nav="settings"]');
    await waitFor(client, fieldValue('theme-mode','light'));
    await snapshot('saved-light');
  });
  assert.deepEqual(report.pageErrors, []);
  assert.deepEqual(report.consoleErrors, []);
  report.status = 'pass';
} catch (error) {
  report.status = 'fail'; report.error = String(error.stack); process.exitCode = 1;
} finally {
  await fs.writeFile(path.join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
  client?.close();
  if (app && app.exitCode === null) {
    execFileSync('powershell.exe', ['-NoProfile', '-Command',
      `(Get-Process -Id ${app.pid} -ErrorAction SilentlyContinue).CloseMainWindow() | Out-Null`
    ], { windowsHide: true });
    for (let tries = 0; tries < 50 && app.exitCode === null; tries++) await sleep(100);
    if (app.exitCode === null) app.kill();
  }
  database.close(); await stdout.close(); await stderr.close();
}
