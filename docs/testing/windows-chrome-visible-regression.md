# Windows Chrome 可见窗口回归

这份说明固定 Windows 原生 Chrome 可见窗口的本地回归入口。

它解决的是 WSL 开发环境下的两个限制：

- WSLg/Linux Chrome 窗口可能不可见、最小化或呈现异常。
- 当前 `mcp__chrome` 会话通常连接 WSL 侧 `127.0.0.1:9222`，不能稳定接管 Windows 侧 localhost 上的 Chrome DevTools 端口。

## 固定入口

- [run_windows_chrome_visible_regression.sh](/home/develata/gitclone/RSS-Reader/scripts/run_windows_chrome_visible_regression.sh)
- [rssr_visible_regression.mjs](/home/develata/gitclone/RSS-Reader/scripts/browser/rssr_visible_regression.mjs)

## 最短用法

```bash
bash scripts/run_windows_chrome_visible_regression.sh --skip-build
```

默认行为：

- 启动带 SPA fallback 的静态 Web 服务，默认端口 `8112`
- 启动 `rssr-web` smoke helper，默认端口 `18098`
- 通过 PowerShell 启动 Windows 原生 Chrome 可见窗口，默认 CDP 端口 `9225`
- 在 Windows Node 中运行 repo 内的 CDP runner
- 生成 `target/windows-chrome-visible-regression/<timestamp>/summary.md`

如果静态服务和 `rssr-web` 已经运行，可以只跑浏览器动作：

```bash
bash scripts/run_windows_chrome_visible_regression.sh \
  --use-existing-servers \
  --keep-browser-open \
  --slow-ms 150
```

## `~/codex-browser-check`

脚本默认使用 `~/codex-browser-check` 作为 Windows Node 的工作目录。

这个目录不承载项目逻辑；项目逻辑保留在 repo 内的 `scripts/browser/rssr_visible_regression.mjs`。这样做的边界是：

- repo 负责测试动作和断言
- `~/codex-browser-check` 负责提供一个稳定的本机浏览器检查工作目录
- Windows Chrome 由 PowerShell 启动，浏览器控制通过 Windows localhost CDP 完成

## 覆盖范围

当前 runner 覆盖：

- 静态 `/entries`
- 静态 `/feeds`
- settings 主题实验室到 `/entries/2` 阅读页的主题矩阵
- 小视口 `390x844` 下的 `/entries`、`/feeds`、`/settings`、`/entries/2`
- `rssr-web` 的 `__codex/browser-feed-smoke` 浏览器态 helper

## 和 Chrome MCP 的区别

Chrome MCP 路径：

```text
Codex -> mcp__chrome -> MCP server -> WSL DevTools endpoint
```

Windows 可见窗口路径：

```text
Codex -> bash -> PowerShell -> Windows Chrome -> Windows Node/CDP
```

后者不是 MCP 工具直连。它的目标是稳定复现用户可见的 Windows Chrome 浏览器回归。

如果后续必须让 `mcp__chrome` 工具直接接管 Windows Chrome，需要单独建立 WSL `9222` 到 Windows `9225` 的 CDP bridge，或在 Windows 侧启动 MCP server。

## 成功标准

脚本成功时应打印：

- `static entries: pass`
- `static feeds: pass`
- `theme reader <preset>: pass`
- `small viewport <url>: pass`
- `rssr-web browser feed smoke: pass`
- `Windows Chrome visible regression passed`

## 已知边界

- 这不是像素级视觉回归。
- 这不是外网真实 feed 长链路压力测试。
- 当前断言仍有一部分依赖页面中文文案；后续应继续迁到 `data-page`、`data-layout`、`data-action`、`data-state` 等 headless active interface。
