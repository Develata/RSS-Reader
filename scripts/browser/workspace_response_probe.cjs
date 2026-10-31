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
    let record;
    if (watched) {
      record = { id: records.length + 1, chunks: [], done: false };
      records.push(record);
      // Match the browser request by its complete payload, not by the order in
      // which DevTools happens to deliver events. Only the request is cloned.
      try {
        const body = init?.body;
        if (body !== undefined) {
          record.requestBody = typeof body === 'string' ? body : new TextDecoder().decode(body);
        } else {
          input.clone().text().then(text => { record.requestBody = text; },
            error => { record.requestBodyError = String(error); });
        }
      } catch (error) { record.requestBodyError = String(error); }
      const signal = init?.signal || input.signal;
      signal?.addEventListener('abort', () => {
        record.aborted = true;
        record.doneWhenAborted = record.done;
      }, { once: true });
    }
    const result = originalFetch.apply(this, arguments);
    if (record) {
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
      const bytes = chunk.value?.length || 0;
      if (bytes > 128) record.truncated = true;
      record.chunks.push({ done: chunk.done, bytes,
        // Keep a BOM as data; silently removing it could turn invalid JSON into
        // a boolean that the application's byte-based decoder never accepted.
        text: chunk.value ? new TextDecoder('utf-8', { ignoreBOM: true }).decode(chunk.value.subarray(0, 128)) : '' });
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

// The API returns a JSON bool. Headers, a persisted initial value, an unrelated
// response, or a DevTools terminal event cannot prove the app consumed that bool.
function workspaceResponseCompleted(records, requestBody) {
  if (typeof requestBody !== 'string') return false;
  const matches = records.filter(record => record.requestBody === requestBody);
  if (matches.length !== 1) return false;
  const record = matches[0];
  if (!(record.status >= 200 && record.status < 300) || record.done !== true
      || record.fetchError || record.readError || record.requestBodyError || record.truncated
      || (record.aborted && record.doneWhenAborted !== true)) return false;
  try { return typeof JSON.parse(record.chunks.map(chunk => chunk.text).join('')) === 'boolean'; }
  catch { return false; }
}

module.exports = { installWorkspaceResponseProbe, workspaceResponseCompleted };
