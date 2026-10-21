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
  }

  async start() {
    const { client } = this;
    client.on('Network.requestWillBeSent', ({ requestId, request, redirectResponse }) => {
      if (!isApi(request.url) && !this.byId.has(requestId)) return;
      const known = this.byId.get(requestId);
      if (known && redirectResponse) {
        // The same request, redirected: record where it went (a redirect is not a new request).
        known.redirects.push({ status: redirectResponse.status, to: request.url, method: request.method });
        return;
      }
      const url = new URL(request.url);
      const entry = { id: requestId, method: request.method, path: url.pathname, query: url.search,
        origin: url.origin, at: Date.now(), done: false, redirects: [] };
      this.entries.push(entry);
      this.byId.set(requestId, entry);
    });
    client.on('Network.responseReceived', ({ requestId, response }) => {
      const entry = this.byId.get(requestId);
      if (entry) entry.status = response.status;
    });
    client.on('Network.loadingFinished', ({ requestId }) => {
      const entry = this.byId.get(requestId);
      if (entry) entry.done = true;
    });
    client.on('Network.loadingFailed', ({ requestId, errorText, canceled }) => {
      const entry = this.byId.get(requestId);
      if (entry) Object.assign(entry, { done: true, failed: errorText ?? 'failed', canceled });
    });
    await client.send('Network.enable');
    return this;
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

  /// Waits until the page has settled: past the reread triggers' 200 ms window, and no request
  /// in flight (job long polls aside) for `quietMs`.
  async settled({ quietMs = 600, timeoutMs = 20000 } = {}) {
    await sleep(250);
    const deadline = Date.now() + timeoutMs;
    let quietSince = Date.now();
    while (Date.now() < deadline) {
      const busy = this.entries.some((entry) => !entry.done && !LONG_POLL.test(entry.path));
      if (busy) quietSince = Date.now();
      else if (Date.now() - quietSince >= quietMs) return;
      await sleep(50);
    }
    const open = this.entries.filter((entry) => !entry.done && !LONG_POLL.test(entry.path));
    throw new Error(`the page did not settle: ${open.map((entry) => `${entry.method} ${entry.path}`).join(', ')}`);
  }
}

/// Steers chosen `/api/` requests of one page (Fetch domain; one interceptor per page, as only one
/// set of patterns is active per session). A rule: `{ match({method, url, path, stage}), action,
/// times, ... }` with action `fail` (network failure, `errorReason`), `fulfill` (`status`, `body`,
/// `contentType`, `headers`), `hold` (until released), `headers` (`rewrite(headers)` → headers sent
/// instead), or `failResponse` (the server answered, the page gets a network failure: a committed
/// write whose answer was lost). The first rule with uses left applies; other requests continue.
/// Requests a rule failed or answered are recorded (`provoked`, Network request ids) so the console
/// errors they cause can be told from real ones.
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
    if (networkId) this.provoked.add(networkId);
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
        rule.held.push({ requestId });
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
    for (const { requestId } of rule.held.splice(0)) {
      const method = how === 'fail' ? 'Fetch.failRequest' : 'Fetch.continueRequest';
      await this.client.send(method, { requestId, ...(how === 'fail' ? { errorReason: 'Failed' } : {}) })
        .catch(() => {});
    }
  }

  /// Releases every hold and drops every rule and extra pattern (between scenarios).
  async reset() {
    for (const rule of this.rules) await this.release(rule, 'continue');
    this.rules = [];
    this.extraPatterns = [];
    await this.enable();
  }
}

/// A rule matcher: `method` (or any) and `path` (a string or a RegExp).
export const request = (method, path) => ({ method: actual, path: actualPath }) =>
  (!method || actual === method) && (path instanceof RegExp ? path.test(actualPath) : actualPath === path);
