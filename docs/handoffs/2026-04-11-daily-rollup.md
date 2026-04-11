# 2026-04-11 Daily Rollup

- 日期：2026-04-11
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：fea79d0
- 相关 commit：2189d8b / b914f4c / 2379557 / d3368b4 / 954b22a / 037e31a / c36edfd / fea79d0 / pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成一轮 Windows 原生 Chrome 可见窗口回归验证，确认当前 Web SPA 与 `rssr-web` 浏览器态 smoke 路径可在用户可见的 Windows Chrome 窗口内通过。
随后将该验证路径固化为 repo 内脚本与文档，避免继续依赖 `target/` 临时 runner。
继续把可见浏览器 runner 从中文文案断言迁到 `data-*` 语义接口。
继续补齐页面层语义接口，让 settings themes、entries groups、reader body 都暴露更稳定的 headless active interface。
继续把 entries/reader/theme 相关 CSS 从深 class selector 迁到语义 `data-*` selector。
继续把 feeds/settings workspace 与主题 CSS 中的卡片、表单、统计块规则迁到 `data-layout` / `data-slot` 语义接口，并给可见回归 CDP runner 增加请求级超时。
继续收 `.page`、`.page-header`、entries controls / overview / source chip 的视觉 class 依赖，页面根改用 `data-page`，普通 page 与 reader page 保持分离。
完成一轮“保留 class 边界”审查，明确设计系统 class / 组件内部 class / 低优先级页面 wrapper 的保留边界，并清理一个明确死 selector。

## 影响范围

- 模块：`scripts/run_web_spa_regression_server.sh`、`scripts/run_windows_chrome_visible_regression.sh`、`scripts/browser/rssr_visible_regression.mjs`、`crates/rssr-app/src/pages/*`、`assets/styles/*`、`assets/themes/*`、Web SPA 静态回归路径
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

### CSS 分离收口

- `assets/styles/entries.css`：entry group/header/title/meta/list/action 规则改用 `data-layout` / `data-slot` / `data-state`。
- `assets/styles/reader.css`：reader HTML/text 正文内部规则改用 `data-slot="reader-body-html"` / `data-slot="reader-body-text"`。
- `assets/styles/workspaces.css`、`assets/styles/responsive.css`：entry list、entry card actions、theme card swatches 等规则改用语义 selector。
- `assets/themes/*`：theme card、reader HTML 段落、entry list 相关规则继续迁到 `data-layout` / `data-slot`。
- 移除 `forest-desk` 中 reader 页下已经没有命中对象的 `.entry-card__actions` 陈旧规则。

### Feeds / Settings 语义接口收口

- feeds 页面统计卡补齐 `data-layout="stat-card"`、`data-stat`、`data-slot="stat-card-label/value"`。
- feeds compose / saved / config exchange 区域补齐 `feed-compose-card`、`feed-form`、`feed-list`、`feed-card`、`exchange-card`、`exchange-header` 等 `data-layout` / `data-section` / `data-slot`。
- settings appearance / preferences / sync 卡片补齐 `settings-card`、`settings-card-section`、`settings-form-grid`、`settings-card-actions/footer` 等语义 hook。
- `assets/styles/workspaces.css`、`assets/styles/shell.css`、`assets/styles/responsive.css` 和四个内置 theme CSS 中对应视觉规则改用语义 selector。
- 删除 `workspaces.css` 中已经没有页面 DOM 对应的 `feed-workbench__note` / intro 类死样式。
- 更新 [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)，把 `feed-workbench__note` 从待办改为已清理。

### Page Shell / Entries Controls 收口

- 页面标题补齐 `data-slot="page-title"`。
- entries / feeds / settings 页头补齐 `data-layout="page-header"`、`data-slot="page-section-header"`、`data-section`。
- entries controls 补齐 `entry-controls-panel/reveal/toggle`、`entry-organize-bar`、`entry-overview`、`entry-overview-metric`、`entry-overview-label/value`、`entry-filters-source-chip` 等 `data-layout` / `data-slot`。
- `.page` CSS 入口迁到 `[data-page]:not([data-page="reader"])`，reader 继续走 `[data-layout="reader-page"]`，避免普通页面壳样式污染 reader。
- `assets/styles/entries.css`、`assets/styles/shell.css`、`assets/styles/responsive.css`、`assets/themes/*` 中对应规则不再依赖 `.page`、`.page-title`、`.reading-header`、`.page-section-header--*`、`.entry-controls-*`、`.entry-overview*`、`.entry-filters__source-chip`。

### Class Boundary Audit

- 审查剩余 CSS class selector 与 Rust class token 的交集 / 差集。
- 明确保留为设计系统边界：
  - `app-shell`
  - `theme-light` / `theme-dark` / `theme-system`
  - `button` / `text-input` / `text-area` / `select-input` / `field-label`
  - `inline-actions` / `inline-actions__item`
  - `status-banner`
  - `icon-link-button`
  - `sr-only` / `sr-only-file-input`
- 明确保留为组件内部实现：
  - `reader-bottom-bar__button`
- 明确低优先级候选：
  - `entries-main`
  - `entries-page__backlink`
  - `entries-page__state`
- 清理 `assets/styles/entries.css` 中已无 DOM 对应的 `.entry-card__action` 死 selector。
- 更新 [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)，避免后续继续机械迁移设计系统 class。

### Regression Runner 稳定性

- `scripts/browser/rssr_visible_regression.mjs` 的 CDP navigation 现在会等待 `Page.loadEventFired`，但不把 load event 作为硬门禁；最终仍由 selector readiness 判断页面可用。
- `scripts/run_windows_chrome_visible_regression.sh` 在启动静态 SPA server / `rssr-web` 后会检查子进程是否仍存活，避免端口冲突时误命中旧服务。
- `scripts/browser/rssr_visible_regression.mjs` 增加 `CDP_COMMAND_TIMEOUT_MS` 请求级超时，避免 Windows Chrome/CDP 会话污染时无限挂起。
- 可见 runner 在主题矩阵和小视口循环中打印阶段 start/pass，方便定位失败步骤。

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
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8311 --rssr-web-port 18811 --chrome-port 9225 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-091253/summary.md`。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `node --check scripts/browser/rssr_visible_regression.mjs`：通过。
- `bash -n scripts/run_windows_chrome_visible_regression.sh`：通过。
- `git diff --check`：通过。
- `rg -n "\\.(feed-form|feed-compose-card|feed-list|feed-card|exchange-card|exchange-header|settings-card|stat-card|card-title|settings-form-grid|preset-grid|theme-gallery)(\\b|__)" assets/styles assets/themes -S`：通过，无剩余命中。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8314 --rssr-web-port 18814 --chrome-port 9226 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-092733/summary.md`。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `node --check scripts/browser/rssr_visible_regression.mjs`：通过。
- `bash -n scripts/run_windows_chrome_visible_regression.sh`：通过。
- `git diff --check`：通过。
- `rg -n "\\.(page|page-title|page-header__title|page-header__actions|page-section-header--|reading-header|entry-organize-bar|entry-overview|entry-overview__|entry-controls|entry-filters__source-chip)(\\b|__|--)" assets/styles assets/themes -S`：通过，无剩余命中。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8315 --rssr-web-port 18815 --chrome-port 9227 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-093306/summary.md`。
- `rg --pcre2 -o '(^|[,\\s])\\.[A-Za-z][A-Za-z0-9_-]*(?=[\\s:{.#\\[,>+~)]|$)' assets/styles assets/themes -S`：已执行，用于剩余 class selector 审查。
- `rg -o 'class: "[^"]+"' crates/rssr-app/src -S`：已执行，用于 Rust DOM class token 对照。
- `rg -n "entry-card__action" assets/styles assets/themes crates/rssr-app/src -S`：通过，剩余命中仅为 `entry-card__actions` 容器，不再有 `.entry-card__action` selector。
- `git diff --check`：通过。

### 手工验收

- Windows 原生 Chrome 可见窗口：通过。
- 静态 `/entries` 页面：通过。
- 静态 `/feeds` 页面：通过。
- `/reader` 实页多主题切换：通过。
- 小视口 `/entries`、`/feeds`、`/settings`、`/entries/2`：通过。
- `rssr-web` 浏览器态真实 feed smoke helper：通过。

## 结果

- 当前 Web SPA 回归路径在可见 Windows Chrome 下通过。
- 当前页面语义接口和 CSS 分离基线继续前移；feeds/settings/stat/exchange 这批视觉 class 已不再被 `assets/styles` / `assets/themes` 作为 CSS 选择器入口。
- page shell 与 entries controls/overview/source chip 这批视觉 class 也已不再被 `assets/styles` / `assets/themes` 作为 CSS 选择器入口。
- 保留 class 边界已明确：设计系统 class 不再作为“必须迁移”的技术债；后续只处理死样式、深 DOM selector、确实需要外部主题控制的页面业务槽。
- 可见回归 runner 已避免 CDP 请求无限挂起；复用污染较重的 9225 Chrome 会话时曾在 `Page.navigate` 超时，改用干净 9226 Windows Chrome profile 后全量通过。
- WSLg 桌面窗口呈现问题继续视为环境层问题，不阻塞 Web SPA 浏览器态验证。

## 风险与后续事项

- 当前 `mcp__chrome` 工具会话仍绑定 WSL 侧 `127.0.0.1:9222`，不能直接控制 Windows 侧 `127.0.0.1:9225` Chrome。
- 如果后续要求“Chrome MCP 工具本身控制 Windows Chrome”，需要建立稳定的 WSL-to-Windows CDP bridge，或在 Windows 侧启动 MCP server。
- WSL 环境代理变量可能误导 localhost 诊断；检查本地端口时应使用 `curl --noproxy '*'`。
- 当前 visible runner 主路径已迁到 `data-*` 语义接口；后续扩展测试时应继续保持 selector-first，避免新增文案驱动断言。
- 可见回归复用 Windows Chrome 端口时，CDP load event 可能不会覆盖所有 SPA route 情况；runner 已改为 selector readiness 作为最终判断。
- 复用长期运行的 Windows Chrome DevTools 端口可能受历史 tab / 旧 CDP 会话污染；建议日常回归优先使用新的 `--chrome-port`，或先清理旧可见回归窗口。
- 下一轮 CSS 分离不建议继续机械迁移 `button` / `field-label` / `inline-actions__item`；应只看新增死样式、深 DOM selector，或主题作者确实需要重排的业务槽。

## 给下一位 Agent 的备注

- 本轮可见验证使用 Windows Chrome CDP/Node，而不是 Dioxus desktop/WSLg 窗口。
- Windows visible Chrome CDP runner 已沉淀进 repo；当前主路径断言也已迁到 headless active interface 风格。
- `037e31a` 已提交上一轮 workspace CSS 语义 hook 收口。
- `c36edfd` 已提交 page shell / entries controls 语义 hook 收口。
- `fea79d0` 已提交 handoff 元数据记录。
- 本轮未提交：`commit: pending`。提交前建议复查 [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md) 和 [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)。
