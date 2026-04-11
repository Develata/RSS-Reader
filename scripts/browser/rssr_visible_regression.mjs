const cdpBase = process.env.CDP_BASE ?? 'http://127.0.0.1:9225';
const staticBase = process.env.STATIC_BASE ?? 'http://127.0.0.1:8112';
const rssrWebBase = process.env.RSSR_WEB_BASE ?? 'http://127.0.0.1:18098';
const keepBrowserOpen = process.env.KEEP_BROWSER_OPEN === 'true';
const slowMs = Number.parseInt(process.env.SLOW_MS ?? '200', 10);
const cdpCommandTimeoutMs = Number.parseInt(process.env.CDP_COMMAND_TIMEOUT_MS ?? '15000', 10);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function newPage(url = 'about:blank') {
  const resp = await fetch(`${cdpBase}/json/new?${encodeURIComponent(url)}`, {
    method: 'PUT',
  });
  if (!resp.ok) {
    throw new Error(`newPage failed ${resp.status}: ${await resp.text()}`);
  }
  return await resp.json();
}

function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let id = 0;
  const pending = new Map();
  const eventWaiters = new Map();

  ws.addEventListener('message', (event) => {
    const msg = JSON.parse(event.data);
    if (msg.method && eventWaiters.has(msg.method)) {
      const waiters = eventWaiters.get(msg.method);
      eventWaiters.delete(msg.method);
      for (const resolve of waiters) {
        resolve(msg.params ?? {});
      }
    }

    if (!msg.id || !pending.has(msg.id)) {
      return;
    }

    const { resolve, reject, timeout } = pending.get(msg.id);
    pending.delete(msg.id);
    clearTimeout(timeout);
    if (msg.error) {
      reject(new Error(JSON.stringify(msg.error)));
    } else {
      resolve(msg.result);
    }
  });

  const ready = new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true });
    ws.addEventListener('error', reject, { once: true });
  });

  ws.addEventListener('close', () => {
    for (const [msgId, { reject, timeout, method }] of pending) {
      clearTimeout(timeout);
      reject(new Error(`CDP socket closed while waiting for ${method}#${msgId}`));
    }
    pending.clear();
  });

  async function send(method, params = {}) {
    await ready;
    const msgId = ++id;
    ws.send(JSON.stringify({ id: msgId, method, params }));
    return await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        pending.delete(msgId);
        reject(new Error(`Timed out waiting for CDP response ${method}#${msgId}`));
      }, cdpCommandTimeoutMs);
      pending.set(msgId, { method, resolve, reject, timeout });
    });
  }

  return {
    send,
    waitForEvent(method, timeoutMs = 20000) {
      return new Promise((resolve, reject) => {
        let timeout = null;
        const resolveOnce = (params) => {
          if (timeout !== null) {
            clearTimeout(timeout);
          }
          resolve(params);
        };
        const waiters = eventWaiters.get(method) ?? [];
        waiters.push(resolveOnce);
        eventWaiters.set(method, waiters);
        timeout = setTimeout(() => {
          const activeWaiters = eventWaiters.get(method);
          if (!activeWaiters) {
            return;
          }
          const index = activeWaiters.indexOf(resolveOnce);
          if (index >= 0) {
            activeWaiters.splice(index, 1);
          }
          if (activeWaiters.length === 0) {
            eventWaiters.delete(method);
          }
          reject(new Error(`Timed out waiting for CDP event ${method}`));
        }, timeoutMs);
      });
    },
    close: () => ws.close(),
  };
}

async function waitFor(client, expression, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await client.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (result.result?.value) {
      return result.result.value;
    }
    await sleep(500);
  }
  throw new Error(`Timed out waiting for ${expression}`);
}

async function selectorExists(client, selector, timeoutMs = 20000) {
  return await waitFor(
    client,
    `document.querySelector(${JSON.stringify(selector)}) !== null`,
    timeoutMs,
  );
}

async function navigate(client, url) {
  const loaded = client.waitForEvent('Page.loadEventFired', 5000).catch(() => null);
  await client.send('Page.navigate', { url });
  await loaded;
  await sleep(slowMs);
}

async function clickSelector(client, selector) {
  const expression = `(() => {
    const target = document.querySelector(${JSON.stringify(selector)});
    if (!target) return false;
    target.click();
    return true;
  })()`;
  const result = await client.send('Runtime.evaluate', {
    expression,
    returnByValue: true,
  });
  if (!result.result.value) {
    throw new Error(`Could not click ${selector}`);
  }
  await sleep(slowMs);
}

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

  for (const theme of ['atlas-sidebar', 'newsprint', 'forest-desk', 'midnight-ledger']) {
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
  const page = await newPage('about:blank');
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
