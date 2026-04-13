# Browser Feed Smoke Diagnostics

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：f184d48
- 相关 commit：f184d48
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续沿 `browser/feed.rs` 的后续建议做边界审查后，结论是共享 feed normalization 已经收敛完成，当前更值得处理的风险不是继续拆 parser，而是补强 `rssr-web browser feed smoke` 的可观测性与失败诊断，减少“刷新其实已经跑完，但 smoke 还在等文本变化”的超时噪音。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/feeds_page/sections/saved.rs`
  - `crates/rssr-app/src/pages/feeds_page/sections/support.rs`
  - `crates/rssr-web/src/smoke.rs`
- 平台：
  - Web
  - Docker / `rssr-web`
- 额外影响：
  - `rssr-web browser feed smoke`
  - feed refresh 诊断属性

## 关键变更

### feed card 暴露稳定诊断属性

- feeds 页面每个 `feed-card` 现在额外暴露：
  - `data-feed-id`
  - `data-feed-title`
  - `data-feed-url`
  - `data-entry-count`
  - `data-unread-count`
  - `data-refresh-state`
  - `data-last-fetched-at`
  - `data-last-success-at`
  - `data-fetch-error`
- 这些属性只表达已经存在的 feed summary 事实，不引入新的状态源。

### 刷新状态语义显式化

- 新增 `feed_refresh_state_attr()`，把 feed summary 压成稳定状态：
  - `never`
  - `attempted`
  - `success`
  - `failed`
- 同时补了单测，覆盖：
  - 首次刷新前
  - 成功刷新后
  - 存在 `fetch_error` 时

### browser smoke 不再依赖脆弱文本等待

- `rssr-web` smoke helper 不再主要依赖：
  - 页面 banner 文本
  - feed-card 文本模糊变化
- 改为直接观察：
  - `data-refresh-state`
  - `data-last-fetched-at`
  - `data-fetch-error`
- 若刷新失败，smoke 会立即把 `fetch_error` 带进失败信息，而不是继续等到超时。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-app`：通过
- `cargo check -p rssr-web`：通过
- `git diff --check`：通过
- `bash scripts/run_rssr_web_browser_feed_smoke.sh --port 18089 --log-dir target/rssr-web-browser-feed-smoke/20260413-codex-diagnostics`：通过

### 手工验收

- 审查文件：
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/feed_normalization.rs`
  - `crates/rssr-app/src/pages/feeds_page/sections/saved.rs`
  - `crates/rssr-web/src/smoke.rs`
- smoke 产物：
  - `target/rssr-web-browser-feed-smoke/20260413-codex-diagnostics/browser-feed-smoke.html`
  - `target/rssr-web-browser-feed-smoke/20260413-codex-diagnostics/browser-feed-smoke.png`
  - `target/rssr-web-browser-feed-smoke/20260413-codex-diagnostics/summary.md`

## 结果

- 本次交付可合并。
- 结论：`browser/feed.rs` 当前已基本收敛到 browser-specific orchestration 边界；这轮更有价值的收敛点是 smoke 诊断，而不是继续硬拆 parser。

## 风险与后续事项

- 这次只补了 smoke 可观测性，没有改变 browser refresh 的业务语义；若后续仍出现超时，应优先看 smoke 产物中的 `data-refresh-state` / `data-fetch-error`，而不是直接怀疑 parser。
- 若下一步继续推进 browser adapter 边界，建议转向：
  - `FeedRefreshSourcePort` / `RefreshStorePort` 的 browser contract tests
  - `rssr-web` 部署壳与 browser refresh path 的契约说明

## 给下一位 Agent 的备注

- 若要复查这次判断，先读：
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-web/src/smoke.rs`
  - `docs/handoffs/2026-04-13-shared-feed-normalization.md`
- 当前最值得继续做的，不是把 `browser/feed.rs` 再切更多 helper，而是补 browser refresh contract / smoke failure triage。
