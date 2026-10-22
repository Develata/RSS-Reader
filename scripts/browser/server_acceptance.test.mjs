// Replay the driver's error classification and orchestration without launching a browser.
// Load the actual functions: importing the CLI would start a real acceptance run.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

const source = readFileSync(new URL('./rssr_server_acceptance.mjs', import.meta.url), 'utf8');
const classification = source.slice(source.indexOf('const networkEntry ='), source.indexOf('async function run()'));
const runner = source.slice(source.indexOf('async function run()'), source.indexOf('\nrun().catch('));
assert.ok(classification && runner, 'driver entry points must be present');
const classify = vm.runInNewContext(`${classification}\nignoredBecause`, { URL });
const expected = { expected: [{ status: 401, path: '/api/v1/settings' }] };
const network = {
  type: 'log.error', source: 'network', page: 'http://localhost:8091',
  url: 'http://localhost:8091/api/v1/settings',
  text: 'Failed to load resource: the server responded with a status of 401 (Unauthorized)',
};

test('only the expected HTTP origin and browser network source may be ignored', () => {
  assert.ok(classify(expected, new Set(), network));
  for (const change of [
    { url: 'https://localhost:8091/api/v1/settings' },
    { url: 'http://localhost:8092/api/v1/settings' },
    { url: 'http://unrelated.example:8091/api/v1/settings' },
    { url: 'http://localhost:8091/api/v1/other' },
    { type: 'exception' }, { type: 'console.error' }, { source: 'javascript' },
  ]) assert.equal(classify(expected, new Set(), { ...network, ...change }), undefined);
});

test('live reload matches the HTTP/WebSocket scheme pair and rejects malformed URLs', () => {
  for (const [page, socket, ignored] of [
    ['http://localhost:8091', 'ws://localhost:8091/_dioxus?build_id=a', true],
    ['https://localhost:8091', 'wss://localhost:8091/_dioxus?build_id=a', true],
    ['http://localhost:8091', 'wss://localhost:8091/_dioxus?build_id=a', false],
    ['https://localhost:8091', 'ws://localhost:8091/_dioxus?build_id=a', false],
    ['http://localhost:8091', 'ws://other.example:8091/_dioxus?build_id=a', false],
    ['http://localhost:8091', 'not-a-url', false],
  ]) {
    const error = { type: 'log.error', source: 'network', page,
      text: `WebSocket connection to '${socket}' failed: closed` };
    assert.equal(Boolean(classify(expected, new Set(), error)), ignored, socket);
    assert.equal(classify(expected, new Set(), { ...error, type: 'console.error' }), undefined);
  }
});

test('a provoked request still needs the browser network source and its page origin', () => {
  const error = { ...network, networkRequestId: 'injected-request' };
  const provoked = new Set(['injected-request']);
  assert.ok(classify({ expected: [] }, provoked, error));
  assert.equal(classify(expected, provoked, { ...error, source: 'javascript' }), undefined);
  assert.equal(classify(expected, provoked, { ...error, url: 'https://localhost:8091/api/v1/settings' }), undefined);
});

async function replay({ initializeFails = false, scenarioCleanupFails = false, finalCleanupFails = false } = {}) {
  let report;
  const closed = [];
  const main = {
    target: { id: 'test' }, client: { send: async () => {} },
    faults: { reset: async () => {} },
    close: async () => { closed.push('page'); if (finalCleanupFails) throw new Error('page close failed'); },
  };
  const run = vm.runInNewContext(`let harness, browser, main, current; const results = []; ${runner}; run`, {
    only: undefined, long: false, artifactDir: 'unused', serverOptions: {}, cdpBase: 'unused', base: 'unused',
    mkdir: async () => {}, writeFile: async (_path, text) => { report = JSON.parse(text); },
    path: { join: (...parts) => parts.join('/') },
    startHarness: async () => {
      if (initializeFails) throw new Error('initialization failed');
      return { close: async () => { closed.push('harness'); } };
    },
    fetch: async () => ({ json: async () => ({ webSocketDebuggerUrl: 'unused' }) }), AbortSignal,
    connect: () => ({ close: () => { closed.push('browser'); } }), openPage: async () => main,
    process: { once: () => {}, stdout: { write: () => {} } }, activate: async () => {},
    Scenario: class { constructor() { Object.assign(this, { checks: [], pages: [], cleanups: [] }); } },
    scenarios: [{ name: 'test', body: async (s) => {
      s.cleanups.push(async () => { closed.push('scenario-last'); });
      s.cleanups.push(async () => { closed.push('scenario-first'); if (scenarioCleanupFails) throw new Error('scenario cleanup failed'); });
    } }],
    sortConsole: () => ({ unexpected: [], ignored: [] }),
  });
  let error;
  try { await run(); } catch (caught) { error = String(caught); }
  return { report, error, closed };
}

test('initialization and final cleanup failures cannot produce a pass report', async () => {
  for (const options of [{ initializeFails: true }, { finalCleanupFails: true }]) {
    const { report, error, closed } = await replay(options);
    assert.equal(report.status, 'fail');
    assert.ok(error);
    if (options.finalCleanupFails) assert.deepEqual(closed.slice(-3), ['page', 'browser', 'harness']);
  }
  const { report, error } = await replay();
  assert.equal(report.status, 'pass');
  assert.equal(error, undefined);
});

test('a rejected scenario cleanup fails the run and remaining cleanup still runs', async () => {
  const { report, error, closed } = await replay({ scenarioCleanupFails: true });
  assert.equal(report.status, 'fail');
  assert.match(error, /scenario cleanup failed/);
  assert.deepEqual(closed, ['scenario-first', 'scenario-last', 'page', 'browser', 'harness']);
});
