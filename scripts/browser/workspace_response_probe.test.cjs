const assert = require('node:assert/strict');
const test = require('node:test');
const vm = require('node:vm');
const { installWorkspaceResponseProbe, workspaceResponseCompleted } = require('./workspace_response_probe.cjs');

const payload = JSON.stringify({ read_filter: 'all', grouping_mode: 'time' });

function fixture() {
  const pending = [];
  class Reader {
    reads = [];
    read() {
      const promise = new Promise((resolve, reject) => pending.push({ resolve, reject }));
      this.reads.push(promise);
      return promise;
    }
  }
  class Stream {
    reader = new Reader();
    getReader() { return this.reader; }
  }
  const stream = new Stream();
  const response = Promise.resolve({ status: 200, body: stream });
  const fetch = () => response;
  const read = Reader.prototype.read, getReader = Stream.prototype.getReader;
  const window = { fetch };
  vm.runInNewContext(`(${installWorkspaceResponseProbe})()`, {
    window, location: { href: 'http://localhost/entries' }, URL, TextDecoder,
    ReadableStream: Stream, ReadableStreamDefaultReader: Reader,
  });
  const controller = new AbortController();
  const start = async () => {
    assert.equal(window.fetch('/api/v1/workspace', { method: 'PUT', body: payload, signal: controller.signal }), response);
    return (await response).body.getReader();
  };
  return { window, stream, response, start, pending, controller, fetch, read, getReader, Reader, Stream };
}

test('observes the original reader without consuming extra bytes or replacing promises', async () => {
  const f = fixture(), reader = await f.start();
  assert.equal(reader.reads.length, 0);
  const first = reader.read();
  assert.equal(first, reader.reads[0]);
  const bytes = new TextEncoder().encode('true');
  f.pending.shift().resolve({ done: false, value: bytes });
  assert.equal((await first).value, bytes);
  assert.equal(f.window.__workspaceResponseProbe.records[0].done, false);
  assert.equal(workspaceResponseCompleted(f.window.__workspaceResponseProbe.records, payload), false);
  const end = reader.read();
  f.pending.shift().resolve({ done: true });
  await end;
  f.controller.abort();
  const record = f.window.__workspaceResponseProbe.records[0];
  assert.equal(record.done, true);
  assert.equal(record.doneWhenAborted, true);
  assert.equal(record.chunks[0].text, 'true');
  assert.equal(workspaceResponseCompleted(f.window.__workspaceResponseProbe.records, payload), true);
  assert.equal(reader.reads.length, 2);
  f.window.__workspaceResponseProbe.dispose();
  assert.equal(f.window.fetch, f.fetch);
  assert.equal(f.Reader.prototype.read, f.read);
  assert.equal(f.Stream.prototype.getReader, f.getReader);
});

test('headers and cancellation do not imply a complete application read', async () => {
  const f = fixture(), reader = await f.start();
  const result = reader.read();
  f.controller.abort();
  f.pending.shift().reject(new Error('body truncated'));
  await assert.rejects(result, /body truncated/);
  const record = f.window.__workspaceResponseProbe.records[0];
  assert.equal(record.status, 200);
  assert.equal(record.done, false);
  assert.equal(record.doneWhenAborted, false);
  assert.match(record.readError, /body truncated/);
  assert.equal(workspaceResponseCompleted(f.window.__workspaceResponseProbe.records, payload), false);
  f.window.__workspaceResponseProbe.dispose();
});

test('completion requires one matching successful boolean body, not an initial value or headers', () => {
  const complete = { requestBody: payload, status: 200, done: true,
    chunks: [{ text: 'true' }, { text: '' }], aborted: true, doneWhenAborted: true };
  assert.equal(workspaceResponseCompleted([], payload), false, 'the final PUT was never issued');
  assert.equal(workspaceResponseCompleted([{ ...complete, requestBody: 'different choice' }], payload), false);
  assert.equal(workspaceResponseCompleted([complete, complete], payload), false, 'duplicate payloads are ambiguous');
  for (const invalid of [
    { status: 503 }, { done: false }, { readError: 'truncated' }, { fetchError: 'failed' },
    { requestBodyError: 'unreadable' }, { truncated: true }, { doneWhenAborted: false },
    { chunks: [{ text: 'tr' }] }, { chunks: [{ text: '{"error":"failed"}' }] },
  ]) assert.equal(workspaceResponseCompleted([{ ...complete, ...invalid }], payload), false, JSON.stringify(invalid));
  assert.equal(workspaceResponseCompleted([{ ...complete, chunks: [{ text: 'false' }] }], payload), true);
});

test('ignores unrelated reads and does not overwrite another observer during cleanup', async () => {
  const f = fixture();
  await f.window.fetch('/api/v1/workspace', { method: 'GET' });
  assert.equal(f.window.__workspaceResponseProbe.records.length, 0);
  const replacement = () => {};
  f.window.fetch = replacement;
  f.window.__workspaceResponseProbe.dispose();
  assert.equal(f.window.fetch, replacement);
  assert.equal(f.window.__workspaceResponseProbe, undefined);
});
