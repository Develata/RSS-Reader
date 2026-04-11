const cdpBase = process.env.CDP_BASE ?? 'http://127.0.0.1:9225';
const staticBase = process.env.STATIC_BASE ?? 'http://127.0.0.1:8112';
const rssrWebBase = process.env.RSSR_WEB_BASE ?? 'http://127.0.0.1:18098';
const keepBrowserOpen = process.env.KEEP_BROWSER_OPEN === 'true';
const slowMs = Number.parseInt(process.env.SLOW_MS ?? '200', 10);

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

  ws.addEventListener('message', (event) => {
    const msg = JSON.parse(event.data);
    if (!msg.id || !pending.has(msg.id)) {
      return;
    }

    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
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

  async function send(method, params = {}) {
    await ready;
    const msgId = ++id;
    ws.send(JSON.stringify({ id: msgId, method, params }));
    return await new Promise((resolve, reject) => {
      pending.set(msgId, { resolve, reject });
    });
  }

  return {
    send,
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

async function textIncludes(client, text, timeoutMs = 20000) {
  return await waitFor(
    client,
    `document.body && document.body.innerText.includes(${JSON.stringify(text)})`,
    timeoutMs,
  );
}

async function navigate(client, url) {
  await client.send('Page.navigate', { url });
  await sleep(Math.max(1000, slowMs));
}

async function clickText(client, text) {
  const expression = `(() => {
    const target = [...document.querySelectorAll('button,a,[role="button"]')]
      .find((el) => (el.innerText || el.textContent || '').trim().includes(${JSON.stringify(text)}));
    if (!target) return false;
    target.click();
    return true;
  })()`;
  const result = await client.send('Runtime.evaluate', {
    expression,
    returnByValue: true,
  });
  if (!result.result.value) {
    throw new Error(`Could not click ${text}`);
  }
  await sleep(slowMs);
}

async function runStaticPageChecks(client) {
  const setup = `${staticBase}/__codex/setup-local-auth?username=smoke&password=smoke-pass-123&seed=reader-demo&next=/entries`;
  await navigate(client, setup);
  await textIncludes(client, 'Demo Entry Two');
  console.log('static entries: pass');

  await navigate(client, `${staticBase}/feeds`);
  await textIncludes(client, '新增订阅');
  await textIncludes(client, '已保存订阅');
  console.log('static feeds: pass');
}

async function runReaderThemeMatrix(client) {
  await navigate(client, `${staticBase}/settings`);
  await textIncludes(client, '主题实验室');

  for (const theme of [
    'Atlas Sidebar',
    'Newsprint',
    'Amethyst Glass',
    'Midnight Ledger',
  ]) {
    await clickText(client, theme);
    await textIncludes(client, `已应用示例主题：${theme}。`);
    await navigate(client, `${staticBase}/entries/2`);
    await textIncludes(client, 'Demo Entry Two');
    await textIncludes(client, 'Demo Entry Two body.');
    console.log(`theme reader ${theme}: pass`);
    await navigate(client, `${staticBase}/settings`);
    await textIncludes(client, '主题实验室');
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
    [`${staticBase}/entries`, 'Demo Entry Two'],
    [`${staticBase}/feeds`, '新增订阅'],
    [`${staticBase}/settings`, 'WebDAV 配置交换'],
    [`${staticBase}/entries/2`, 'Demo Entry Two body.'],
  ]) {
    await navigate(client, url);
    await textIncludes(client, marker);
    console.log(`small viewport ${url}: pass`);
  }

  await client.send('Emulation.clearDeviceMetricsOverride');
}

async function runRssrWebFeedSmoke(client) {
  await navigate(client, `${rssrWebBase}/__codex/browser-feed-smoke`);
  await textIncludes(client, '"status": "pass"', 30000);
  await textIncludes(client, 'Codex Smoke Entry', 30000);
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
