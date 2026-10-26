// Isolated Playwright fixture: no HTTP listener, personal profile or external feeds.
const fs = require('node:fs/promises');
const path = require('node:path');
const crypto = require('node:crypto');
const origin = 'http://127.0.0.1:8119';
const keys = ['rssr-web-state-v1', 'rssr-web-app-state-v2', 'rssr-web-entry-flags-v1', 'rssr-web-entry-content-v1'];
exports.origin = origin;
exports.install = async (context, repo) => {
  const publicRoot = path.resolve(repo, process.env.PUBLIC_DIR || 'target/dx/rssr-app/release/web/public');
  const hash = s => crypto.createHash('sha256').update(s).digest('base64url');
  await context.route(origin + '/**', async route => {
    const url = new URL(route.request().url());
    if (url.pathname === '/__codex/setup-local-auth') {
      const states = await Promise.all(['core', 'app_state', 'entry_flags', 'entry_content'].map(async suffix =>
        JSON.parse(await fs.readFile(path.join(repo, 'tests/fixtures/browser_state/directory_contract_' + suffix + '.json'), 'utf8'))));
      const [core, app, flags, content] = states;
      core.feeds.forEach(feed => { feed.url = origin + '/__codex/feed/' + feed.id; });
      if (url.searchParams.get('seed') === 'directory-window') {
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
      const salt = hash('smoke:codex-static-smoke'), passwordHash = hash('smoke\nsmoke-pass-123\n' + salt);
      const next = url.searchParams.get('next') || '/entries';
      if (!next.startsWith('/') || next.startsWith('//')) throw new Error('Fixture next must be local');
      await route.fulfill({contentType:'text/html; charset=utf-8', body:`<!doctype html><meta charset="utf-8"><script>
        localStorage.clear(); sessionStorage.clear();
        localStorage.setItem('rssr-web-auth-config-v1', ${JSON.stringify('smoke\n' + passwordHash + '\n' + salt)});
        sessionStorage.setItem('rssr-web-auth-session-v1', ${JSON.stringify(hash('smoke:' + passwordHash))});
        ${keys.map((key,i)=>`localStorage.setItem(${JSON.stringify(key)},${JSON.stringify(JSON.stringify(states[i]))});`).join('\n')}
        location.replace(${JSON.stringify(next)});
      </script>`});
      return;
    }
    if (url.pathname.startsWith('/__codex/feed/')) { await route.fulfill({status:304}); return; }
    if (url.pathname.startsWith('/api/')) { await route.fulfill({status:404}); return; }
    const requested = path.resolve(publicRoot, '.' + decodeURIComponent(url.pathname));
    if (requested !== publicRoot && !requested.startsWith(publicRoot + path.sep)) throw new Error('Outside fixture root');
    const file = await fs.stat(requested).then(s=>s.isFile()?requested:null).catch(()=>null) || path.join(publicRoot,'index.html');
    const mime = {'.js':'text/javascript','.css':'text/css','.wasm':'application/wasm','.html':'text/html','.svg':'image/svg+xml','.png':'image/png','.ico':'image/x-icon','.woff2':'font/woff2'};
    await route.fulfill({body:await fs.readFile(file),contentType:mime[path.extname(file)] || 'application/octet-stream'});
  });
};
