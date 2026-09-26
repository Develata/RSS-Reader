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

// Inspect the currently published slice; alternating slots keep interrupted writes invisible.
const committedCore = `JSON.parse(localStorage.getItem('rssr-web-state-v1' +
  (JSON.parse(localStorage.getItem('rssr-web-commit-v1'))?.revisions[0] % 2 === 1 ? '-next' : '')))`;

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
// Native mode attaches only to an explicitly selected, already running Dioxus
// WebView with an isolated, pre-seeded SQLite fixture supplied by the caller.
const nativeTarget = cli.get('--native-target');
if (cli.has('--native-target') && !nativeTarget.trim()) {
  throw new Error('--native-target must identify an existing Dioxus WebView');
}
if (nativeTarget && ['--static-base', '--width', '--height', '--dpr', '--preset'].some(option => cli.has(option))) {
  throw new Error('Native mode records the real window; static seeding and viewport/theme overrides are not supported');
}

const assertions = [];
const consoleErrors = [];
const ignoredConsoleErrors = [];
const nativeMeasurements = [];
let nativeEvidence;

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

function setupUrl(seed, nextPath, themePreset = preset) {
  const params = new URLSearchParams({
    username: 'smoke',
    password: 'smoke-pass-123',
    seed,
    next: nextPath,
  });
  if (themePreset) {
    params.set('preset', themePreset);
  }
  return `${staticBase}/__codex/setup-local-auth?${params}`;
}

async function seedAndNavigate(client, seed, nextPath, marker, themePreset = preset) {
  await navigate(client, setupUrl(seed, nextPath, themePreset));
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
    evidence.buttons.length === 4 && evidence.buttons.every((button) => meetsTouchTarget(button.width, button.height)),
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
    assertThat('successful manual refresh shows a completion message', await evaluate(client,
      `/新增 [0-9]+ 篇文章|没有新文章/.test(document.querySelector('[data-slot="manual-refresh-status"]').textContent)`));
    await sleep(1150);
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'idle'`);
    assertThat('successful manual refresh feedback clears instead of returning on navigation', await evaluate(client,
      `document.querySelector('[data-slot="manual-refresh-status"]').textContent === '' && !/新增 [0-9]+ 篇文章|没有新文章/.test(document.querySelector('[data-action="activate-home"]').title)`));
    await waitFor(client, `${committedCore}.feeds.every(f => !f.last_fetched_at.startsWith('2099'))`);
    assertThat('automatic and manual refresh complete one shared batch',
      requestCount === 2 && requestsByFeed.size === 2 && [...requestsByFeed.values()].every(count => count === 1),
      {requestCount, requestsByFeed: Object.fromEntries(requestsByFeed)});
    await clickSelector(client, '[data-slot="entry-card-title"]');
    await selectorExists(client, '[data-page="reader"]');
    assertThat('finished refresh does not reappear when Reader mounts', await evaluate(client,
      `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'idle' && document.querySelector('[data-slot="manual-refresh-status"]').textContent === ''`));
    const normalShell = await shellEvidence(client, true);
    const initialRequests = requestCount;
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
    assertThat('Reader to Home only navigates', requestCount === initialRequests, {initialRequests, requestCount});
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
    await beginManualScroll(client);
    await evaluate(client, `window.__readerBeforeRefresh = document.querySelector('[data-layout="reader-body"]'); window.__readerTextBeforeRefresh = window.__readerBeforeRefresh.innerHTML; window.scrollTo(0, 80); window.__readerScroll = scrollY`);
    await checkReaderRefreshFeedback(client, 'refreshing while reading');
    hold = false;
    await client.send('Fetch.continueRequest', {requestId: pending.shift()});
    await waitFor(client, `document.querySelector('[data-action="activate-home"]').dataset.refreshState === 'finished'`);
    assertThat('manual batch survives page unmount and completes every subscription', requestCount === beforeRefresh + 2, {beforeRefresh, requestCount});
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
    await checkReaderRefreshFeedback(client, 'refresh failed while reading');
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
  await waitFor(client, `${committedCore}.settings.reader_font_scale === 1.25`);
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
  const before = await evaluate(client, `${committedCore}.feeds.filter(f => !f.is_deleted).length`);
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
  await waitFor(client, `${committedCore}.feeds.filter(f => !f.is_deleted).length === ${before + 1}`);
  await waitFor(client, `document.querySelector('[data-field="feed-url-input"]').value === ''`);
  assertThat('Enter adds the subscription exactly once', await evaluate(client,
    `${committedCore}.feeds.filter(f => !f.is_deleted && f.url === ${JSON.stringify(feedUrl)}).length === 1`));
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
        `${committedCore}.feeds.filter(f => !f.is_deleted).length === ${before + 2}`), {requests:pausedRequests.length});
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
    `document.querySelector('[data-field="feed-url-input"]').value === 'https://example.com/should-not-submit.xml' && ${committedCore}.feeds.filter(f => !f.is_deleted).length === ${before + 2}`));
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
    await navigate(client, `${staticBase}/entries/2`);
    await selectorExists(client, '[data-slot="reader-body-html"]');
    await clickSelector(client, '[data-action="toggle-search"]');
    await searchInputEvidence(client);
    if (themePreset.endsWith('-v1')) await captureArtifact(client, 'legacy-atlas-search-reader');
  }
}

async function run() {
  await mkdir(artifactDir, { recursive: true });
  let client;
  try {
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
    if (nativeTarget) {
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
      await checkHomeRefreshAndGestures(client);
      await checkReaderImagesAndSelection(client);
      await checkReadingPreferencesAndFeedInput(client);
      await checkShortDirectory(client);
      await checkDesktop(client);
      await checkNarrowSidebarSearch(client);
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
  }
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
