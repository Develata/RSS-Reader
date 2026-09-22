import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

import {
  clickSelector,
  connect,
  evaluate,
  navigate,
  newPage,
  selectorExists,
  sleep,
  waitFor,
} from './cdp_session.mjs';

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
const staticBase = cli.get('--static-base') ?? process.env.STATIC_BASE ?? 'http://127.0.0.1:8091';
const artifactDir =
  cli.get('--artifact-dir') ??
  process.env.ARTIFACT_DIR ??
  'target/static-web-small-viewport-smoke';
const width = Number.parseInt(cli.get('--width') ?? process.env.VIEWPORT_WIDTH ?? '360', 10);
const height = Number.parseInt(cli.get('--height') ?? process.env.VIEWPORT_HEIGHT ?? '800', 10);
const deviceScaleFactor = Number.parseFloat(
  cli.get('--dpr') ?? process.env.DEVICE_SCALE_FACTOR ?? '3',
);
const preset = cli.get('--preset') ?? process.env.THEME_PRESET ?? '';

const assertions = [];
const consoleErrors = [];
const ignoredConsoleErrors = [];

function recordConsoleError(error) {
  if (error.text?.includes("/_dioxus?build_id=") && error.text?.includes('WebSocket connection')) {
    ignoredConsoleErrors.push({ ...error, reason: 'debug bundle live-reload endpoint is absent on static server' });
    return;
  }
  consoleErrors.push(error);
}

function assertThat(name, condition, details) {
  const result = { name, status: condition ? 'pass' : 'fail', details };
  assertions.push(result);
  if (!condition) {
    throw new Error(`${name}: ${JSON.stringify(details)}`);
  }
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

function setupUrl(seed, nextPath) {
  const params = new URLSearchParams({
    username: 'smoke',
    password: 'smoke-pass-123',
    seed,
    next: nextPath,
  });
  if (preset) {
    params.set('preset', preset);
  }
  return `${staticBase}/__codex/setup-local-auth?${params}`;
}

async function seedAndNavigate(client, seed, nextPath, marker) {
  await navigate(client, setupUrl(seed, nextPath));
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
          text: span.textContent,
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
    geometry.chip.height >= 44 &&
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

  const maskStates = await evaluate(
    client,
    `(async () => {
      const directory = document.querySelector('[data-layout="entry-top-directory"]');
      const maximum = directory.scrollWidth - directory.clientWidth;
      async function readAt(fraction) {
        directory.scrollLeft = maximum * fraction;
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
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
  await evaluate(client, `document.querySelector('[data-layout="entry-top-directory"]').scrollLeft = 0`);
  await captureArtifact(client, 'entries');
}

async function checkSettings(client) {
  await navigate(client, `${staticBase}/settings`);
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
  await navigate(client, `${staticBase}/feeds`);
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
}

async function checkReader(client) {
  await navigate(client, `${staticBase}/entries/2`);
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
          return { text: button.textContent.trim(), width: rect.width, height: rect.height };
        });
      const shortcuts = [...document.querySelectorAll('[data-slot="reader-bottom-bar-shortcut"]')]
        .map((element) => getComputedStyle(element).display);
      return {
        rootClientWidth: root.clientWidth,
        rootScrollWidth: root.scrollWidth,
        titleBoundsOk: titleRect.left >= rootRect.left - 1 && titleRect.right <= rootRect.right + 1,
        buttons,
        shortcuts,
      };
    })()`,
  );
  assertThat('reader title stays inside the viewport', evidence.titleBoundsOk, evidence);
  assertThat('reader page has no horizontal overflow', evidence.rootScrollWidth <= evidence.rootClientWidth + 1, evidence);
  assertThat(
    'reader bottom actions keep touch targets',
    evidence.buttons.length === 4 && evidence.buttons.every((button) => button.width >= 44 && button.height >= 44),
    evidence.buttons,
  );
  assertThat(
    'keyboard shortcuts stay hidden for touch emulation',
    evidence.shortcuts.length > 0 && evidence.shortcuts.every((display) => display === 'none'),
    evidence.shortcuts,
  );
  await captureArtifact(client, 'reader');
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
    `new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => {
      const directory = document.querySelector('[data-layout="entry-top-directory"]');
      const style = getComputedStyle(directory);
      resolve({
        chips: directory.querySelectorAll('[data-layout="entry-top-directory-chip"]').length,
        scrollWidth: directory.scrollWidth,
        clientWidth: directory.clientWidth,
        maskImage: style.maskImage || style.webkitMaskImage,
      });
    })))`,
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
  assertThat('desktop source row keeps touch target', evidence.chipHeight >= 44, evidence);
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
  const windowsVirtualKeyCode = ({Escape:27, Enter:13, Tab:9, Backspace:8})[key] ?? key.toUpperCase().charCodeAt(0);
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
  assertThat('shell icons fit and have labels titles and touch targets', result.fits && result.icons.every(x => x.label && x.title && x.width >= 44 && x.height >= 44), result);
  if (reader) assertThat('reader back is first in shell and old toolbar is absent', result.backFirst && !result.oldBack, result);
  return result;
}

async function waitForPaused(pending) {
  const deadline = Date.now() + 10000;
  while (!pending.length && Date.now() < deadline) await sleep(50);
  if (!pending.length) throw new Error('Refresh did not issue a feed request within 10 seconds');
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

async function checkHomeRefreshAndGestures(client) {
  let hold = false;
  let requestCount = 0;
  const requestsByFeed = new Map();
  const pending = [];
  const interceptErrors = [];
  const detach = client.on('Fetch.requestPaused', ({requestId, request}) => {
    requestCount++;
    const feed = new URL(request.url).searchParams.get('feed');
    requestsByFeed.set(feed, (requestsByFeed.get(feed) ?? 0) + 1);
    if (hold) pending.push(requestId);
    else client.send('Fetch.continueRequest', {requestId}).catch(error => interceptErrors.push(String(error)));
  });
  await client.send('Fetch.enable', {patterns: [{urlPattern: '*__codex/mobile-ui-feed.xml*', requestStage: 'Request'}]});
  try {
    await evaluate(client, `localStorage.setItem('rssr-nav-hidden', '1'); localStorage.removeItem('rssr-entry-search'); localStorage.setItem('rssr-entry-controls-hidden', '1')`);
    hold = true;
    await seedAndNavigate(client, 'home-reader', '/entries/2', '[data-slot="reader-body-html"]');
    // Join the real startup auto-refresh instead of starting another all-feed batch.
    await waitForPaused(pending);
    const autoRequests = requestCount;
    await tapSelector(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-page="entries"]');
    await tapSelector(client, '[data-action="activate-home"]');
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'refreshing'`);
    assertThat('Home joins an already running automatic refresh', requestCount === autoRequests, {autoRequests, requestCount});
    hold = false;
    await client.send('Fetch.continueRequest', {requestId: pending.shift()});
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
    await waitFor(client, `JSON.parse(localStorage.getItem('rssr-web-state-v1')).feeds.every(f => !f.last_fetched_at.startsWith('2099'))`);
    assertThat('automatic and manual refresh complete one shared batch',
      requestCount === 2 && requestsByFeed.size === 2 && [...requestsByFeed.values()].every(count => count === 1),
      {requestCount, requestsByFeed: Object.fromEntries(requestsByFeed)});
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-page="reader"]');
    const normalShell = await shellEvidence(client, true);
    const initialRequests = requestCount;
    await clickSelector(client, '[data-action="toggle-search"]');
    await selectorExists(client, '[data-field="entry-search"]');
    const searchShell = await shellEvidence(client, true);
    const animation = await evaluate(client, `getComputedStyle(document.querySelector('[data-layout="app-nav-search"]')).animationName`);
    assertThat('search reveal animation has a valid computed declaration', animation === 'search-reveal', {animation});
    assertThat('search mode preserves shell height', Math.abs(normalShell.height - searchShell.height) <= 1, {normalShell, searchShell});
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
    assertThat('Reader to Home only navigates', requestCount === initialRequests, {initialRequests, requestCount});
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
      assertThat(`${destination} to Home only navigates`, requestCount === initialRequests, {initialRequests, requestCount});
    }
    // Exercise the actual toolbar back, not only its geometry.
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-page="reader"]');
    await clickSelector(client, '[data-nav="back"]');
    await selectorExists(client, '[data-page="entries"]');

    const beforeRefresh = requestCount;
    hold = true;
    await clickSelector(client, '[data-action="activate-home"]');
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'refreshing'`);
    await waitForPaused(pending);
    await evaluate(client, `for (let i = 0; i < 5; i++) document.querySelector('[data-action="activate-home"]').click()`);
    await sleep(150);
    assertThat('repeated Home activation starts one batch', requestCount === beforeRefresh + 1 && pending.length === 1, {beforeRefresh, requestCount, pending: pending.length});
    await clickSelector(client, '[data-nav="feeds"]');
    await selectorExists(client, '[data-action="refresh-all"]');
    await clickSelector(client, '[data-action="refresh-all"]');
    assertThat('Feeds refresh uses the same in-flight gate', requestCount === beforeRefresh + 1, {requestCount});
    await clickSelector(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-slot="entry-card-title"]');
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-layout="reader-body"]');
    await evaluate(client, `window.__readerBeforeRefresh = document.querySelector('[data-layout="reader-body"]'); window.__readerTextBeforeRefresh = window.__readerBeforeRefresh.innerHTML; window.scrollTo(0, 80); window.__readerScroll = scrollY`);
    hold = false;
    await client.send('Fetch.continueRequest', {requestId: pending.shift()});
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
    assertThat('manual batch survives page unmount and completes every subscription', requestCount === beforeRefresh + 2, {beforeRefresh, requestCount});
    assertThat('refresh completion preserves Reader DOM and scroll position', await evaluate(client, `document.querySelector('[data-layout="reader-body"]') === window.__readerBeforeRefresh && window.__readerBeforeRefresh.innerHTML === window.__readerTextBeforeRefresh && Math.abs(scrollY - window.__readerScroll) <= 1`));
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
    const beforePull = requestCount;
    assertThat('short pull stays below threshold', (await touchPull(client, 145, 190)) !== 'armed');
    assertThat('short pull does not refresh', requestCount === beforePull, {beforePull, requestCount});
    await evaluate(client, 'window.scrollTo(0, 350)');
    await touchPull(client, 145, 260);
    assertThat('pull starting away from top does not refresh', requestCount === beforePull, {requestCount});
    await evaluate(client, 'window.scrollTo(0, 0)');
    hold = true;
    const phase = await touchPull(client, 145, 275);
    assertThat('top pull arms at threshold', phase === 'armed', {phase});
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'refreshing'`);
    await clickSelector(client, '[data-action="activate-home"]');
    assertThat('pull and Home share in-flight refresh', requestCount === beforePull + 1, {beforePull, requestCount});
    hold = false;
    await client.send('Fetch.continueRequest', {requestId: pending.shift()});
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);

    hold = true;
    await clickSelector(client, '[data-action="activate-home"]');
    await waitForPaused(pending);
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-layout="reader-body"]');
    await evaluate(client, `window.scrollTo(0, 80); window.__errorReaderScroll = scrollY; window.__errorReaderBody = document.querySelector('[data-layout="reader-body"]')`);
    hold = false;
    await client.send('Fetch.fulfillRequest', {requestId: pending.shift(), responseCode: 200,
      responseHeaders: [{name:'Content-Type',value:'application/rss+xml'}], body: Buffer.from('invalid RSS fixture').toString('base64')});
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'error'`);
    assertThat('refresh failure is exposed and releases the gate', await evaluate(client, `document.querySelector('[data-slot="manual-refresh-status"]').textContent.includes('失败')`));
    assertThat('refresh failure also preserves Reader position', await evaluate(client, `document.querySelector('[data-layout="reader-body"]') === window.__errorReaderBody && Math.abs(scrollY - window.__errorReaderScroll) <= 1`));
    await clickSelector(client, '[data-action="activate-home"]');
    await selectorExists(client, '[data-page="entries"]');
    await clickSelector(client, '[data-action="activate-home"]');
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
    assertThat('network interception completed cleanly', pending.length === 0 && interceptErrors.length === 0, {pending, interceptErrors});
  } finally {
    for (const requestId of pending) await client.send('Fetch.continueRequest', {requestId}).catch(() => {});
    await client.send('Fetch.disable');
    detach();
  }
}

async function selectorExistsOptional(client, selector) {
  return evaluate(client, `!!document.querySelector(${JSON.stringify(selector)})`);
}

async function checkReaderImagesAndSelection(client) {
  await seedAndNavigate(client, 'home-reader', '/entries/2', '[data-action="open-reader-image"]');
  await shellEvidence(client, true);
  assertThat('Reader has no pull gesture surface', !(await selectorExistsOptional(client, '[data-slot="pull-refresh"]')));
  const source = await evaluate(client, `(() => {
    const image = document.querySelector('[data-action="open-reader-image"]');
    image.scrollIntoView({block: 'center'});
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
  await client.send('Browser.grantPermissions', {origin: staticBase, permissions:['clipboardReadWrite','clipboardSanitizedWrite']});
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
  await evaluate(client, 'window.scrollTo(0, 120)');
  const homeScroll = await evaluate(client, 'scrollY');
  await clickSelector(client, imageEntry);
  await selectorExists(client, '[data-action="open-reader-image"]');
  const readerRoute = await evaluate(client, 'location.pathname');
  await evaluate(client, `document.querySelector('[data-action="open-reader-image"]').scrollIntoView({block:'center'})`);
  await tapSelector(client, '[data-action="open-reader-image"]');
  await waitFor(client, `document.querySelector('[data-layout="reader-image-viewer"]')?.open === true`);
  await evaluate(client, 'history.back()');
  await waitFor(client, `location.pathname === '/entries' && document.querySelector('[data-page="entries"]') && document.body.style.position !== 'fixed'`);
  // Wait past both the router RAF and the asynchronous viewer cleanup.
  await sleep(150);
  const afterBack = await evaluate(client, `({scroll:scrollY, position:document.body.style.position,
    top:document.body.style.top, viewer:!!document.querySelector('[data-layout="reader-image-viewer"]')})`);
  assertThat('leaving Reader with an open image releases lock and restores Home scroll',
    !afterBack.viewer && afterBack.position !== 'fixed' && afterBack.top === '' && Math.abs(afterBack.scroll - homeScroll) <= 1,
    {homeScroll, afterBack});
  await evaluate(client, 'history.forward()');
  await waitFor(client, `location.pathname === ${JSON.stringify(readerRoute)} && !!document.querySelector('[data-action="open-reader-image"]')`);
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
  await waitFor(client, `JSON.parse(localStorage.getItem('rssr-web-state-v1')).settings.reader_font_scale === 1.25`);
  // Reopen through a real persisted setting, not an injected CSS variable.
  await navigate(client, `${staticBase}/entries/2`);
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
  const before = await evaluate(client, `JSON.parse(localStorage.getItem('rssr-web-state-v1')).feeds.filter(f => !f.is_deleted).length`);
  const feedUrl = `${staticBase}/__codex/mobile-ui-feed.xml?seed=mobile-ui-short&feed=keyboard-submit`;
  await evaluate(client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
  await client.send('Input.insertText', {text:feedUrl});
  const focus = await evaluate(client, `(() => {
    const input = document.activeElement;
    const style = getComputedStyle(input);
    return {visible:input.matches(':focus-visible'), outline:style.outlineStyle, width:parseFloat(style.outlineWidth)};
  })()`);
  assertThat('subscription input has a visible keyboard focus indicator', focus.visible && focus.outline !== 'none' && focus.width >= 2, focus);
  await key(client, 'Enter', 'Enter');
  await waitFor(client, `JSON.parse(localStorage.getItem('rssr-web-state-v1')).feeds.filter(f => !f.is_deleted).length === ${before + 1}`);
  await waitFor(client, `document.querySelector('[data-field="feed-url-input"]').value === ''`);
  assertThat('Enter adds the subscription exactly once', await evaluate(client,
    `JSON.parse(localStorage.getItem('rssr-web-state-v1')).feeds.filter(f => !f.is_deleted && f.url === ${JSON.stringify(feedUrl)}).length === 1`));
  // Pause the actual first refresh so duplicate submission and draft editing have
  // deterministic overlap, without timing sleeps or changing production code.
  const slowFeedUrl = `${staticBase}/__codex/mobile-ui-feed.xml?seed=mobile-ui-short&feed=pending-submit`;
  const nextDraftUrl = `${staticBase}/__codex/mobile-ui-feed.xml?feed=next-draft`;
  const pausedRequests = [];
  const offPaused = client.on('Fetch.requestPaused', event => pausedRequests.push(event.requestId));
  await client.send('Fetch.enable', {patterns:[{urlPattern:`${slowFeedUrl}*`, requestStage:'Request'}]});
  try {
    await evaluate(client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
    await client.send('Input.insertText', {text:slowFeedUrl});
    const paused = client.waitForEvent('Fetch.requestPaused');
    await key(client, 'Enter', 'Enter');
    const request = await paused;
    await waitFor(client, `document.querySelector('[data-action="add-feed"]').disabled`);
    assertThat('pending subscription exposes busy feedback while the input stays editable',
      await evaluate(client, `document.querySelector('[data-action="add-feed"]').getAttribute('aria-busy') === 'true' && !document.querySelector('[data-field="feed-url-input"]').disabled`));
    await evaluate(client, `document.querySelector('[data-layout="feed-form"]').requestSubmit()`);
    await key(client, 'a', 'KeyA', 2);
    await client.send('Input.insertText', {text:nextDraftUrl});
    await evaluate(client, `document.querySelector('[data-layout="feed-form"]').requestSubmit()`);
    await client.send('Fetch.continueRequest', {requestId:request.requestId});
    await waitFor(client, `!document.querySelector('[data-action="add-feed"]').disabled`);
    assertThat('completed subscription keeps the next address typed during refresh',
      await evaluate(client, `document.querySelector('[data-field="feed-url-input"]').value === ${JSON.stringify(nextDraftUrl)}`));
    assertThat('repeated pending submits produce one refresh and do not add the next draft',
      pausedRequests.length === 1 && await evaluate(client,
        `JSON.parse(localStorage.getItem('rssr-web-state-v1')).feeds.filter(f => !f.is_deleted).length === ${before + 2}`), {requests:pausedRequests.length});
  } finally {
    offPaused();
    await client.send('Fetch.disable');
  }
  await evaluate(client, `document.querySelector('[data-field="feed-url-input"]').focus()`);
  await key(client, 'a', 'KeyA', 2);
  await client.send('Input.insertText', {text:'https://example.com/should-not-submit.xml'});
  await clickSelector(client, '[data-action="refresh-all"]');
  await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
  assertThat('refresh button does not submit the subscription form', await evaluate(client,
    `document.querySelector('[data-field="feed-url-input"]').value === 'https://example.com/should-not-submit.xml' && JSON.parse(localStorage.getItem('rssr-web-state-v1')).feeds.filter(f => !f.is_deleted).length === ${before + 2}`));
}

async function run() {
  await mkdir(artifactDir, { recursive: true });
  const page = await newPage('about:blank', cdpBase);
  const client = connect(page.webSocketDebuggerUrl);

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

  try {
    await client.send('Page.enable');
    await client.send('Runtime.enable');
    await client.send('Log.enable');
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
    await checkHomeRefreshAndGestures(client);
    await checkReaderImagesAndSelection(client);
    await checkReadingPreferencesAndFeedInput(client);
    await checkShortDirectory(client);
    await checkDesktop(client);

    assertThat('browser console has no errors', consoleErrors.length === 0, consoleErrors);
    await writeFile(
      path.join(artifactDir, 'assertions.json'),
      JSON.stringify({ status: 'pass', assertions, consoleErrors, ignoredConsoleErrors }, null, 2),
      'utf8',
    );
    await client.send('Page.close');
  } catch (error) {
    await captureArtifact(client, 'failure').catch(() => {});
    await writeFile(
      path.join(artifactDir, 'assertions.json'),
      JSON.stringify(
        {
          status: 'fail',
          error: error.stack ?? String(error),
          assertions,
          consoleErrors,
          ignoredConsoleErrors,
        },
        null,
        2,
      ),
      'utf8',
    );
    throw error;
  } finally {
    client.close();
  }
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
