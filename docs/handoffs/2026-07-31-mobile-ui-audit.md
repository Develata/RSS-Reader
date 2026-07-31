# 移动端 UI 审计（Vibe-Coding 七类框架 + claude-code 清单复验）

- 日期：2026-07-31
- 作者 / Agent：WorkBuddy (Kimi-K3)
- 分支：main
- 当前 HEAD：c397826
- 相关 commit：pending（audit-only，仅新增两份文档，未提交）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

在 360×800 Android 模拟环境下对 RSS-Reader 移动端 UI 做实证审计：claude-code 的 7 项发现全部复现（6 live / 1 latent），并按「Vibe Coding 七类丑」框架新发现 5 项问题，产出修复优先级建议（P0 增量 CSS / P1 data-nav 作用域 / P2 组织治理）。未修改任何生产代码。

## 影响范围

- 模块：
  - `docs/mobile-ui-audit-2026-07.md`（新增，审计报告）
  - `docs/handoffs/2026-07-31-mobile-ui-audit.md`（本记录）
- 平台：
  - Android / 移动 Web（360×800 视口）；桌面端未受影响
- 额外影响：
  - N/A（无 workflow / release 变更）

## 关键变更

### 审计交付物

- 新增 `docs/mobile-ui-audit-2026-07.md`：含验证环境、7 项复验表（含实测几何数据）、5 项新发现、P0/P1/P2 修复建议。

### 实测确认的核心事实（供后续修复引用）

- `workspaces.css:215` 标题规则缺 `min-width:0`/`overflow-wrap`：hashtag 标题 681px → entry-list 单列吹胀至 688px → 页面 709px 横向溢出；同一缺口使 /feeds 被吹至 631px。
- `feed-card-actions` 全仓库 0 条 CSS：删除/刷新按钮垂直间距实测 0px（有内联两步确认兜底）。
- `responsive.css:294` 的 `[data-nav]` 以同优先级后加载覆盖 `.button`：返回/同订阅导航按钮 36px，订阅卡片标题 14.08px 胶囊（URL 14.72px）。
- `browser_interactions.rs`：`remember_entry_controls_hidden` 在 non-wasm 为 no-op（Android 原生端控件展开状态不持久）。
- app.rs:53 注入 `viewport-fit=cover`，全部样式表无 `env(safe-area-inset-*)`；`target_sdk=34` 时未激活。

## 验证与验收

### 自动化验证

- `dx build --platform web --package rssr-app`：通过（105s）
- `cargo test --workspace`：未执行（audit-only，无代码变更）

### 手工验收

- Chrome DevTools MCP 360×800 mobile/touch 模拟，reader-demo 种子 + 注入极端标题条目：通过
- /entries 溢出、/feeds 粘连与胶囊标题、/entries/15 阅读页、/settings、Amethyst Glass 主题：均已实测并内联截图确认
- console：无 app 相关错误（仅种子源 CORS 与 dx dev WebSocket 噪音）

## 结果

- 审计报告可直接作为修复批次的输入；P0 三项为纯增量 CSS，风险最低，建议先行。
- P1 的 `[data-nav]` 作用域收敛会改变移动端返回按钮与订阅卡片标题外观，属可见修正，需 Develata 拍板。

## 风险与后续事项

- 截图证据为 MCP 内联（chrome-devtools MCP 的 workspace roots 限制，无法落盘至仓库）；报告以 DOM 几何实测数据为准，可复现。
- latent 项：targetSdk 升 35 前必须补 safe-area-inset，否则 reader-bottom-bar 落入手势导航区。
- 建议把 360×800 纳入固定 UI 回归视口，将本报告实测断言固化进 `scripts/run_static_web_small_viewport_smoke.sh`。

## 给下一位 Agent 的备注

- 入口文档：`docs/mobile-ui-audit-2026-07.md`（含每项的实测数据与修复批次）。
- Windows 下跑 SPA server 必须设 `RSSR_REPO_ROOT=E:\gitclone\RSS-Reader`，否则 `__codex` helper 报 `E:\e\gitclone\...` 路径错误（Git Bash `pwd` 直译问题）。
- 复现极端值数据：向 localStorage `rssr-web-state-v1` 注入长 token 标题条目即可，seed helper 见 `scripts/run_static_web_browser_smoke.sh`。
