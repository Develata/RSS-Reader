# Web 阅读页主题矩阵 Smoke

保留旧文件名以兼容引用。v0.2 C4c 起在真实 `rssr-web` 上运行，使用服务器夹具资料库和真实登录；统一入口见 [服务器浏览器回归](./server-web-regression.md)。

```bash
bash scripts/run_server_web_regression.sh --suite theme-matrix
bash scripts/run_server_web_regression.sh --suite theme-matrix --release --port 8103
```

默认依次检查默认主题、Atlas Sidebar、Newsprint、Amethyst Glass、Midnight Ledger 的真实 `/entries/2` 阅读页，保存 HTML、PNG、`assertions.json` 与 `summary.md`，以及构建、服务器与浏览器日志。默认产物位于 `target/server-web-regression/theme-matrix/<timestamp>/`。

`--skip-build` 只复用已有页面；服务器与夹具工具仍会构建。三者应来自同一源码版本，页面请求标识必须与服务器完整相等；换 commit 或 feature 后重建页面。

固定断言检查阅读页语义、标题与主题 CSS 已注入；人工复看标题、元信息、正文和底部栏是否可读、有无遮挡。默认矩阵不穷举浅/深色、跟随系统、全部视口或键盘状态；不能用结构断言代替视觉验收。

历史证据：2026-04-10 的五主题人工结论仅适用于当时构建（`target/static-web-reader-theme-matrix/20260410-213206/`）。后续记录见 [2026-09-26 集成复验](../handoffs/archive/2026-09/2026-09-26-integration-revalidation.md)；每次新构建都要检查自己的产物。
