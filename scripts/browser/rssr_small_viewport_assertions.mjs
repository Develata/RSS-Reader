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
        .find((element) => element.querySelector('span')?.scrollWidth > element.querySelector('span')?.clientWidth);
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
    'long source chip stays single-line and truncated',
    geometry.chip.height <= 44 &&
      geometry.chip.scrollWidth > geometry.chip.clientWidth &&
      geometry.chip.whiteSpace === 'nowrap' &&
      geometry.chip.textOverflow === 'ellipsis',
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
      };
    })()`,
  );
  assertThat('feed titles stay inside the viewport', evidence.titleBoundsOk, evidence);
  assertThat('feeds page has no horizontal overflow', evidence.rootScrollWidth <= evidence.rootClientWidth + 1, evidence);
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
    '[data-layout="entry-top-directory"]',
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
  assertThat('desktop source chip remains compact', evidence.chipHeight <= 44, evidence);
  assertThat('desktop entries has no horizontal overflow', evidence.rootScrollWidth <= evidence.rootClientWidth + 1, evidence);
  await captureArtifact(client, 'entries-desktop');
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
