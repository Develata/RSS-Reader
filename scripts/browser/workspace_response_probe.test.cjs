const assert = require('node:assert/strict');
const test = require('node:test');
const vm = require('node:vm');
const { installWorkspaceResponseProbe } = require('./workspace_response_probe.cjs');

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
    assert.equal(window.fetch('/api/v1/workspace', { method: 'PUT', signal: controller.signal }), response);
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
  const end = reader.read();
  f.pending.shift().resolve({ done: true });
  await end;
  f.controller.abort();
  const record = f.window.__workspaceResponseProbe.records[0];
  assert.equal(record.done, true);
  assert.equal(record.doneWhenAborted, true);
  assert.equal(record.chunks[0].text, 'true');
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
  f.window.__workspaceResponseProbe.dispose();
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
