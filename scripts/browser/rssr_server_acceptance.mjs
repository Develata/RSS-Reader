// Browser acceptance of the page against the real server (phase C, C4c-2): the V1 list and the
// recovery invariants (R2-07) of `docs/design/v0.2-phase-c-plan.md` ("C4c 浏览器回归、CI 与文档"),
// run in headless Chrome over CDP against `rssr-web` on fixture libraries (server_harness.mjs).
// Every scenario runs (a failure does not stop the rest) on a fresh library; a failing scenario
// leaves its page (HTML, screenshot) and its API traffic in the artifact directory. Scenarios that
// wait 30 s or more run only with `--long`.
//
// What is real and what is provoked: the server, its sessions, CSRF, build check and jobs are
// real; network failures, lost answers, malformed answers and held requests are provoked through
// the CDP Fetch domain (requests the page makes to `/api/`), and recorded so the console errors
// they cause are told from real ones. Focus, visibility and online triggers are dispatched as DOM
// events (the page's listeners are what native tests cannot reach) and, once each, made real: a
// tab hidden and shown again, the network taken offline and back.

import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { clickSelector, connect, evaluate, navigate, newPage, selectorExists, sleep, waitFor } from './cdp_session.mjs';
import { ApiInterceptor, ApiTraffic, request } from './api_traffic.mjs';
import { startHarness, waitUntil } from './server_harness.mjs';
import { ServerPage, VISIBLE, capturePage, fillField, pointerClick, textShown } from './server_page.mjs';

function parseOptions(argv) {
  const options = new Map();
  for (let index = 0; index < argv.length; index += 1) {
    const name = argv[index];
    if (!name.startsWith('--')) throw new Error(`Unexpected argument: ${name}`);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith('--')) options.set(name, 'true');
    else {
      options.set(name, value);
      index += 1;
    }
  }
  return options;
}

const cli = parseOptions(process.argv.slice(2));
const cdpBase = cli.get('--cdp-base') ?? 'http://127.0.0.1:9226';
const artifactDir = cli.get('--artifact-dir') ?? 'target/server-web-acceptance';
const long = cli.get('--long') === 'true';
const only = cli.get('--only')?.split(',');
const serverOptions = {
  serverBin: cli.get('--server-bin') ?? process.env.RSSR_WEB_BIN,
  staticDir: cli.get('--static-dir') ?? process.env.RSSR_WEB_STATIC_DIR,
  fixtureBin: cli.get('--fixture-bin') ?? process.env.RSSR_FIXTURE_BIN,
  seedsDir: cli.get('--seeds-dir') ?? 'tests/fixtures/browser_state',
  themesDir: cli.get('--themes-dir') ?? 'assets/themes',
  workDir: cli.get('--work-dir') ?? path.join(artifactDir, 'server-work'),
  artifactDir,
  port: Number.parseInt(cli.get('--port') ?? '8091', 10),
  username: 'smoke',
  password: 'smoke-pass-123',
};
for (const [option, value] of [['--server-bin', serverOptions.serverBin], ['--static-dir', serverOptions.staticDir], ['--fixture-bin', serverOptions.fixtureBin]]) {
  if (!value) throw new Error(`${option} is required`);
}
const base = `http://127.0.0.1:${serverOptions.port}`;

// Page markers.
const ENTRIES = '[data-page="entries"][data-position-ready="true"] [data-layout="entry-card"]';
const FEEDS = '[data-page="feeds"] [data-layout="feed-card"]';
const SETTINGS = '[data-page="settings"] [data-field="theme-mode"]';
const READER = '[data-page="reader"][data-position-ready="true"]';
const LONG_POLL = /^\/api\/v1\/jobs\/[0-9a-f]{32}$/;
const BANNER = {
  entries: '[data-layout="entries-main"] > [data-layout="status-banner"]',
  feeds: '[data-page="feeds"] > [data-layout="status-banner"]',
  settings: '[data-page="settings"] > [data-layout="status-banner"]',
  reader: '[data-page="reader"] [data-layout="status-banner"]',
};
const busy = (message) => JSON.stringify({ code: 'unavailable', message });

let harness;
let browser; // the browser-level CDP connection (targets)
let main; // the page every scenario starts from (the server's own origin)
let current; // the running scenario
const results = [];

/// One page of the app: its CDP session, its seeding and sign-in, its API traffic and interceptor,
/// and its console errors (filed under the running scenario).
async function openPage(origin) {
  const target = await newPage('about:blank', cdpBase);
  const client = connect(target.webSocketDebuggerUrl);
  const page = { target, client, origin, console: [] };
  const record = (entry) => current?.console.push({ ...entry, page: origin });
  client.on('Runtime.exceptionThrown', (event) => record({ type: 'exception', text: event.exceptionDetails?.exception?.description ?? event.exceptionDetails?.text }));
  client.on('Runtime.consoleAPICalled', (event) => {
    if (event.type === 'error') record({ type: 'console.error', text: event.args?.map((arg) => arg.value ?? arg.description).join(' ') });
  });
  client.on('Log.entryAdded', ({ entry }) => {
    if (entry?.level === 'error') record({ type: 'log.error', source: entry.source, text: entry.text, url: entry.url, networkRequestId: entry.networkRequestId, page: origin });
  });
  await client.send('Page.enable');
  await client.send('Runtime.enable');
  await client.send('Log.enable');
  await client.send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false });
  page.server = await new ServerPage({ client, harness, base: origin, username: serverOptions.username, password: serverOptions.password }).start();
  page.traffic = await new ApiTraffic(client).start();
  page.faults = await new ApiInterceptor(client).start();
  page.close = async () => {
    client.close();
    await fetch(`${cdpBase}/json/close/${target.id}`, { signal: AbortSignal.timeout(5000) }).catch(() => {});
  };
  return page;
}

/// The running scenario: checks, expected console errors, extra pages and clean-ups.
class Scenario {
  constructor(name) {
    Object.assign(this, { name, checks: [], console: [], expected: [], pages: [], cleanups: [] });
  }

  check(name, ok, details) {
    this.checks.push({ name, ok: Boolean(ok), details });
    return Boolean(ok);
  }

  /// A failed network request the scenario causes itself (e.g. the 401s of an ended session): the
  /// browser's network log entry for an answer of `status` to a request for `path` (a string or a
  /// RegExp on the URL's path). Nothing else (exceptions, `console.error`) can be expected (review
  /// C4C2-06).
  expectConsole({ status, path }) {
    this.expected.push({ status, path });
  }

  async open(origin) {
    const page = await openPage(origin);
    this.pages.push(page);
    return page;
  }

  onCleanup(fn) {
    this.cleanups.push(fn);
  }
}

const scenarios = [];
const define = (name, options, body) => scenarios.push({ name, ...options, body });

// --- Helpers ------------------------------------------------------------------------------------

// The harness's own session on the current library (its reads of the server's state).
let harnessCookie;
const state = (path) => harness.server.api('GET', path, harnessCookie);

/// The `reader-demo` library (its entries are from April: never archived here) on the main page.
async function seedDemo(nextPath, marker, { edit } = {}) {
  await main.server.seed('reader-demo', nextPath, marker, { edit: (parts) => {
    parts.core.settings.archive_after_months = 1200;
    edit?.(parts);
  } });
  harnessCookie = main.server.cookie;
}
const entry = async (id) => (await state(`/api/v1/entries/${id}`)).entry;
const location = (page) => evaluate(page.client, 'location.pathname');

/// The three reread triggers at once, as DOM events (they reach the page's listeners; the
/// coalescing window is 200 ms).
const triggerAll = (page) => evaluate(page.client, `(() => {
  window.dispatchEvent(new Event('focus'));
  document.dispatchEvent(new Event('visibilitychange'));
  window.dispatchEvent(new Event('online'));
})()`);
const triggerFocus = (page) => evaluate(page.client, `window.dispatchEvent(new Event('focus'))`);

/// A banner's state and text (`null` when absent).
const banner = (page, selector) => evaluate(page.client, `(banner => banner && {state: banner.dataset.state,
  text: banner.textContent.trim(), visible: ${VISIBLE}(banner)})(document.querySelector(${JSON.stringify(selector)}))`);
const waitBanner = (page, selector, tone, part, timeoutMs = 20000) => waitFor(page.client,
  `(banner => !!banner && banner.dataset.state === ${JSON.stringify(tone)} && banner.textContent.includes(${JSON.stringify(part)}))(document.querySelector(${JSON.stringify(selector)}))`, timeoutMs);

/// Chooses `value` in a select as a person does (change event).
const chooseScript = (field, value) => `(() => {
  const select = document.querySelector('[data-field="${field}"]');
  select.value = ${JSON.stringify(value)};
  select.dispatchEvent(new Event('input', {bubbles: true}));
  select.dispatchEvent(new Event('change', {bubbles: true}));
})()`;
const choose = (page, field, value) => evaluate(page.client, chooseScript(field, value));
const fieldValue = (page, field) => evaluate(page.client, `document.querySelector('[data-field="${field}"]')?.value`);
const draft = (page) => evaluate(page.client, `sessionStorage.getItem('rssr.settings-draft')`);
const RESTORED = '已恢复重新加载前未保存的设置修改，尚未保存。';

/// Marks the document: a full reload loses the mark.
const markDocument = (page) => evaluate(page.client, 'window.__acceptanceDocument = true');
const sameDocument = (page) => evaluate(page.client, 'window.__acceptanceDocument === true');

/// Sends the build check of every write astray: the server refuses it as another build's.
const staleBuild = (page) => page.faults.add({
  match: ({ method }) => method !== 'GET',
  action: 'headers',
  rewrite: (headers) => {
    for (const name of Object.keys(headers)) if (name.toLowerCase() === 'x-rssr-build') headers[name] = 'another-build';
    return headers;
  },
});

const card = (id) => `li[data-layout="entry-card"][data-position-entry="${id}"]`;
const cardMeta = (page, id) => evaluate(page.client, `document.querySelector('${card(id)} [data-slot="entry-card-meta"]')?.textContent ?? ''`);

/// Activates a CDP target (the others in its window become hidden).
const activate = (targetId) => browser.send('Target.activateTarget', { targetId });

// --- V1: deployment origin, reverse proxy, cookie, Origin, CSRF, redirects ------------------------

/// The page behind the reverse proxy: the server trusts the proxy's origin only.
async function behindProxy(scenario, seedPath, marker) {
  const proxy = await harness.proxy();
  scenario.onCleanup(() => { proxy.redirect = null; });
  await navigate(main.client, 'about:blank');
  ({ cookie: harnessCookie } = await harness.server.useSeed('reader-demo', {
    env: { RSS_READER_WEB_PUBLIC_ORIGIN: proxy.origin },
    edit: ({ core }) => { core.settings.archive_after_months = 1200; },
  }));
  const page = await scenario.open(proxy.origin);
  page.signInMark = page.traffic.mark();
  page.server.build.reset();
  await page.server.signIn(seedPath);
  await selectorExists(page.client, marker, 30000);
  page.server.build.verify();
  return { proxy, page };
}

define('proxy-origin-cookie-csrf', {}, async (s) => {
  const { proxy, page } = await behindProxy(s, '/entries/2', READER);
  let mark = page.traffic.mark();
  const forwarded = proxy.requests.length;
  await pointerClick(page.client, '[data-action="mark-read"]');
  await waitFor(page.client, `document.querySelector('[data-action="mark-read"]')?.dataset.state === 'read'`);
  await page.traffic.settled();
  const writes = page.traffic.since(mark).filter((r) => r.path === '/api/v1/entries/2/read');
  // The token is fetched before the first write (opening an HTML entry already writes: it asks
  // the server to localize the entry's images), once for the session.
  s.check('a write through the proxy: one write, accepted; the token fetched once in the session',
    page.traffic.count(page.signInMark, 'GET', '/api/v1/session') === 1 && writes.length === 1
      && writes[0].status >= 200 && writes[0].status < 300, page.traffic.log(page.signInMark));
  const write = proxy.requests.slice(forwarded).find((r) => r.method === 'PUT');
  s.check('the proxy forwarded the page origin unchanged', write?.origin === proxy.origin, write);
  s.check('the server stored it', (await entry(2)).is_read === true);

  // A new session for this browser (as after signing in again elsewhere): the cached CSRF token is
  // stale; the server refuses the write before running it, the page fetches a new token and sends
  // the write once more.
  const cookie = await harness.server.signIn('/entries');
  const [name, value] = cookie.split('=');
  await page.client.send('Network.setCookie', { name, value, url: proxy.origin, httpOnly: true, sameSite: 'Lax' });
  s.expectConsole({ status: 403, path: '/api/v1/entries/2/read' });
  mark = page.traffic.mark();
  await pointerClick(page.client, '[data-action="mark-read"]');
  await waitFor(page.client, `document.querySelector('[data-action="mark-read"]')?.dataset.state === 'unread'`);
  await page.traffic.settled();
  const resent = page.traffic.since(mark).filter((r) => r.path === '/api/v1/entries/2/read').map((r) => r.status);
  s.check('a stale CSRF token: refused once (403), one new token, one resend, applied once',
    resent.length === 2 && resent[0] === 403 && resent[1] >= 200 && resent[1] < 300
      && page.traffic.count(mark, 'GET', '/api/v1/session') === 1, page.traffic.log(mark));
  s.check('the server stored the second write', (await entry(2)).is_read === false);
});

define('proxy-redirects-api', {}, async (s) => {
  const { proxy, page } = await behindProxy(s, '/entries/2', READER);
  // A proxy that redirects API writes (it must not): the browser follows to an HTML page; the page
  // cannot know whether the write ran.
  proxy.redirect = (method, pathname) => method === 'PUT' && pathname === '/api/v1/entries/2/read';
  s.expectConsole({ status: 405, path: '/login' }); // the follow-up PUT to /login
  const mark = page.traffic.mark();
  const forwarded = proxy.requests.length;
  await pointerClick(page.client, '[data-action="mark-read"]');
  await waitBanner(page, BANNER.reader, 'unknown', '结果未知');
  await page.traffic.settled();
  // (A 302 keeps PUT: the browser's follow-up goes to /login, not to the API.)
  const sent = proxy.requests.slice(forwarded).filter((r) => r.path === '/api/v1/entries/2/read');
  s.check('a redirected write is an unknown outcome, sent once, not resent',
    sent.length === 1 && sent[0].redirected && page.traffic.writes(mark).length === 1, { sent, traffic: page.traffic.log(mark) });
  s.check('nothing claims it succeeded', await evaluate(page.client, `document.querySelector('[data-action="mark-read"]').dataset.state`) === 'unread'
    && (await banner(page, BANNER.reader)).visible);
  s.check('the write never reached the server', (await entry(2)).is_read === false);
  // A redirected read: the page shows the read failed.
  proxy.redirect = (method, pathname) => method === 'GET' && pathname === '/api/v1/feeds';
  await clickSelector(page.client, '[data-nav="feeds"]');
  await waitBanner(page, BANNER.feeds, 'error', '读取订阅失败');
  s.check('a redirected read is a failed read', true, await banner(page, BANNER.feeds));
});

// --- R2-07: outcomes ------------------------------------------------------------------------------

define('unknown-write-then-reread', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  // The write runs on the server; its answer is lost.
  await main.faults.add({ match: request('PUT', '/api/v1/entries/2/read'), action: 'failResponse', times: 1 });
  await main.traffic.settled();
  // The reread must come from the unknown outcome itself, not from a focus event: a trusted click
  // can focus the window (a reread trigger), so the button is clicked from the page's script.
  await evaluate(main.client, `window.__acceptanceFocus = 0; addEventListener('focus', () => window.__acceptanceFocus++)`);
  const mark = main.traffic.mark();
  await clickSelector(main.client, `${card(2)} [data-action="mark-read"]`);
  await waitBanner(main, BANNER.entries, 'unknown', '更改已读状态');
  await main.traffic.settled();
  s.check('no reread trigger fired meanwhile', await evaluate(main.client, 'window.__acceptanceFocus') === 0);
  s.check('the write was sent once (no automatic resend)', main.traffic.count(mark, 'PUT', '/api/v1/entries/2/read') === 1, main.traffic.log(mark));
  s.check('it ran on the server', (await entry(2)).is_read === true);
  const afterWrite = main.traffic.since(mark).findIndex((r) => r.method === 'PUT');
  s.check('the page reread (read only) after it', main.traffic.since(mark).slice(afterWrite + 1)
    .some((r) => r.method === 'GET' && r.path === '/api/v1/entries'), main.traffic.log(mark));
  s.check('the reread shows the stored state', (await cardMeta(main, 2)).includes('已读'), await cardMeta(main, 2));
  const shown = await banner(main, BANNER.entries);
  s.check('no success receipt: the outcome stays unknown', shown.state === 'unknown' && !shown.text.includes('已将'), shown);
});

define('malformed-answers', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  // A 2xx answer to a write that is not the protocol's.
  await main.faults.add({ match: request('PUT', '/api/v1/entries/1/starred'), action: 'fulfill', status: 200, contentType: 'text/html', body: '<html>ok</html>', times: 1 });
  let mark = main.traffic.mark();
  await pointerClick(main.client, `${card(1)} [data-action="toggle-starred"]`);
  await waitBanner(main, BANNER.entries, 'unknown', '更改收藏状态');
  await main.traffic.settled();
  s.check('a 2xx answer that is not the protocol\'s: unknown, sent once, no success',
    main.traffic.count(mark, 'PUT', '/api/v1/entries/1/starred') === 1 && !(await banner(main, BANNER.entries)).text.includes('已将'), main.traffic.log(mark));
  // A job created without an id.
  await clickSelector(main.client, '[data-nav="feeds"]');
  await selectorExists(main.client, FEEDS);
  await main.faults.add({ match: request('POST', '/api/v1/jobs/refresh-all'), action: 'fulfill', status: 202, body: '{}', times: 1 });
  mark = main.traffic.mark();
  await clickSelector(main.client, '[data-action="refresh-all"]');
  await waitFor(main.client, `document.querySelector('[data-slot="manual-refresh-status"]')?.dataset.state === 'unknown'`);
  await main.traffic.settled();
  s.check('a job created without an id: unknown, nothing polled', main.traffic.count(mark, 'GET', LONG_POLL) === 0, main.traffic.log(mark));
  await waitFor(main.client, `document.querySelector('[data-slot="manual-refresh-status"]')?.dataset.state !== 'unknown'`, 15000);
  // A job's result of another kind.
  await main.faults.add({ match: request('GET', LONG_POLL), action: 'fulfill', status: 200,
    body: JSON.stringify({ state: 'succeeded', result: { kind: 'remote_config_push', outcome: { exported_feed_count: 0 } } }), times: 1 });
  await clickSelector(main.client, '[data-action="refresh-all"]');
  await waitFor(main.client, `(output => output?.dataset.state === 'unknown' && output.textContent.includes('另一种任务'))(document.querySelector('[data-slot="manual-refresh-status"]'))`);
  const status = await evaluate(main.client, `document.querySelector('[data-slot="manual-refresh-status"]').textContent`);
  s.check('a result of another kind: unknown, no success claimed', !/新增|没有新文章/.test(status), status);
});

define('malformed-poll-backoff', { long: true }, async (s) => {
  await seedDemo('/feeds', FEEDS);
  await main.faults.add({ match: request('GET', LONG_POLL), action: 'fulfill', status: 200, body: '{"state":"bogus"}' });
  const mark = main.traffic.mark();
  await clickSelector(main.client, '[data-action="refresh-all"]');
  await waitFor(main.client, `(output => output?.dataset.state === 'unknown' && output.textContent.includes('与服务器失去联系'))(document.querySelector('[data-slot="manual-refresh-status"]'))`, 60000);
  s.check('unreadable polls: retried after 1, 2, 4, 8, 16 s, then unknown (six polls)',
    main.traffic.count(mark, 'GET', LONG_POLL) === 6, main.traffic.log(mark));
});

define('read-failure', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  await main.faults.add({ match: request('GET', '/api/v1/feeds'), action: 'fail', times: 1 });
  await clickSelector(main.client, '[data-nav="feeds"]');
  await waitBanner(main, BANNER.feeds, 'error', '读取订阅失败');
  s.check('a failed read says so', (await banner(main, BANNER.feeds)).text.includes('无法连接服务器'), await banner(main, BANNER.feeds));
});

// --- V1: jobs and page changes ------------------------------------------------------------------

define('job-survives-page-switch', {}, async (s) => {
  await seedDemo('/feeds', FEEDS);
  await markDocument(main);
  harness.feeds.nextRound('reader-demo');
  const held = harness.feeds.hold((pathname) => pathname === '/reader-demo/1.xml');
  s.onCleanup(() => held.release());
  await clickSelector(main.client, '[data-action="refresh-all"]');
  await waitUntil('the held refresh', 15000, async () => held.waiting() === 1);
  await clickSelector(main.client, '[data-action="activate-home"]');
  await selectorExists(main.client, ENTRIES);
  held.release();
  await waitFor(main.client, `document.querySelector('[data-slot="manual-refresh-status"]')?.textContent.includes('新增 1 篇文章')`);
  await textShown(main.client, '手动刷新新增文章 1 / 1', '[data-slot="entry-card-title"]');
  s.check('the job finished after the page changed and updated the page now shown, without a reload', await sameDocument(main));
});

define('reload-loses-the-job', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  harness.feeds.nextRound('reader-demo');
  const held = harness.feeds.hold((pathname) => pathname === '/reader-demo/1.xml');
  s.onCleanup(() => held.release());
  await clickSelector(main.client, '[data-action="activate-home"]');
  await waitUntil('the held refresh', 15000, async () => held.waiting() === 1);
  // Counted from before the reload, by document: whatever the new one asks for at its start
  // counts too (review C4C2-08).
  const mark = main.traffic.mark();
  await navigate(main.client, `${base}/entries`);
  await selectorExists(main.client, ENTRIES);
  const { frameTree } = await main.client.send('Page.getFrameTree');
  const reloaded = frameTree.frame.loaderId;
  held.release();
  await sleep(1500);
  await main.traffic.settled();
  const jobs = main.traffic.since(mark).filter((r) => r.loaderId === reloaded && r.path.startsWith('/api/v1/jobs'));
  s.check('after a reload the job is not followed (its id is gone): the new document asks nothing of jobs',
    jobs.length === 0 && main.traffic.since(mark).some((r) => r.loaderId === reloaded), main.traffic.log(mark));
  const status = await evaluate(main.client, `(o => ({state: o.dataset.state, text: o.textContent}))(document.querySelector('[data-slot="manual-refresh-status"]'))`);
  s.check('and no result is claimed for it', status.state === 'idle' && !status.text.includes('新增'), status);
  await triggerFocus(main);
  await textShown(main.client, '手动刷新新增文章 1 / 1', '[data-slot="entry-card-title"]');
  s.check('a later reread shows what the job stored', true);
});

define('job-ends-after-the-page-was-left', {}, async (s) => {
  await seedDemo('/feeds', FEEDS);
  const held = harness.feeds.hold((pathname) => pathname === '/new/left-page.xml');
  s.onCleanup(() => held.release());
  await evaluate(main.client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
  await main.client.send('Input.insertText', { text: `${harness.feeds.base}/new/left-page.xml` });
  await clickSelector(main.client, '[data-action="add-feed"]');
  await waitUntil('the held first fetch', 15000, async () => held.waiting() === 1);
  await clickSelector(main.client, '[data-action="activate-home"]');
  await selectorExists(main.client, ENTRIES);
  await clickSelector(main.client, '[data-nav="feeds"]');
  await selectorExists(main.client, FEEDS);
  // The new page starts its own add, held too: its in-progress state is not the old job's to end
  // (review C4C2-07).
  const second = harness.feeds.hold((pathname) => pathname === '/new/second-add.xml');
  s.onCleanup(() => second.release());
  const typed = `${harness.feeds.base}/new/second-add.xml`;
  await evaluate(main.client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
  await main.client.send('Input.insertText', { text: typed });
  await clickSelector(main.client, '[data-action="add-feed"]');
  await waitUntil('the held second fetch', 15000, async () => second.waiting() === 1);
  const form = async () => evaluate(main.client, `(() => { const button = document.querySelector('[data-action="add-feed"]');
    return {input: document.querySelector('[data-field="feed-url-input"]').value, disabled: button.disabled,
      busy: button.getAttribute('aria-busy'), label: button.textContent.trim()}; })()`);
  const before = await form();
  held.release();
  await waitFor(main.client, `(n => n?.textContent.includes('订阅已保存并完成首次刷新'))(document.querySelector('[data-slot="app-notice"][data-notice="job"]'))`);
  await textShown(main.client, 'Added fixture left-page');
  const after = await form();
  s.check("the old page's job ended without touching the new page's add in progress (input, busy button)",
    before.disabled && before.label === '正在添加…' && JSON.stringify(after) === JSON.stringify(before), { before, after });
  second.release();
  await textShown(main.client, 'Added fixture second-add');
  const done = await form();
  s.check('the new add ends with its own result', !done.disabled && done.label === '添加订阅', done);
});

define('page-gone-stops-polling', {}, async (s) => {
  await seedDemo('/feeds', FEEDS);
  await main.faults.add({ match: request('GET', LONG_POLL), action: 'fail' });
  const start = main.traffic.mark();
  await clickSelector(main.client, '[data-action="refresh-all"]');
  await waitUntil('two failed polls', 15000, async () => main.traffic.count(start, 'GET', LONG_POLL) >= 2);
  await navigate(main.client, 'about:blank');
  const mark = main.traffic.mark();
  await sleep(8000);
  s.check('the app gone, no poll and no backoff timer runs', main.traffic.count(mark, 'GET', LONG_POLL) === 0, main.traffic.log(start));
});

// --- R2-07: rereads -------------------------------------------------------------------------------

/// After the triggers: no write, each read at most once, the page's own read done.
async function checkReread(s, page, label, mark, reads) {
  await page.traffic.settled();
  const since = page.traffic.since(mark);
  s.check(`${label}: no write`, page.traffic.writes(mark).length === 0, page.traffic.log(mark));
  const counts = {};
  for (const r of since.filter((r) => r.method === 'GET')) counts[r.path] = (counts[r.path] ?? 0) + 1;
  s.check(`${label}: one read, not one per trigger`, Object.values(counts).every((n) => n === 1)
    && reads.every((path) => counts[path] === 1), counts);
  // Answered: the answer's status arrived. (Under latency or interception Chrome reports some
  // reads the page did receive as canceled afterwards; whether the read reached the page is checked
  // on the page itself, below.)
  s.check(`${label}: each read answered`, since.every((r) => r.status >= 200 && r.status < 300), page.traffic.log(mark));
}

/// A write by another session (the harness's), straight to the server: what another browser or
/// device does meanwhile.
async function elsewhereWrite(method, pathname, body) {
  const { csrf_token: csrf } = await state('/api/v1/session');
  const answer = await fetch(`${base}${pathname}`, { method, signal: AbortSignal.timeout(10000),
    headers: { Cookie: harnessCookie, Origin: base, 'Content-Type': 'application/json', 'x-rssr-csrf': csrf,
      'x-rssr-build': harness.server.buildId }, body: JSON.stringify(body) });
  if (!answer.ok) throw new Error(`the other session's write: ${answer.status}`);
}
const elsewhereSetRead = (id, read) => elsewhereWrite('PUT', `/api/v1/entries/${id}/read`, { read });

/// The stored list preferences (answered with the list page's first read).
async function storedWorkspace() {
  const input = encodeURIComponent(JSON.stringify({ load_preferences: true, load_feeds: false }));
  return (await state(`/api/v1/entries/bootstrap?input=${input}`)).workspace;
}

define('reread-triggers', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  // Changed meanwhile by another session: the reread brings it to the page.
  await main.traffic.settled();
  await elsewhereSetRead(2, true);
  let mark = main.traffic.mark();
  await triggerAll(main);
  await checkReread(s, main, '/entries: focus, visible and online together', mark,
    ['/api/v1/entries/bootstrap', '/api/v1/entries', '/api/v1/settings']);
  s.check('/entries: the reread shows what another session changed meanwhile', (await cardMeta(main, 2)).includes('已读'), await cardMeta(main, 2));
  for (const [route, marker, reads] of [
    ['/feeds', FEEDS, ['/api/v1/feeds', '/api/v1/settings']],
    ['/settings', SETTINGS, ['/api/v1/settings']],
    ['/entries/2', READER, ['/api/v1/entries/2', '/api/v1/settings']],
  ]) {
    await navigate(main.client, `${base}${route}`);
    await selectorExists(main.client, marker);
    await main.traffic.settled();
    mark = main.traffic.mark();
    await triggerAll(main);
    await checkReread(s, main, `${route}: focus, visible and online together`, mark, reads);
  }
  // Real triggers: the tab hidden and shown again; the network gone and back.
  await navigate(main.client, `${base}/entries`);
  await selectorExists(main.client, ENTRIES);
  await main.traffic.settled();
  const other = await s.open('about:blank');
  await elsewhereSetRead(2, false);
  mark = main.traffic.mark();
  await activate(other.target.id);
  await waitFor(main.client, 'document.hidden');
  await activate(main.target.id);
  await waitFor(main.client, '!document.hidden');
  await checkReread(s, main, '/entries: the tab hidden and shown (visible and focus)', mark, ['/api/v1/entries/bootstrap', '/api/v1/entries']);
  s.check('/entries: shown again, the page has the change', (await cardMeta(main, 2)).includes('未读'), await cardMeta(main, 2));
  await elsewhereSetRead(2, true);
  mark = main.traffic.mark();
  await main.client.send('Network.emulateNetworkConditions', { offline: true, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
  await sleep(300);
  await main.client.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
  await checkReread(s, main, '/entries: offline and online again', mark, ['/api/v1/entries/bootstrap', '/api/v1/entries']);
  s.check('/entries: back online, the page has the change', (await cardMeta(main, 2)).includes('已读'), await cardMeta(main, 2));
  // A list preference changed by another session (only unread): the reread brings it to the page,
  // whose preference-saving effect sees the filter change; it must not write it back (review
  // C4C2-03). The list is read again for the new filter, so reads are not counted here.
  await main.traffic.settled();
  await elsewhereWrite('PUT', '/api/v1/workspace', { ...(await storedWorkspace()), read_filter: 'unread_only' });
  mark = main.traffic.mark();
  await triggerAll(main);
  // The page takes the preference: it lists again with it (the filter controls are folded away
  // at this width), and entry 2, read, leaves the list.
  await waitUntil('the list read with the new filter', 10000, async () => main.traffic.since(mark)
    .some((r) => r.path === '/api/v1/entries' && decodeURIComponent(r.query).includes('"read_filter":"unread_only"')));
  await waitFor(main.client, `!document.querySelector('${card(2)}')`);
  await main.traffic.settled();
  s.check("/entries: another session's list preference shows and filters the list (entry 2, read, gone)", true);
  s.check('/entries: the preference read is not written back', main.traffic.writes(mark).length === 0
    && main.traffic.count(mark, 'GET', '/api/v1/entries/bootstrap') === 1, main.traffic.log(mark));
  s.check("/entries: the stored preference is the other session's", (await storedWorkspace()).read_filter === 'unread_only');
});

define('two-browsers', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  // The second browser: another host for the same server, so cookies and session of its own.
  const other = await s.open(`http://localhost:${serverOptions.port}`);
  await other.server.signIn('/entries');
  await selectorExists(other.client, ENTRIES, 30000);
  other.server.build.verify();
  await pointerClick(main.client, `${card(2)} [data-action="mark-read"]`);
  await waitFor(main.client, `document.querySelector('${card(2)} [data-slot="entry-card-meta"]').textContent.includes('已读')`);
  await triggerFocus(other);
  await waitFor(other.client, `document.querySelector('${card(2)} [data-slot="entry-card-meta"]').textContent.includes('已读')`);
  s.check('a change in one browser shows in the other after a reread', true);
  await pointerClick(other.client, `${card(1)} [data-action="toggle-starred"]`);
  await waitFor(other.client, `document.querySelector('${card(1)} [data-slot="entry-card-meta"]').textContent.includes('已收藏')`);
  await triggerFocus(main);
  await waitFor(main.client, `document.querySelector('${card(1)} [data-slot="entry-card-meta"]').textContent.includes('已收藏')`);
  s.check('and the other way round', true);
});

// --- R2-07: build mismatch; V1: drafts ----------------------------------------------------------

define('build-mismatch-keeps-edits', {}, async (s) => {
  await seedDemo('/settings', SETTINGS);
  const stored = await state('/api/v1/settings');
  await choose(main, 'theme-mode', 'dark');
  await fillField(main.client, '[data-field="custom-css"]', ':root { --acceptance-mark: #123456; }');
  const stale = await staleBuild(main);
  await clickSelector(main.client, '[data-action="save-settings"]');
  await waitFor(main.client, `(n => n && ${VISIBLE}(n) && n.textContent.includes('页面版本与服务器不一致'))(document.querySelector('[data-slot="app-notice"][data-notice="reload"]'))`);
  await waitBanner(main, BANNER.settings, 'error', '草稿已保留');
  await choose(main, 'list-density', 'compact');
  s.check('nothing was stored', JSON.stringify(await state('/api/v1/settings')) === JSON.stringify(stored));
  await stale.remove();
  const mark = main.traffic.mark();
  await markDocument(main);
  await clickSelector(main.client, '[data-action="reload-page"]');
  await waitUntil('the reload', 15000, async () => !(await sameDocument(main).catch(() => true)));
  await selectorExists(main.client, SETTINGS);
  await textShown(main.client, RESTORED);
  const restored = { theme: await fieldValue(main, 'theme-mode'), density: await fieldValue(main, 'list-density'), css: await fieldValue(main, 'custom-css') };
  s.check('after the reload the committed and the later edits are back', restored.theme === 'dark' && restored.density === 'compact'
    && restored.css.includes('#123456'), restored);
  await main.traffic.settled();
  s.check('restored, not saved by itself', main.traffic.count(mark, 'PATCH', '/api/v1/settings') === 0
    && JSON.stringify(await state('/api/v1/settings')) === JSON.stringify(stored), main.traffic.log(mark));
  s.check('the kept draft is gone once restored', (await draft(main)) === null);
  // Edited again after the restore, then saved: one save with everything.
  await choose(main, 'theme-mode', 'light');
  const save = main.traffic.mark();
  await clickSelector(main.client, '[data-action="save-settings"]');
  await waitBanner(main, BANNER.settings, 'info', '设置已保存');
  const saved = await state('/api/v1/settings');
  s.check('re-edited and saved: one save holding every edit', main.traffic.count(save, 'PATCH', '/api/v1/settings') === 1
    && saved.theme === 'light' && saved.list_density === 'compact' && saved.custom_css.includes('#123456'), saved);
  await navigate(main.client, `${base}/settings`);
  await selectorExists(main.client, SETTINGS);
  await sleep(500);
  s.check('restored only once', !(await evaluate(main.client, `document.body.textContent.includes(${JSON.stringify(RESTORED)})`)));
});

define('draft-storage-unavailable', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  const { identifier } = await main.client.send('Page.addScriptToEvaluateOnNewDocument', { source: `(() => {
    const setItem = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key, value) {
      if (this === window.sessionStorage) throw new DOMException('acceptance: no session storage', 'QuotaExceededError');
      return setItem.call(this, key, value);
    };
  })()` });
  s.onCleanup(() => main.client.send('Page.removeScriptToEvaluateOnNewDocument', { identifier }));
  await navigate(main.client, `${base}/settings`);
  await selectorExists(main.client, SETTINGS);
  await markDocument(main);
  await choose(main, 'theme-mode', 'dark');
  await staleBuild(main);
  await clickSelector(main.client, '[data-action="save-settings"]');
  await selectorExists(main.client, '[data-action="reload-page"]');
  await clickSelector(main.client, '[data-action="reload-page"]');
  await waitFor(main.client, `(n => n?.textContent.includes('浏览器暂时无法保留设置草稿，因此没有重新加载'))(document.querySelector('[data-slot="app-notice"][data-notice="reload"]'))`);
  s.check('storage unavailable: the page stays, with its edits', await sameDocument(main) && (await fieldValue(main, 'theme-mode')) === 'dark');
  s.check('and offers to reload anyway, saying the draft is lost',
    (await evaluate(main.client, `document.querySelector('[data-action="reload-anyway"]')?.textContent`))?.includes('草稿会丢失'));
});

define('restored-draft-then-sign-in', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  // A draft kept by an earlier departure (the way it is kept is the build-mismatch scenario's).
  await evaluate(main.client, `sessionStorage.setItem('rssr.settings-draft', JSON.stringify({version: 1, edits: {theme: 'dark'}}))`);
  await navigate(main.client, `${base}/settings`);
  await selectorExists(main.client, SETTINGS);
  await textShown(main.client, RESTORED);
  // The session ends right then: the next read is refused and the page goes to sign in.
  s.expectConsole({ status: 401, path: '/api/v1/settings' });
  await main.client.send('Network.clearBrowserCookies');
  await triggerFocus(main);
  await waitUntil('the sign-in page', 20000, async () => (await location(main).catch(() => '')) === '/login');
  s.check('the restored draft is kept for after signing in', JSON.parse(await draft(main) ?? 'null')?.edits?.theme === 'dark', await draft(main));
  await evaluate(main.client, `(() => { const form = document.querySelector('form[action="/login"]');
    form.querySelector('[name="username"]').value = ${JSON.stringify(serverOptions.username)};
    form.querySelector('[name="password"]').value = ${JSON.stringify(serverOptions.password)};
    form.submit(); })()`);
  await selectorExists(main.client, SETTINGS, 30000);
  await textShown(main.client, RESTORED);
  s.check('signed in again, the draft is restored', (await fieldValue(main, 'theme-mode')) === 'dark');
  await navigate(main.client, `${base}/settings`);
  await selectorExists(main.client, SETTINGS);
  await sleep(500);
  s.check('and only once', !(await evaluate(main.client, `document.body.textContent.includes(${JSON.stringify(RESTORED)})`)) && (await draft(main)) === null);
  // The page's cookie is gone: the next seed signs in through the form again.
  main.server.signedInThroughForm = false;
});

/// Ends the session: the next read is refused and the app leaves for the sign-in page. The
/// navigation is answered 204 as when the user stops it (`stop`: the app runs on, signed in again
/// as before), or held until the returned rule is released (`hold`: it lands late).
async function leaveToSignIn(s, how) {
  await markDocument(main);
  await main.faults.addPattern({ urlPattern: '*/login?next=*', resourceType: 'Document', requestStage: 'Request' });
  const rule = await main.faults.add(how === 'stop'
    ? { match: request('GET', '/login'), action: 'fulfill', status: 204, contentType: 'text/html', times: 1 }
    : { match: request('GET', '/login'), action: 'hold', times: 1 });
  s.onCleanup(() => rule.remove());
  s.expectConsole({ status: 401, path: /^\/api\/v1\// });
  const cookie = main.server.cookie;
  await main.client.send('Network.clearBrowserCookies');
  await triggerFocus(main);
  await waitUntil('the navigation to the sign-in page', 20000, async () => rule.hits() === 1);
  if (how === 'stop') {
    await sleep(1000);
    const [name, value] = cookie.split('=');
    await main.client.send('Network.setCookie', { name, value, url: base, httpOnly: true, sameSite: 'Lax' });
    s.check('the stopped departure leaves the app running', await sameDocument(main));
  }
  return rule;
}

/// Opens the settings page again inside the app (not a reload) and says whether it restored.
async function reopenSettings() {
  await clickSelector(main.client, '[data-action="activate-home"]');
  await selectorExists(main.client, ENTRIES);
  await clickSelector(main.client, '[data-nav="settings"]');
  await selectorExists(main.client, SETTINGS);
  await sleep(800);
  return evaluate(main.client, `document.body.textContent.includes(${JSON.stringify(RESTORED)})`);
}

const keptEdits = async (page) => JSON.parse(await draft(page) ?? 'null')?.edits ?? null;

define('stopped-departure-restores-once', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  // A draft kept by an earlier page, waiting for the next settings page; the app leaves from the
  // list, the navigation is stopped.
  await evaluate(main.client, `sessionStorage.setItem('rssr.settings-draft', JSON.stringify({version: 1, edits: {theme: 'dark'}}))`);
  await leaveToSignIn(s, 'stop');
  await clickSelector(main.client, '[data-nav="settings"]');
  await selectorExists(main.client, SETTINGS);
  await textShown(main.client, RESTORED);
  s.check('the settings page opened after the stopped departure restores the draft',
    (await fieldValue(main, 'theme-mode')) === 'dark' && (await keptEdits(main))?.theme === 'dark', await draft(main));
  s.check('reopened at once: not restored a second time', !(await reopenSettings()), await draft(main));
  s.check('the settings page closed, nothing is left stored', (await draft(main)) === null, await draft(main));
  await navigate(main.client, `${base}/settings`);
  await selectorExists(main.client, SETTINGS);
  await sleep(800);
  s.check('nor after a reload', !(await evaluate(main.client, `document.body.textContent.includes(${JSON.stringify(RESTORED)})`)));
});

define('stopped-departure-then-saved', {}, async (s) => {
  await seedDemo('/settings', SETTINGS);
  await choose(main, 'theme-mode', 'dark');
  await leaveToSignIn(s, 'stop');
  s.check('the draft is kept as the app leaves', (await keptEdits(main))?.theme === 'dark', await draft(main));
  await choose(main, 'list-density', 'compact');
  await waitUntil('the kept draft following the edit', 5000, async () => (await keptEdits(main))?.list_density === 'compact');
  s.check('the kept draft follows the edits made after the departure', true);
  await clickSelector(main.client, '[data-action="save-settings"]');
  await waitUntil('the save', 10000, async () => (await state('/api/v1/settings')).list_density === 'compact');
  await waitUntil('the kept draft gone', 5000, async () => (await draft(main)) === null);
  s.check('saved: nothing stale is left stored', true);
  await navigate(main.client, `${base}/settings`);
  await selectorExists(main.client, SETTINGS);
  await sleep(800);
  s.check('a later reload restores nothing; the saved settings show',
    !(await evaluate(main.client, `document.body.textContent.includes(${JSON.stringify(RESTORED)})`))
      && (await fieldValue(main, 'theme-mode')) === 'dark' && (await fieldValue(main, 'list-density')) === 'compact');
});

define('slow-departure-lands-late', { long: true }, async (s) => {
  await seedDemo('/settings', SETTINGS);
  await choose(main, 'theme-mode', 'dark');
  // While a navigation is on its way, Chrome holds DevTools commands to the page until it ends;
  // the page is still in use, so its own timer makes an edit once the departure has kept the
  // draft (only the draft following the edits can bring it along).
  await evaluate(main.client, `(() => { const wait = setInterval(() => {
    if (!sessionStorage.getItem('rssr.settings-draft')) return;
    clearInterval(wait);
    setTimeout(() => ${chooseScript('list-density', 'compact')}, 1000);
  }, 50); })()`);
  const left = Date.now();
  const navigation = await leaveToSignIn(s, 'hold');
  await sleep(Math.max(0, left + 35000 - Date.now()));
  await navigation.release();
  await waitUntil('the sign-in page', 20000, async () => (await location(main).catch(() => '')) === '/login');
  await evaluate(main.client, `(() => { const form = document.querySelector('form[action="/login"]');
    form.querySelector('[name="username"]').value = ${JSON.stringify(serverOptions.username)};
    form.querySelector('[name="password"]').value = ${JSON.stringify(serverOptions.password)};
    form.submit(); })()`);
  await selectorExists(main.client, SETTINGS, 30000);
  await textShown(main.client, RESTORED);
  s.check('the late navigation restores the latest edits after signing in',
    (await fieldValue(main, 'theme-mode')) === 'dark' && (await fieldValue(main, 'list-density')) === 'compact');
  // The page's cookie is new: the next seed signs in through the form again.
  main.server.signedInThroughForm = false;
});

// --- V1: reader -----------------------------------------------------------------------------------

define('reader-recovers-without-localizing', {}, async (s) => {
  await seedDemo('/entries/1', READER);
  let mark = main.traffic.mark();
  await navigate(main.client, `${base}/entries/1`);
  await selectorExists(main.client, READER);
  await main.traffic.settled();
  s.check('opening an HTML entry localizes its images (once)', main.traffic.count(mark, 'POST', '/api/v1/entries/1/localize-assets') === 1, main.traffic.log(mark));
  await main.faults.add({ match: request('GET', '/api/v1/entries/2'), action: 'fail', times: 1 });
  mark = main.traffic.mark();
  await navigate(main.client, `${base}/entries/2`);
  await waitFor(main.client, `document.querySelector('[data-page="reader"]')?.dataset.state === 'error'`);
  await waitBanner(main, BANNER.reader, 'error', '读取文章失败');
  await triggerFocus(main);
  await textShown(main.client, 'Demo Entry Two body.');
  await main.traffic.settled();
  s.check('a reread recovers the entry', (await evaluate(main.client, `document.querySelector('[data-page="reader"]').dataset.state`)) !== 'error');
  s.check('the recovery load does not ask to localize images', main.traffic.count(mark, 'POST', '/api/v1/entries/2/localize-assets') === 0
    && main.traffic.writes(mark).length === 0, main.traffic.log(mark));
});

const MALICIOUS = [
  `<script>window.__pwned = 'script'</script>`,
  `<img src="x" onerror="window.__pwned = 'onerror'">`,
  `<svg onload="window.__pwned = 'svg'"><circle r="1"></circle></svg>`,
  `<a id="js-link" href="javascript:window.__pwned = 'javascript-url'">link</a>`,
  `<iframe srcdoc="&lt;script&gt;parent.__pwned = 'iframe'&lt;/script&gt;"></iframe>`,
  `<object data="javascript:parent.__pwned = 'object'"></object>`,
  `<embed src="javascript:parent.__pwned = 'embed'">`,
  `<form action="javascript:parent.__pwned = 'form'"><button>go</button></form>`,
  `<meta http-equiv="refresh" content="0;url=javascript:window.__pwned = 'meta'">`,
  `<style>@import url("javascript:window.__pwned = 'style'");</style>`,
  `<details open ontoggle="window.__pwned = 'details'"><summary>more</summary></details>`,
  `<base href="javascript:window.__pwned = 'base'//">`,
  `<p>Safe text.</p>`,
].join('');

define('reader-malicious-body', {}, async (s) => {
  await seedDemo('/entries/2', '[data-slot="reader-body-html"]', {
    edit: ({ entry_content }) => {
      const body = entry_content.entries.find((item) => item.entry_id === 2);
      body.content_html = MALICIOUS;
    },
  });
  await textShown(main.client, 'Safe text.');
  await sleep(1000);
  s.check('nothing in the body ran', await evaluate(main.client, 'window.__pwned === undefined'), await evaluate(main.client, 'window.__pwned ?? null'));
  const left = await evaluate(main.client, `(() => {
    const body = document.querySelector('[data-slot="reader-body-html"]');
    const elements = [...body.querySelectorAll('*')];
    return {
      tags: [...body.querySelectorAll('script, iframe, object, embed, form, meta, style, link, base')].map(e => e.tagName),
      handlers: elements.flatMap(e => [...e.attributes].filter(a => a.name.toLowerCase().startsWith('on')).map(a => e.tagName + ' ' + a.name)),
      urls: elements.flatMap(e => [...e.attributes].filter(a => /^\\s*javascript:/i.test(a.value)).map(a => e.tagName + ' ' + a.name)),
    };
  })()`);
  s.check('no script, frame, form, style or base element, no handler, no javascript: address in the body',
    left.tags.length === 0 && left.handlers.length === 0 && left.urls.length === 0, left);
  s.check('the reader stays on the entry', (await location(main)) === '/entries/2');
});

// --- V1 and R2-07: start-up failures ----------------------------------------------------------------

define('settings-read-fails-at-start', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  await main.faults.add({ match: request('GET', '/api/v1/settings'), action: 'fail', times: 1 });
  await navigate(main.client, `${base}/entries`);
  const notice = '[data-slot="app-notice"][data-notice="settings-load"]';
  await waitFor(main.client, `(n => n?.textContent.includes('读取设置失败') && n.textContent.includes('暂用默认外观'))(document.querySelector('${notice}'))`);
  s.check('the start-up failure is shown, with a retry', await evaluate(main.client, `!!document.querySelector('[data-action="retry-settings-load"]')`));
  const mark = main.traffic.mark();
  await clickSelector(main.client, '[data-nav="settings"]');
  await selectorExists(main.client, SETTINGS);
  await waitFor(main.client, `!document.querySelector('${notice}')`);
  await main.traffic.settled();
  s.check('the settings page recovers it, without writing or starting a refresh', main.traffic.writes(mark).length === 0, main.traffic.log(mark));
  await main.faults.add({ match: request('GET', '/api/v1/settings'), action: 'fail', times: 1 });
  await navigate(main.client, `${base}/entries`);
  await selectorExists(main.client, '[data-action="retry-settings-load"]');
  await clickSelector(main.client, '[data-action="retry-settings-load"]');
  await waitFor(main.client, `!document.querySelector('${notice}')`);
  s.check('the retry recovers it too', true);
});

define('startup-target-fails', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  await main.faults.add({ match: request('GET', '/api/v1/startup-target'), action: 'fail', times: 1 });
  await navigate(main.client, `${base}/`);
  const notice = '[data-slot="app-notice"][data-notice="startup-route"]';
  await waitFor(main.client, `(n => n?.textContent.includes('解析启动页面失败') && n.textContent.includes('已打开全部文章'))(document.querySelector('${notice}'))`);
  await waitUntil('the redirect to all entries', 10000, async () => (await location(main)) === '/entries');
  await sleep(7000);
  s.check('the notice stays after the redirect (no timer)', await evaluate(main.client, `!!document.querySelector('${notice}')`));
  await clickSelector(main.client, '[data-action="dismiss-startup-failure"]');
  await waitFor(main.client, `!document.querySelector('${notice}')`);
  s.check('and can be dismissed', true);
});

// --- R2-07: failures and unknown outcomes are visible -------------------------------------------

/// Waits for a page's status banner to show `tone` with `part`, then checks that it is rendered
/// (computed visibility and a box), not only present (review C4C2-04).
async function bannerShown(s, label, page, selector, tone, part) {
  await waitBanner(page, selector, tone, part);
  const shown = await banner(page, selector);
  s.check(`${label}: shown (${tone})`, shown?.visible === true, shown);
}

/// The refresh badge a button draws (`::after`): its symbol, rendered, on a rendered button.
const badge = (page, selector) => evaluate(page.client, `(button => {
  if (!button) return null;
  const after = getComputedStyle(button, '::after');
  return { state: button.dataset.refreshState, content: after.content, display: after.display,
    visibility: after.visibility, buttonVisible: ${VISIBLE}(button) };
})(document.querySelector(${JSON.stringify(selector)}))`);
const badgeShows = (shown, symbol) => shown?.content === `"${symbol}"` && shown.display !== 'none'
  && shown.visibility === 'visible' && shown.buttonVisible === true;

define('failures-and-unknowns-visible', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  const failed = { action: 'fulfill', status: 503, body: busy('服务器繁忙（测试）'), times: 1 };
  const lost = { action: 'fail', times: 1 };
  // Entries: a failed write, then one whose outcome is unknown.
  await main.faults.add({ match: request('PUT', '/api/v1/entries/2/read'), ...failed });
  await pointerClick(main.client, `${card(2)} [data-action="mark-read"]`);
  await bannerShown(s, 'entries', main, BANNER.entries, 'error', '更改已读状态失败：服务器繁忙（测试）');
  await main.faults.add({ match: request('PUT', '/api/v1/entries/2/read'), ...lost });
  await pointerClick(main.client, `${card(2)} [data-action="mark-read"]`);
  await bannerShown(s, 'entries', main, BANNER.entries, 'unknown', '更改已读状态');
  // Feeds: an add whose outcome is unknown keeps the address; then one that failed.
  await clickSelector(main.client, '[data-nav="feeds"]');
  await selectorExists(main.client, FEEDS);
  await main.faults.add({ match: request('POST', '/api/v1/jobs/add-subscription'), ...lost });
  const address = `${harness.feeds.base}/new/unknown-add.xml`;
  await evaluate(main.client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
  await main.client.send('Input.insertText', { text: address });
  await clickSelector(main.client, '[data-action="add-feed"]');
  await bannerShown(s, 'feeds', main, BANNER.feeds, 'unknown', '添加订阅');
  s.check('feeds: an unknown add, the address kept', (await fieldValue(main, 'feed-url-input')) === address);
  await main.faults.add({ match: request('POST', '/api/v1/jobs/add-subscription'), ...failed });
  await clickSelector(main.client, '[data-action="add-feed"]');
  await bannerShown(s, 'feeds', main, BANNER.feeds, 'error', '服务器繁忙（测试）');
  // Settings: unknown, then failed.
  await clickSelector(main.client, '[data-nav="settings"]');
  await selectorExists(main.client, SETTINGS);
  await choose(main, 'theme-mode', 'dark');
  await main.faults.add({ match: request('PATCH', '/api/v1/settings'), ...lost });
  await clickSelector(main.client, '[data-action="save-settings"]');
  await bannerShown(s, 'settings', main, BANNER.settings, 'unknown', '草稿已保留');
  await main.faults.add({ match: request('PATCH', '/api/v1/settings'), ...failed });
  await clickSelector(main.client, '[data-action="save-settings"]');
  await bannerShown(s, 'settings', main, BANNER.settings, 'error', '保存设置失败：服务器繁忙（测试）');
  // Reader: an unknown write, then a failed one (info statuses stay hidden there).
  await navigate(main.client, `${base}/entries/2`);
  await selectorExists(main.client, READER);
  await main.faults.add({ match: request('PUT', '/api/v1/entries/2/read'), ...lost });
  await pointerClick(main.client, '[data-action="mark-read"]');
  await bannerShown(s, 'reader', main, BANNER.reader, 'unknown', '结果未知');
  await main.faults.add({ match: request('PUT', '/api/v1/entries/2/read'), ...failed });
  await pointerClick(main.client, '[data-action="mark-read"]');
  await bannerShown(s, 'reader', main, BANNER.reader, 'error', '服务器繁忙（测试）');
  // The manual refresh's result drawn on the reader's Home button and on the collapsed toggle.
  for (const [tone, symbol, rule] of [['unknown', '?', lost], ['error', '!', failed]]) {
    await navigate(main.client, `${base}/entries`);
    await selectorExists(main.client, ENTRIES);
    await main.faults.add({ match: request('POST', '/api/v1/jobs/refresh-all'), ...rule });
    await clickSelector(main.client, '[data-action="activate-home"]');
    await waitFor(main.client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === '${tone}'`);
    await clickSelector(main.client, `${card(2)} [data-slot="entry-card-title"]`);
    await selectorExists(main.client, '[data-page="reader"]');
    const home = await badge(main, '[data-action="activate-home"]');
    s.check(`the reader's Home button shows "${symbol}" for a refresh ended ${tone}`, home?.state === tone && badgeShows(home, symbol), home);
    await clickSelector(main.client, '[data-action="toggle-nav"]');
    await waitFor(main.client, `document.querySelector('[data-layout="app-nav-shell"]').dataset.state === 'collapsed'`);
    const toggle = await badge(main, '[data-action="toggle-nav"]');
    s.check(`the collapsed navigation's toggle shows "${symbol}" too`, toggle?.state === tone && badgeShows(toggle, symbol), toggle);
    await clickSelector(main.client, '[data-action="toggle-nav"]');
  }
});

// --- V1: the formal build ---------------------------------------------------------------------------

define('http-backend-only', {}, async (s) => {
  await seedDemo('/entries', ENTRIES);
  const mark = main.traffic.mark();
  await pointerClick(main.client, `${card(2)} [data-action="mark-read"]`);
  await waitFor(main.client, `document.querySelector('${card(2)} [data-slot="entry-card-meta"]').textContent.includes('已读')`);
  await main.traffic.settled();
  const store = await evaluate(main.client, `Object.keys(localStorage).filter(k => k.startsWith('rssr-web-'))`);
  s.check('no library in the browser (no browser-store keys)', store.length === 0, store);
  const resources = await evaluate(main.client, `performance.getEntriesByType('resource').map(r => r.name)
    .filter(n => /^https?:/.test(n) && (!n.startsWith(location.origin) || n.includes('feed-proxy')))`);
  s.check('everything from the server\'s own origin, through /api/v1 (no feed proxy, no other host)', resources.length === 0
    && main.traffic.since(mark).every((r) => r.path.startsWith('/api/v1/')), resources);
});

define('no-browser-periodic-refresh', {}, async (s) => {
  await seedDemo('/entries', ENTRIES, { edit: ({ core }) => { core.settings.refresh_interval_minutes = 1; } });
  // A tab of its own (virtual time stays paused on a page once its budget is spent).
  const page = await s.open(base);
  await navigate(page.client, `${base}/entries`);
  await selectorExists(page.client, ENTRIES);
  await page.traffic.settled();
  const mark = page.traffic.mark();
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('virtual time budget did not expire')), 60000);
    page.client.on('Emulation.virtualTimeBudgetExpired', () => { clearTimeout(timer); resolve(); });
    page.client.send('Emulation.setVirtualTimePolicy', { policy: 'advance', budget: 10 * 60 * 1000 }).catch(reject);
  });
  const advanced = await evaluate(page.client, 'performance.now()');
  await sleep(1000);
  s.check('ten minutes of page time at a one-minute interval: the page starts no refresh (the server does)',
    page.traffic.count(mark, 'POST', /^\/api\/v1\/jobs\//) === 0 && advanced > 9 * 60 * 1000, { advanced, traffic: page.traffic.log(mark) });
});

// --- V1: background tab ---------------------------------------------------------------------------

define('background-tab-time-limit', { long: true }, async (s) => {
  await seedDemo('/entries/2', READER);
  const held = await main.faults.add({ match: request('PUT', '/api/v1/entries/2/read'), action: 'hold' });
  const mark = main.traffic.mark();
  await pointerClick(main.client, '[data-action="mark-read"]');
  await waitUntil('the held write', 10000, async () => held.waiting() === 1);
  const other = await s.open('about:blank');
  await activate(other.target.id);
  await waitFor(main.client, 'document.hidden');
  await sleep(36000);
  const hiddenState = await banner(main, BANNER.reader);
  await activate(main.target.id);
  await waitFor(main.client, '!document.hidden');
  s.check('in a background tab the request time limit (30 s) still ended the write as unknown',
    hiddenState?.state === 'unknown' && hiddenState.text.includes('结果未知'), hiddenState);
  await held.release('fail');
  s.check('sent once', main.traffic.count(mark, 'PUT', '/api/v1/entries/2/read') === 1, main.traffic.log(mark));
});

// --- Run --------------------------------------------------------------------------------------------

/// Whether `error` is the network log entry of an answer `expected` describes.
function expectedAnswer(error, { status, path }) {
  if (error.type !== 'log.error' || error.source !== 'network' || !error.url) return false;
  if (!error.text?.includes(`the server responded with a status of ${status} `)) return false;
  const pathname = new URL(error.url).pathname;
  return path instanceof RegExp ? path.test(pathname) : pathname === path;
}

/// Why a console error is part of the scenario, or undefined when it is a failure.
function ignoredBecause(scenario, provoked, error) {
  if (error.text?.includes('/_dioxus?build_id=') && error.text?.includes('WebSocket')) {
    return 'debug bundle live-reload endpoint is absent on rssr-web';
  }
  if (error.type === 'log.error' && error.networkRequestId && provoked.has(error.networkRequestId)) {
    return 'a request the scenario failed on purpose';
  }
  const expected = scenario.expected.find((answer) => expectedAnswer(error, answer));
  return expected && `expected by the scenario: ${expected.status} for ${expected.path}`;
}

/// The scenario's console errors, split into failures and the ignored ones with their reasons.
function sortConsole(scenario, pages) {
  const provoked = new Set(pages.flatMap((page) => [...page.faults.provoked]));
  const unexpected = [];
  const ignored = [];
  for (const error of scenario.console) {
    const reason = ignoredBecause(scenario, provoked, error);
    if (reason) ignored.push({ ...error, reason });
    else unexpected.push(error);
  }
  return { unexpected, ignored };
}

async function run() {
  const unknown = (only ?? []).filter((name) => !scenarios.some((scenario) => scenario.name === name));
  if (unknown.length) throw new Error(`--only: no such scenario: ${unknown.join(', ')}`);
  await mkdir(artifactDir, { recursive: true });
  harness = await startHarness(serverOptions);
  for (const [signal, code] of [['SIGINT', 130], ['SIGTERM', 143]]) {
    process.once(signal, () => { harness.close().finally(() => process.exit(code)); });
  }
  let failed = false;
  // A failure outside any scenario (the browser, the harness): the run did not happen as asked.
  let runError;
  try {
    const version = await (await fetch(`${cdpBase}/json/version`, { signal: AbortSignal.timeout(10000) })).json();
    browser = connect(version.webSocketDebuggerUrl);
    main = await openPage(base);
    for (const scenario of scenarios) {
      if (only && !only.includes(scenario.name)) continue;
      if (scenario.long && !long) {
        results.push({ name: scenario.name, status: 'skipped', reason: 'waits 30 s or more: run with --long' });
        continue;
      }
      const s = new Scenario(scenario.name);
      current = s;
      const started = Date.now();
      let error;
      try {
        await scenario.body(s);
      } catch (caught) {
        error = caught.stack ?? String(caught);
      }
      const pages = [main, ...s.pages];
      const { unexpected, ignored } = sortConsole(s, pages);
      const ok = !error && s.checks.every((check) => check.ok) && unexpected.length === 0;
      if (!ok) {
        failed = true;
        for (const [index, page] of pages.entries()) {
          const name = `${scenario.name}${index ? `-page${index}` : ''}`;
          await capturePage(page.client, artifactDir, name).catch(() => {});
          await writeFile(path.join(artifactDir, `${name}-traffic.txt`), page.traffic.log().join('\n'), 'utf8').catch(() => {});
        }
      }
      results.push({ name: scenario.name, status: ok ? 'pass' : 'fail', seconds: Math.round((Date.now() - started) / 1000),
        error, checks: s.checks, unexpectedConsole: unexpected, ignoredConsole: ignored });
      process.stdout.write(`${ok ? 'pass' : 'FAIL'} ${scenario.name} (${Math.round((Date.now() - started) / 1000)} s)\n`);
      current = undefined;
      for (const cleanup of s.cleanups.reverse()) await Promise.resolve(cleanup()).catch(() => {});
      for (const page of s.pages) await page.close();
      await main.faults.reset();
      await main.client.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 }).catch(() => {});
      await activate(main.target.id).catch(() => {});
    }
  } catch (error) {
    runError = error.stack ?? String(error);
    throw error;
  } finally {
    await writeFile(path.join(artifactDir, 'assertions.json'), JSON.stringify({
      status: failed || runError ? 'fail' : 'pass', long, error: runError, scenarios: results }, null, 2), 'utf8');
    await main?.close();
    browser?.close();
    await harness.close();
  }
  if (failed) throw new Error('acceptance failed; see assertions.json');
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
