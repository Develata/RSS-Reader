// Isolated real-server fixture for the Playwright directory/workspace contracts.
// Requires the page, rssr-web and browser_fixture_library from the same build.
const path = require('node:path');
const fs = require('node:fs/promises');
const origin = 'http://127.0.0.1:8119';
const contexts = new WeakMap();
exports.origin = origin;
exports.get = page => contexts.get(page.context());
exports.install = async (context, repo) => {
  const { startHarness } = await import('./server_harness.mjs');
  const artifacts = path.resolve(process.env.ARTIFACT_DIR || 'target/workspace-server-regression');
  await fs.mkdir(artifacts, { recursive: true });
  const suffix = process.platform === 'win32' ? '.exe' : '';
  const target = process.env.CARGO_TARGET_DIR || path.join(repo, 'target');
  const harness = await startHarness({
    serverBin: process.env.RSSR_WEB_BIN || path.join(target, 'debug/rssr-web' + suffix),
    fixtureBin: process.env.RSSR_FIXTURE_BIN || path.join(target, 'debug/examples/browser_fixture_library' + suffix),
    staticDir: path.resolve(repo, process.env.PUBLIC_DIR || 'target/dx/rssr-app/release/web/public'),
    seedsDir: path.join(repo, 'tests/fixtures/browser_state'), themesDir: path.join(repo, 'assets/themes'),
    workDir: path.join(artifacts, 'server-work'), artifactDir: artifacts,
    port: 8119, username: 'smoke', password: 'smoke-pass-123',
  });
  contexts.set(context, harness);
  context.on('close', () => { void harness.close().catch(console.error); });
  await context.route(origin + '/__codex/setup-local-auth**', async route => {
    const url = new URL(route.request().url());
    const windowSeed = url.searchParams.get('seed') === 'directory-window';
    const seed = windowSeed ? 'directory-contract' : url.searchParams.get('seed');
    const session = await harness.server.useSeed(seed, { edit: ({core, app_state: app, entry_flags: flags, entry_content: content}) => {
      if (url.searchParams.get('workspace') === 'true') {
        core.settings.entries_page_size = 8;
        core.settings.archive_after_months = 120;
        app.entries_workspace.show_archived = false;
      }
      if (url.searchParams.get('long-title') === 'true') core.feeds[0].title = 'LongSourceName'.repeat(35);
      if (windowSeed) {
        const template = core.entries[0];
        const multiple = url.searchParams.get('branches') === '2';
        core.feeds = core.feeds.slice(0, multiple ? 2 : 1);
        core.entries = Array.from({length:600}, (_, i) => {
          const id = 600 - i;
          return {...template, id, feed_id:multiple && i >= 300 ? 2 : 1, external_id:'window-' + id, dedup_key:'window-' + id,
            url:origin + '/article/' + id, title:`Window ${id} ` + (id % 3 === 0 ? '长标题检验自然换行与变高的目录条目 '.repeat(4) : '短标题'),
            published_at:`2026-09-02 12:${String(Math.floor(id/60)).padStart(2,'0')}:${String(id%60).padStart(2,'0')}.0 +00:00:00`};
        });
        core.next_entry_id = 601;
        core.settings.entries_page_size = 50;
        flags.entries = [];
        content.entries = [];
        app.entries_workspace.grouping_mode = url.searchParams.get('group') || 'source';
        app.entries_workspace.show_archived = url.searchParams.get('archived') !== 'false';
      }
    }});
    const split = session.cookie.indexOf('=');
    await context.addCookies([{name:session.cookie.slice(0,split), value:session.cookie.slice(split+1), url:origin, httpOnly:true, sameSite:'Strict'}]);
    const next = url.searchParams.get('next') || '/entries';
    if (!next.startsWith('/') || next.startsWith('//')) throw new Error('Fixture next must be local');
    // A new fixture library starts with an empty device search. Ordinary reloads
    // keep device preferences and the server's persisted workspace unchanged.
    await route.fulfill({contentType:'text/html; charset=utf-8', body:`<!doctype html><meta charset="utf-8"><script>localStorage.removeItem('rssr-entry-search');location.replace(${JSON.stringify(next)});</script>`});
  });
};
