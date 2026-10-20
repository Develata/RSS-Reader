import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { checkDirectoryContracts } from './directory_contract.mjs';
import { measureDirectory } from './directory_performance.mjs';

import {
  clickSelector,
  connect,
  evaluate,
  navigate,
  newPage,
  selectorExists,
  sleep,
  submitLoginForm,
  waitFor,
} from './cdp_session.mjs';
import { startHarness, waitUntil } from './server_harness.mjs';

function parseOptions(argv) {
  const options = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith('--') || value === undefined) {
      throw new Error(`Invalid CLI option near ${name ?? '<end>'}`);
    }
    options.set(name, value);
  }
  return options;
}

const cli = parseOptions(process.argv.slice(2));
const cdpBase = cli.get('--cdp-base') ?? process.env.CDP_BASE ?? 'http://127.0.0.1:9226';
const artifactDir =
  cli.get('--artifact-dir') ??
  process.env.ARTIFACT_DIR ??
  'target/server-web-regression';
// The page against the real server (phase C, C4c): `rssr-web` restarted on a fixture library per
// seed, its subscriptions served by a loopback feed fixture server (see server_harness.mjs).
const serverOptions = {
  serverBin: cli.get('--server-bin') ?? process.env.RSSR_WEB_BIN,
  staticDir: cli.get('--static-dir') ?? process.env.RSSR_WEB_STATIC_DIR,
  fixtureBin: cli.get('--fixture-bin') ?? process.env.RSSR_FIXTURE_BIN,
  seedsDir: cli.get('--seeds-dir') ?? 'tests/fixtures/browser_state',
  themesDir: cli.get('--themes-dir') ?? 'assets/themes',
  workDir: cli.get('--work-dir') ?? path.join(artifactDir, 'server-work'),
  artifactDir,
  port: Number.parseInt(cli.get('--port') ?? process.env.RSSR_WEB_PORT ?? '8091', 10),
  username: 'smoke',
  password: 'smoke-pass-123',
};
const base = `http://127.0.0.1:${serverOptions.port}`;
// The harness (web mode), the session the harness signed in with on the current seed (its own
// reads of the server's state), and whether the page signed in through the form yet.
let harness;
let session;
let signedInThroughForm = false;
const width = Number.parseInt(cli.get('--width') ?? process.env.VIEWPORT_WIDTH ?? '360', 10);
const height = Number.parseInt(cli.get('--height') ?? process.env.VIEWPORT_HEIGHT ?? '800', 10);
const deviceScaleFactor = Number.parseFloat(
  cli.get('--dpr') ?? process.env.DEVICE_SCALE_FACTOR ?? '3',
);
const preset = cli.get('--preset') ?? process.env.THEME_PRESET ?? '';
// Native mode attaches only to an explicitly selected, already running Dioxus
// WebView with an isolated, pre-seeded SQLite fixture supplied by the caller.
const nativeTarget = cli.get('--native-target');
const directoryOnly = cli.get('--directory-only') === 'true';
if (cli.has('--native-target') && !nativeTarget.trim()) {
  throw new Error('--native-target must identify an existing Dioxus WebView');
}
if (nativeTarget && ['--server-bin', '--width', '--height', '--dpr', '--preset'].some(option => cli.has(option))) {
  throw new Error('Native mode records the real window; server seeding and viewport/theme overrides are not supported');
}
if (!nativeTarget) {
  for (const [option, value] of [['--server-bin', serverOptions.serverBin], ['--static-dir', serverOptions.staticDir], ['--fixture-bin', serverOptions.fixtureBin]]) {
    if (!value) throw new Error(`${option} is required`);
  }
}

const assertions = [];
const consoleErrors = [];
const ignoredConsoleErrors = [];
const nativeMeasurements = [];
let nativeEvidence;

// Requests a check fails on purpose (path fragments): the browser logs their failure.
const provokedFailures = new Set();

function recordConsoleError(error) {
  if (error.text?.includes("/_dioxus?build_id=") && error.text?.includes('WebSocket connection')) {
    ignoredConsoleErrors.push({ ...error, reason: 'debug bundle live-reload endpoint is absent on rssr-web' });
    return;
  }
  const provoked = [...provokedFailures].find((part) => error.url?.includes(part) || error.text?.includes(part));
  if (provoked) {
    ignoredConsoleErrors.push({ ...error, reason: `failure provoked by the check (${provoked})` });
    return;
  }
  consoleErrors.push(error);
}

/// Fails the page's requests whose URL matches `pattern` (a CDP URL pattern) while `body` runs:
/// with `answer` (status and protocol error body) or, without it, as a network failure.
async function withFailedRequests(client, pattern, fragment, answer, body) {
  provokedFailures.add(fragment);
  const detach = client.on('Fetch.requestPaused', ({ requestId }) => {
    const reply = answer
      ? client.send('Fetch.fulfillRequest', { requestId, responseCode: answer.status,
        responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
        body: Buffer.from(JSON.stringify(answer.error)).toString('base64') })
      : client.send('Fetch.failRequest', { requestId, errorReason: 'Failed' });
    reply.catch(() => {});
  });
  await client.send('Fetch.enable', { patterns: [{ urlPattern: pattern, requestStage: 'Request' }] });
  try {
    return await body();
  } finally {
    await client.send('Fetch.disable');
    detach();
  }
}

/// A read of the server's state by the harness (its own session), in place of the old reads of
/// the browser's storage.
async function serverState(pathname) {
  return harness.server.api('GET', pathname, session);
}

/// The stored list preferences (the server answers them with the list page's first read).
async function storedWorkspace() {
  const input = encodeURIComponent(JSON.stringify({ load_preferences: true, load_feeds: false }));
  return (await serverState(`/api/v1/entries/bootstrap?input=${input}`)).workspace;
}

function assertThat(name, condition, details) {
  const result = { name, status: condition ? 'pass' : 'fail', details };
  assertions.push(result);
  if (!condition) {
    throw new Error(`${name}: ${JSON.stringify(details)}`);
  }
}

// WebView layout can report 43.999996px for a 44px CSS target. Allow only
// subpixel rounding noise; an actual 43px target must still fail.
function meetsTouchTarget(...dimensions) {
  return dimensions.every(size => Number.isFinite(size) && size >= 44 - 0.01);
}

async function setViewport(client, viewportWidth, viewportHeight, mobile, dpr) {
  await client.send('Emulation.setDeviceMetricsOverride', {
    width: viewportWidth,
    height: viewportHeight,
    screenWidth: viewportWidth,
    screenHeight: viewportHeight,
    deviceScaleFactor: dpr,
    mobile,
  });
  await client.send('Emulation.setTouchEmulationEnabled', {
    enabled: mobile,
    maxTouchPoints: mobile ? 5 : 1,
  });
  await client.send('Emulation.setEmulatedMedia', {
    features: [{ name: 'prefers-reduced-motion', value: 'no-preference' }],
  });
}

async function dispatchTouchScroll(client, point, scrollDeltaY) {
  const x = Math.round(point.x);
  const startY = Math.round(point.y);
  const fingerDeltaY = -scrollDeltaY;
  const steps = 6;
  const touch = (y) => [{ x, y: Math.round(y), radiusX: 1, radiusY: 1, force: 1, id: 1 }];

  await client.send('Input.dispatchTouchEvent', {
    type: 'touchStart',
    touchPoints: touch(startY),
  });
  for (let step = 1; step <= steps; step += 1) {
    await client.send('Input.dispatchTouchEvent', {
      type: 'touchMove',
      touchPoints: touch(startY + (fingerDeltaY * step) / steps),
    });
    await sleep(16);
  }
  await client.send('Input.dispatchTouchEvent', {
    type: 'touchEnd',
    touchPoints: [],
  });
  await sleep(120);
}

/// Restarts the server on a fresh library of `seed` (with the theme preset as custom CSS, and the
/// seed changed by `edit` if given) and opens `nextPath`. The first time the page signs in through
/// the login form; afterwards it gets the harness's session cookie for the new server's session
/// store.
async function seedAndNavigate(client, seed, nextPath, marker, themePreset = preset, { edit } = {}) {
  // No page stays open on the server being replaced: its requests would fail (and log errors).
  await navigate(client, 'about:blank');
  ({ cookie: session } = await harness.server.useSeed(seed, { preset: themePreset, edit }));
  if (!signedInThroughForm) {
    await navigate(client, `${base}/login?next=${encodeURIComponent(nextPath)}`);
    await submitLoginForm(client, serverOptions.username, serverOptions.password);
    signedInThroughForm = true;
  } else {
    const [name, value] = session.split('=');
    await client.send('Network.setCookie', { name, value, url: base, httpOnly: true, sameSite: 'Lax' });
    await navigate(client, `${base}${nextPath}`);
  }
  await selectorExists(client, marker, 30000);
  await sleep(300);
}

async function ensureEntryControlsOpen(client) {
  const hidden = await evaluate(
    client,
    `document.querySelector('[data-action="show-entry-controls"]') !== null`,
  );
  if (hidden) {
    await clickSelector(client, '[data-action="show-entry-controls"]');
  }
  await selectorExists(client, '[data-layout="entry-controls-panel"]');
}

async function captureArtifact(client, name) {
  const html = await evaluate(client, 'document.documentElement.outerHTML');
  await writeFile(path.join(artifactDir, `${name}.html`), html, 'utf8');
  const screenshot = await client.send('Page.captureScreenshot', {
    format: 'png',
    fromSurface: true,
    captureBeyondViewport: false,
  });
  await writeFile(path.join(artifactDir, `${name}.png`), screenshot.data, 'base64');
}

async function commonPageEvidence(client) {
  return await evaluate(
    client,
    `(() => {
      const root = document.documentElement;
      const bodyText = document.body?.innerText?.trim() ?? '';
      const overlay = document.querySelector(
        'vite-error-overlay, [data-vite-dev-id], [data-dioxus-error], #webpack-dev-server-client-overlay'
      );
      return {
        path: location.pathname,
        title: document.title,
        innerWidth,
        innerHeight,
        clientWidth: root.clientWidth,
        scrollWidth: root.scrollWidth,
        bodyTextLength: bodyText.length,
        overlay: overlay?.textContent ?? null,
      };
    })()`,
  );
}

async function checkEntriesOverflow(client) {
  await ensureEntryControlsOpen(client);
  const common = await commonPageEvidence(client);
  assertThat('mobile viewport is exact', common.innerWidth === width && common.innerHeight === height, common);
  assertThat('entries page has meaningful content', common.path === '/entries' && common.bodyTextLength > 100, common);
  assertThat('entries page has no framework overlay', common.overlay === null, common);
  assertThat('entries root has no horizontal overflow', common.scrollWidth <= common.clientWidth + 1, common);

  const geometry = await evaluate(
    client,
    `(() => {
      const chip = [...document.querySelectorAll('[data-layout="entry-filters-source-chip"]')]
        .find((element) => element.querySelector('span')?.textContent.length > 40);
      const span = chip?.querySelector('span');
      const directory = document.querySelector('[data-layout="entry-top-directory"]');
      const root = document.documentElement.getBoundingClientRect();
      const titles = [...document.querySelectorAll('[data-slot="entry-card-title"]')]
        .filter((element) => element.getClientRects().length > 0)
        .map((element) => {
          const rect = element.getBoundingClientRect();
          return { text: element.textContent, left: rect.left, right: rect.right };
        });
      const actionRows = [...document.querySelectorAll('[data-layout="entry-card-actions"]')]
        .filter((element) => element.getClientRects().length > 0)
        .map((row) => [...row.querySelectorAll('button, a')]
          .filter((element) => element.getClientRects().length > 0)
          .map((element) => {
            const rect = element.getBoundingClientRect();
            return { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom };
          }));
      const overlaps = actionRows.some((row) => row.some((rect, index) => row.slice(index + 1).some((other) =>
        rect.left < other.right && rect.right > other.left && rect.top < other.bottom && rect.bottom > other.top
      )));
      const chipRect = chip?.getBoundingClientRect();
      const style = span ? getComputedStyle(span) : null;
      return {
        chip: chip && span ? {
          height: chipRect.height,
          title: chip.getAttribute('title'),
          ariaLabel: chip.getAttribute('aria-label'),
          // The nested count has its own aria-describedby text; the control name is the title.
          text: [...span.childNodes].filter(node => node.nodeType === Node.TEXT_NODE)
            .map(node => node.textContent).join(''),
          scrollWidth: span.scrollWidth,
          clientWidth: span.clientWidth,
          whiteSpace: style.whiteSpace,
          textOverflow: style.textOverflow,
        } : null,
        directory: directory ? {
          chips: directory.querySelectorAll('[data-layout="entry-top-directory-chip"]').length,
          scrollWidth: directory.scrollWidth,
          clientWidth: directory.clientWidth,
        } : null,
        titleBoundsOk: titles.length > 0 && titles.every((title) => title.left >= root.left - 1 && title.right <= root.right + 1),
        titleCount: titles.length,
        actionOverlaps: overlaps,
      };
    })()`,
  );

  assertThat('long source chip is present', geometry.chip !== null, geometry);
  assertThat(
    'long source name wraps visibly without horizontal clipping',
    meetsTouchTarget(geometry.chip.height) &&
      geometry.chip.scrollWidth <= geometry.chip.clientWidth + 1 &&
      geometry.chip.whiteSpace !== 'nowrap' &&
      geometry.chip.textOverflow !== 'ellipsis',
    geometry.chip,
  );
  assertThat(
    'long source chip exposes the full accessible name',
    geometry.chip.title === geometry.chip.text && geometry.chip.ariaLabel === geometry.chip.text,
    geometry.chip,
  );
  assertThat(
    'overflow fixture renders a horizontally scrollable month directory',
    geometry.directory?.chips >= 10 && geometry.directory.scrollWidth > geometry.directory.clientWidth,
    geometry.directory,
  );
  assertThat('entry titles stay inside the viewport', geometry.titleBoundsOk, geometry);
  assertThat('entry card actions do not overlap', !geometry.actionOverlaps, geometry);

  const sourceScrollSetup = await evaluate(
    client,
    `(async () => {
      const grid = document.querySelector('[data-layout="entry-filters-source-grid"]');
      const seed = grid?.querySelector('[data-layout="entry-filters-source-chip"]');
      if (!grid || !seed) return null;
      for (let index = 0; index < 14; index += 1) {
        const clone = seed.cloneNode(true);
        clone.dataset.scrollChainFixture = 'true';
        clone.setAttribute('aria-hidden', 'true');
        grid.appendChild(clone);
      }
      grid.scrollIntoView({block:'center', inline:'nearest', behavior:'instant'});
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const rect = grid.getBoundingClientRect();
      const maximum = grid.scrollHeight - grid.clientHeight;
      return {
        x: (rect.left + rect.right) / 2,
        y: (rect.top + rect.bottom) / 2,
        maximum,
        policy: getComputedStyle(grid).overscrollBehaviorY,
        pageY: scrollY,
      };
    })()`,
  );
  assertThat(
    'source filter enables native vertical scroll chaining',
    sourceScrollSetup?.maximum > 80 &&
      ['auto', 'chain'].includes(sourceScrollSetup.policy),
    sourceScrollSetup,
  );

  await evaluate(client, `(() => {
    const grid = document.querySelector('[data-layout="entry-filters-source-grid"]');
    grid.scrollTop = (grid.scrollHeight - grid.clientHeight) / 2;
    window.__sourceScrollBefore = {inner:grid.scrollTop, page:scrollY};
  })()`);
  await dispatchTouchScroll(client, sourceScrollSetup, 120);
  const sourceScrollMiddle = await evaluate(client, `(() => {
    const grid = document.querySelector('[data-layout="entry-filters-source-grid"]');
    return {before:window.__sourceScrollBefore, inner:grid.scrollTop, page:scrollY};
  })()`);
  assertThat(
    'source filter consumes touch scroll while it has room',
    sourceScrollMiddle.inner > sourceScrollMiddle.before.inner + 10 &&
      Math.abs(sourceScrollMiddle.page - sourceScrollMiddle.before.page) <= 2,
    sourceScrollMiddle,
  );

  const sourceScrollBottomSetup = await evaluate(client, `(async () => {
    const grid = document.querySelector('[data-layout="entry-filters-source-grid"]');
    grid.scrollTop = grid.scrollHeight - grid.clientHeight;
    grid.scrollIntoView({block:'center', inline:'nearest', behavior:'instant'});
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const rect = grid.getBoundingClientRect();
    return {
      x:(rect.left + rect.right) / 2,
      y:(rect.top + rect.bottom) / 2,
      inner:grid.scrollTop,
      page:scrollY,
    };
  })()`);
  await dispatchTouchScroll(client, sourceScrollBottomSetup, 180);
  const sourceScrollBottom = await evaluate(client, `(() => {
    const grid = document.querySelector('[data-layout="entry-filters-source-grid"]');
    return {inner:grid.scrollTop, page:scrollY};
  })()`);
  assertThat(
    'source filter hands downward touch scroll to the page at its bottom edge',
    Math.abs(sourceScrollBottom.inner - sourceScrollBottomSetup.inner) <= 2 &&
      sourceScrollBottom.page > sourceScrollBottomSetup.page + 10,
    {before:sourceScrollBottomSetup, after:sourceScrollBottom},
  );

  const sourceScrollTopSetup = await evaluate(client, `(async () => {
    const grid = document.querySelector('[data-layout="entry-filters-source-grid"]');
    grid.scrollTop = 0;
    grid.scrollIntoView({block:'center', inline:'nearest', behavior:'instant'});
    if (scrollY < 120) window.scrollBy({top:120 - scrollY, behavior:'instant'});
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const rect = grid.getBoundingClientRect();
    return {
      x:(rect.left + rect.right) / 2,
      y:(rect.top + rect.bottom) / 2,
      inner:grid.scrollTop,
      page:scrollY,
    };
  })()`);
  await dispatchTouchScroll(client, sourceScrollTopSetup, -180);
  const sourceScrollTop = await evaluate(client, `(() => {
    const grid = document.querySelector('[data-layout="entry-filters-source-grid"]');
    return {inner:grid.scrollTop, page:scrollY};
  })()`);
  assertThat(
    'source filter hands upward touch scroll to the page at its top edge',
    sourceScrollTopSetup.page > 10 &&
      sourceScrollTop.inner <= 2 &&
      sourceScrollTop.page < sourceScrollTopSetup.page - 10,
    {before:sourceScrollTopSetup, after:sourceScrollTop},
  );

  await evaluate(client, `(() => {
    document.querySelectorAll('[data-scroll-chain-fixture="true"]').forEach(element => element.remove());
    const grid = document.querySelector('[data-layout="entry-filters-source-grid"]');
    if (grid) grid.scrollTop = 0;
    delete window.__sourceScrollBefore;
    window.scrollTo({top:0, behavior:'instant'});
  })()`);

  const maskStates = await evaluate(
    client,
    `(async () => {
      const directory = document.querySelector('[data-layout="entry-top-directory"]');
      const maximum = directory.scrollWidth - directory.clientWidth;
      async function readAt(fraction) {
        directory.scrollLeft = maximum * fraction;
        // Headless Chrome may throttle animation frames under CI load. Keep the
        // style assertion bounded so a stalled frame cannot hang the CDP call.
        await Promise.race([
          new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))),
          new Promise((resolve) => setTimeout(resolve, 1000)),
        ]);
        const style = getComputedStyle(directory);
        return {
          fraction,
          scrollLeft: directory.scrollLeft,
          maskImage: style.maskImage || style.webkitMaskImage,
        };
      }
      return {
        maximum,
        start: await readAt(0),
        middle: await readAt(0.5),
        nearEnd: await readAt(0.99),
        end: await readAt(1),
      };
    })()`,
  );
  const hasGradient = (state) => state.maskImage && state.maskImage !== 'none';
  assertThat(
    'directory overflow hint remains until the end',
    hasGradient(maskStates.start) && hasGradient(maskStates.middle) && hasGradient(maskStates.nearEnd),
    maskStates,
  );
  assertThat('directory overflow hint disappears at the end', maskStates.end.maskImage === 'none', maskStates);

  const verticalDirectoryTracking = await evaluate(
    client,
    `(async () => {
      const visible = (element) =>
        !!element && !!(element.offsetWidth || element.offsetHeight || element.getClientRects().length);
      const anchors = [...document.querySelectorAll('[data-entry-scroll-anchor]')].filter(visible);
      const target = anchors[Math.min(anchors.length - 1, Math.max(3, Math.floor(anchors.length * 0.6)))];
      target?.scrollIntoView({block:'start', inline:'nearest', behavior:'instant'});
      const requested = scrollY;
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      await new Promise((resolve) => setTimeout(resolve, 80));
      const rail = document.querySelector('[data-layout="entry-directory-rail"]');
      const topDirectory = document.querySelector('[data-layout="entry-top-directory"]');
      return {
        anchorCount: anchors.length,
        target: target?.id ?? null,
        requested,
        actual: scrollY,
        delta: scrollY - requested,
        railDisplay: rail ? getComputedStyle(rail).display : null,
        topDirectoryDisplay: topDirectory ? getComputedStyle(topDirectory).display : null,
      };
    })()`,
  );
  assertThat(
    'mobile directory tracking never rewinds document scroll',
    verticalDirectoryTracking.anchorCount >= 4 &&
      verticalDirectoryTracking.requested > 0 &&
      verticalDirectoryTracking.railDisplay === 'none' &&
      verticalDirectoryTracking.topDirectoryDisplay !== 'none' &&
      Math.abs(verticalDirectoryTracking.delta) <= 1,
    verticalDirectoryTracking,
  );

  await evaluate(client, `(() => {
    document.querySelector('[data-layout="entry-top-directory"]').scrollLeft = 0;
    window.scrollTo({top:0, behavior:'instant'});
  })()`);
  await captureArtifact(client, 'entries');
}

async function checkSettings(client) {
  await navigate(client, `${base}/settings`);
  await selectorExists(client, '[data-page="settings"] [data-layout="theme-lab"]');
  const common = await commonPageEvidence(client);
  assertThat('settings page has no horizontal overflow', common.scrollWidth <= common.clientWidth + 1, common);
  assertThat('settings page has no framework overlay', common.overlay === null, common);

  const verbs = await evaluate(
    client,
    `(() => {
      const buttons = [...document.querySelectorAll(
        '[data-action="apply-custom-css"], [data-action="apply-selected-theme"], [data-action="apply-theme-preset"]'
      )].filter((button) => button.getClientRects().length > 0);
      const texts = buttons.map((button) => button.textContent.trim());
      return {
        texts,
        invalid: texts.filter((text) => !text.startsWith('应用') && text !== '当前已选'),
        oldText: document.body.innerText.includes('载入所选主题') || document.body.innerText.includes('使用这套主题'),
      };
    })()`,
  );
  assertThat('theme application actions use application semantics', verbs.texts.length > 0 && verbs.invalid.length === 0, verbs);
  assertThat('obsolete theme verbs are absent', !verbs.oldText, verbs);
  await captureArtifact(client, 'settings');
}

async function checkFeeds(client) {
  await navigate(client, `${base}/feeds`);
  await selectorExists(client, '[data-page="feeds"] [data-slot="feed-card-title"]');
  const evidence = await evaluate(
    client,
    `(() => {
      const root = document.documentElement;
      const rootRect = root.getBoundingClientRect();
      const titles = [...document.querySelectorAll('[data-slot="feed-card-title"]')]
        .filter((element) => element.getClientRects().length > 0)
        .map((element) => {
          const rect = element.getBoundingClientRect();
          return { text: element.textContent, left: rect.left, right: rect.right };
        });
      return {
        rootClientWidth: root.clientWidth,
        rootScrollWidth: root.scrollWidth,
        titleCount: titles.length,
        titleBoundsOk: titles.length > 0 && titles.every((title) => title.left >= rootRect.left - 1 && title.right <= rootRect.right + 1),
        stats: [...document.querySelectorAll('[data-layout="stats-grid"] [data-layout="stat-card"]')]
          .map(card => {const r=card.getBoundingClientRect();return {top:r.top, left:r.left, right:r.right};}),
        inputBottom: document.querySelector('[data-field="feed-url-input"]').getBoundingClientRect().bottom,
        viewportHeight: innerHeight,
      };
    })()`,
  );
  assertThat('feed titles stay inside the viewport', evidence.titleBoundsOk, evidence);
  assertThat('feeds page has no horizontal overflow', evidence.rootScrollWidth <= evidence.rootClientWidth + 1, evidence);
  assertThat('feed statistics share one row and leave the address input in view',
    evidence.stats.length === 2 && Math.abs(evidence.stats[0].top - evidence.stats[1].top) <= 1 &&
    evidence.stats[0].right <= evidence.stats[1].left && evidence.inputBottom <= evidence.viewportHeight, evidence);
  await captureArtifact(client, 'feeds');

  // A live region must exist before an asynchronous result is inserted. Mounting
  // an already-populated banner can look correct while remaining silent to AT.
  const initialFeedback = await evaluate(client, `(() => {
    const region = document.querySelector('[data-page="feeds"] [data-layout="status-banner"][role="status"]');
    window.__feedsFeedbackRegion = region;
    return region && {text:region.textContent.trim(), live:region.getAttribute('aria-live'), atomic:region.getAttribute('aria-atomic'), display:getComputedStyle(region).display};
  })()`);
  assertThat('Feeds mounts an empty polite live region before an operation',
    initialFeedback && initialFeedback.text === '' && initialFeedback.live === 'polite' &&
    initialFeedback.atomic === 'true' && initialFeedback.display !== 'none', initialFeedback);
  await clickSelector(client, '[data-action="export-config"]');
  await waitFor(client, `document.querySelector('[data-page="feeds"] [data-layout="status-banner"][role="status"]')?.textContent.includes('已导出配置包 JSON')`);
  assertThat('asynchronous feedback updates the existing live region without moving focus',
    await evaluate(client, `document.querySelector('[data-page="feeds"] [data-layout="status-banner"][role="status"]') === window.__feedsFeedbackRegion && document.activeElement !== window.__feedsFeedbackRegion`));
}

async function checkReader(client) {
  await navigate(client, `${base}/entries/2`);
  await selectorExists(client, '[data-page="reader"][data-state="loaded"] [data-slot="reader-title"]');
  const evidence = await evaluate(
    client,
    `(() => {
      const root = document.documentElement;
      const rootRect = root.getBoundingClientRect();
      const title = document.querySelector('[data-slot="reader-title"]');
      const titleRect = title.getBoundingClientRect();
      const buttons = [...document.querySelectorAll('[data-layout="reader-bottom-bar"] button')]
        .filter((button) => button.getClientRects().length > 0)
        .map((button) => {
          const rect = button.getBoundingClientRect();
          return { text: button.textContent.trim(), width: rect.width, height: rect.height,
            left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom };
        });
      const shortcuts = [...document.querySelectorAll('[data-slot="reader-bottom-bar-shortcut"]')]
        .map((element) => getComputedStyle(element).display);
      const bar = document.querySelector('[data-layout="reader-bottom-bar"]');
      const barStyle = getComputedStyle(bar);
      scrollTo(0, document.documentElement.scrollHeight);
      const barRect = bar.getBoundingClientRect();
      const lastControl = [...document.querySelectorAll('[data-layout="reader-page"] button, [data-layout="reader-page"] a')]
        .filter((element) => element.getClientRects().length > 0)
        .map((element) => element.getBoundingClientRect().bottom)
        .reduce((lowest, bottom) => Math.max(lowest, bottom), 0);
      scrollTo(0, 0);
      return {
        rootClientWidth: root.clientWidth,
        rootScrollWidth: root.scrollWidth,
        titleBoundsOk: titleRect.left >= rootRect.left - 1 && titleRect.right <= rootRect.right + 1,
        buttons,
        shortcuts,
        barPosition: barStyle.position,
        barOutsidePage: !bar.closest('[data-layout="reader-page"]'),
        barGap: innerHeight - barRect.bottom,
        barTop: barRect.top,
        lastControl,
      };
    })()`,
  );
  assertThat('reader bottom bar is fixed beside the page, not inside it',
    evidence.barPosition === 'fixed' && evidence.barOutsidePage, evidence);
  assertThat('reader bottom bar sits just above the viewport bottom whatever the article length',
    evidence.barGap >= 8 && evidence.barGap <= 24, evidence);
  assertThat('the page keeps room below its last control for the bar',
    evidence.lastControl <= evidence.barTop, evidence);
  assertThat('reader title stays inside the viewport', evidence.titleBoundsOk, evidence);
  assertThat('reader page has no horizontal overflow', evidence.rootScrollWidth <= evidence.rootClientWidth + 1, evidence);
  assertThat(
    'reader bottom actions keep touch targets',
    evidence.buttons.length === 4 && evidence.buttons.every((button) => meetsTouchTarget(button.width, button.height)),
    evidence.buttons,
  );
  assertThat('reader bottom action hit areas do not overlap',
    evidence.buttons.every((button, index) => evidence.buttons.slice(index + 1).every(other =>
      button.right <= other.left || other.right <= button.left || button.bottom <= other.top || other.bottom <= button.top)),
    evidence.buttons);
  assertThat(
    'keyboard shortcuts stay hidden for touch emulation',
    evidence.shortcuts.length > 0 && evidence.shortcuts.every((display) => display === 'none'),
    evidence.shortcuts,
  );
  await captureArtifact(client, 'reader');
  const readerSemantics = await evaluate(client, `(() => {
    const region = document.querySelector('[data-layout="reader-shortcut-scope"]');
    const star = document.querySelector('[data-action="toggle-starred"]');
    return {role:region.getAttribute('role'), name:region.getAttribute('aria-label'),
      help:document.getElementById(region.getAttribute('aria-describedby'))?.textContent,
      pressed:star.getAttribute('aria-pressed'), state:star.dataset.state,
      icons:[...document.querySelectorAll('[data-slot="reader-bottom-bar-icon"]')].map(e=>e.getAttribute('aria-hidden'))};
  })()`);
  assertThat('Reader exposes a named shortcut region and the favorite state without decorative glyphs',
    readerSemantics.role === 'region' && readerSemantics.name && readerSemantics.help &&
    readerSemantics.pressed === String(readerSemantics.state === 'starred') &&
    readerSemantics.icons.length === 4 && readerSemantics.icons.every(value=>value === 'true'), readerSemantics);
}

async function checkEntriesInitialization(client) {
  await seedAndNavigate(client, 'mobile-ui-short', '/entries', '[data-page="entries"][data-position-ready="true"]');
  // The page's first read (preferences and subscriptions) held at the server: the page waits and
  // does not claim the library is empty (formerly a Web Lock held by another tab).
  const held = [];
  const detach = client.on('Fetch.requestPaused', ({ requestId }) => held.push(requestId));
  await client.send('Fetch.enable', { patterns: [{ urlPattern: '*/api/v1/entries/bootstrap*', requestStage: 'Request' }] });
  try {
    await navigate(client, `${base}/entries`);
    await selectorExists(client, '[data-page="entries"]');
    await waitUntil('the held first read', 10000, async () => held.length > 0);
    const pending = await evaluate(client, `({
      ready:document.querySelector('[data-page="entries"]').dataset.positionReady,
      message:document.querySelector('[data-layout="entries-main"] > [role="status"]').textContent,
      empty:!!document.querySelector('[data-layout="entries-page-state"]')
    })`);
    assertThat('pending entries do not claim the library is empty',
      pending.ready === 'false' && pending.message.includes('正在加载') && !pending.empty, pending);
    for (const requestId of held.splice(0)) await client.send('Fetch.continueRequest', { requestId });
  } finally {
    await client.send('Fetch.disable');
    detach();
  }
  await selectorExists(client, '[data-slot="entry-card-title"]');
  assertThat('entries recover after the first read answers', await evaluate(client,
    `document.querySelector('[data-page="entries"]').dataset.positionReady === 'true'`));

  // The first read fails (a network failure): the failure shows, the list still loads, and no
  // empty-library claim is made (formerly a failing browser storage).
  await withFailedRequests(client, '*/api/v1/entries/bootstrap*', '/api/v1/entries/bootstrap', null, async () => {
    await navigate(client, `${base}/entries`);
    await selectorExists(client, '[data-layout="entries-main"] > [role="status"][data-state="error"]');
    await selectorExists(client, '[data-slot="entry-card-title"]');
    const failed = await evaluate(client, `({
      message:document.querySelector('[data-layout="entries-main"] > [role="status"]').textContent,
      empty:!!document.querySelector('[data-layout="entries-page-state"]')
    })`);
    assertThat('failed initial reads expose the error without an empty-library claim',
      failed.message.includes('加载文章页初始状态失败') && !failed.empty, failed);
  });
  await navigate(client, `${base}/entries`);
  await selectorExists(client, '[data-slot="entry-card-title"]');
  assertThat('reopening after a read failure preserves existing entries', await evaluate(client,
    `document.querySelector('[data-page="entries"]').dataset.positionReady === 'true' &&
     !document.querySelector('[data-layout="entries-main"] > [role="status"][data-state="error"]')`));
}

async function checkPrimaryWorkflow(client) {
  const tap = async selector => {
    await evaluate(client, `document.querySelector(${JSON.stringify(selector)}).scrollIntoView({block:'center', behavior:'instant'})`);
    await tapSelector(client, selector);
  };
  // The server fetches the subscriptions itself, from the fixture server; nothing reaches the
  // network and the page has nothing to intercept.
  {
    await seedAndNavigate(client, 'reader-demo', '/entries/1', '[data-page="reader"][data-position-ready="true"]');
    // A key typed into a field is the field's, not a shortcut (the nav search stays collapsed
    // here, so a field is placed in the reading region for the check).
    await evaluate(client, `(() => {
      const field = document.createElement('input');
      document.querySelector('[data-layout="reader-body"]').append(field);
      field.dispatchEvent(new KeyboardEvent('keydown', {key: 'k', bubbles: true, cancelable: true}));
      field.remove();
    })()`);
    await sleep(100);
    assertThat('a key typed into a field does not switch entries',
      await evaluate(client, `location.pathname === '/entries/1'`));
    // Shortcuts belong to the whole page: nothing needs focus first.
    await evaluate(client, `document.activeElement?.blur()`);
    await key(client, 'k', 'KeyK');
    await waitFor(client, `location.pathname === '/entries/2' && document.querySelector('[data-page="reader"][data-position-ready="true"]')`);
    assertThat('unread keyboard navigation reaches the actual unread entry',
      await evaluate(client, `document.querySelector('[data-action="mark-read"]').dataset.state === 'unread'`));
    await tap('[data-action="mark-read"]');
    await waitFor(client, `document.querySelector('[data-action="mark-read"]').dataset.state === 'read'`);
    assertThat('finishing all unread entries disables both unread directions', await evaluate(client,
      `['previous','next'].every(direction => document.querySelector('[data-nav="' + direction + '-unread-entry"]').disabled)`));
    await evaluate(client, `document.activeElement?.blur()`);
    await key(client, 'ArrowLeft', 'ArrowLeft');
    await key(client, 'ArrowRight', 'ArrowRight');
    await key(client, 'j', 'KeyJ');
    await sleep(100);
    assertThat('unread shortcuts do not fall back to already read feed entries', await evaluate(client, `location.pathname === '/entries/2'`));
    await tap('[data-nav="next-feed-entry"]');
    await waitFor(client, `location.pathname === '/entries/1' && document.querySelector('[data-page="reader"][data-position-ready="true"]')`);
    assertThat('separate feed navigation can still open a read entry',
      await evaluate(client, `document.querySelector('[data-action="mark-read"]').dataset.state === 'read'`));
  }

  await seedAndNavigate(client, 'mobile-ui-short', '/entries', '[data-page="entries"][data-position-ready="true"]');
  await ensureEntryControlsOpen(client);
  await evaluate(client, `const select = document.querySelector('[data-field="entry-grouping-mode"]'); select.value='source'; select.dispatchEvent(new Event('change', {bubbles:true}))`);
  await tap('[data-field="search-title"]');
  await client.send('Input.insertText', {text:'no matching title'});
  await tap('[data-field="read-filter-read"]');
  await tap('[data-field="starred-filter-starred"]');
  await tap('[data-field="entry-source-filter"]');
  await tap('[data-action="hide-entry-controls"]');
  await waitFor(client, `document.querySelector('[data-layout="entries-page-state"][data-state="empty"]')?.textContent.includes('当前筛选没有匹配')`);
  const filtered = await evaluate(client, `(() => {
    const summary = document.querySelector('[data-slot="entry-filter-summary-text"]');
    const clear = document.querySelector('[data-action="clear-entry-filters"]').getBoundingClientRect();
    return {summary:summary.textContent, hidden:!!document.querySelector('[data-action="show-entry-controls"]'),
      clearWidth:clear.width,clearHeight:clear.height,overflow:document.documentElement.scrollWidth>innerWidth};
  })()`);
  assertThat('collapsed filters expose all active conditions and a usable reset',
    filtered.hidden && ['no matching title','仅已读','仅收藏','来源：'].every(part=>filtered.summary.includes(part)) &&
    meetsTouchTarget(filtered.clearWidth,filtered.clearHeight) && !filtered.overflow, filtered);
  await captureArtifact(client, 'entries-filter-summary');
  await tap('[data-action="clear-entry-filters"]');
  await selectorExists(client, '[data-slot="entry-card-title"]');
  await waitUntil('the reset preferences stored by the server', 10000, async () => {
    const state = await storedWorkspace();
    return state.read_filter === 'all' && state.starred_filter === 'all' &&
      state.selected_feed_urls.length === 0 && state.grouping_mode === 'source';
  });
  await navigate(client, `${base}/entries`);
  await selectorExists(client, '[data-slot="entry-card-title"]');
  await ensureEntryControlsOpen(client);
  assertThat('clear filters persists the reset while preserving grouping and archive display', await evaluate(client, `
    !document.querySelector('[data-layout="entry-filter-summary"]') &&
    document.querySelector('[data-field="search-title"]').value === '' &&
    !document.querySelector('[data-field="read-filter-read"]').checked &&
    !document.querySelector('[data-field="starred-filter-starred"]').checked &&
    !document.querySelector('[data-field="entry-source-filter"]').checked &&
    document.querySelector('[data-field="entry-grouping-mode"]').value === 'source' &&
    document.querySelector('[data-field="show-archived"]').checked &&
    document.querySelector('[data-page="entries"]').dataset.positionPage === '1'`));

  await seedAndNavigate(client, 'home-reader', '/entries/2', '[data-action="open-reader-image"]');
  await beginManualScroll(client);
  await evaluate(client, `(async () => {
    await Promise.all([...document.querySelectorAll('[data-layout="reader-body"] img')].map(image=>image.decode()));
    window.__actionBody = document.querySelector('[data-layout="reader-body"]');
    window.__actionAnchor = window.__actionBody.querySelectorAll('p')[15];
    window.__actionAnchor.scrollIntoView({block:'center',behavior:'instant'});
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
    window.__actionPosition = window.__actionAnchor.getBoundingClientRect().top;
    window.__actionScroll = scrollY;
    window.__actionStatus = document.querySelector('[data-page="reader"] [data-layout="status-banner"][role="status"]');
  })()`);
  for (const action of ['toggle-starred','mark-read']) {
    const selector = `[data-action="${action}"]`;
    const before = await evaluate(client, `document.querySelector(${JSON.stringify(selector)}).dataset.state`);
    await tapSelector(client, selector);
    await waitFor(client, `document.querySelector(${JSON.stringify(selector)}).dataset.state !== ${JSON.stringify(before)}`);
    const result = await evaluate(client, `({sameBody:document.querySelector('[data-layout="reader-body"]')===window.__actionBody,
      before:window.__actionPosition,after:window.__actionAnchor.getBoundingClientRect().top,
      beforeScroll:window.__actionScroll,afterScroll:scrollY,
      sameStatus:document.querySelector('[data-page="reader"] [data-layout="status-banner"][role="status"]')===window.__actionStatus,
      message:window.__actionStatus.textContent,hidden:window.__actionStatus.classList.contains('sr-only'),
      polite:window.__actionStatus.getAttribute('aria-live')==='polite'})`);
    assertThat(`${action} success preserves the visible text and announces without a layout banner`,
      result.sameBody && result.sameStatus && Math.abs(result.after-result.before)<=1 &&
      Math.abs(result.afterScroll-result.beforeScroll)<=1 && result.message && result.hidden && result.polite, result);
  }
  await withFailedRequests(client, '*/api/v1/entries/2/read*', '/api/v1/entries/2/read',
    { status: 503, error: { code: 'unavailable', message: '服务器繁忙（测试）' } }, async () => {
      await tapSelector(client, '[data-action="mark-read"]');
      await selectorExists(client, '[data-page="reader"] [data-layout="status-banner"][data-state="error"]');
      assertThat('reader write errors remain visible and leave the button state unchanged', await evaluate(client, `
        !window.__actionStatus.classList.contains('sr-only') && window.__actionStatus.textContent.trim().length>0 &&
        window.__actionStatus.getBoundingClientRect().height>1 &&
        document.querySelector('[data-action="mark-read"]').dataset.state==='read'`));
    });
}

async function checkShortDirectory(client) {
  await seedAndNavigate(
    client,
    'mobile-ui-short',
    '/entries',
    '[data-page="entries"]',
  );
  await ensureEntryControlsOpen(client);
  const evidence = await evaluate(
    client,
    `(async () => {
      await Promise.race([
        new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))),
        new Promise((resolve) => setTimeout(resolve, 1000)),
      ]);
      const directory = document.querySelector('[data-layout="entry-top-directory"]');
      const style = getComputedStyle(directory);
      return {
        chips: directory.querySelectorAll('[data-layout="entry-top-directory-chip"]').length,
        scrollWidth: directory.scrollWidth,
        clientWidth: directory.clientWidth,
        maskImage: style.maskImage || style.webkitMaskImage,
      };
    })()`,
  );
  assertThat(
    'short directory has no overflow',
    evidence.chips === 1 && evidence.scrollWidth <= evidence.clientWidth + 1,
    evidence,
  );
  assertThat('short directory has no false overflow hint', evidence.maskImage === 'none', evidence);
  await captureArtifact(client, 'entries-short-directory');
}

async function checkDesktop(client) {
  await setViewport(client, 1280, 800, false, 1);
  await seedAndNavigate(
    client,
    'mobile-ui-overflow',
    '/entries',
    '[data-layout="entry-directory-rail"]',
  );
  await ensureEntryControlsOpen(client);
  const evidence = await evaluate(
    client,
    `(() => {
      const root = document.documentElement;
      const rail = document.querySelector('[data-layout="entry-directory-rail"]');
      const topDirectory = document.querySelector('[data-layout="entry-top-directory"]');
      const chip = document.querySelector('[data-layout="entry-filters-source-chip"]');
      return {
        innerWidth,
        innerHeight,
        rootClientWidth: root.clientWidth,
        rootScrollWidth: root.scrollWidth,
        railDisplay: getComputedStyle(rail).display,
        topDirectoryDisplay: getComputedStyle(topDirectory).display,
        chipHeight: chip.getBoundingClientRect().height,
      };
    })()`,
  );
  assertThat('desktop viewport is exact', evidence.innerWidth === 1280 && evidence.innerHeight === 800, evidence);
  assertThat('desktop directory rail remains visible', evidence.railDisplay !== 'none', evidence);
  assertThat('mobile top directory remains hidden on desktop', evidence.topDirectoryDisplay === 'none', evidence);
  assertThat('desktop source row keeps touch target', meetsTouchTarget(evidence.chipHeight), evidence);
  assertThat('desktop entries has no horizontal overflow', evidence.rootScrollWidth <= evidence.rootClientWidth + 1, evidence);
  await checkDirectoryLabels(client, 'desktop time directory');
  await captureArtifact(client, 'entries-desktop');
  await seedAndNavigate(client, 'home-reader', '/entries', '[data-layout="entry-groups"][data-state="populated"]');
  await ensureEntryControlsOpen(client);
  await evaluate(client, `const select = document.querySelector('[data-field="entry-grouping-mode"]'); select.value = 'source'; select.dispatchEvent(new Event('change', {bubbles:true}));`);
  await waitFor(client, `document.querySelector('[data-layout="entry-directory-rail"] [data-slot="entry-directory-title"]').textContent.includes('LongUnbrokenSourceName')`);
  await checkDirectoryLabels(client, 'desktop source directory');
  await captureArtifact(client, 'entries-desktop-source-directory');
  await setViewport(client, width, height, true, deviceScaleFactor);
  const mobile = await evaluate(client, `(() => {
    const chip = document.querySelector('[data-layout="entry-top-directory-chip"]');
    const title = chip.querySelector('[data-slot="entry-directory-title"]');
    return {text:title.textContent, titleWidth:title.clientWidth, titleScrollWidth:title.scrollWidth,
      rootWidth:document.documentElement.clientWidth, rootScrollWidth:document.documentElement.scrollWidth};
  })()`);
  assertThat('mobile source directory wraps full unbroken names', mobile.text.includes('LongUnbrokenSourceName') && mobile.titleScrollWidth <= mobile.titleWidth + 1 && mobile.rootScrollWidth <= mobile.rootWidth + 1, mobile);
  await captureArtifact(client, 'entries-mobile-source-directory');
}

async function checkDirectoryLabels(client, label) {
  const evidence = await evaluate(client, `(() => {
    const rail = document.querySelector('[data-layout="entry-directory-rail"]');
    const rect = rail.getBoundingClientRect();
    const clipped = [...rail.querySelectorAll('[data-slot="entry-directory-title"], [data-slot="entry-directory-meta"]')]
      .filter(el => el.getClientRects().length > 0).filter(el => {
        const bounds = el.getBoundingClientRect();
        const range = document.createRange(); range.selectNodeContents(el);
        const text = range.getBoundingClientRect();
        const right = rect.left + rail.clientLeft + rail.clientWidth;
        return el.scrollWidth > el.clientWidth + 1 || bounds.left < rect.left - 1 || bounds.right > right + 1 || text.right > right + 1;
      }).map(el => el.textContent);
    return {width:rail.clientWidth, scrollWidth:rail.scrollWidth, clipped};
  })()`);
  assertThat(label + ' names and counts remain readable', evidence.scrollWidth <= evidence.width + 1 && evidence.clipped.length === 0, evidence);
}

async function key(client, key, code, modifiers = 0) {
  const windowsVirtualKeyCode = ({Escape:27, Enter:13, Tab:9, Backspace:8, PageDown:34, ArrowLeft:37, ArrowRight:39})[key] ?? key.toUpperCase().charCodeAt(0);
  await client.send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, modifiers, windowsVirtualKeyCode, ...(key === 'Enter' ? {text:'\r', unmodifiedText:'\r'} : {}) });
  await client.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, modifiers, windowsVirtualKeyCode });
}

// Exercise browser hit testing as well as the Dioxus handler. Do not auto-scroll:
// Home and the persistent paginator must already be reachable in the viewport.
async function tapSelector(client, selector) {
  const point = await evaluate(client, `(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (!element) throw new Error('Missing tap target');
    const rect = element.getBoundingClientRect();
    const x = (Math.max(0, rect.left) + Math.min(innerWidth, rect.right)) / 2;
    const y = (Math.max(0, rect.top) + Math.min(innerHeight, rect.bottom)) / 2;
    if (rect.right <= 0 || rect.left >= innerWidth || rect.bottom <= 0 || rect.top >= innerHeight ||
        !element.contains(document.elementFromPoint(x, y))) throw new Error('Tap target is outside the viewport or obscured: ' + ${JSON.stringify(selector)});
    return {x, y};
  })()`);
  await client.send('Input.dispatchTouchEvent', {type: 'touchStart', touchPoints: [point]});
  await client.send('Input.dispatchTouchEvent', {type: 'touchEnd', touchPoints: []});
}

// Programmatic scrollTo is fixture setup, not user input. Send real wheel input first
// so pending entry-position restoration yields, just as it does when a person scrolls.
async function beginManualScroll(client, page = "reader") {
  await selectorExists(client, `[data-page="${page}"][data-position-ready="true"]`);
  const point = await evaluate(client, '({x:innerWidth / 2, y:innerHeight / 2})');
  await client.send('Input.dispatchMouseEvent', {
    type: 'mouseWheel', ...point, deltaX: 0, deltaY: 1,
  });
  await evaluate(client, 'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
}

async function shellEvidence(client, reader = false) {
  const result = await evaluate(client, `(() => {
    const nav = document.querySelector('[data-layout="app-nav-shell"]');
    const home = nav.querySelector('[data-action="activate-home"]');
    const back = nav.querySelector('[data-nav="back"]');
    const rect = home.getBoundingClientRect();
    const icons = [...nav.querySelectorAll('button, a')].map(el => {
      const r = el.getBoundingClientRect();
      return {label: el.getAttribute('aria-label'), title: el.title, width: r.width, height: r.height};
    });
    return {visible: rect.top >= 0 && rect.bottom <= innerHeight, icons,
      fits: nav.scrollWidth <= nav.clientWidth + 1,
      independentEntries: nav.querySelectorAll('[data-nav="entries"]').length,
      homeHasNav: home.hasAttribute('data-nav'),
      backFirst: back && back === nav.querySelector('[data-layout="app-nav-topline"]').firstElementChild,
      oldBack: !!document.querySelector('[data-layout="reader-toolbar"]'),
      height: nav.getBoundingClientRect().height, state: nav.dataset.state};
  })()`);
  assertThat('Home remains visible and uses explicit action semantics', result.visible && !result.homeHasNav && result.independentEntries === 0, result);
  assertThat('shell icons fit and have labels titles and touch targets', result.fits && result.icons.every(x => x.label && x.title && meetsTouchTarget(x.width, x.height)), result);
  if (reader) assertThat('reader back is first in shell and old toolbar is absent', result.backFirst && !result.oldBack, result);
  return result;
}

async function searchInputEvidence(client) {
  const result = await evaluate(client, `(() => {
    const input = document.querySelector('[data-field="entry-search"]');
    if (!input) return null;
    const rect = input.getBoundingClientRect();
    const style = getComputedStyle(input);
    const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
    return {width:rect.width, height:rect.height, visibility:style.visibility,
      display:style.display, hit:hit === input, focused:document.activeElement === input};
  })()`);
  assertThat('expanded search field is visible and wide enough to type into',
    result && result.width >= 140 && result.height >= 40 && result.visibility === 'visible' &&
    result.display !== 'none' && result.hit && result.focused, result);
  return result;
}

async function checkReaderRefreshFeedback(client, label) {
  const evidence = await evaluate(client, `(async () => {
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const status = document.querySelector('[data-slot="manual-refresh-status"]');
    const home = document.querySelector('[data-action="activate-home"]');
    const r = status.getBoundingClientRect(), button = home.getBoundingClientRect();
    const style = getComputedStyle(status), badge = getComputedStyle(home, '::after');
    // A clipped one-pixel live region paints no text over the reading surface.
    // Otherwise verify its actual box against the title and body geometry.
    const clipped = r.width <= 1.01 && r.height <= 1.01 &&
      (style.overflow === 'hidden' || style.clipPath !== 'none');
    const visible = style.display !== 'none' && style.visibility !== 'hidden' && r.width > 0 && r.height > 0;
    const overlaps = ['reader-title', 'reader-body'].filter(slot => {
      const content = document.querySelector('[data-slot="' + slot + '"], [data-layout="' + slot + '"]');
      if (!content) throw new Error('Missing Reader content: ' + slot);
      const box = content.getBoundingClientRect();
      return visible && !clipped && r.left < box.right && r.right > box.left && r.top < box.bottom && r.bottom > box.top;
    });
    return {phase:home.dataset.refreshState, message:status.textContent.trim(), title:home.title,
      role:status.getAttribute('role'), ariaLive:status.getAttribute('aria-live'),
      display:style.display, visibility:style.visibility, clipped, overlaps,
      buttonWidth:button.width, buttonHeight:button.height,
      badge:{content:badge.content, display:badge.display, visibility:badge.visibility,
        width:parseFloat(badge.width), height:parseFloat(badge.height)}};
  })()`);
  assertThat(`${label}: refresh feedback does not cover Reader title or body`, evidence.overlaps.length === 0, evidence);
  assertThat(`${label}: Home retains its touch target`, meetsTouchTarget(evidence.buttonWidth, evidence.buttonHeight), evidence);
  assertThat(`${label}: active refresh phases retain a message`, evidence.phase === 'idle' || evidence.message.length > 0, evidence);
  if (evidence.message) {
    assertThat(`${label}: live region and Home title retain the full refresh result`,
      evidence.role === 'status' && evidence.ariaLive === 'polite' && evidence.display !== 'none' &&
      evidence.visibility !== 'hidden' && evidence.title.includes(evidence.message), evidence);
    if (['finished', 'error'].includes(evidence.phase)) {
      assertThat(`${label}: completed refresh has a visible Home badge`,
        !['none', 'normal', '""', "''", ''].includes(evidence.badge.content) &&
        evidence.badge.display !== 'none' && evidence.badge.visibility !== 'hidden' &&
        evidence.badge.width > 0 && evidence.badge.height > 0, evidence);
    }
  }
}

async function touchPull(client, fromY, toY) {
  const x = Math.round(width / 2);
  await client.send('Input.dispatchTouchEvent', {type: 'touchStart', touchPoints: [{x, y: fromY}]});
  for (let step = 1; step <= 8; step++) {
    await client.send('Input.dispatchTouchEvent', {type: 'touchMove', touchPoints: [{x, y: fromY + (toY - fromY) * step / 8}]});
    await sleep(35);
  }
  const phase = await evaluate(client, `document.querySelector('[data-slot="pull-refresh"]')?.dataset.state`);
  await client.send('Input.dispatchTouchEvent', {type: 'touchEnd', touchPoints: []});
  await sleep(150);
  return phase;
}

// The server refreshes (the page only starts and waits for the job): the feed fixture server sees
// its fetches, two subscriptions fetched concurrently per refresh. Each manual refresh that should
// bring new entries arms one round first (one new entry per subscription). The server's own
// periodic refresh runs at its start, before the page signs in, so the page has no start-up
// refresh to join; joining is the server's refresh flight (its own tests).
async function checkHomeRefreshAndGestures(client) {
  const feeds = harness.feeds;
  const subscriptions = ['/home-reader/1.xml', '/home-reader/2.xml'];
  const counts = () => subscriptions.map(feed => feeds.count(feed));
  const requestCount = () => feeds.count();
  const holdFetches = () => feeds.hold(pathname => pathname.startsWith('/home-reader/'));
  const bothHeld = held => waitUntil('both subscriptions fetched', 15000, async () => held.waiting() === 2);
  let held;
  try {
    await evaluate(client, `localStorage.removeItem('rssr-entry-search'); localStorage.setItem('rssr-entry-controls-hidden', '1')`);
    await seedAndNavigate(client, 'home-reader', '/entries/2', '[data-slot="reader-body-html"]');
    feeds.nextRound('home-reader');
    held = holdFetches();
    await tapSelector(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-page="entries"]');
    assertThat('Reader to Home only navigates before the refresh', requestCount() === 0, {requests: requestCount()});
    await tapSelector(client, '[data-action="activate-home"]');
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'refreshing'`);
    await bothHeld(held);
    held.release();
    held = undefined;
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
    assertThat('successful manual refresh shows a completion message', await evaluate(client,
      `/新增 [0-9]+ 篇文章|没有新文章/.test(document.querySelector('[data-slot="manual-refresh-status"]').textContent)`));
    await sleep(1150);
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'idle'`);
    assertThat('successful manual refresh feedback clears instead of returning on navigation', await evaluate(client,
      `document.querySelector('[data-slot="manual-refresh-status"]').textContent === '' && !/新增 [0-9]+ 篇文章|没有新文章/.test(document.querySelector('[data-action="activate-home"]').title)`));
    assertThat('a manual refresh fetches every subscription once',
      requestCount() === 2 && counts().every(count => count === 1), {counts: counts()});
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-page="reader"]');
    assertThat('finished refresh does not reappear when Reader mounts', await evaluate(client,
      `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'idle' && document.querySelector('[data-slot="manual-refresh-status"]').textContent === ''`));
    const normalShell = await shellEvidence(client, true);
    const initialRequests = requestCount();
    await clickSelector(client, '[data-action="toggle-search"]');
    await selectorExists(client, '[data-field="entry-search"]');
    await searchInputEvidence(client);
    const searchShell = await shellEvidence(client, true);
    const animation = await evaluate(client, `getComputedStyle(document.querySelector('[data-layout="app-nav-search"]')).animationName`);
    assertThat('search reveal animation has a valid computed declaration', animation === 'search-reveal', {animation});
    const wraps = await evaluate(client, `getComputedStyle(document.querySelector('[data-layout="app-nav-topline"]')).flexWrap === 'wrap'`);
    const heightChange = searchShell.height - normalShell.height;
    assertThat('search adds at most one row in narrow navigation and preserves wide shell height',
      heightChange >= -1 && heightChange <= (wraps ? 51 : 1), {normalShell, searchShell, wraps});
    assertThat('search replaces subscription/settings while retaining back and Home', await evaluate(client, `!document.querySelector('[data-nav="feeds"]') && !document.querySelector('[data-nav="settings"]') && document.activeElement.matches('[data-field="entry-search"]')`));
    await captureArtifact(client, 'reader-search');
    await evaluate(client, `document.querySelector('[data-field="entry-search"]').dispatchEvent(new KeyboardEvent('keydown', {key:'Escape', code:'Escape', bubbles:true, isComposing:true}))`);
    assertThat('Escape during IME composition leaves search open', await selectorExistsOptional(client, '[data-field="entry-search"]'));
    await key(client, 'Escape', 'Escape');
    await selectorExists(client, '[data-nav="feeds"]');
    await clickSelector(client, '[data-action="toggle-search"]');
    await clickSelector(client, '[data-action="toggle-search"]');
    assertThat('search toggle restores navigation', !(await selectorExistsOptional(client, '[data-field="entry-search"]')));

    await clickSelector(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-page="entries"][data-entry-scope="all"]');
    assertThat('Reader to Home only navigates', requestCount() === initialRequests, {initialRequests, requests: requestCount()});
    assertThat('finished refresh does not reappear on Home', await evaluate(client,
      `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'idle' && document.querySelector('[data-slot="manual-refresh-status"]').textContent === ''`));
    await shellEvidence(client);
    await selectorExists(client, '[data-layout="entry-pagination"]');
    const pagination = await evaluate(client, `(() => {
      window.scrollTo(0, Math.min(500, document.documentElement.scrollHeight - innerHeight));
      const p = document.querySelector('[data-layout="entry-pagination"]');
      const r = p.getBoundingClientRect();
      return {count: document.querySelectorAll('[data-layout="entry-pagination"]').length,
        top: r.top, bottom: r.bottom, height: innerHeight,
        previousDisabled: p.querySelector('[data-action="entry-page-previous"]').disabled};
    })()`);
    assertThat('single pagination remains in viewport midway through list', pagination.count === 1 && pagination.top >= 0 && pagination.bottom <= pagination.height && pagination.previousDisabled, pagination);
    await captureArtifact(client, 'entries-persistent-pagination');
    await tapSelector(client, '[data-action="entry-page-next"]');
    await waitFor(client, `document.querySelector('[data-slot="entry-pagination-status"]').textContent.trim().startsWith('2 /') && scrollY < 2`);
    await tapSelector(client, '[data-action="entry-page-previous"]');

    for (const destination of ['feeds', 'settings']) {
      await clickSelector(client, `[data-nav="${destination}"]`);
      await selectorExists(client, `[data-page="${destination}"]`);
      await shellEvidence(client);
      if (destination === 'feeds') {
        await clickSelector(client, '[data-nav="feed-entries"]');
        await selectorExists(client, '[data-entry-scope="feed"]');
        assertThat('feed-specific list has no pull gesture', !(await selectorExistsOptional(client, '[data-slot="pull-refresh"]')));
      }
      await clickSelector(client, '[data-action="activate-home"]');
      await selectorExists(client, '[data-page="entries"][data-entry-scope="all"]');
      assertThat(`${destination} to Home only navigates`, requestCount() === initialRequests, {initialRequests, requests: requestCount()});
    }
    // Exercise the actual toolbar back, not only its geometry.
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-page="reader"]');
    await clickSelector(client, '[data-nav="back"]');
    await selectorExists(client, '[data-page="entries"]');

    const beforeRefresh = requestCount();
    feeds.nextRound('home-reader');
    held = holdFetches();
    await clickSelector(client, '[data-action="activate-home"]');
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'refreshing'`);
    await bothHeld(held);
    await evaluate(client, `for (let i = 0; i < 5; i++) document.querySelector('[data-action="activate-home"]').click()`);
    await sleep(150);
    assertThat('repeated Home activation starts one batch', requestCount() === beforeRefresh + 2 && held.waiting() === 2, {beforeRefresh, requests: requestCount(), held: held.waiting()});
    await clickSelector(client, '[data-nav="feeds"]');
    await selectorExists(client, '[data-action="refresh-all"]');
    const refreshBusy = await evaluate(client, `(() => {
      const button = document.querySelector('[data-action="refresh-all"]');
      return {disabled:button.disabled, busy:button.getAttribute('aria-busy'), text:button.textContent.trim(), opacity:getComputedStyle(button).opacity};
    })()`);
    assertThat('Feeds reflects a refresh started on Home with disabled busy feedback',
      refreshBusy.disabled && refreshBusy.busy === 'true' && refreshBusy.text === '正在刷新…' && Number(refreshBusy.opacity) < 1, refreshBusy);
    await clickSelector(client, '[data-action="refresh-all"]');
    assertThat('Feeds refresh uses the same in-flight gate', requestCount() === beforeRefresh + 2, {requests: requestCount()});
    await clickSelector(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-slot="entry-card-title"]');
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-layout="reader-body"]');
    await beginManualScroll(client);
    await evaluate(client, `window.__readerBeforeRefresh = document.querySelector('[data-layout="reader-body"]'); window.__readerTextBeforeRefresh = window.__readerBeforeRefresh.innerHTML; window.scrollTo(0, 80); window.__readerScroll = scrollY`);
    await checkReaderRefreshFeedback(client, 'refreshing while reading');
    held.release();
    held = undefined;
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
    assertThat('manual batch survives page unmount and completes every subscription',
      requestCount() === beforeRefresh + 2 && counts().every(count => count === 2), {beforeRefresh, counts: counts()});
    assertThat('refresh completion preserves Reader DOM and scroll position', await evaluate(client, `document.querySelector('[data-layout="reader-body"]') === window.__readerBeforeRefresh && window.__readerBeforeRefresh.innerHTML === window.__readerTextBeforeRefresh && Math.abs(scrollY - window.__readerScroll) <= 1`));
    await checkReaderRefreshFeedback(client, 'refresh finished while reading');
    await clickSelector(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-slot="entry-card-title"]');
    await ensureEntryControlsOpen(client);
    assertThat('completed refresh becomes visible in the article list', await evaluate(client, `document.querySelector('[data-slot="entry-pagination-status"]').textContent.includes('/ 8')`));
    const source = await evaluate(client, `(() => {
      const row = document.querySelector('[data-layout="entry-filters-source-chip"] span');
      row.scrollIntoView({block:'center'});
      return {text: row.textContent, width: row.clientWidth, scrollWidth: row.scrollWidth, whiteSpace: getComputedStyle(row).whiteSpace,
        rootWidth: document.documentElement.clientWidth, rootScrollWidth: document.documentElement.scrollWidth};
    })()`);
    assertThat('extreme unbroken source remains fully wrapped', source.text.includes('LongUnbrokenSourceName') && source.whiteSpace !== 'nowrap' && source.scrollWidth <= source.width + 1 && source.rootScrollWidth <= source.rootWidth + 1, source);
    await captureArtifact(client, 'sources-full-names');
    const sources = '[data-field="entry-source-filter"]';
    await evaluate(client, `const boxes = [...document.querySelectorAll('${sources}')]; boxes[0].click(); boxes[1].click()`);
    await waitFor(client, `[...document.querySelectorAll('${sources}')].every(x => x.checked)`);
    await evaluate(client, `[...document.querySelectorAll('${sources}')].forEach(x => x.click())`);
    await waitFor(client, `[...document.querySelectorAll('${sources}')].every(x => !x.checked)`);
    await clickSelector(client, '[data-action="hide-entry-controls"]');

    // Real CDP touch events, not synthetic reducer calls. Start on the page header.
    await evaluate(client, 'window.scrollTo(0, 0)');
    const beforePull = requestCount();
    assertThat('short pull stays below threshold', (await touchPull(client, 145, 190)) !== 'armed');
    assertThat('short pull does not refresh', requestCount() === beforePull, {beforePull, requests: requestCount()});
    await evaluate(client, 'window.scrollTo(0, 350)');
    await touchPull(client, 145, 260);
    assertThat('pull starting away from top does not refresh', requestCount() === beforePull, {requests: requestCount()});
    await evaluate(client, 'window.scrollTo(0, 0)');
    held = holdFetches();
    const phase = await touchPull(client, 145, 275);
    assertThat('top pull arms at threshold', phase === 'armed', {phase});
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'refreshing'`);
    await bothHeld(held);
    await clickSelector(client, '[data-action="activate-home"]');
    await sleep(150);
    assertThat('pull and Home share in-flight refresh', requestCount() === beforePull + 2, {beforePull, requests: requestCount()});
    held.release();
    held = undefined;
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);

    held = holdFetches();
    await clickSelector(client, '[data-action="activate-home"]');
    await bothHeld(held);
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-layout="reader-body"]');
    await beginManualScroll(client);
    await evaluate(client, `window.scrollTo(0, 80); window.__errorReaderScroll = scrollY; window.__errorReaderBody = document.querySelector('[data-layout="reader-body"]'); window.__errorReaderReady = document.querySelector('[data-page="reader"]').dataset.positionReady`);
    feeds.failNext('/home-reader/1.xml');
    held.release();
    held = undefined;
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'error'`);
    assertThat('refresh failure is exposed and releases the gate', await evaluate(client, `document.querySelector('[data-slot="manual-refresh-status"]').textContent.includes('失败')`));
    const errorReaderPosition = await evaluate(client, `({sameBody: document.querySelector('[data-layout="reader-body"]') === window.__errorReaderBody,
      before: window.__errorReaderScroll, after: scrollY, readyBefore: window.__errorReaderReady,
      readyAfter: document.querySelector('[data-page="reader"]').dataset.positionReady})`);
    assertThat('refresh failure also preserves Reader position', errorReaderPosition.sameBody && Math.abs(errorReaderPosition.after - errorReaderPosition.before) <= 1, errorReaderPosition);
    await checkReaderRefreshFeedback(client, 'refresh failed while reading');
    await clickSelector(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-page="entries"]');
    await clickSelector(client, '[data-action="activate-home"]');
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
  } finally {
    held?.release();
  }
}

async function selectorExistsOptional(client, selector) {
  return evaluate(client, `!!document.querySelector(${JSON.stringify(selector)})`);
}

async function checkReaderImagesAndSelection(client) {
  await seedAndNavigate(client, 'home-reader', '/entries/2', '[data-action="open-reader-image"]');
  await selectorExists(client, '[data-page="reader"][data-position-ready="true"]');
  await beginManualScroll(client);
  await shellEvidence(client, true);
  assertThat('Reader has no pull gesture surface', !(await selectorExistsOptional(client, '[data-slot="pull-refresh"]')));
  const source = await evaluate(client, `(async () => {
    const image = document.querySelector('[data-action="open-reader-image"]');
    await image.decode();
    image.scrollIntoView({block: 'center', behavior: 'instant'});
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    window.__imageScroll = scrollY;
    return {src: image.currentSrc, inlineHandler: image.hasAttribute('onclick')};
  })()`);
  assertThat('image activation preserves sanitizer policy', source.src.startsWith('data:') && !source.inlineHandler, source);
  await tapSelector(client, '[data-action="open-reader-image"]');
  await waitFor(client, `document.querySelector('[data-layout="reader-image-viewer"]')?.open === true`);
  const modal = await evaluate(client, `({sourceMatches:document.querySelector('[data-layout="reader-image-viewer"] img').src === ${JSON.stringify(source.src)},
    activeAction:document.activeElement.getAttribute('data-action'), position:document.body.style.position})`);
  assertThat('viewer uses rendered image and native modal focus', modal.sourceMatches && modal.activeAction === 'close-reader-image' && modal.position === 'fixed', modal);
  await captureArtifact(client, 'reader-image-viewer');
  await key(client, 'Escape', 'Escape');
  await waitFor(client, `!document.querySelector('[data-layout="reader-image-viewer"]') && document.body.style.position !== 'fixed'`);
  assertThat('closing image restores scroll and focus', await evaluate(client, `Math.abs(scrollY - window.__imageScroll) <= 1 && document.activeElement.matches('[data-action="open-reader-image"]')`));
  await key(client, 'Enter', 'Enter');
  await selectorExists(client, '[data-action="close-reader-image"]');
  await clickSelector(client, '[data-action="close-reader-image"]');
  await waitFor(client, `!document.querySelector('[data-layout="reader-image-viewer"]')`);
  await clickSelector(client, '[data-action="open-reader-image"]');
  await selectorExists(client, '[data-layout="reader-image-viewer"]');
  await clickSelector(client, '[data-layout="reader-image-viewport"]');
  await waitFor(client, `!document.querySelector('[data-layout="reader-image-viewer"]')`);

  for (const alt of ['超大竖图', '超大横图']) {
    await evaluate(client, `document.querySelector('[data-action="open-reader-image"][alt="${alt}"]').scrollIntoView({block:'center'})`);
    const before = await evaluate(client, 'scrollY');
    await clickSelector(client, `[data-action="open-reader-image"][alt="${alt}"]`);
    await waitFor(client, `document.querySelector('[data-layout="reader-image-viewer"]')?.open && document.querySelector('[data-layout="reader-image-viewer"] img').complete`);
    const bounds = await evaluate(client, `(() => {
      const img = document.querySelector('[data-layout="reader-image-viewer"] img');
      const r = img.getBoundingClientRect();
      return {naturalWidth:img.naturalWidth, naturalHeight:img.naturalHeight, left:r.left, top:r.top, right:r.right, bottom:r.bottom, width:innerWidth, height:innerHeight};
    })()`);
    assertThat(alt + ' fits the image viewer', bounds.naturalWidth > 0 && bounds.left >= 0 && bounds.top >= 0 && bounds.right <= bounds.width + 1 && bounds.bottom <= bounds.height + 1, bounds);
    await clickSelector(client, '[data-action="close-reader-image"]');
    await waitFor(client, `!document.querySelector('[data-layout="reader-image-viewer"]') && Math.abs(scrollY - ${before}) <= 1`);
  }

  await checkImageViewerLifecycle(client);

  await setViewport(client, 1280, 800, false, 1);
  const coords = await evaluate(client, `(() => {
    const paragraph = document.querySelector('[data-slot="reader-body-html"] p');
    paragraph.scrollIntoView({block:'center'});
    const range = document.createRange(); range.selectNodeContents(paragraph);
    const rect = range.getClientRects()[0];
    return {x: rect.left + 1, y: rect.top + rect.height / 2, end: Math.min(rect.right - 1, rect.left + 250)};
  })()`);
  await client.send('Input.dispatchMouseEvent', {type:'mousePressed', button:'left', buttons:1, clickCount:1, x:coords.x, y:coords.y});
  await client.send('Input.dispatchMouseEvent', {type:'mouseMoved', button:'left', buttons:1, x:coords.end, y:coords.y});
  await client.send('Input.dispatchMouseEvent', {type:'mouseReleased', button:'left', buttons:0, clickCount:1, x:coords.end, y:coords.y});
  const selected = await evaluate(client, 'getSelection().toString()');
  assertThat('reader body supports native mouse drag selection', selected.length > 5, {selected});
  await client.send('Browser.grantPermissions', {origin: base, permissions:['clipboardReadWrite','clipboardSanitizedWrite']});
  await key(client, 'c', 'KeyC', 2);
  const clipboard = await evaluate(client, 'navigator.clipboard.readText()');
  assertThat('Ctrl+C copies native selection', clipboard === selected, {selected, clipboard});
  const route = await evaluate(client, 'location.pathname');
  await key(client, 'a', 'KeyA', 2);
  const selection = await evaluate(client, `({text:getSelection().toString(), path:location.pathname})`);
  assertThat('Ctrl+A is not swallowed by application shortcuts', selection.text.includes('阅读位置 24') && selection.path === route, {length:selection.text.length, path:selection.path});
  await evaluate(client, 'getSelection().removeAllRanges(); window.scrollTo(0, 0)');
  await captureArtifact(client, 'reader-desktop');
  await setViewport(client, width, height, true, deviceScaleFactor);
  await clickSelector(client, '[data-action="toggle-search"]');
  await client.send('Input.insertText', {text:'移动端'});
  await key(client, 'Enter', 'Enter');
  await selectorExists(client, '[data-page="entries"]');
  assertThat('search submit keeps shell text and enters Home', await evaluate(client, `document.querySelector('[data-field="entry-search"]').value === '移动端' && location.pathname === '/entries'`));
  await evaluate(client, `const input = document.querySelector('[data-field="entry-search"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,''); input.dispatchEvent(new Event('input',{bubbles:true}));`);
  await key(client, 'Escape', 'Escape');
  await clickSelector(client, '[data-nav="feeds"]');
  await selectorExists(client, '[data-page="feeds"]');
  await shellEvidence(client);
  await clickSelector(client, '[data-nav="settings"]');
  await selectorExists(client, '[data-page="settings"]');
  await shellEvidence(client);
}

async function checkImageViewerLifecycle(client) {
  await tapSelector(client, '[data-action="activate-home"]');
  await selectorExists(client, '[data-slot="entry-card-title"]');
  // Auto-refresh inserts newer text-only entries. Find the seeded image article
  // through the real paginator instead of assuming it remains the first card.
  const imageEntry = '[data-slot="entry-card-title"][href="/entries/2"]';
  for (let page = 0; !(await selectorExistsOptional(client, imageEntry)) && page < 20; page++) {
    const previous = await evaluate(client, `document.querySelector('[data-slot="entry-pagination-status"]').textContent`);
    await tapSelector(client, '[data-action="entry-page-next"]');
    await waitFor(client, `document.querySelector('[data-slot="entry-pagination-status"]').textContent !== ${JSON.stringify(previous)}`);
  }
  // A small recorded offset remains valid when the list remounts on page one.
  // This lifecycle check uses the link handler; image/pagination hit testing is
  // covered separately above with real CDP taps.
  await beginManualScroll(client, 'entries');
  await evaluate(client, 'window.scrollTo(0, 120)');
  const homeScroll = await evaluate(client, 'scrollY');
  await clickSelector(client, imageEntry);
  await selectorExists(client, '[data-action="open-reader-image"]');
  await beginManualScroll(client);
  const readerRoute = await evaluate(client, 'location.pathname');
  await evaluate(client, `document.querySelector('[data-action="open-reader-image"]').scrollIntoView({block:'center'})`);
  await tapSelector(client, '[data-action="open-reader-image"]');
  await waitFor(client, `document.querySelector('[data-layout="reader-image-viewer"]')?.open === true`);
  await evaluate(client, 'history.back()');
  await waitFor(client, `location.pathname === '/entries' && document.querySelector('[data-page="entries"]') && document.body.style.position !== 'fixed'`);
  // History restoration waits for the asynchronous list query and its saved page.
  await selectorExists(client, '[data-page="entries"][data-position-ready="true"]');
  await waitFor(client, `Math.abs(scrollY - ${homeScroll}) <= 1`);
  const afterBack = await evaluate(client, `({scroll:scrollY, position:document.body.style.position,
    top:document.body.style.top, viewer:!!document.querySelector('[data-layout="reader-image-viewer"]')})`);
  assertThat('leaving Reader with an open image releases lock and restores Home scroll',
    !afterBack.viewer && afterBack.position !== 'fixed' && afterBack.top === '' && Math.abs(afterBack.scroll - homeScroll) <= 1,
    {homeScroll, afterBack});
  await evaluate(client, 'history.forward()');
  await waitFor(client, `location.pathname === ${JSON.stringify(readerRoute)} && !!document.querySelector('[data-action="open-reader-image"]')`);
  await beginManualScroll(client);
  await evaluate(client, `document.querySelector('[data-action="open-reader-image"]').scrollIntoView({block:'center'})`);
  const readerScroll = await evaluate(client, 'scrollY');
  for (let iteration = 0; iteration < 3; iteration++) {
    await tapSelector(client, '[data-action="open-reader-image"]');
    await waitFor(client, `document.querySelector('[data-layout="reader-image-viewer"]')?.open === true`);
    assertThat(`reopened viewer ${iteration + 1} owns the body lock`, await evaluate(client, `document.body.style.position === 'fixed'`));
    await tapSelector(client, '[data-action="close-reader-image"]');
    await waitFor(client, `!document.querySelector('[data-layout="reader-image-viewer"]')`);
  }
  await waitFor(client, `document.body.style.position !== 'fixed'`);
  assertThat('repeated image open/close restores Reader scroll and focus',
    await evaluate(client, `Math.abs(scrollY - ${readerScroll}) <= 1 && document.activeElement.matches('[data-action="open-reader-image"]')`));
}

async function checkReadingPreferencesAndFeedInput(client) {
  await seedAndNavigate(client, 'home-reader', '/entries/2', '[data-action="open-reader-image"]');
  const typography = await evaluate(client, `(() => {
    const link = document.querySelector('[data-slot="reader-body-html"] a[href="https://example.com/reader-link"]');
    const pre = document.querySelector('[data-slot="reader-body-html"] pre');
    return {linkDecoration: getComputedStyle(link).textDecorationLine,
      preOverflow: getComputedStyle(pre).overflowX, preWidth:pre.clientWidth, preScrollWidth:pre.scrollWidth,
      rootWidth:document.documentElement.clientWidth, rootScrollWidth:document.documentElement.scrollWidth};
  })()`);
  assertThat('body links are visibly distinguishable from prose', typography.linkDecoration.includes('underline'), typography);
  assertThat('long code scrolls locally without widening Reader',
    ['auto','scroll'].includes(typography.preOverflow) && typography.preScrollWidth > typography.preWidth && typography.rootScrollWidth <= typography.rootWidth + 1, typography);

  const fontSize = () => evaluate(client, `parseFloat(getComputedStyle(document.querySelector('[data-layout="reader-body"]')).fontSize)`);
  const mobileBefore = await fontSize();
  await setViewport(client, 1280, 800, false, 1);
  const desktopBefore = await fontSize();
  await setViewport(client, width, height, true, deviceScaleFactor);
  await clickSelector(client, '[data-nav="settings"]');
  await selectorExists(client, '[data-field="reader-font-scale"]');
  await evaluate(client, `document.querySelector('[data-field="reader-font-scale"]').focus()`);
  await key(client, 'a', 'KeyA', 2);
  await client.send('Input.insertText', {text:'1.25'});
  await clickSelector(client, '[data-action="save-settings"]');
  await waitUntil('the font scale stored by the server', 10000, async () =>
    (await serverState('/api/v1/settings')).reader_font_scale === 1.25);
  // Reopen through a real persisted setting, not an injected CSS variable.
  await navigate(client, `${base}/entries/2`);
  await selectorExists(client, '[data-action="open-reader-image"]');
  const mobileAfter = await fontSize();
  await setViewport(client, 1280, 800, false, 1);
  const desktopAfter = await fontSize();
  assertThat('saved reader font scale applies at mobile and desktop breakpoints',
    Math.abs(mobileAfter / mobileBefore - 1.25) < 0.01 && Math.abs(desktopAfter / desktopBefore - 1.25) < 0.01,
    {preset, mobileBefore, mobileAfter, desktopBefore, desktopAfter});
  await setViewport(client, width, height, true, deviceScaleFactor);
  await captureArtifact(client, 'reader-scaled');
  await clickSelector(client, '[data-nav="feeds"]');
  await selectorExists(client, '[data-field="feed-url-input"]');
  // Subscriptions the server adds by fetching them from the feed fixture server.
  const subscriptions = async () => (await serverState('/api/v1/feeds')).feeds;
  const before = (await subscriptions()).length;
  const feedUrl = `${harness.feeds.base}/new/keyboard-submit.xml`;
  await evaluate(client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
  await client.send('Input.insertText', {text:feedUrl});
  const focus = await evaluate(client, `(() => {
    const input = document.activeElement;
    const style = getComputedStyle(input);
    return {visible:input.matches(':focus-visible'), outline:style.outlineStyle, width:parseFloat(style.outlineWidth)};
  })()`);
  assertThat('subscription input has a visible keyboard focus indicator', focus.visible && focus.outline !== 'none' && focus.width >= 2, focus);
  await key(client, 'Enter', 'Enter');
  await waitUntil('the added subscription', 15000, async () => (await subscriptions()).length === before + 1);
  await waitFor(client, `document.querySelector('[data-field="feed-url-input"]').value === ''`);
  assertThat('Enter adds the subscription exactly once',
    (await subscriptions()).filter(feed => feed.url === feedUrl).length === 1, {feeds: await subscriptions()});
  // Hold the server's first fetch of the new subscription so duplicate submission and draft
  // editing overlap deterministically, without timing sleeps or changing production code.
  const slowFeedUrl = `${harness.feeds.base}/new/pending-submit.xml`;
  const nextDraftUrl = `${harness.feeds.base}/new/next-draft.xml`;
  const held = harness.feeds.hold(pathname => pathname === '/new/pending-submit.xml');
  try {
    await evaluate(client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
    await client.send('Input.insertText', {text:slowFeedUrl});
    await key(client, 'Enter', 'Enter');
    await waitUntil('the held first fetch', 15000, async () => held.waiting() === 1);
    await waitFor(client, `document.querySelector('[data-action="add-feed"]').disabled`);
    assertThat('pending subscription exposes busy feedback while the input stays editable',
      await evaluate(client, `document.querySelector('[data-action="add-feed"]').getAttribute('aria-busy') === 'true' && !document.querySelector('[data-field="feed-url-input"]').disabled`));
    await evaluate(client, `document.querySelector('[data-layout="feed-form"]').requestSubmit()`);
    await key(client, 'a', 'KeyA', 2);
    await client.send('Input.insertText', {text:nextDraftUrl});
    await evaluate(client, `document.querySelector('[data-layout="feed-form"]').requestSubmit()`);
    await sleep(300);
    const fetches = harness.feeds.count('/new/pending-submit.xml');
    held.release();
    await waitFor(client, `!document.querySelector('[data-action="add-feed"]').disabled`);
    assertThat('completed subscription keeps the next address typed during refresh',
      await evaluate(client, `document.querySelector('[data-field="feed-url-input"]').value === ${JSON.stringify(nextDraftUrl)}`));
    assertThat('repeated pending submits produce one refresh and do not add the next draft',
      fetches === 1 && (await subscriptions()).length === before + 2 &&
        harness.feeds.count('/new/next-draft.xml') === 0, {fetches, feeds: await subscriptions()});
  } finally {
    held.release();
  }
  await evaluate(client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
  await key(client, 'a', 'KeyA', 2);
  await client.send('Input.insertText', {text:'https://example.com/should-not-submit.xml'});
  await clickSelector(client, '[data-action="refresh-all"]');
  await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
  assertThat('Feeds refresh button becomes available after the shared task finishes', await evaluate(client, `(() => {
    const button = document.querySelector('[data-action="refresh-all"]');
    return !button.disabled && button.getAttribute('aria-busy') === 'false' && button.textContent.trim() === '刷新全部';
  })()`));
  assertThat('refresh button does not submit the subscription form', await evaluate(client,
    `document.querySelector('[data-field="feed-url-input"]').value === 'https://example.com/should-not-submit.xml'`) &&
    (await subscriptions()).length === before + 2, {feeds: await subscriptions()});
}

async function nativePage() {
  const response = await fetch(`${cdpBase}/json/list`, {signal: AbortSignal.timeout(10000)});
  if (!response.ok) throw new Error(`Native target discovery failed: HTTP ${response.status}`);
  const targets = (await response.json()).filter(target => target.id === nativeTarget);
  if (targets.length !== 1) throw new Error('Expected exactly one existing target matching --native-target');
  const target = targets[0];
  const url = new URL(target.url);
  if (target.type !== 'page' || !['http:', 'https:'].includes(url.protocol) || url.hostname !== 'dioxus.index.html') {
    throw new Error(`Refusing non-Dioxus native target: ${target.url}`);
  }
  return target;
}

// Start the clock at the actual browser click event, excluding CDP transport
// and pre-click scrolling. Stop after the expected DOM state and two frames.
async function nativeClick(client, name, selector, condition, scroll = true) {
  const point = await evaluate(client, `(async () => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (!element || element.disabled) throw new Error('Missing or disabled native click target: ' + ${JSON.stringify(selector)});
    if (${scroll}) element.scrollIntoView({block:'nearest', inline:'nearest', behavior:'instant'});
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const r = element.getBoundingClientRect();
    const x = (Math.max(0, r.left) + Math.min(innerWidth, r.right)) / 2;
    const y = (Math.max(0, r.top) + Math.min(innerHeight, r.bottom)) / 2;
    if (r.right <= 0 || r.left >= innerWidth || r.bottom <= 0 || r.top >= innerHeight ||
        !element.contains(document.elementFromPoint(x, y))) throw new Error('Native click target is outside viewport or obscured: ' + ${JSON.stringify(selector)});
    let frame, timer, resolveResult, done = false;
    const promise = new Promise(resolve => { resolveResult = resolve; });
    const finish = result => {
      if (done) return;
      done = true; clearTimeout(timer); cancelAnimationFrame(frame);
      element.removeEventListener('click', clicked, true);
      resolveResult(result);
    };
    const clicked = () => {
      const start = performance.now();
      const poll = () => {
        try {
          if (${condition}) frame = requestAnimationFrame(() => {
            frame = requestAnimationFrame(() => finish({durationMs:performance.now() - start}));
          });
          else frame = requestAnimationFrame(poll);
        } catch (error) { finish({error:String(error)}); }
      };
      poll();
    };
    timer = setTimeout(() => finish({error:'Native interaction timed out after 12 seconds'}), 12000);
    element.addEventListener('click', clicked, {capture:true, once:true});
    window.__rssrNativeSmokeTiming = {promise, cancel:() => finish({error:'Measurement cancelled'})};
    return {x,y};
  })()`);
  try {
    await client.send('Input.dispatchMouseEvent', {type:'mousePressed', button:'left', buttons:1, clickCount:1, ...point});
    await client.send('Input.dispatchMouseEvent', {type:'mouseReleased', button:'left', buttons:0, clickCount:1, ...point});
    const measurement = await evaluate(client, 'window.__rssrNativeSmokeTiming.promise');
    assertThat(name, !measurement.error, measurement);
    nativeMeasurements.push({name, ...measurement});
    return measurement;
  } finally {
    await evaluate(client, 'window.__rssrNativeSmokeTiming?.cancel(); delete window.__rssrNativeSmokeTiming').catch(() => {});
  }
}

async function checkNativeWindow(client, target) {
  const environment = await evaluate(client, `({url:location.href, userAgent:navigator.userAgent,
    width:innerWidth, height:innerHeight, devicePixelRatio, screenWidth:screen.width, screenHeight:screen.height})`);
  assertThat('attached window is Windows Dioxus WebView',
    new URL(environment.url).hostname === 'dioxus.index.html' && environment.userAgent.includes('Windows'), environment);
  nativeEvidence = {
    mode:'windows-native-existing-webview', targetId:target.id, targetUrl:target.url,
    environment, browser:await client.send('Browser.getVersion'),
    fixture:'caller-provided isolated native SQLite; no browser seed or device emulation',
    timing:'browser performance.now from trusted click to expected DOM condition plus two requestAnimationFrame callbacks; excludes transport and pre-click scrolling',
    limitations:['No operating-system clipboard access', 'No Android or macOS device evidence'],
  };
  await selectorExists(client, '[data-action="activate-home"]');
  if (await selectorExistsOptional(client, '[data-field="entry-search"]')) {
    await nativeClick(client, 'native existing search mode returns to normal', '[data-action="toggle-search"]',
      `document.querySelector('[data-layout="app-nav-shell"]').dataset.state === 'normal'`, false);
  }
  const home = '[data-page="entries"][data-entry-scope="all"]';
  if (!(await selectorExistsOptional(client, home))) {
    await nativeClick(client, 'initial Home navigation', '[data-action="activate-home"]', `!!document.querySelector('${home}')`, false);
  }
  await selectorExists(client, '[data-layout="entry-groups"][data-state="populated"]');
  const normalShell = await shellEvidence(client);
  await nativeClick(client, 'native search expands', '[data-action="toggle-search"]', `!!document.querySelector('[data-field="entry-search"]')`, false);
  await searchInputEvidence(client);
  const searchShell = await shellEvidence(client);
  assertThat('native search preserves shell height and replaces secondary navigation',
    Math.abs(normalShell.height - searchShell.height) <= 1 && await evaluate(client,
      `!document.querySelector('[data-nav="feeds"]') && !document.querySelector('[data-nav="settings"]') && document.activeElement.matches('[data-field="entry-search"]')`),
    {normalShell, searchShell});
  await key(client, 'Escape', 'Escape');
  await selectorExists(client, '[data-nav="feeds"]');
  assertThat('native Escape restores normal navigation', !(await selectorExistsOptional(client, '[data-field="entry-search"]')));

  for (const destination of ['feeds', 'settings']) {
    await nativeClick(client, `native ${destination} navigation`, `[data-nav="${destination}"]`, `!!document.querySelector('[data-page="${destination}"]')`, false);
    await shellEvidence(client);
    const common = await commonPageEvidence(client);
    assertThat(`native ${destination} fits window`, common.scrollWidth <= common.clientWidth + 1 && common.overlay === null, common);
    await captureArtifact(client, `native-${destination}`);
    const before = await evaluate(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState`);
    await nativeClick(client, `native ${destination} to Home navigation`, '[data-action="activate-home"]', `!!document.querySelector('${home}')`, false);
    assertThat(`native ${destination} to Home does not enter manual refresh`,
      await evaluate(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState`) === before, {before});
  }

  await ensureEntryControlsOpen(client);
  const sources = await evaluate(client, `(() => {
    const names = [...document.querySelectorAll('[data-layout="entry-filters-source-chip"] span')];
    return {longNames:names.filter(el => el.textContent.length > 40).map(el => {
      const s = getComputedStyle(el); return {text:el.textContent, width:el.clientWidth, scrollWidth:el.scrollWidth,
        height:el.clientHeight, scrollHeight:el.scrollHeight, whiteSpace:s.whiteSpace, textOverflow:s.textOverflow};
    }), rootWidth:document.documentElement.clientWidth, rootScrollWidth:document.documentElement.scrollWidth,
    pagination:document.querySelector('[data-layout="entry-pagination-summary"]')?.textContent};
  })()`);
  nativeEvidence.list = sources;
  assertThat('native long source names are fully readable without ellipsis or overflow',
    sources.longNames.length > 0 && sources.longNames.every(item => item.whiteSpace !== 'nowrap' && item.textOverflow !== 'ellipsis' &&
      item.scrollWidth <= item.width + 1 && item.scrollHeight <= item.height + 1) && sources.rootScrollWidth <= sources.rootWidth + 1, sources);
  await captureArtifact(client, 'native-entries-sources');
  await clickSelector(client, '[data-action="hide-entry-controls"]');
  await selectorExists(client, '[data-layout="entry-pagination"]');
  for (let iteration = 0; iteration < 10; iteration++) {
    const before = await evaluate(client, `document.querySelector('[data-slot="entry-pagination-status"]').textContent`);
    const geometry = await evaluate(client, `(() => {
      window.scrollTo({top:(document.documentElement.scrollHeight - innerHeight) / 2, behavior:'instant'});
      const nav = document.querySelector('[data-layout="entry-pagination"]'); const r = nav.getBoundingClientRect();
      return {count:document.querySelectorAll('[data-layout="entry-pagination"]').length, top:r.top, bottom:r.bottom,
        height:innerHeight, scrollY, targets:[...nav.querySelectorAll('button')].map(el => {const b = el.getBoundingClientRect(); return {width:b.width,height:b.height};})};
    })()`);
    assertThat(`native paginator reachable at list midpoint ${iteration + 1}`, geometry.count === 1 && geometry.scrollY > 0 && geometry.top >= 0 &&
      geometry.bottom <= geometry.height && geometry.targets.every(r => meetsTouchTarget(r.width, r.height)), geometry);
    if (iteration === 0) await captureArtifact(client, 'native-entries-midpoint');
    const direction = iteration % 2 === 0 ? 'next' : 'previous';
    await nativeClick(client, `native pagination ${direction} ${iteration + 1}`, `[data-action="entry-page-${direction}"]`,
      `!!document.querySelector('[data-slot="entry-pagination-status"]') && document.querySelector('[data-slot="entry-pagination-status"]').textContent !== ${JSON.stringify(before)} && scrollY < 2`, false);
  }

  const reader = '[data-page="reader"][data-state="loaded"] [data-slot="reader-body-html"]';
  for (let iteration = 0; iteration < 10; iteration++) {
    await nativeClick(client, `native reader open ${iteration + 1}`, '[data-slot="entry-card-title"]', `!!document.querySelector('${reader}')`);
    if (iteration === 0) await shellEvidence(client, true);
    await nativeClick(client, `native reader back ${iteration + 1}`, '[data-nav="back"]', `!!document.querySelector('[data-slot="entry-card-title"]')`, false);
  }
  await nativeClick(client, 'native reader open for image and selection', '[data-slot="entry-card-title"]', `!!document.querySelector('${reader}')`);
  await checkNativeReader(client);
  await nativeClick(client, 'native Reader to Home navigation', '[data-action="activate-home"]', `!!document.querySelector('${home}')`, false);
  await shellEvidence(client);
  const finalWindow = await commonPageEvidence(client);
  assertThat('native window size was not emulated or resized',
    finalWindow.innerWidth === environment.width && finalWindow.innerHeight === environment.height, finalWindow);
  nativeEvidence.measurements = nativeMeasurements;
}

async function checkNativeReader(client) {
  await shellEvidence(client, true);
  assertThat('native Reader does not cancel the browser context menu', await evaluate(client, `(() => {
    const body = document.querySelector('[data-slot="reader-body-html"]');
    const event = new MouseEvent('contextmenu', {bubbles:true, cancelable:true, button:2});
    body.dispatchEvent(event);
    return !event.defaultPrevented;
  })()`));
  await checkReaderRefreshFeedback(client, 'native Reader refresh feedback');
  assertThat('native Reader has no pull gesture surface', !(await selectorExistsOptional(client, '[data-slot="pull-refresh"]')));
  const common = await commonPageEvidence(client);
  assertThat('native Reader fits window without framework overlay', common.scrollWidth <= common.clientWidth + 1 && common.overlay === null, common);
  await selectorExists(client, '[data-action="open-reader-image"]');
  const source = await evaluate(client, `(async () => {
    const image = document.querySelector('[data-action="open-reader-image"]');
    image.scrollIntoView({block:'center', behavior:'instant'});
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return {src:image.currentSrc || image.src, scroll:scrollY, inlineHandler:image.hasAttribute('onclick')};
  })()`);
  assertThat('native fixture image is local and has no inline handler', source.src.startsWith('data:') && !source.inlineHandler, source);
  await nativeClick(client, 'native image opens', '[data-action="open-reader-image"]', `document.querySelector('[data-layout="reader-image-viewer"]')?.open === true`, false);
  const viewer = await evaluate(client, `(() => {
    const dialog = document.querySelector('[data-layout="reader-image-viewer"]');
    const image = dialog.querySelector('img'); const r = image.getBoundingClientRect();
    const close = dialog.querySelector('[data-action="close-reader-image"]').getBoundingClientRect();
    return {src:image.src, focus:document.activeElement.getAttribute('data-action'), position:document.body.style.position,
      fits:r.left >= 0 && r.top >= 0 && r.right <= innerWidth + 1 && r.bottom <= innerHeight + 1,
      closeWidth:close.width, closeHeight:close.height};
  })()`);
  assertThat('native image viewer reuses rendered source, fits and owns focus and scroll lock',
    viewer.src === source.src && viewer.focus === 'close-reader-image' && viewer.position === 'fixed' && viewer.fits &&
    meetsTouchTarget(viewer.closeWidth, viewer.closeHeight), viewer);
  await captureArtifact(client, 'native-reader-image-viewer');
  await key(client, 'Escape', 'Escape');
  await waitFor(client, `!document.querySelector('[data-layout="reader-image-viewer"]') && document.body.style.position !== 'fixed'`);
  assertThat('native image close restores scroll and focus', await evaluate(client,
    `Math.abs(scrollY - ${source.scroll}) <= 1 && document.activeElement.matches('[data-action="open-reader-image"]')`));
  const coords = await evaluate(client, `(async () => {
    const paragraph = document.querySelector('[data-slot="reader-body-html"] p');
    paragraph.scrollIntoView({block:'center', behavior:'instant'});
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const range = document.createRange(); range.selectNodeContents(paragraph);
    const r = [...range.getClientRects()].find(rect => rect.width > 100 && rect.height > 0);
    return {x:r.left + 1, y:r.top + r.height / 2, end:Math.min(r.right - 1, r.left + 250)};
  })()`);
  await client.send('Input.dispatchMouseEvent', {type:'mousePressed', button:'left', buttons:1, clickCount:1, x:coords.x, y:coords.y});
  await client.send('Input.dispatchMouseEvent', {type:'mouseMoved', button:'left', buttons:1, x:coords.end, y:coords.y});
  await client.send('Input.dispatchMouseEvent', {type:'mouseReleased', button:'left', buttons:0, clickCount:1, x:coords.end, y:coords.y});
  const selection = await evaluate(client, 'getSelection().toString()');
  assertThat('native Reader supports mouse drag selection', selection.length > 5, {selection});
  const title = await evaluate(client, `document.querySelector('[data-slot="reader-title"]').textContent`);
  await key(client, 'a', 'KeyA', 2);
  const all = await evaluate(client, `({text:getSelection().toString(), title:document.querySelector('[data-slot="reader-title"]')?.textContent})`);
  assertThat('native Ctrl+A selects text without triggering Reader shortcuts', all.title === title && all.text.includes(selection) && all.text.length > selection.length, {selectedLength:all.text.length, title:all.title});
  await evaluate(client, 'getSelection().removeAllRanges(); window.scrollTo({top:0, behavior:"instant"})');
  await captureArtifact(client, 'native-reader');
}

async function checkNarrowSidebarSearch(client) {
  await setViewport(client, 1280, 800, false, 1);
  for (const themePreset of ['atlas-sidebar', 'atlas-sidebar-v1']) {
    await seedAndNavigate(client, 'home-reader', '/entries', '[data-page="entries"]', themePreset);
    await clickSelector(client, '[data-action="toggle-search"]');
    const entriesInput = await searchInputEvidence(client);
    assertThat(`${themePreset} narrow sidebar gives search its own row without page overflow`,
      entriesInput.width >= 140 && await evaluate(client,
        `document.documentElement.scrollWidth <= innerWidth + 1 && getComputedStyle(document.querySelector('[data-layout="app-nav-topline"]')).flexWrap === 'wrap'`),
      entriesInput);
    if (themePreset.endsWith('-v1')) await captureArtifact(client, 'legacy-atlas-search-entries');
    await navigate(client, `${base}/entries/2`);
    await selectorExists(client, '[data-slot="reader-body-html"]');
    await clickSelector(client, '[data-action="toggle-search"]');
    await searchInputEvidence(client);
    if (themePreset.endsWith('-v1')) await captureArtifact(client, 'legacy-atlas-search-reader');
  }
}

// --- Formerly the Playwright acceptances (nav collapse, reading position, reader metadata and
// time); seeds that used to be edited in browser storage are now edited before the library is
// built (`useSeed`'s `edit`).

/// A trusted mouse click at the element's centre (focuses it, as a person's click does).
async function pointerClick(client, selector) {
  const point = await evaluate(client, `(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (!element) throw new Error('Missing click target: ' + ${JSON.stringify(selector)});
    element.scrollIntoView({block:'nearest', inline:'nearest'});
    const rect = element.getBoundingClientRect();
    return {x: rect.left + rect.width / 2, y: rect.top + rect.height / 2};
  })()`);
  await client.send('Input.dispatchMouseEvent', {type:'mousePressed', button:'left', buttons:1, clickCount:1, ...point});
  await client.send('Input.dispatchMouseEvent', {type:'mouseReleased', button:'left', buttons:0, clickCount:1, ...point});
}

/// Replaces a text field's value as typing does (select all, then insert or delete).
async function fillField(client, selector, text) {
  await evaluate(client, `(() => { const field = document.querySelector(${JSON.stringify(selector)}); field.focus(); field.select(); })()`);
  if (text) await client.send('Input.insertText', {text});
  else await key(client, 'Backspace', 'Backspace');
}

/// Waits until an element's whole text is `text`.
async function textShown(client, text, selector = 'body *') {
  await waitFor(client, `[...document.querySelectorAll(${JSON.stringify(selector)})].some(element => element.textContent.trim() === ${JSON.stringify(text)})`);
}

const pathname = client => evaluate(client, 'location.pathname');
// The page's own preferences in this browser (the search text, hidden list controls) outlive a
// seed; these checks start, as they did in fresh browser contexts, without them.
const forgetShellPreferences = client => evaluate(client,
  `localStorage.removeItem('rssr-entry-search'); localStorage.removeItem('rssr-entry-controls-hidden')`);
const readerReady = async client => {
  await selectorExists(client, '[data-page="reader"][data-position-ready="true"]');
  await sleep(2300);
};

async function checkNavCollapse(client) {
  const theme = preset || 'none';
  await forgetShellPreferences(client);
  await seedAndNavigate(client, 'reader-demo', '/entries/1', '[data-page="reader"][data-position-ready="true"]', preset,
    {edit: ({core}) => { core.settings.archive_after_months = 0; }});
  for (const navWidth of [360, 1280]) {
    const label = `nav collapse ${theme} ${navWidth}`;
    await setViewport(client, navWidth, 800, false, 1);
    await navigate(client, `${base}/entries/1`);
    await selectorExists(client, '[data-page="reader"][data-position-ready="true"]');
    await selectorExists(client, '[data-action="toggle-nav"]');
    const rect = selector => evaluate(client, `(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return {x:r.x, y:r.y, width:r.width, height:r.height}; })()`);
    const fits = () => evaluate(client, 'document.documentElement.scrollWidth <= innerWidth');
    const navState = () => evaluate(client, `document.querySelector('[data-layout="app-nav-shell"]').dataset.state`);
    const expanded = await rect('[data-layout="app-nav-shell"]');
    const target = await rect('[data-action="toggle-nav"]');
    assertThat(`${label}: toggle is a touch target at the right edge`, target.width >= 44 && target.height >= 44 &&
      target.x >= expanded.x + expanded.width - target.width - 20, {expanded, target});
    assertThat(`${label}: expanded navigation fits`, await fits());
    if (['none', 'atlas-sidebar'].includes(theme)) await captureArtifact(client, `nav-${theme}-${navWidth}-expanded`);
    await clickSelector(client, '[data-action="toggle-search"]');
    await selectorExists(client, '[data-field="entry-search"]');
    assertThat(`${label}: search stays usable beside the collapse control`, (await rect('[data-field="entry-search"]')).width >= 140);
    await fillField(client, '[data-field="entry-search"]', 'preserved query');
    await pointerClick(client, '[data-action="toggle-nav"]');
    await waitFor(client, `document.querySelector('[data-layout="app-nav-shell"]').dataset.state === 'collapsed'`);
    const collapsedState = await evaluate(client, `(() => {
      const nav = document.querySelector('[data-layout="app-nav-shell"]');
      const toggle = document.querySelector('[data-action="toggle-nav"]');
      return {expanded: toggle.getAttribute('aria-expanded'), label: toggle.getAttribute('aria-label'),
        controls: nav.querySelectorAll('button, a, input').length, focused: document.activeElement === toggle};
    })()`);
    assertThat(`${label}: collapsed navigation keeps only the focused expand control`,
      collapsedState.expanded === 'false' && collapsedState.label === '向右展开导航' &&
      collapsedState.controls === 1 && collapsedState.focused, collapsedState);
    const collapsed = await rect('[data-layout="app-nav-shell"]');
    assertThat(`${label}: collapsed header is small and collapses toward the left`,
      collapsed.width <= 45 && collapsed.height <= 45 && collapsed.x <= expanded.x + 1, {expanded, collapsed});
    assertThat(`${label}: collapsed navigation fits`, await fits());
    if (['none', 'atlas-sidebar'].includes(theme)) await captureArtifact(client, `nav-${theme}-${navWidth}-collapsed`);
    await key(client, 'Enter', 'Enter');
    await waitFor(client, `document.querySelector('[data-layout="app-nav-shell"]').dataset.state === 'normal'`);
    await clickSelector(client, '[data-action="toggle-search"]');
    assertThat(`${label}: the search query survives collapsing`, await evaluate(client,
      `document.querySelector('[data-field="entry-search"]').value === 'preserved query'`));
    await fillField(client, '[data-field="entry-search"]', '');
    await key(client, 'Escape', 'Escape');
    // A rendered article DOM node survives shell changes and flag updates.
    assertThat(`${label}: fixture has rendered HTML`, await evaluate(client, `(() => {
      window.__bodyNode = document.querySelector('[data-slot="reader-body-html"]')?.firstChild;
      return !!window.__bodyNode;
    })()`));
    await clickSelector(client, '[data-action="toggle-starred"]', 200);
    assertThat(`${label}: a flag change does not replace the body`, await evaluate(client,
      `window.__bodyNode === document.querySelector('[data-slot="reader-body-html"]')?.firstChild`));
    await clickSelector(client, '[data-nav="settings"]');
    await selectorExists(client, '[data-page="settings"]');
    await pointerClick(client, '[data-action="toggle-nav"]');
    await evaluate(client, 'history.back()');
    await selectorExists(client, '[data-page="reader"]');
    assertThat(`${label}: shell state survives routes`, await navState() === 'collapsed');
    await navigate(client, `${base}/entries/1`);
    await selectorExists(client, '[data-action="toggle-nav"]');
    assertThat(`${label}: a new session starts expanded`, await navState() === 'normal');
  }
  await setViewport(client, width, height, true, deviceScaleFactor);
}

async function checkReadingPosition(client) {
  const longBody = Array.from({length: 2000}, (_, i) =>
    `<p>Block ${i}: ${'Reading position performance. '.repeat(12)}</p>`).join('');
  const scrollY = () => evaluate(client, 'scrollY');
  for (const positionWidth of [360, 1280]) {
    const label = `reading position ${positionWidth}`;
    await forgetShellPreferences(client);
    await setViewport(client, positionWidth, 800, false, 1);
    await seedAndNavigate(client, 'reader-demo', '/entries', '[data-page="entries"][data-position-ready="true"]', preset, {edit: ({core, entry_content}) => {
      core.settings.archive_after_months = 0;
      for (const entry of entry_content.entries) entry.content_html = longBody;
    }});
    await selectorExists(client, 'a[href="/entries/1"]');
    await pointerClick(client, 'a[href="/entries/1"]');
    await readerReady(client);
    await evaluate(client, `(() => {
      window.__positionReads = 0;
      const original = Element.prototype.getBoundingClientRect;
      Element.prototype.getBoundingClientRect = function () {
        if (this.closest('[data-layout="reader-body"]')) window.__positionReads++;
        return original.call(this);
      };
      scrollTo(0, document.documentElement.scrollHeight * 0.8);
    })()`);
    const centre = await evaluate(client, '({x: innerWidth / 2, y: innerHeight / 2})');
    for (let i = 0; i < 12; i++) {
      await client.send('Input.dispatchMouseEvent', {type:'mouseWheel', ...centre, deltaX:0, deltaY: i % 2 ? -180 : 240});
      await sleep(30);
    }
    await key(client, 'PageDown', 'PageDown');
    await sleep(300);
    assertThat(`${label}: continuous scrolling does not scan reader blocks`, await evaluate(client, 'window.__positionReads') === 0);
    // Editing the shell search is outside the reader shortcut scope: caret movement must not scan a
    // long article, even though an article remains open underneath.
    await clickSelector(client, '[data-action="toggle-search"]');
    await selectorExists(client, '[data-field="entry-search"]');
    await fillField(client, '[data-field="entry-search"]', 'caret movement');
    await evaluate(client, 'window.__positionReads = 0');
    await key(client, 'ArrowLeft', 'ArrowLeft');
    await key(client, 'ArrowRight', 'ArrowRight');
    await sleep(100);
    assertThat(`${label}: search caret movement does not scan reader blocks`,
      await evaluate(client, 'window.__positionReads') === 0 && await pathname(client) === '/entries/1');
    await fillField(client, '[data-field="entry-search"]', '');
    await key(client, 'Escape', 'Escape');
    await evaluate(client, `document.querySelector('[data-layout="reader-shortcut-scope"]').focus({preventScroll: true})`);
    const forward = await evaluate(client, `!document.querySelector('[data-nav="next-unread-entry"]').disabled`);
    const arrow = forward ? 'ArrowRight' : 'ArrowLeft';
    await key(client, arrow, arrow, 2);
    await evaluate(client, `document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key: ${JSON.stringify(arrow)}, bubbles: true, isComposing: true}))`);
    await sleep(200);
    assertThat(`${label}: modified and composing keys neither navigate nor scan`,
      await pathname(client) === '/entries/1' && await evaluate(client, 'window.__positionReads') === 0);
    const saved = await scrollY();
    assertThat(`${label}: the article was scrolled`, saved > 1000, {saved});
    // Browser history has no preceding DOM click: it must capture the departing article.
    await evaluate(client, 'history.back()');
    await selectorExists(client, 'a[href="/entries/1"]');
    await pointerClick(client, 'a[href="/entries/1"]');
    await readerReady(client);
    const restored = await scrollY();
    assertThat(`${label}: browser back captures the position`, Math.abs(restored - saved) < 4, {saved, restored});
    const adjacent = forward ? 'next-unread-entry' : 'previous-unread-entry';
    await evaluate(client, `document.querySelector('[data-layout="reader-shortcut-scope"]').focus({preventScroll: true})`);
    await key(client, arrow, arrow);
    await waitFor(client, `/^\\/entries\\/\\d+$/.test(location.pathname) && location.pathname !== '/entries/1'`);
    await readerReady(client);
    await evaluate(client, 'history.back()');
    await readerReady(client);
    const afterKeyboard = await scrollY();
    assertThat(`${label}: keyboard navigation captures the position`, Math.abs(afterKeyboard - saved) < 4, {saved, afterKeyboard});
    // The native host's capture returns a fact synchronously, without changing history.
    const nativeFact = await evaluate(client, `(() => {
      const event = new CustomEvent('rssr-capture-position', {detail: {fact: null}});
      document.dispatchEvent(event);
      return event.detail.fact;
    })()`);
    assertThat(`${label}: the native capture bridge reports the position`, nativeFact?.kind === 'capture' &&
      nativeFact.key === 'reader:1' && Math.abs(nativeFact.point.y - saved) < 4, nativeFact);
    for (const edge of ['top', 'bottom']) {
      await evaluate(client, `scrollTo(0, ${edge === 'top' ? 0 : 'document.documentElement.scrollHeight'})`);
      const expected = await scrollY();
      await clickSelector(client, `[data-nav="${adjacent}"]`);
      await waitFor(client, `location.pathname !== '/entries/1'`);
      await readerReady(client);
      await evaluate(client, 'history.back()');
      await readerReady(client);
      const actual = await scrollY();
      assertThat(`${label}: ${edge} edge is captured`, Math.abs(actual - expected) < 4, {expected, actual});
    }
  }
  await setViewport(client, width, height, true, deviceScaleFactor);
}

/// Reader metadata (long, missing), the original link, unread counts and local-time offsets
/// across a daylight-saving change.
async function checkReaderMetadataAndTime(client) {
  const feeds = harness.feeds;
  const long = '长来源LongUnbrokenSource'.repeat(25);
  const sourceCount = `document.querySelector('[data-slot="entry-filters-source-unread-count"]')?.textContent.trim()`;
  const fits = () => evaluate(client, 'document.documentElement.scrollWidth <= innerWidth');
  const meta = () => evaluate(client, `[...document.querySelectorAll('[data-slot="reader-meta"]')].map(element => element.textContent)`);
  await forgetShellPreferences(client);
  await client.send('Emulation.setTimezoneOverride', {timezoneId: 'America/New_York'});
  let script;
  try {
    // The start-up refresh fails once, so the seeded last success is kept (a success would store
    // the time of the refresh).
    feeds.failNext('/reader-demo/1.xml');
    await setViewport(client, 360, 800, false, 1);
    await seedAndNavigate(client, 'reader-demo', '/entries/2', '[data-slot="reader-title"]', preset, {edit: ({core}) => {
      core.settings.archive_after_months = 0;
      core.feeds[0].title = long;
      core.feeds[0].last_success_at = '2026-03-08 07:00:00.0 +00:00:00';
      core.entries[0].published_at = '2026-03-08 06:59:59.0 +00:00:00';
      core.entries[1].published_at = '2026-03-08 07:00:00.0 +00:00:00';
      core.entries[1].author = '长作者Author'.repeat(35);
      core.entries[1].url = `${feeds.base}/original/${'long-url'.repeat(80)}`;
    }});
    await textShown(client, 'Demo Entry Two', '[data-slot="reader-title"]');
    await textShown(client, '发布时间：2026-03-08 03:00 UTC-04:00');
    const lines = await meta();
    assertThat('a directly opened reader loads its source and author; the offset after the DST change',
      lines[0] === `来源：${long}` && /^作者：长作者/.test(lines[1]), lines);
    for (const readerWidth of [360, 1280]) {
      await setViewport(client, readerWidth, 800, false, 1);
      await sleep(200);
      const link = await evaluate(client, `document.querySelector('[data-action="open-original"]').getBoundingClientRect().height`);
      assertThat(`reader ${readerWidth}: long source, author and address fit; the original link is a touch target`,
        await fits() && link >= 44, {link});
      await captureArtifact(client, `task1-reader-${readerWidth}`);
    }
    await checkOpenOriginal(client);

    await pointerClick(client, '[data-action="mark-read"]');
    await selectorExists(client, '[data-action="mark-read"][data-state="read"]');
    await pointerClick(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-page="entries"]');
    if (!(await evaluate(client, `!!document.querySelector('[data-slot="entry-filters-source-unread-count"]')?.getClientRects().length`))) {
      await clickSelector(client, '[data-action="show-entry-controls"]');
    }
    await waitFor(client, `${sourceCount} === '· 0'`);
    const describedBy = await evaluate(client, `document.querySelector('[data-field="entry-source-filter"]').getAttribute('aria-describedby')`);
    for (const listWidth of [360, 1280]) {
      await setViewport(client, listWidth, 800, false, 1);
      await sleep(200);
      const chip = await evaluate(client, `document.querySelector('[data-layout="entry-filters-source-chip"]').getBoundingClientRect().height`);
      assertThat(`sources ${listWidth}: long source names fit; the source chip is a touch target`, await fits() && chip >= 44, {chip});
      await captureArtifact(client, `task1-sources-${listWidth}`);
    }
    assertThat('marking read in the reader shows 0 unread; the count is described to assistive technology',
      /^source-unread-/.test(describedBy ?? ''), {describedBy});
    await clickSelector(client, '[data-action="mark-read"]');
    await waitFor(client, `${sourceCount} === '· 1'`);
    await fillField(client, '[data-field="search-title"]', 'does not match');
    await sleep(400);
    const searched = await evaluate(client, sourceCount);
    await fillField(client, '[data-field="search-title"]', '');
    await clickSelector(client, '[data-field="show-archived"]');
    const archived = await evaluate(client, sourceCount);
    assertThat('marking unread in the list updates the count; search and archive filters do not change it',
      searched === '· 1' && archived === '· 1', {searched, archived});
    // A flag write the server refuses (formerly a failing browser storage): the counts stay.
    await withFailedRequests(client, '*/api/v1/entries/2/read*', '/api/v1/entries/2/read',
      {status: 503, error: {code: 'unavailable', message: '服务器繁忙（测试）'}}, async () => {
        await evaluate(client, `[...document.querySelectorAll('[data-action="mark-read"]')].find(button => button.textContent.includes('标已读')).click()`);
        await selectorExists(client, '[data-state="error"]');
      });
    const afterFailure = await evaluate(client, sourceCount);
    await clickSelector(client, '[data-nav="feeds"]');
    await textShown(client, '本地文章 2 · 未读 1');
    await waitFor(client, `document.body.innerText.includes('最近成功：2026-03-08 03:00 UTC-04:00')`);
    assertThat('after a refused write the list and the subscription page agree; the last success follows the local offset',
      afterFailure === '· 1', {afterFailure});
    await navigate(client, `${base}/entries/1`);
    await textShown(client, '发布时间：2026-03-08 01:59 UTC-05:00');
    assertThat('one session shows both offsets around the DST change', true);

    // Missing metadata: the source falls back to the subscription address, a blank author and a
    // missing original link are hidden, a missing time is unknown.
    await seedAndNavigate(client, 'reader-demo', '/entries/1', '[data-slot="reader-title"]', preset, {edit: ({core}) => {
      core.settings.archive_after_months = 0;
      core.feeds[0].title = null;
      core.entries[0].author = '   ';
      core.entries[0].published_at = null;
      core.entries[0].url = null;
    }});
    await textShown(client, `来源：${feeds.base}/reader-demo/1.xml`);
    await textShown(client, '发布时间：未知发布时间');
    const missing = await evaluate(client, `({original: document.querySelectorAll('[data-action="open-original"]').length,
      meta: document.querySelectorAll('[data-slot="reader-meta"]').length})`);
    assertThat('missing title falls back to the subscription address; blank author and missing link are hidden; missing time is unknown',
      missing.original === 0 && missing.meta === 2, missing);

    // A subscription that is gone (deleted) while its entry is still stored: the entry stays readable.
    await seedAndNavigate(client, 'reader-demo', '/entries/1', '[data-slot="reader-title"]', preset, {edit: ({core}) => {
      core.settings.archive_after_months = 0;
      core.feeds[0].is_deleted = true;
    }});
    await textShown(client, '来源：未知来源');
    await textShown(client, 'Demo Entry One body.');
    assertThat('an entry whose subscription is gone shows an unknown source and its body', true);

    // The card and its date group agree in several offsets, including a failed offset (UTC).
    await seedAndNavigate(client, 'reader-demo', '/entries/2', '[data-slot="reader-title"]', preset, {edit: ({core}) => {
      core.settings.archive_after_months = 0;
      for (const entry of core.entries) entry.published_at = '2026-03-01 00:15:00.0 +00:00:00';
    }});
    for (const [timezoneId, date, full, fail] of [
      ['America/New_York', '2026-02-28', '2026-02-28 19:15 UTC-05:00', false],
      ['Asia/Kathmandu', '2026-03-01', '2026-03-01 06:00 UTC+05:45', false],
      ['Pacific/Chatham', '2026-03-01', '2026-03-01 14:00 UTC+13:45', false],
      ['America/New_York', '2026-03-01', '2026-03-01 00:15 UTC', true],
    ]) {
      await client.send('Emulation.setTimezoneOverride', {timezoneId});
      if (fail) ({identifier: script} = await client.send('Page.addScriptToEvaluateOnNewDocument',
        {source: 'Date.prototype.getTimezoneOffset = () => NaN;'}));
      await navigate(client, `${base}/entries/2`);
      await textShown(client, 'Demo Entry Two', '[data-slot="reader-title"]');
      await textShown(client, `发布时间：${full}`);
      await pointerClick(client, '[data-action="activate-home"]');
      await selectorExists(client, '[data-slot="entry-card-meta"]');
      const dates = await evaluate(client, `({cards: [...document.querySelectorAll('[data-slot="entry-card-meta"]')].map(element => element.textContent),
        groups: [...document.querySelectorAll('[data-slot="entry-group-title"]')].map(element => element.textContent)})`);
      assertThat(`card and date group agree: ${timezoneId}, ${full}, UTC fallback=${fail}`,
        dates.cards.every(text => text.includes(date)) && dates.groups.some(text => text.includes(date)), dates);
      if (script) {
        await client.send('Page.removeScriptToEvaluateOnNewDocument', {identifier: script});
        script = undefined;
      }
    }
  } finally {
    if (script) await client.send('Page.removeScriptToEvaluateOnNewDocument', {identifier: script}).catch(() => {});
    await client.send('Emulation.setTimezoneOverride', {timezoneId: ''});
    await setViewport(client, width, height, true, deviceScaleFactor);
  }
}

/// The original link opens a separate tab without an opener; the reader stays.
async function checkOpenOriginal(client) {
  const before = await evaluate(client, 'location.href');
  const listTargets = async () => (await (await fetch(`${cdpBase}/json/list`)).json());
  const known = new Set((await listTargets()).map(target => target.id));
  await pointerClick(client, '[data-action="open-original"]');
  let opened;
  await waitUntil('the original page in a new tab', 10000, async () => {
    opened = (await listTargets()).find(target => !known.has(target.id) && target.type === 'page' &&
      target.url.startsWith(`${harness.feeds.base}/original/`));
    return opened !== undefined;
  });
  const popup = connect(opened.webSocketDebuggerUrl);
  try {
    await popup.send('Runtime.enable');
    const opener = await evaluate(popup, 'window.opener === null');
    assertThat('the original link opens a new tab without an opener and keeps the reader',
      opener && await evaluate(client, 'location.href') === before, {opener});
  } finally {
    popup.close();
    await fetch(`${cdpBase}/json/close/${opened.id}`).catch(() => {});
  }
}

async function run() {
  await mkdir(artifactDir, { recursive: true });
  let client;
  try {
    if (!nativeTarget) harness = await startHarness(serverOptions);
    const page = nativeTarget ? await nativePage() : await newPage('about:blank', cdpBase);
    client = connect(page.webSocketDebuggerUrl);

    client.on('Runtime.exceptionThrown', (event) => {
      recordConsoleError({ type: 'exception', text: event.exceptionDetails?.text, event });
    });
    client.on('Runtime.consoleAPICalled', (event) => {
      if (event.type === 'error') {
        recordConsoleError({
          type: 'console.error',
          text: event.args?.map((arg) => arg.value ?? arg.description).join(' '),
        });
      }
    });
    client.on('Log.entryAdded', ({ entry }) => {
      if (entry?.level === 'error') {
        recordConsoleError({ type: 'log.error', text: entry.text, source: entry.source, url: entry.url });
      }
    });

    await client.send('Page.enable');
    await client.send('Runtime.enable');
    await client.send('Log.enable');
    const directoryEnvironment = {
      native: Boolean(nativeTarget), assertThat, consoleErrors,
      setViewport: (w, h, mobile, dpr) => setViewport(client, w, h, mobile, dpr),
      seed: (seed, route) => seedAndNavigate(client, seed, route, '[data-layout="entry-groups"][data-state="populated"]'),
      capture: name => captureArtifact(client, name),
      manualScroll: () => beginManualScroll(client, 'entries'),
    };
    if (nativeTarget && (directoryOnly || cli.has('--directory-perf'))) {
      nativeEvidence = {targetId:page.id, browser:await client.send('Browser.getVersion'),
        window:await evaluate(client, '({url:location.href,userAgent:navigator.userAgent,width:innerWidth,height:innerHeight,dpr:devicePixelRatio})')};
    }
    if (cli.has('--directory-perf')) {
      await measureDirectory(client, directoryEnvironment, cli.get('--directory-perf'), artifactDir);
    } else if (directoryOnly) {
      await checkDirectoryContracts(client, directoryEnvironment);
    } else if (nativeTarget) {
      await checkNativeWindow(client, page);
    } else {
      await setViewport(client, width, height, true, deviceScaleFactor);

      await seedAndNavigate(
        client,
        'mobile-ui-overflow',
        '/entries',
        '[data-layout="entry-groups"][data-state="populated"]',
      );
      await checkEntriesOverflow(client);
      await checkSettings(client);
      await checkFeeds(client);
      await checkReader(client);
      await checkEntriesInitialization(client);
      await checkPrimaryWorkflow(client);
      await checkHomeRefreshAndGestures(client);
      await checkReaderImagesAndSelection(client);
      await checkReadingPreferencesAndFeedInput(client);
      await checkShortDirectory(client);
      await checkDesktop(client);
      await checkNarrowSidebarSearch(client);
      await checkNavCollapse(client);
      // The default CI theme runs the deeper behavioral contracts once; the
      // existing five-theme layout coverage remains in the shared smoke above.
      if (!preset) {
        await checkReadingPosition(client);
        await checkReaderMetadataAndTime(client);
        await checkDirectoryContracts(client, directoryEnvironment);
      }
    }

    assertThat('browser console has no errors', consoleErrors.length === 0, consoleErrors);
    await writeFile(
      path.join(artifactDir, 'assertions.json'),
      JSON.stringify({ status: 'pass', assertions, consoleErrors, ignoredConsoleErrors, nativeEvidence }, null, 2),
      'utf8',
    );
    if (!nativeTarget) await client.send('Page.close');
  } catch (error) {
    if (client) await captureArtifact(client, 'failure').catch(() => {});
    await writeFile(
      path.join(artifactDir, 'assertions.json'),
      JSON.stringify(
        {
          status: 'fail',
          error: error.stack ?? String(error),
          assertions,
          consoleErrors,
          ignoredConsoleErrors,
          nativeEvidence: nativeEvidence && {...nativeEvidence, measurements:nativeMeasurements},
        },
        null,
        2,
      ),
      'utf8',
    );
    throw error;
  } finally {
    client?.close();
    await harness?.close();
  }
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
