// The page's traffic with the server's API, seen and steered over CDP (phase C, C4c-2): a recorder
// of every `/api/` request (method, path, status, failure, redirects) that can wait until the
// page has settled, and one interceptor per page that fails, answers, holds or rewrites chosen
// requests. Node built-ins only.

import { sleep } from './cdp_session.mjs';

const SAFE = new Set(['GET', 'HEAD', 'OPTIONS']);
/// Long polls of a job's status stay open up to 10 s by design: they do not keep a page unsettled.
const LONG_POLL = /^\/api\/v1\/jobs\/[0-9a-f]{32}$/;

const isApi = (url) => {
  try {
    return new URL(url).pathname.startsWith('/api/');
  } catch {
    return false;
  }
};

/// Every `/api/` request the page makes (Network domain, enabled by the caller or here).
export class ApiTraffic {
  constructor(client) {
    this.client = client;
    this.entries = [];
    this.byId = new Map();
    this.mainLoaderId = undefined;
    this.mainLoaders = new Set();
    // When a request (job long polls aside) last started or ended.
    this.lastActivity = Date.now();
  }

  /// A request (job long polls aside) started or ended now.
  touch(entry) {
    if (!LONG_POLL.test(entry.path)) this.lastActivity = Date.now();
  }

  async start() {
    const { client } = this;
    let navigations = 0;
    const mainDocument = (frame) => {
      if (frame.parentId || !frame.loaderId) return;
      this.mainLoaderId = frame.loaderId;
      this.mainLoaders.add(frame.loaderId);
      this.lastActivity = Date.now();
      navigations += 1;
    };
    // Chrome can freeze an old document in BFCache before its pending fetch emits a terminal
    // Network event. Keep that request in the history, but do not wait for it on another
    // document. A BFCache restore reactivates the same loader, including its pending requests.
    client.on('Page.frameNavigated', ({ frame }) => mainDocument(frame));
    client.on('Network.requestWillBeSent', ({ requestId, loaderId, request, redirectResponse }) => {
      if (!isApi(request.url) && !this.byId.has(requestId)) return;
      const known = this.byId.get(requestId);
      if (known && redirectResponse) {
        // The same request, redirected: record where it went (a redirect is not a new request).
        known.redirects.push({ status: redirectResponse.status, to: request.url, method: request.method });
        return;
      }
      const url = new URL(request.url);
      // `loaderId`: the document that made the request (a reload starts another).
      const entry = { id: requestId, loaderId, method: request.method, path: url.pathname, query: url.search,
        origin: url.origin, at: Date.now(), done: false, redirects: [] };
      this.entries.push(entry);
      this.byId.set(requestId, entry);
      this.touch(entry);
    });
    client.on('Network.responseReceived', ({ requestId, response }) => {
      const entry = this.byId.get(requestId);
      if (entry) entry.status = response.status;
    });
    client.on('Network.loadingFinished', ({ requestId }) => {
      const entry = this.byId.get(requestId);
      if (!entry) return;
      entry.done = true;
      this.touch(entry);
    });
    client.on('Network.loadingFailed', ({ requestId, errorText, canceled }) => {
      const entry = this.byId.get(requestId);
      if (!entry) return;
      Object.assign(entry, { done: true, failed: errorText ?? 'failed', canceled });
      this.touch(entry);
    });
    await client.send('Page.enable');
    const before = navigations;
    const { frameTree } = await client.send('Page.getFrameTree');
    // Do not overwrite a navigation received while the initial snapshot was on its way.
    if (navigations === before) mainDocument(frameTree.frame);
    await client.send('Network.enable');
    return this;
  }

  /// Only a known former main document is excluded. Unknown loaders (including workers and
  /// subframes) remain conservative: their requests must still end. History/counts are unchanged.
  inFlight() {
    return this.entries.filter((entry) => !entry.done && !LONG_POLL.test(entry.path)
      && (!this.mainLoaders.has(entry.loaderId) || entry.loaderId === this.mainLoaderId));
  }

  /// A position in the record; `since(mark)` lists what came after.
  mark() {
    return this.entries.length;
  }

  since(mark = 0) {
    return this.entries.slice(mark);
  }

  /// Requests since `mark` matching `method` (or any) and `path` (a string or a RegExp).
  count(mark, method, path) {
    return this.since(mark).filter((entry) => (!method || entry.method === method)
      && (path === undefined || (path instanceof RegExp ? path.test(entry.path) : entry.path === path))).length;
  }

  /// The writes since `mark` (anything but GET, HEAD, OPTIONS).
  writes(mark) {
    return this.since(mark).filter((entry) => !SAFE.has(entry.method));
  }

  /// A compact list for evidence: `METHOD path status`.
  log(mark = 0) {
    return this.since(mark).map((entry) => `${entry.method} ${entry.path}${entry.query} `
      + `${entry.status ?? 'no answer'}${entry.failed ? ` failed(${entry.failed}${entry.canceled ? ', canceled' : ''})` : ''}`
      + `${entry.redirects.length ? ` redirected ${entry.redirects.map((r) => r.status).join(',')}` : ''}`);
  }

  /// Waits until the page has settled: past the reread triggers' 200 ms window, no request in
  /// flight for this document and none started or ended (job long polls aside) for `quietMs`
  /// (review C4C2-05: a request between two looks counts too). A response status is not an end.
  async settled({ quietMs = 600, timeoutMs = 20000 } = {}) {
    await sleep(250);
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const busy = this.inFlight().length > 0;
      if (!busy && Date.now() - this.lastActivity >= quietMs) return;
      await sleep(50);
    }
    const open = this.inFlight();
    throw new Error(`the page did not settle (document ${this.mainLoaderId ?? 'unknown'}): `
      + open.map((entry) => `${entry.method} ${entry.path} [request ${entry.id}, loader ${entry.loaderId}]`).join(', '));
  }
}

/// Steers chosen `/api/` requests of one page (Fetch domain; one interceptor per page, as only one
/// set of patterns is active per session). A rule: `{ match({method, url, path, stage}), action,
/// times, ... }` with action `fail` (network failure, `errorReason`), `fulfill` (`status`, `body`,
/// `contentType`, `headers`), `hold` (until released), `headers` (`rewrite(headers)` → headers sent
/// instead), or `failResponse` (the server answered, the page gets a network failure: a committed
/// write whose answer was lost). The first rule with uses left applies; other requests continue.
/// Requests a rule failed, answered or sent astray (`headers`) are recorded (`provoked`, Network
/// request ids) so the console errors they cause can be told from real ones; a held request only
/// when it is released as failed.
export class ApiInterceptor {
  constructor(client) {
    this.client = client;
    this.rules = [];
    this.provoked = new Set();
    this.extraPatterns = [];
  }

  async start() {
    this.client.on('Fetch.requestPaused', (event) => {
      this.paused(event).catch(() => {});
    });
    await this.enable();
    return this;
  }

  /// The patterns: every API request at the request stage; the response stage only while a rule
  /// needs it (every paused answer costs a round trip); any extra patterns (`addPattern`).
  async enable() {
    const patterns = [{ urlPattern: '*/api/*', requestStage: 'Request' }, ...this.extraPatterns];
    if (this.rules.some((rule) => rule.action === 'failResponse')) {
      patterns.push({ urlPattern: '*/api/*', requestStage: 'Response' });
    }
    await this.client.send('Fetch.enable', { patterns });
  }

  /// Also pauses requests matching `pattern` (a CDP request pattern, e.g. a document navigation).
  async addPattern(pattern) {
    this.extraPatterns.push(pattern);
    await this.enable();
  }

  async add(rule) {
    const entry = { times: Infinity, ...rule, used: 0, held: [] };
    this.rules.push(entry);
    if (rule.action === 'failResponse') await this.enable();
    return {
      hits: () => entry.used,
      waiting: () => entry.held.length,
      release: (how = 'continue') => this.release(entry, how),
      remove: async () => {
        await this.release(entry, 'continue');
        this.rules = this.rules.filter((other) => other !== entry);
        if (rule.action === 'failResponse') await this.enable();
      },
    };
  }

  async paused({ requestId, request, networkId, responseStatusCode, responseErrorReason }) {
    const stage = responseStatusCode !== undefined || responseErrorReason !== undefined ? 'Response' : 'Request';
    const url = new URL(request.url);
    const facts = { method: request.method, url: request.url, path: url.pathname, query: url.search, stage };
    const rule = this.rules.find((candidate) => candidate.used < candidate.times
      && (candidate.action === 'failResponse') === (stage === 'Response') && candidate.match(facts));
    const send = (method, params) => this.client.send(method, { requestId, ...params });
    if (!rule) {
      await send(stage === 'Response' ? 'Fetch.continueResponse' : 'Fetch.continueRequest', {});
      return;
    }
    rule.used += 1;
    if (networkId && rule.action !== 'hold') this.provoked.add(networkId);
    switch (rule.action) {
      case 'fail':
      case 'failResponse':
        await send('Fetch.failRequest', { errorReason: rule.errorReason ?? 'Failed' });
        break;
      case 'fulfill':
        await send('Fetch.fulfillRequest', {
          responseCode: rule.status ?? 200,
          responseHeaders: [{ name: 'Content-Type', value: rule.contentType ?? 'application/json' },
            ...(rule.headers ?? [])],
          body: Buffer.from(rule.body ?? '').toString('base64'),
        });
        break;
      case 'hold':
        rule.held.push({ requestId, networkId });
        break;
      case 'headers': {
        // `headers` replaces the whole list: start from the request's own.
        const headers = rule.rewrite({ ...request.headers });
        await send('Fetch.continueRequest', {
          headers: Object.entries(headers).map(([name, value]) => ({ name, value: String(value) })),
        });
        break;
      }
      default:
        throw new Error(`unknown action ${rule.action}`);
    }
  }

  /// Lets held requests go on (`continue`) or fail them (`fail`). A request the page already gave
  /// up on (its time limit) cannot be continued any more: that is ignored.
  async release(rule, how) {
    for (const { requestId, networkId } of rule.held.splice(0)) {
      if (how === 'fail' && networkId) this.provoked.add(networkId);
      const method = how === 'fail' ? 'Fetch.failRequest' : 'Fetch.continueRequest';
      await this.client.send(method, { requestId, ...(how === 'fail' ? { errorReason: 'Failed' } : {}) })
        .catch(() => {});
    }
  }

  /// Releases every hold and drops every rule, extra pattern and provoked request (between
  /// scenarios, once their console errors are sorted).
  async reset() {
    for (const rule of this.rules) await this.release(rule, 'continue');
    this.rules = [];
    this.extraPatterns = [];
    this.provoked.clear();
    await this.enable();
  }
}

/// A rule matcher: `method` (or any) and `path` (a string or a RegExp).
export const request = (method, path) => ({ method: actual, path: actualPath }) =>
  (!method || actual === method) && (path instanceof RegExp ? path.test(actualPath) : actualPath === path);
