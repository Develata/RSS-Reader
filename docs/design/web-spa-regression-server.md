# Web SPA 回归服务脚本

## 用途

- 为 `rssr-app` 的 web 构建提供一个本地静态服务入口。
- 对未知路径自动回退到 `index.html`，避免直接访问 `/feeds`、`/settings`、`/entries` 时落到 404。
- 给 Chrome MCP、本地浏览器回归和 CSS 完全分离检查提供固定入口。
- 给同源本地 Web auth helper 提供承载入口：
  - `/__codex/setup-local-auth`

如果需要把发布前自动化门禁和静态 Web 回归串成一条命令，优先执行：

```bash
bash scripts/run_release_ui_regression.sh --debug --port 8091
```

这条脚本会先跑 `rssr-app / rssr-infra / rssr-web` 的发布前 UI 自动化门禁，再进入这里的 SPA fallback 服务。

如果还想同时串上 `rssr-web` 的最小部署壳 smoke，可以加：

```bash
bash scripts/run_release_ui_regression.sh --debug --port 8091 --with-rssr-web
```

对应日志和结果模板会落在：

- `target/release-ui-regression/<timestamp>/automated-gates.log`
- `target/release-ui-regression/<timestamp>/rssr-web.log`
- `target/release-ui-regression/<timestamp>/summary.md`

如果只想做静态 Web 的真实浏览器态 smoke，优先用：

```bash
bash scripts/run_static_web_browser_smoke.sh
```

## 脚本

- [run_web_spa_regression_server.sh](/home/develata/gitclone/RSS-Reader/scripts/run_web_spa_regression_server.sh)

## 用法

```bash
bash scripts/run_web_spa_regression_server.sh
```

默认行为：
- 构建 `rssr-app` web bundle
- 读取 `target/dx/rssr-app/debug/web/public`
- 在 `http://127.0.0.1:8091` 提供带 SPA fallback 的静态服务

可选参数：

```bash
bash scripts/run_web_spa_regression_server.sh --port 8092
bash scripts/run_web_spa_regression_server.sh --skip-build
bash scripts/run_web_spa_regression_server.sh --debug
bash scripts/run_web_spa_regression_server.sh --release
```

说明：

- 当前 `dx build --platform web --package rssr-app` 默认产出 `debug/web/public`
- 如果要检查 release 构建，应显式使用 `--release`

## 推荐回归路径

1. 启动脚本
2. 打开 `http://127.0.0.1:8091/`
3. 在浏览器内完成本地 Web 初始化登录
4. 通过应用内部导航检查：
   - `/entries`
   - `/feeds`
   - `/settings`

## 备注

- 这个脚本只解决本地静态构建回归的 SPA fallback 问题。
- 它不是 `dx serve` 的替代品。
- 如果要检查热更新或 dev server 行为，仍然应使用 `dx serve`。
