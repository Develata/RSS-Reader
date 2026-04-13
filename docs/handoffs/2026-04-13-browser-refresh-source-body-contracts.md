# Browser Refresh Source Body Contracts

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：2625961
- 相关 commit：2625961
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

开始实现 browser refresh source-side harness 的第一刀，但只覆盖 body classification，不碰网络请求层。新增 `classify_browser_refresh_body()`，让 “HTTP 成功且不是 304 后，body 如何映射为 `Updated` 或 `Failed`” 变成可测试的稳定 helper。

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
  - browser refresh source-side body classification coverage

## 关键变更

### source-side body classification helper

- 新增 `classify_browser_refresh_body(metadata, body)`。
- `BrowserFeedRefreshSource::refresh()` 在成功读到 body 后改用这个 helper。
- helper 只做：
  - valid XML body -> `FeedRefreshSourceOutput::Updated`
  - parse/html failure -> `FeedRefreshSourceOutput::Failed`
- 不做：
  - fetch
  - request fallback
  - non-success status 分类
  - store-side 写回

### wasm harness 覆盖

- `wasm_refresh_contract_harness.rs` 新增两条 source-side body classification 测试：
  - valid XML body -> `Updated`
  - HTML shell body -> `Failed`，message 前缀为 `解析订阅失败:`，并保留 metadata

### 测试计划回写

- `contract-harness-rebuild-plan.md` 更新当前进度：
  - body classification harness 已落地
  - request-level harness 仍待实现

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`：通过
- `bash scripts/run_wasm_refresh_contract_harness.sh`：通过，9 tests passed
- `git diff --check`：通过

### 手工验收

- 审查文件：
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`

## 结果

- 本次交付可合并。
- source-side 现在已有第一批 body classification contract，但 request-level source harness 仍未实现。

## 风险与后续事项

- 这次没有覆盖：
  - network / CORS failure
  - proxy shell / login shell 的 request-level fallback
  - non-success status -> `Failed`
  - bad XML parse failure
- 下一步如果继续推进，应优先判断 request-level harness 是否可以基于现有 `feed_request.rs` / `feed_response.rs` 纯函数入口完成，不要先改网络层。

## 给下一位 Agent 的备注

- 若继续做 source-side harness，先看：
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_response.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`
- 当前不要再拆 `BrowserFeedRefreshSource` 主流程；下一步应优先补 request-level failure 分类测试。
