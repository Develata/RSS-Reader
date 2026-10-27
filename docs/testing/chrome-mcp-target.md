# Chrome MCP 目标浏览器

这份说明固定了本地 Chrome DevTools MCP 要连接的浏览器启动方式，避免每次手工拼：

- `--remote-debugging-port`
- `--user-data-dir`
- `--no-sandbox`
- `--disable-dev-shm-usage`

## 适用场景

- 需要用 Chrome MCP 实际点开 `/entries`、`/feeds`、`/settings`、阅读页 `/entries/:entry_id`
- 需要在本地检查正式 Web 服务器模式，或单独构建的 Pages 演示
- 当前 `127.0.0.1:9222` 没有 DevTools 目标，或者 MCP 连不上浏览器

## 固定入口

默认端口是 `9222`：

```bash
bash scripts/run_chrome_mcp_target.sh
```

如果要强制重启已有实例：

```bash
bash scripts/run_chrome_mcp_target.sh --restart
```

如果不想占用默认端口，可以改端口：

```bash
bash scripts/run_chrome_mcp_target.sh --port 9223 --profile-dir target/chrome-mcp-profile-9223
```

## 成功标准

脚本成功后会打印：

- Chrome 可执行文件路径
- profile 目录
- log 文件
- `http://127.0.0.1:<port>/json/version`

同时该 URL 应能直接返回 JSON，其中包含：

- `Browser`
- `webSocketDebuggerUrl`

## 常见问题

### 1. MCP 报无法连接 `127.0.0.1:9222`

先确认：

```bash
curl http://127.0.0.1:9222/json/version
```

如果失败，先跑：

```bash
bash scripts/run_chrome_mcp_target.sh --restart
```

### 2. 端口已占用但 `/json/version` 不可达

说明旧浏览器进程或 profile 有残留。优先用：

```bash
bash scripts/run_chrome_mcp_target.sh --restart
```

### 3. 想用仓库内的 Chrome for Testing，而不是系统 Chrome

可以显式指定：

```bash
bash scripts/run_chrome_mcp_target.sh --chrome-bin "$HOME/.local/opt/chrome-for-testing/chrome"
```

## 推荐搭配

- 正式 Web（真实服务器与资料库）：
  - [服务器浏览器回归](./server-web-regression.md)
  - [Web 阅读页 `/entries/:entry_id` 主题矩阵 Smoke](./static-web-reader-theme-matrix.md)
  - [Web 小视口 Smoke](./static-web-small-viewport-smoke.md)
  - [`rssr-web` 浏览器手工 Smoke](./rssr-web-browser-smoke.md)
  - [`rssr-web` 浏览器自动 Feed Smoke](./rssr-web-browser-feed-smoke.md)
- 静态演示：[Pages 构建与验收](../deployment/github-pages.md)。
