# Web SPA 回归服务脚本

## 用途

- 为 `rssr-app` 的 web 构建提供一个本地静态服务入口。
- 对未知路径自动回退到 `index.html`，避免直接访问 `/feeds`、`/settings`、`/entries` 时落到 404。
- 给 Chrome MCP、本地浏览器回归和 CSS 完全分离检查提供固定入口。

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
