import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import test from 'node:test';
import { ApiTraffic } from './api_traffic.mjs';

async function recorder() {
  const client = new EventEmitter();
  client.send = async (method) => method === 'Page.getFrameTree'
    ? { frameTree: { frame: { id: 'main', loaderId: 'first' } } } : {};
  const traffic = await new ApiTraffic(client).start();
  const navigate = (loaderId, parentId) => client.emit('Page.frameNavigated',
    { frame: { id: parentId ? 'child' : 'main', loaderId, parentId } });
  const request = (requestId, loaderId = 'first', method = 'GET') => {
    client.emit('Network.requestWillBeSent', { requestId, loaderId,
      request: { method, url: 'http://localhost:8091/api/v1/settings' } });
    client.emit('Network.responseReceived', { requestId, response: { status: 200 } });
  };
  return { client, traffic, navigate, request };
}

const shortWait = { quietMs: 0, timeoutMs: 60 };

test('a frozen old document cannot block another document; history and writes remain', async () => {
  const { traffic, navigate, request } = await recorder();
  request('old-read');
  request('old-write', 'first', 'PATCH');
  navigate('second');
  await traffic.settled(shortWait);
  assert.equal(traffic.count(0, 'GET', '/api/v1/settings'), 1);
  assert.equal(traffic.writes(0).length, 1);
  assert.ok(traffic.entries.every(entry => !entry.done), 'a navigation must not invent terminal events');
});

test('200 headers do not finish a current request, even after an old document was retired', async () => {
  const { client, traffic, navigate, request } = await recorder();
  request('old');
  navigate('second');
  request('current', 'second');
  await assert.rejects(traffic.settled(shortWait), /request current, loader second/);
  client.emit('Network.loadingFinished', { requestId: 'current' });
  await traffic.settled(shortWait);
});

test('BFCache restoration waits for the original request until its real terminal event', async () => {
  const { client, traffic, navigate, request } = await recorder();
  request('restored');
  navigate('second');
  navigate('first');
  await assert.rejects(traffic.settled(shortWait), /request restored, loader first/);
  client.emit('Network.loadingFailed', { requestId: 'restored', errorText: 'net::ERR_ABORTED', canceled: true });
  await traffic.settled(shortWait);
});

test('a child navigation cannot retire the main document; unknown loaders still block', async () => {
  const { client, traffic, navigate, request } = await recorder();
  request('main-request');
  navigate('child-loader', 'main');
  await assert.rejects(traffic.settled(shortWait), /request main-request, loader first/);
  client.emit('Network.loadingFinished', { requestId: 'main-request' });
  request('unknown', 'worker-or-unseen-frame');
  navigate('second');
  await assert.rejects(traffic.settled(shortWait), /request unknown, loader worker-or-unseen-frame/);
});

test('a request that starts and ends between polls resets the full quiet window', async () => {
  const { client, traffic, request } = await recorder();
  const waiting = traffic.settled({ quietMs: 600, timeoutMs: 3000 });
  let ended;
  const quickRequest = new Promise(resolve => setTimeout(() => {
    request('quick');
    client.emit('Network.loadingFinished', { requestId: 'quick' });
    ended = Date.now();
    resolve();
  }, 400));
  await quickRequest;
  await waiting;
  assert.ok(Date.now() - ended >= 600, 'all 600 ms must follow the last request event');
});
