# Web 浏览器手工 Smoke：入口迁移

保留本文件名以兼容已有链接。v0.2 C4c 起正式 Web 通过同源 API 访问服务器资料库，旧静态本地口令 helper 已退役。

手工检查用：

```bash
bash scripts/run_rssr_web_browser_smoke.sh
```

经真实 `/login` 登录，检查 `/entries`、`/feeds`、`/settings`，添加订阅后再打开实际存在的 `/entries/:entry_id`。操作与结果模板见 [服务器手工 Smoke](./rssr-web-browser-smoke.md)。

自动化使用 [服务器浏览器回归](./server-web-regression.md)，每个场景有独立 SQLite 夹具资料库，不写浏览器文章库或口令。Pages 静态演示另用 `scripts/test_pages.sh`，见 [Pages 说明](../deployment/github-pages.md)；它仍使用 BrowserStore，但禁用在线订阅、WebDAV 与自动刷新。
