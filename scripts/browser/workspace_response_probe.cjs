// Test-only observation of the application's workspace response reader. Do not
// clone/consume the response or replace the promises returned to the application.
function installWorkspaceResponseProbe() {
  const records = [];
  const streams = new WeakMap(), readers = new WeakMap();
  const originalFetch = window.fetch;
  const originalGetReader = ReadableStream.prototype.getReader;
  const originalRead = ReadableStreamDefaultReader.prototype.read;
  const fetch = function(input, init) {
    const url = typeof input === 'string' ? input : input.url;
    const watched = new URL(url, location.href).pathname === '/api/v1/workspace'
      && (init?.method || input.method) === 'PUT';
    const result = originalFetch.apply(this, arguments);
    if (watched) {
      const record = { id: records.length + 1, chunks: [], done: false };
      records.push(record);
      const signal = init?.signal || input.signal;
      signal?.addEventListener('abort', () => {
        record.aborted = true;
        record.doneWhenAborted = record.done;
      }, { once: true });
      result.then(response => {
        record.status = response.status;
        if (response.body) streams.set(response.body, record);
      }, error => { record.fetchError = String(error); });
    }
    return result;
  };
  const getReader = function() {
    const reader = originalGetReader.apply(this, arguments);
    if (streams.has(this)) readers.set(reader, streams.get(this));
    return reader;
  };
  const read = function() {
    const result = originalRead.apply(this, arguments), record = readers.get(this);
    if (record) result.then(chunk => {
      record.chunks.push({ done: chunk.done, bytes: chunk.value?.length || 0,
        text: chunk.value ? new TextDecoder().decode(chunk.value).slice(0, 128) : '' });
      if (chunk.done) record.done = true;
    }, error => { record.readError = String(error); });
    return result;
  };
  window.fetch = fetch;
  ReadableStream.prototype.getReader = getReader;
  ReadableStreamDefaultReader.prototype.read = read;
  window.__workspaceResponseProbe = { records, dispose() {
    if (window.fetch === fetch) window.fetch = originalFetch;
    if (ReadableStream.prototype.getReader === getReader) ReadableStream.prototype.getReader = originalGetReader;
    if (ReadableStreamDefaultReader.prototype.read === read) ReadableStreamDefaultReader.prototype.read = originalRead;
    delete window.__workspaceResponseProbe;
  } };
}

module.exports = { installWorkspaceResponseProbe };
