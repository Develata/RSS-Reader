const cdpBase = process.env.CDP_BASE ?? 'http://127.0.0.1:9225';
const staticBase = process.env.STATIC_BASE ?? 'http://127.0.0.1:8112';
const rssrWebBase = process.env.RSSR_WEB_BASE ?? 'http://127.0.0.1:18098';
const keepBrowserOpen = process.env.KEEP_BROWSER_OPEN === 'true';
import {
  clickSelector,
  connect,
  navigate,
  newPage,
  selectorExists,
} from './cdp_session.mjs';

async function runStaticPageChecks(client) {
  const setup = `${staticBase}/__codex/setup-local-auth?username=smoke&password=smoke-pass-123&seed=reader-demo&next=/entries`;
  await navigate(client, setup);
  await selectorExists(client, '[data-page="entries"][data-entry-scope="all"]');
  await selectorExists(client, '[data-layout="entries-layout"]');
  await selectorExists(
    client,
    '[data-layout="entry-groups"][data-state="populated"][data-grouping-mode] [data-layout="entry-list"][data-state="populated"] [data-slot="entry-card-title"]',
  );
  console.log('static entries: pass');

  await navigate(client, `${staticBase}/feeds`);
  await selectorExists(client, '[data-page="feeds"]');
  await selectorExists(client, '[data-layout="feed-workbench-single"]');
  await selectorExists(client, '[data-field="feed-url-input"]');
  await selectorExists(client, '[data-action="add-feed"]');
  await selectorExists(client, '[data-nav="feed-entries"]');
  await selectorExists(client, '[data-state="populated"]');
  console.log('static feeds: pass');
}

async function runReaderThemeMatrix(client) {
  await navigate(client, `${staticBase}/settings`);
  await selectorExists(client, '[data-page="settings"] [data-layout="settings-grid"]');
  await selectorExists(client, '[data-page="settings"] [data-layout="theme-lab"]');
  await selectorExists(client, '[data-page="settings"] [data-layout="theme-presets"]');
  await selectorExists(client, '[data-page="settings"] [data-layout="theme-gallery"]');
  await selectorExists(client, '[data-field="preset-theme-select"]');

  for (const theme of ['atlas-sidebar', 'newsprint', 'amethyst-glass', 'midnight-ledger']) {
    console.log(`theme reader ${theme}: start`);
    await clickSelector(client, `button[data-action="apply-theme-preset"][data-theme-preset="${theme}"]`);
    await selectorExists(client, `article[data-theme-preset="${theme}"][data-state="active"]`);
    await selectorExists(client, '#user-custom-css');
    await navigate(client, `${staticBase}/entries/2`);
    await selectorExists(client, '[data-page="reader"][data-state="loaded"]');
    await selectorExists(client, '[data-layout="reader-page"] [data-slot="reader-title"]');
    await selectorExists(client, '[data-layout="reader-body"][data-state] [data-slot^="reader-body-"]');
    console.log(`theme reader ${theme}: pass`);
    await navigate(client, `${staticBase}/settings`);
    await selectorExists(client, '[data-page="settings"] [data-layout="settings-grid"]');
  }
}

async function runSmallViewportChecks(client) {
  await client.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 1,
    mobile: false,
  });

  for (const [url, marker] of [
    [
      `${staticBase}/entries`,
      '[data-layout="entry-groups"][data-state="populated"] [data-layout="entry-list"] [data-slot="entry-card-title"]',
    ],
    [`${staticBase}/feeds`, '[data-page="feeds"] [data-field="feed-url-input"]'],
    [`${staticBase}/settings`, '[data-page="settings"] [data-layout="theme-presets"]'],
    [`${staticBase}/entries/2`, '[data-page="reader"] [data-layout="reader-body"] [data-slot^="reader-body-"]'],
  ]) {
    console.log(`small viewport ${url}: start`);
    await navigate(client, url);
    await selectorExists(client, marker);
    console.log(`small viewport ${url}: pass`);
  }

  await client.send('Emulation.clearDeviceMetricsOverride');
}

async function runRssrWebFeedSmoke(client) {
  await navigate(client, `${rssrWebBase}/__codex/browser-feed-smoke`);
  await selectorExists(
    client,
    '[data-smoke="rssr-web-browser-feed-smoke"][data-result="pass"]',
    30000,
  );
  console.log('rssr-web browser feed smoke: pass');
}

async function run() {
  const page = await newPage('about:blank', cdpBase);
  const client = connect(page.webSocketDebuggerUrl);

  try {
    await client.send('Page.enable');
    await client.send('Runtime.enable');
    await client.send('Emulation.clearDeviceMetricsOverride');

    await runStaticPageChecks(client);
    await runReaderThemeMatrix(client);
    await runSmallViewportChecks(client);
    await runRssrWebFeedSmoke(client);

    if (!keepBrowserOpen) {
      await client.send('Page.close');
    }
  } finally {
    client.close();
  }
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
