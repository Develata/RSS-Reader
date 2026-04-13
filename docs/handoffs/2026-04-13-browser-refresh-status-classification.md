# Browser Refresh Status Classification

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：abae12f
- 相关 commit：abae12f
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续推进 browser refresh source-side harness，补齐 status-level 分类：`304 Not Modified` 和非成功 HTTP 状态现在通过可测试 helper 映射到稳定 `FeedRefreshSourceOutput`，不再依赖 `reqwest::Error` 的展示文本。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters.rs`
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - Web
  - wasm32 browser harness
- 额外影响：
  - browser refresh source-side status classification coverage

## 关键变更

### status classification helper

- 新增 `classify_browser_refresh_status(status, metadata)`。
- `BrowserFeedRefreshSource::refresh()` 现在先通过该 helper 处理：
  - `304 Not Modified` -> `FeedRefreshSourceOutput::NotModified`
  - 非成功 HTTP status -> `FeedRefreshSourceOutput::Failed`
  - 成功 status -> `None`，继续读取 body 并走 body classification
- 非成功状态 message 稳定为：
  - `feed 抓取返回非成功状态: HTTP status <status>`

### wasm harness 覆盖

- 新增 `304 Not Modified` 分类测试。
- 新增 `403 Forbidden` 非成功状态分类测试。
- 新增 `200 OK` 会继续进入 body classification 的测试。

### 测试计划回写

- `contract-harness-rebuild-plan.md` 已把：
  - `304 Not Modified`
  - non-success status
  移入 source-side 已完成覆盖。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `bash scripts/run_wasm_refresh_contract_harness.sh`：通过，13 tests passed
- `git diff --check`：通过

### 手工验收

- 审查文件：
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`

## 结果

- 本次交付可合并。
- browser refresh source-side 目前已覆盖：
  - valid XML body
  - HTML shell body
  - bad XML body
  - request fallback 纯函数
  - `304 Not Modified`
  - non-success status

## 风险与后续事项

- 当前 source-side 只剩 network / CORS failure 没有自动化 contract 覆盖。
- 这类失败需要真实 browser fetch 环境或更明确的环境约束；下一步不应继续抽 helper，应该先判断是否值得做真实 wasm fetch harness，还是把它作为环境限制/真实 smoke 覆盖。

## 给下一位 Agent 的备注

- 如果继续推进 source-side，下一步只剩：
  - network / CORS failure
- 优先看：
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
  - `docs/testing/environment-limitations.md`
  - `scripts/run_rssr_web_browser_feed_smoke.sh`
  - `scripts/run_rssr_web_proxy_feed_smoke.sh`
