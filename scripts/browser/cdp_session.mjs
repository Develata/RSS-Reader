const defaultCommandTimeoutMs = Number.parseInt(
  process.env.CDP_COMMAND_TIMEOUT_MS ?? '15000',
  10,
);
const defaultSlowMs = Number.parseInt(process.env.SLOW_MS ?? '200', 10);

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export async function newPage(
  url = 'about:blank',
  cdpBase = process.env.CDP_BASE ?? 'http://127.0.0.1:9225',
) {
  const response = await fetch(`${cdpBase}/json/new?${encodeURIComponent(url)}`, {
    method: 'PUT',
  });
  if (!response.ok) {
    throw new Error(`newPage failed ${response.status}: ${await response.text()}`);
  }
  return await response.json();
}

export function connect(wsUrl, commandTimeoutMs = defaultCommandTimeoutMs) {
  const ws = new WebSocket(wsUrl);
  let id = 0;
  const pending = new Map();
  const eventWaiters = new Map();
  const eventListeners = new Map();

  ws.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (message.method) {
      const listeners = eventListeners.get(message.method);
      if (listeners) {
        for (const listener of listeners) {
          listener(message.params ?? {});
        }
      }

      if (eventWaiters.has(message.method)) {
        const waiters = eventWaiters.get(message.method);
        eventWaiters.delete(message.method);
        for (const resolve of waiters) {
          resolve(message.params ?? {});
        }
      }
    }

    if (!message.id || !pending.has(message.id)) {
      return;
    }

    const { resolve, reject, timeout } = pending.get(message.id);
    pending.delete(message.id);
    clearTimeout(timeout);
    if (message.error) {
      reject(new Error(JSON.stringify(message.error)));
    } else {
      resolve(message.result);
    }
  });

  const ready = new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true });
    ws.addEventListener('error', reject, { once: true });
  });

  ws.addEventListener('close', () => {
    for (const [messageId, { reject, timeout, method }] of pending) {
      clearTimeout(timeout);
      reject(new Error(`CDP socket closed while waiting for ${method}#${messageId}`));
    }
    pending.clear();
  });

  async function send(method, params = {}) {
    await ready;
    const messageId = ++id;
    ws.send(JSON.stringify({ id: messageId, method, params }));
    return await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        pending.delete(messageId);
        reject(new Error(`Timed out waiting for CDP response ${method}#${messageId}`));
      }, commandTimeoutMs);
      pending.set(messageId, { method, resolve, reject, timeout });
    });
  }

  return {
    send,
    on(method, listener) {
      const listeners = eventListeners.get(method) ?? new Set();
      listeners.add(listener);
      eventListeners.set(method, listeners);
      return () => listeners.delete(listener);
    },
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

export async function evaluate(client, expression) {
  const result = await client.send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (result.exceptionDetails) {
    throw new Error(
      result.exceptionDetails.exception?.description ??
        result.exceptionDetails.text ??
        `Runtime.evaluate failed for ${expression}`,
    );
  }
  return result.result?.value;
}

export async function waitFor(client, expression, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await evaluate(client, expression);
    if (value) {
      return value;
    }
    await sleep(500);
  }
  throw new Error(`Timed out waiting for ${expression}`);
}

export async function selectorExists(client, selector, timeoutMs = 20000) {
  return await waitFor(
    client,
    `document.querySelector(${JSON.stringify(selector)}) !== null`,
    timeoutMs,
  );
}

export async function navigate(client, url, slowMs = defaultSlowMs) {
  const loaded = client.waitForEvent('Page.loadEventFired', 5000).catch(() => null);
  await client.send('Page.navigate', { url });
  await loaded;
  await sleep(slowMs);
}

export async function clickSelector(client, selector, slowMs = defaultSlowMs) {
  const clicked = await evaluate(
    client,
    `(() => {
      const target = document.querySelector(${JSON.stringify(selector)});
      if (!target) return false;
      target.click();
      return true;
    })()`,
  );
  if (!clicked) {
    throw new Error(`Could not click ${selector}`);
  }
  await sleep(slowMs);
}
