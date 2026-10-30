// Real Windows host + isolated SQLite. DOM focus/visible/online events are controlled inputs;
// this does not claim OS foreground, IME, or Android acceptance. No production data is opened.
// node scripts/browser/rssr_shell_settings_acceptance.mjs --app-bin <exe> --seed-dir <seed>
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
const stored = () => JSON.parse(database.prepare("SELECT value FROM app_settings WHERE key='user_settings'").get().value);
const replace = value => database.prepare("UPDATE app_settings SET value=?, updated_at=? WHERE key='user_settings'")
  .run(JSON.stringify(value), new Date().toISOString());
const patch = value => replace({ ...stored(), ...value });
patch({ theme: 'dark', list_density: 'comfortable', refresh_interval_minutes: 525600 });
const original = stored();
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const report = {
  status: 'running', platform: 'windows', exe, artifactSha256: sha(await fs.readFile(exe)),
  scriptSha256: sha(await fs.readFile(fileURLToPath(import.meta.url))), checks: [], pageErrors: [], consoleErrors: [],
  injected: ['external SQLite settings change', 'DOM focus / visibilitychange / online'],
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
  const screenshot = async name => {
    const { data } = await client.send('Page.captureScreenshot', { format: 'png' });
    await fs.writeFile(path.join(output, name + '.png'), Buffer.from(data, 'base64'));
  };
  await check('startup-applies-stored-settings', async () => {
    await selectorExists(client, '[data-page="entries"][data-position-ready="true"]', 20000);
    await waitFor(client, hasTheme('dark'));
    assert.deepEqual(stored(), original);
    await screenshot('startup');
  });
  await check('app-reread-applies-external-settings-without-writing', async () => {
    patch({ theme: 'light' });
    const external = stored();
    await triggers();
    await waitFor(client, hasTheme('light'));
    assert.deepEqual(stored(), external);
    await screenshot('reread');
  });
  await check('settings-page-reread-preserves-pending-fields', async () => {
    await pointerClick(client, '[data-nav="settings"]');
    await waitFor(client, fieldValue('theme-mode', 'light'));
    await evaluate(client, `(() => { const select = document.querySelector('${field('theme-mode')}');
      select.value = 'dark'; select.dispatchEvent(new Event('change', { bubbles: true })); })()`);
    await waitFor(client, fieldValue('theme-mode', 'dark'));
    await waitFor(client, hasTheme('dark'), 10000);
    patch({ theme: 'system', list_density: 'compact' });
    const external = stored();
    await triggers();
    await waitFor(client, fieldValue('list-density', 'compact'));
    assert.ok(await evaluate(client, fieldValue('theme-mode', 'dark')));
    assert.deepEqual(stored(), external, 'reread and unsaved input did not write settings');
    await screenshot('settings-pending');
    await pointerClick(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-page="entries"][data-position-ready="true"]');
    await waitFor(client, hasTheme('system'));
    assert.deepEqual(stored(), external, 'leaving preview restored stored settings');
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
