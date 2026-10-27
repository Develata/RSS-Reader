# `rssr-web` 浏览器自动 Feed Smoke

2026-10-22 核对 v0.2 C4c。使用真实页面、服务器与全新资料库，经真实登录表单添加固定本地订阅、刷新并进入订阅文章列表。

```bash
bash scripts/run_server_web_regression.sh --suite feed-smoke
bash scripts/run_server_web_regression.sh --suite feed-smoke --release --port 18089
```

页面、服务器和夹具工具由同一检出构建。feed fixture 是独立、无需登录的本地 HTTP 服务，服务器出站规则只放行它所在的 `127.0.0.1`；浏览器不直接抓取 feed，不使用静态注入 helper。`--skip-build` 仅复用页面，提交或源码变化后须重新构建。

成功条件由 [`rssr_small_viewport_assertions.mjs`](../../scripts/browser/rssr_small_viewport_assertions.mjs) 的 `feed-smoke` 套件定义：订阅已添加、刷新完成、对应文章列表包含夹具文章。结果以本次 `assertions.json` 的检查与控制台条目为准。

产物默认在 `target/server-web-regression/feed-smoke/<timestamp>/`：页面 HTML/PNG、`assertions.json`、`summary.md`、构建/Chrome/服务器/feed 日志。退出码必须为 0，意外控制台错误必须为零。

本地夹具只验证登录、页面到 API、服务器抓取和资料库存储的流程；不证明真实外部源的 DNS、出站策略或站点响应正常。真实远端源另按 [手工 Smoke](./rssr-web-browser-smoke.md) 留证。完整恢复验收用 `--suite acceptance --long`，见 [统一说明](./server-web-regression.md)。
