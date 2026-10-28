// Exercise startup failures with real child processes and an owned headless CDP endpoint.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const { spawn } = require('node:child_process');
const { DatabaseSync } = require('node:sqlite');
const { chromium } = require('playwright');
const hash = async file => crypto.createHash('sha256').update(await fs.readFile(file)).digest('hex');

(async () => {
  const output = path.resolve(process.argv[2] || 'target/feeds-lifecycle-startup-test');
  await fs.mkdir(output, { recursive: true });
  const receipt = { status: 'running', results: [] };
  await fs.writeFile(path.join(output, 'results.json'), JSON.stringify(receipt, null, 2));
  let context, server;
  try {
    const runner = process.argv[3] || 'feeds';
    assert.ok(['feeds', 'settings-sync'].includes(runner), 'known acceptance runner');
    const script = runner === 'settings-sync' ? 'rssr_settings_sync_acceptance.cjs' : 'rssr_feeds_lifecycle_acceptance.cjs';
    const run = await fs.mkdtemp(path.join(output, 'run-'));
    const database = path.join(run, 'fixture.db');
    const fixture = new DatabaseSync(database);
    try { fixture.exec('CREATE TABLE marker (value TEXT); INSERT INTO marker VALUES (\'unchanged\')'); }
    finally { fixture.close(); }
    const originalHash = await hash(database);
    context = await chromium.launchPersistentContext(path.join(run, 'profile'), {
      channel: 'chrome', headless: true, args: ['--remote-debugging-port=0'],
    });
    const port = (await fs.readFile(path.join(run, 'profile/DevToolsActivePort'), 'utf8')).split('\n')[0];
    const endpoint = `http://127.0.0.1:${port}`;
    server = http.createServer((request, response) => { response.writeHead(503); response.end('fixture unavailable'); });
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
    const cases = [
      { name: 'missing-page', cdp: endpoint, database, error: /explicit target page/ },
      { name: 'missing-database', cdp: endpoint, database: path.join(run, 'missing.db'), error: /unable to open database/i },
      { name: 'rejected-cdp', cdp: `http://127.0.0.1:${server.address().port}`, database, error: /503/ },
    ];
    for (const test of cases) {
      const artifacts = path.join(run, test.name);
      await fs.mkdir(artifacts);
      const reportPath = path.join(artifacts, 'acceptance.json');
      // A failed rerun must not retain an earlier successful receipt.
      await fs.writeFile(reportPath, JSON.stringify({ status: 'passed', oldRun: true }));
      const child = spawn(process.execPath, [path.join(__dirname, script),
        '--cdp', test.cdp, '--database', test.database, '--feed-origin', 'http://127.0.0.1:9277',
        '--artifact-dir', artifacts], { windowsHide: true });
      let log = '', timedOut = false;
      child.stdout.on('data', data => { log += data; }); child.stderr.on('data', data => { log += data; });
      const result = await new Promise((resolve, reject) => {
        const timer = setTimeout(() => { timedOut = true; child.kill(); }, 10000);
        child.on('error', error => { clearTimeout(timer); reject(error); });
        child.on('close', (code, signal) => { clearTimeout(timer); resolve({ code, signal }); });
      });
      await fs.writeFile(path.join(artifacts, 'process.log'), log);
      const report = JSON.parse(await fs.readFile(reportPath, 'utf8'));
      const evidence = { name: test.name, artifacts, ...result, timedOut, report, pass: false };
      receipt.results.push(evidence);
      assert.equal(timedOut, false, `${test.name}: process released its connections`);
      assert.equal(result.code, 1); assert.equal(result.signal, null);
      assert.equal(report.status, 'failed'); assert.equal(report.oldRun, undefined);
      assert.match(report.error, test.error); assert.match(log, test.error);
      assert.deepEqual(report.results, []);
      assert.equal(await hash(database), originalHash, 'fixture database remains unchanged');
      // Closing the script's CDP connection must leave the caller-owned browser alive.
      assert.equal(await context.pages()[0].evaluate(() => location.href), 'about:blank');
      evidence.pass = true;
      console.log('PASS ' + test.name);
    }
    receipt.status = 'passed';
  } catch (error) {
    receipt.status = 'failed'; receipt.error = String(error.stack); throw error;
  } finally {
    const cleanup = await Promise.allSettled([
      (async () => context?.close())(),
      (async () => { if (server?.listening) await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve())); })(),
    ]);
    const errors = cleanup.filter(result => result.status === 'rejected').map(result => result.reason);
    if (errors.length) { receipt.status = 'failed'; receipt.cleanupErrors = errors.map(String); }
    await fs.writeFile(path.join(output, 'results.json'), JSON.stringify(receipt, null, 2));
    if (errors.length) throw new AggregateError(errors, 'startup test cleanup failed');
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
