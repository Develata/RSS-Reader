// Real Chromium/HTTP checks for the test observer and completion predicate.
// No application data or server library is used by this adapter-level test.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const http = require('node:http');
const path = require('node:path');
const { chromium } = require('playwright');
const { installWorkspaceResponseProbe, workspaceResponseCompleted } = require('./workspace_response_probe.cjs');

(async () => {
  const held = new Map();
  const server = http.createServer((request, response) => {
    const url = new URL(request.url, 'http://localhost');
    request.resume();
    request.on('end', () => {
      if (url.pathname === '/break') {
        held.get('cut')?.destroy(); held.delete('cut'); response.end('ok');
      } else if (url.pathname === '/api/v1/workspace') {
        const kind = url.searchParams.get('case');
        response.setHeader('Content-Type', 'application/json');
        if (kind === 'cut') {
          response.setHeader('Content-Length', '20');
          response.write('tr'); held.set(kind, response);
        } else {
          if (kind === 'http-error') response.statusCode = 503;
          response.end(kind === 'invalid-json' ? 'tr' : kind === 'bom-body' ? '\ufefftrue' : 'true');
        }
      } else response.end('<!doctype html><title>Workspace response probe</title>');
    });
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  const report = { cases: [], pageErrors: [] };
  try {
    browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_BIN });
    report.browser = browser.version();
    const page = await browser.newPage();
    page.on('pageerror', error => report.pageErrors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    for (const [kind, expected] of [
      ['never-issued', false], ['headers-only', false], ['without-eof', false],
      ['cut', false], ['http-error', false], ['invalid-json', false], ['bom-body', false],
      ['late-abort', true], ['different-payload', false], ['duplicate', false],
    ]) {
      await page.evaluate(installWorkspaceResponseProbe);
      const payload = JSON.stringify({ read_filter: 'all', grouping_mode: 'time', kind });
      try {
        const outcome = await page.evaluate(async ({ kind, payload }) => {
          const controller = new AbortController();
          let reader;
          window.__probeRequestCleanup = () => {
            controller.abort();
            try { reader?.releaseLock(); } catch { /* the body may already be released */ }
          };
          if (kind === 'never-issued') return {};
          try {
            for (let index = 0; index < (kind === 'duplicate' ? 2 : 1); index++) {
              const response = await fetch(new Request(`/api/v1/workspace?case=${kind}`, {
                method: 'PUT', body: payload, signal: controller.signal,
              }));
              if (kind === 'headers-only') return { status: response.status };
              reader = response.body.getReader();
              let text = '';
              while (true) {
                const chunk = await reader.read();
                if (chunk.done) break;
                text += new TextDecoder().decode(chunk.value);
                if (kind === 'without-eof') return { text };
                if (kind === 'cut') await (await fetch('/break')).text();
              }
              reader.releaseLock();
              if (kind === 'late-abort') controller.abort();
            }
            return {};
          } catch (error) { return { readError: String(error) }; }
        }, { kind, payload });
        const records = await page.evaluate(() => window.__workspaceResponseProbe.records);
        const completed = workspaceResponseCompleted(records, kind === 'different-payload' ? 'another choice' : payload);
        assert.equal(completed, expected, kind);
        if (kind === 'cut') {
          assert.equal(records[0].status, 200);
          assert.ok(records[0].readError, 'cut after the first chunk must reach the actual reader');
        }
        if (kind === 'late-abort') assert.equal(records[0].doneWhenAborted, true);
        report.cases.push({ kind, passed: true, completed, outcome, records });
      } finally {
        await page.evaluate(() => {
          window.__probeRequestCleanup?.(); delete window.__probeRequestCleanup;
          window.__workspaceResponseProbe?.dispose();
        });
      }
    }
    assert.deepEqual(report.pageErrors, []);
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed'; report.error = String(error); throw error;
  } finally {
    try {
      await fs.mkdir(process.env.ARTIFACT_DIR, { recursive: true });
      await fs.writeFile(path.join(process.env.ARTIFACT_DIR, 'response-probe.json'), JSON.stringify(report, null, 2));
    } finally {
      try { await browser?.close(); }
      finally {
        for (const response of held.values()) response.destroy();
        server.close();
      }
    }
  }
  console.log(`PASS response probe: ${report.cases.length} cases`);
})().catch(error => { console.error(error); process.exitCode = 1; });
