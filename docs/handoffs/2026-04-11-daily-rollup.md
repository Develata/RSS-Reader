# 2026-04-11 Daily Rollup

- 日期：2026-04-11
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：2379557
- 相关 commit：2189d8b / b914f4c / 2379557 / pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成一轮 Windows 原生 Chrome 可见窗口回归验证，确认当前 Web SPA 与 `rssr-web` 浏览器态 smoke 路径可在用户可见的 Windows Chrome 窗口内通过。
随后将该验证路径固化为 repo 内脚本与文档，避免继续依赖 `target/` 临时 runner。
继续把可见浏览器 runner 从中文文案断言迁到 `data-*` 语义接口。
继续补齐页面层语义接口，让 settings themes、entries groups、reader body 都暴露更稳定的 headless active interface。

## 影响范围

- 模块：`scripts/run_web_spa_regression_server.sh`、`scripts/run_windows_chrome_visible_regression.sh`、`scripts/browser/rssr_visible_regression.mjs`、Web SPA 静态回归路径
- 平台：Windows Chrome、Web、WSL 开发环境
- 额外影响：browser regression workflow / handoff docs

## 关键变更

### Windows Chrome 可见验证

- 使用 Windows 原生 Chrome，而不是 WSLg/Linux Chrome 或 headless Chrome。
- Windows Chrome 启动参数包含独立调试 profile、`--remote-debugging-port=9225`、`--remote-allow-origins=*`。
- 由于 Windows Chrome DevTools 端口绑定在 Windows localhost，当前 WSL 内固定到 `127.0.0.1:9222` 的 Chrome MCP 会话不能直接接管该窗口；本轮改用 Windows 侧 Node/CDP 驱动可见窗口完成实际验证。

### 回归覆盖

- 静态 SPA server：`http://127.0.0.1:8112`
- `rssr-web` smoke helper：`http://127.0.0.1:18098/__codex/browser-feed-smoke`
- 覆盖页面：`/entries`、`/feeds`、`/settings`、`/entries/2`
- 覆盖主题：Atlas Sidebar、Newsprint、Amethyst Glass、Midnight Ledger
- 覆盖视口：默认桌面视口、小视口 `390x844`

### 固定工具化

- 新增 `scripts/run_windows_chrome_visible_regression.sh`，负责从 WSL 编排静态 SPA server、`rssr-web`、Windows Chrome 和 Windows Node/CDP runner。
- 新增 `scripts/browser/rssr_visible_regression.mjs`，集中维护浏览器动作与断言，不再把大段 JS 写在 shell heredoc 内。
- 新增 `docs/testing/windows-chrome-visible-regression.md`，说明 Windows visible Chrome 路径与 Chrome MCP 路径的控制链路差异。
- 更新 `docs/testing/README.md` 与 `docs/design/web-spa-regression-server.md`，把 Windows visible Chrome 纳入推荐回归入口。

### Selector-first 断言

- `scripts/browser/rssr_visible_regression.mjs` 不再通过页面中文文案点击主题或判断核心页面。
- 静态页面断言改为检查 `data-page`、`data-layout`、`data-field`、`data-action`、`data-nav`、`data-state`。
- 主题矩阵改为通过 `data-theme-preset`、`data-state="active"` 与 `#user-custom-css` 判断主题应用。
- `rssr-web` feed smoke 改为通过 `data-smoke="rssr-web-browser-feed-smoke"` 与 `data-result="pass"` 判断结果。

### 页面语义接口补齐

- settings themes：`theme-lab`、`theme-presets`、`theme-preset-selector`、`theme-preset-quick-actions`、`theme-gallery`、`theme-card` 等区域补齐 `data-layout` / `data-section` / `data-slot`。
- entries groups：列表容器补齐 `data-state="populated"` 与 `data-grouping-mode`；分组、日期组、来源组和列表补齐 `data-layout`、`data-group-level`、`data-slot`。
- reader body：HTML / text 正文分别补齐 `data-slot="reader-body-html"` 与 `data-slot="reader-body-text"`。
- 可见浏览器 runner 更新为使用这些更细的语义 selector。

## 验证与验收

### 自动化验证

- `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8112`：通过，静态 SPA fallback server 可服务多路由。
- `cargo run -p rssr-web` with smoke env on `127.0.0.1:18098`：通过，`rssr-web` smoke helper 可访问。
- Windows Node/CDP visible Chrome regression script：通过。
- `scripts/run_windows_chrome_visible_regression.sh --use-existing-servers --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-082128/summary.md`。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8114 --rssr-web-port 18104 --chrome-port 9225 --skip-build --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-083451/summary.md`。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `node --check scripts/browser/rssr_visible_regression.mjs`：通过。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8120 --rssr-web-port 18110 --chrome-port 9225 --skip-build --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-084846/summary.md`。

### 手工验收

- Windows 原生 Chrome 可见窗口：通过。
- 静态 `/entries` 页面：通过。
- 静态 `/feeds` 页面：通过。
- `/reader` 实页多主题切换：通过。
- 小视口 `/entries`、`/feeds`、`/settings`、`/entries/2`：通过。
- `rssr-web` 浏览器态真实 feed smoke helper：通过。

## 结果

- 当前 Web SPA 回归路径在可见 Windows Chrome 下通过。
- 这轮未修改生产代码；只新增本地验证脚本与测试文档。
- WSLg 桌面窗口呈现问题继续视为环境层问题，不阻塞 Web SPA 浏览器态验证。

## 风险与后续事项

- 当前 `mcp__chrome` 工具会话仍绑定 WSL 侧 `127.0.0.1:9222`，不能直接控制 Windows 侧 `127.0.0.1:9225` Chrome。
- 如果后续要求“Chrome MCP 工具本身控制 Windows Chrome”，需要建立稳定的 WSL-to-Windows CDP bridge，或在 Windows 侧启动 MCP server。
- WSL 环境代理变量可能误导 localhost 诊断；检查本地端口时应使用 `curl --noproxy '*'`。
- 当前 visible runner 主路径已迁到 `data-*` 语义接口；后续扩展测试时应继续保持 selector-first，避免新增文案驱动断言。

## 给下一位 Agent 的备注

- 本轮可见验证使用 Windows Chrome CDP/Node，而不是 Dioxus desktop/WSLg 窗口。
- Windows visible Chrome CDP runner 已沉淀进 repo；当前主路径断言也已迁到 headless active interface 风格。
