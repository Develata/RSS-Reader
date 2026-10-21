// The browser regression against the real server (phase C, C4c): a loopback feed fixture server
// and the `rssr-web` server on a fixture library per seed. The page no longer keeps a library in
// the browser; each seed is a fresh data directory prepared by
// `cargo run -p rssr-infra --example browser_fixture_library`, the server is restarted on it, and
// the page signs in. Node built-ins only.
//
// The server refreshes every subscription when it starts, and fetches them itself (not the page),
// under an outbound policy that allows only 127.0.0.1. Every fixture subscription is served by
// `FeedFixtureServer` with an entity tag: the server's conditional fetches get `304 Not Modified`
// unless a check arms more (new entries per round, a held request, an invalid body).

import { spawn } from 'node:child_process';
import { appendFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import path from 'node:path';

import { sleep } from './cdp_session.mjs';

const xmlEscape = (text) => String(text).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

function rss(title, items) {
  const body = items.map((item) => `<item><title>${xmlEscape(item.title)}</title>`
    + `<link>${xmlEscape(item.link)}</link><guid isPermaLink="false">${xmlEscape(item.guid)}</guid>`
    + `<pubDate>${item.pubDate}</pubDate><description>${xmlEscape(item.description ?? item.title)}</description></item>`).join('');
  return `<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>${xmlEscape(title)}</title>`
    + `<link>https://example.com/</link><description>fixture</description>${body}</channel></rss>`;
}

/// The feeds the server fetches. Paths are `/<seed>/<feed id>.xml` for the fixture library's
/// subscriptions and `/new/<name>.xml` for subscriptions a check adds; `/original/…` is a plain page
/// for entries' original links (the page opens it in a new tab).
export class FeedFixtureServer {
  constructor(logFile) {
    this.logFile = logFile;
    this.requests = [];
    this.rounds = new Map(); // seed -> rounds of new entries armed
    this.holds = []; // { match, waiting: [{ respond }] }
    this.invalid = new Set(); // paths answered once with an invalid body
    this.titles = new Map(); // seed -> Map(feed id -> the seeded title)
    this.addedAt = new Map(); // added feed name -> publication date of its entry
  }

  async start() {
    this.server = http.createServer((request, response) => this.handle(request, response));
    await new Promise((resolve) => this.server.listen(0, '127.0.0.1', resolve));
    this.base = `http://127.0.0.1:${this.server.address().port}`;
    return this.base;
  }

  close() {
    for (const hold of this.holds) for (const waiting of hold.waiting) waiting.respond();
    this.server?.close();
  }

  log(line) {
    if (this.logFile) appendFileSync(this.logFile, `${new Date().toISOString()} ${line}\n`);
  }

  /// Requests seen for `pathname` (or all), since the last `resetCounts`.
  count(pathname) {
    return pathname ? this.requests.filter((seen) => seen === pathname).length : this.requests.length;
  }

  resetCounts() {
    this.requests = [];
  }

  /// From now on every fetch of a subscription of `seed` answers the entries of all rounds armed
  /// so far: one new entry per subscription and round. Fetches within a round agree, so the
  /// number of new entries does not depend on how the server's concurrent fetches interleave.
  /// Rounds belong to one library: a new library of the seed starts without them (`newLibrary`).
  nextRound(seed) {
    this.rounds.set(seed, (this.rounds.get(seed) ?? 0) + 1);
  }

  /// A new library of `seed` is about to be served: its subscriptions answer the seed's entity
  /// tags again (no rounds). Holds and an armed `failNext` stay: arming one before a new library
  /// holds or fails that library's start-up fetch.
  newLibrary(seed) {
    this.rounds.delete(seed);
  }

  /// Holds requests whose path `match`es until `release()`; returns the hold.
  hold(match) {
    const hold = { match, waiting: [] };
    this.holds.push(hold);
    return {
      waiting: () => hold.waiting.length,
      release: () => {
        this.holds = this.holds.filter((other) => other !== hold);
        for (const waiting of hold.waiting.splice(0)) waiting.respond();
      },
    };
  }

  /// The next fetch of `pathname` answers 200 with a body that is not a feed.
  failNext(pathname) {
    this.invalid.add(pathname);
  }

  handle(request, response) {
    const { pathname } = new URL(request.url, this.base);
    this.requests.push(pathname);
    this.log(`${request.method} ${pathname} if-none-match=${request.headers['if-none-match'] ?? '-'}`);
    const respond = () => this.respond(request, response, pathname);
    const hold = this.holds.find((candidate) => candidate.match(pathname));
    if (hold) {
      hold.waiting.push({ respond });
      return;
    }
    respond();
  }

  respond(request, response, pathname) {
    if (response.writableEnded) return;
    const headers = { 'Content-Type': 'application/rss+xml; charset=utf-8', 'Cache-Control': 'no-store' };
    if (this.invalid.delete(pathname)) {
      this.log(`  ${pathname} -> 200 invalid body`);
      response.writeHead(200, headers).end('invalid RSS fixture');
      return;
    }
    if (pathname.startsWith('/original/')) {
      response.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' })
        .end('<!doctype html><meta charset="utf-8"><title>original</title><p>original fixture page</p>');
      return;
    }
    const added = pathname.match(/^\/new\/([^/]+)\.xml$/);
    if (added) {
      const name = decodeURIComponent(added[1]);
      // Dated when it is first served (then kept), so the entry is never in the archive.
      if (!this.addedAt.has(name)) this.addedAt.set(name, new Date().toUTCString());
      response.writeHead(200, headers).end(rss(`Added fixture ${name}`, [{
        title: `Added fixture entry ${name}`,
        link: `https://example.com/added/${name}`,
        guid: `added-${name}`,
        pubDate: this.addedAt.get(name),
      }]));
      return;
    }
    const seeded = pathname.match(/^\/([^/]+)\/(\d+)\.xml$/);
    if (!seeded) {
      response.writeHead(404).end();
      return;
    }
    const [, seed, feed] = seeded;
    const rounds = this.rounds.get(seed) ?? 0;
    const tag = `"${seed}-${feed}${rounds ? `-r${rounds}` : ''}"`;
    if (request.headers['if-none-match'] === tag) {
      this.log(`  ${pathname} -> 304`);
      response.writeHead(304, { ETag: tag }).end();
      return;
    }
    const items = [];
    for (let round = 1; round <= rounds; round += 1) {
      items.push({
        title: `手动刷新新增文章 ${feed} / ${round}`,
        link: `https://example.com/manual-refresh/${feed}/${round}`,
        guid: `manual-refresh-new-${feed}-${round}`,
        pubDate: 'Sat, 01 Aug 2026 00:00:00 GMT',
      });
    }
    // The seeded title: a refresh stores the channel's title, which must not replace it.
    const title = this.titles.get(seed)?.get(feed) ?? `Fixture feed ${feed}`;
    this.log(`  ${pathname} -> 200 ${items.length} entries`);
    response.writeHead(200, { ...headers, ETag: tag }).end(rss(title, items));
  }
}

/// `rssr-web` on a fixture library per seed, on one loopback port.
export class RssrWebServer {
  constructor(options) {
    this.options = options; // serverBin, staticDir, fixtureBin, seedsDir, themesDir, workDir, port, username, password, logFile, feeds
    this.generation = 0;
    this.base = `http://127.0.0.1:${options.port}`;
  }

  log(line) {
    appendFileSync(this.options.logFile, `${new Date().toISOString()} [harness] ${line}\n`);
  }

  async stop() {
    const child = this.child;
    this.child = undefined;
    if (!child || exited(child)) return;
    const gone = new Promise((resolve) => child.once('exit', resolve));
    child.kill();
    const timer = setTimeout(() => child.kill('SIGKILL'), 10000);
    await gone;
    clearTimeout(timer);
  }

  /// Restarts the server on a fresh library of `seed` (with the theme `preset` as custom CSS),
  /// waits until its start-up refresh has fetched and stored every active subscription, and
  /// returns a session cookie (signed in through the login form's endpoint). `edit` changes a copy
  /// of the seed first: it gets the four parsed files (`core`, `app_state`, `entry_flags`,
  /// `entry_content`) and mutates them. Without a seed (`null`) the server starts on an empty data
  /// directory and creates the library itself, as on a first run.
  async useSeed(seed, { preset, edit } = {}) {
    const { options } = this;
    await this.stop();
    this.generation += 1;
    const dataDir = path.join(options.workDir, `data-${this.generation}-${seed ?? 'empty'}`);
    rmSync(dataDir, { recursive: true, force: true });
    const args = ['--data-dir', dataDir];
    if (seed) {
      options.feeds.newLibrary(seed);
      const seedsDir = edit ? editedSeed(options.seedsDir, seed,
        path.join(options.workDir, `seeds-${this.generation}`), edit) : options.seedsDir;
      const core = readJson(path.join(seedsDir, `${seed.replace(/-/g, '_')}_core.json`));
      options.feeds.titles.set(seed, new Map(core.feeds.filter((feed) => feed.title)
        .map((feed) => [String(feed.id), feed.title])));
      args.push('--seeds', seedsDir, '--seed', seed, '--feed-base', options.feeds.base);
      const css = preset ? themeFile(options.themesDir, preset) : undefined;
      if (css) args.push('--custom-css', css);
    } else if (preset || edit) {
      throw new Error('an empty library has no preset or edits');
    }
    const prepared = await run(options.fixtureBin, args);
    const feeds = JSON.parse(prepared.stdout).feeds;
    options.feeds.resetCounts();
    this.log(`seed ${seed ?? '(empty library)'}${preset ? ` (${preset})` : ''} in ${dataDir}`);

    const startedAt = Date.now();
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) =>
      !/^RSS_READER_WEB_/i.test(key) && !/^RSSR_DATA_DIR$/i.test(key)));
    Object.assign(env, {
      RSS_READER_WEB_ENV: 'development',
      RSS_READER_WEB_BIND: `127.0.0.1:${options.port}`,
      RSS_READER_WEB_STATIC_DIR: options.staticDir,
      RSS_READER_WEB_USERNAME: options.username,
      RSS_READER_WEB_PASSWORD: options.password,
      RSSR_DATA_DIR: dataDir,
      // The harness reads the build id from the server's start-up line (rssr_web, info).
      RUST_LOG: process.env.RUST_LOG ? `${process.env.RUST_LOG},rssr_web=info` : 'info',
      NO_PROXY: '127.0.0.1,localhost',
      no_proxy: '127.0.0.1,localhost',
    });
    const child = spawn(options.serverBin, [], { env, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = '';
    const write = (chunk) => {
      appendFileSync(options.logFile, chunk);
      output += chunk;
    };
    child.stdout.on('data', write);
    child.stderr.on('data', write);
    this.child = child;
    await waitUntil(`rssr-web on ${this.base}`, 30000, async () => {
      if (exited(child)) throw new Error(`rssr-web exited (${child.exitCode ?? child.signalCode})`);
      return (await fetch(`${this.base}/healthz`, { signal: AbortSignal.timeout(REQUEST_MS) })
        .catch(() => null))?.ok === true;
    });
    this.buildId = await startedBuild(() => output);
    const cookie = await this.signIn('/entries');
    // The start-up refresh of this server, finished: every active subscription fetched by it
    // (counts were reset before it started) and its result stored, which replaces the seeded
    // fetch time (`fetched_at`, as the fixture tool wrote it) with one of this run. A time alone
    // shows nothing: a seed may carry any time, recent or in the future (2099). The change is
    // judged at full precision; the window, at the milliseconds `Date` has.
    await waitUntil(`start-up refresh of ${seed}`, 30000, async () => {
      const snapshot = await this.api('GET', '/api/v1/feeds', cookie);
      return feeds.every(({ id, url, fetched_at: seeded }) => {
        const stored = snapshot.feeds.find((candidate) => candidate.id === id)?.last_fetched_at;
        const changed = epochNanos(stored) !== null && epochNanos(stored) !== epochNanos(seeded);
        const fetchedAt = Date.parse(stored ?? '');
        return options.feeds.count(new URL(url).pathname) > 0 && changed
          && fetchedAt >= startedAt - 1000 && fetchedAt <= Date.now() + 1000;
      });
    });
    options.feeds.resetCounts();
    return { cookie, feeds };
  }

  /// Signs in like the login form and returns the session cookie (`name=value`).
  async signIn(next) {
    const { username, password } = this.options;
    const response = await fetch(`${this.base}/login`, {
      method: 'POST',
      redirect: 'manual',
      signal: AbortSignal.timeout(REQUEST_MS),
      headers: { Origin: this.base, 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ username, password, next }).toString(),
    });
    const cookie = response.headers.getSetCookie().map((line) => line.split(';')[0])
      .find((pair) => pair.startsWith('rssr_web_session='));
    if (response.status !== 303 || !cookie) throw new Error(`sign-in failed: ${response.status}`);
    return cookie;
  }

  /// A read of the server's API with `cookie` (the harness's own view of the library).
  async api(method, pathname, cookie) {
    // The signal also bounds reading the body.
    const response = await fetch(`${this.base}${pathname}`,
      { method, headers: { Cookie: cookie }, signal: AbortSignal.timeout(REQUEST_MS) });
    if (!response.ok) throw new Error(`${method} ${pathname}: ${response.status}`);
    return response.json();
  }
}

/// An RFC 3339 time as nanoseconds since the epoch (a BigInt: `Date` keeps milliseconds only), or
/// null.
export function epochNanos(text) {
  const parts = text?.match(/^(.+T\d\d:\d\d:\d\d)(\.\d+)?(Z|[+-]\d\d:\d\d)$/i);
  const seconds = parts ? Date.parse(`${parts[1]}${parts[3]}`) : NaN;
  if (Number.isNaN(seconds)) return null;
  const fraction = (parts[2] ?? '.').slice(1).padEnd(9, '0').slice(0, 9);
  return BigInt(seconds) * 1000000n + BigInt(fraction);
}

/// A bound on each request the harness makes, its body included (`waitUntil` checks its deadline
/// only between checks).
const REQUEST_MS = 10000;

const exited = (child) => child.exitCode !== null || child.signalCode !== null;

/// The server's build id, from its start-up line (terminal colours stripped).
async function startedBuild(output) {
  const startLine = () => output().replace(/\x1b\[[0-9;]*m/g, '').match(/starting rssr-web on \S+ build="([^"]+)"/);
  await waitUntil('the build id in the server output', 10000, async () => startLine() !== null);
  return startLine()[1];
}

/// The page must come from the server's build: the server refuses writes from another build, which
/// would fail a check's writes with a misleading message. The page sends its build id with every
/// request to the server (`x-rssr-build`); this watches them (CDP Network domain on `client`) and
/// `verify()` throws unless every one since `reset()` (before each page load) carried exactly the
/// server's id, and at least one was seen.
export async function watchPageBuild(client, base, server) {
  const seen = new Set();
  client.on('Network.requestWillBeSent', ({ request }) => {
    if (!request.url.startsWith(`${base}/api/`)) return;
    const header = Object.entries(request.headers).find(([name]) => name.toLowerCase() === 'x-rssr-build');
    seen.add(header?.[1] ?? '(none)');
  });
  await client.send('Network.enable');
  return {
    reset() {
      seen.clear();
    },
    verify() {
      const others = [...seen].filter((id) => id !== server.buildId);
      if (seen.size === 0) throw new Error('this page made no request to the server');
      if (others.length) {
        throw new Error(`the page is from build ${others.join(', ')}, the server from ${server.buildId}: ` +
          'build both from one checkout (or with the same RSSR_BUILD_ID)');
      }
    },
  };
}

const SEED_PARTS = ['core', 'app_state', 'entry_flags', 'entry_content'];

/// Writes `seed` edited by `edit` into `dir` (fresh) and returns `dir`.
function editedSeed(seedsDir, seed, dir, edit) {
  const prefix = seed.replace(/-/g, '_');
  const parts = Object.fromEntries(SEED_PARTS.map((part) =>
    [part, readJson(path.join(seedsDir, `${prefix}_${part}.json`))]));
  edit(parts);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  for (const part of SEED_PARTS) {
    writeFileSync(path.join(dir, `${prefix}_${part}.json`), JSON.stringify(parts[part], null, 2));
  }
  return dir;
}

const THEME_FILES = {
  'atlas-sidebar': 'atlas-sidebar.css',
  'atlas-sidebar-v1': 'legacy/atlas-sidebar-v1.css',
  newsprint: 'newsprint.css',
  'amethyst-glass': 'amethyst-glass.css',
  'midnight-ledger': 'midnight-ledger.css',
};

function themeFile(themesDir, preset) {
  const file = THEME_FILES[preset];
  if (!file) throw new Error(`unknown theme preset ${preset}`);
  return path.join(themesDir, file);
}

export async function waitUntil(what, timeoutMs, check) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      if (await check()) return;
    } catch (error) {
      lastError = error;
      if (/exited/.test(String(error))) throw error;
    }
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${what}${lastError ? `: ${lastError}` : ''}`);
}

function run(program, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(program, args, { stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('error', reject);
    // `close`, not `exit`: output may still be unread when the process exits.
    child.on('close', (code, signal) => (code === 0 ? resolve({ stdout, stderr })
      : reject(new Error(`${program} ${args.join(' ')} failed (${code ?? signal}): ${stderr}`))));
  });
}

/// Sets up both servers for a run. `workDir` is created fresh.
export async function startHarness(options) {
  rmSync(options.workDir, { recursive: true, force: true });
  mkdirSync(options.workDir, { recursive: true });
  const feeds = new FeedFixtureServer(path.join(options.artifactDir, 'feed-fixture.log'));
  await feeds.start();
  const server = new RssrWebServer({ ...options, feeds, logFile: path.join(options.artifactDir, 'server.log') });
  return {
    feeds,
    server,
    async close() {
      await server.stop();
      feeds.close();
    },
  };
}

export function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}
