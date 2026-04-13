# Browser Refresh Request Fallback

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：f171393
- 相关 commit：f171393
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续推进 browser refresh source-side harness，实现 request-level fallback 的纯函数覆盖，并补齐 bad XML body 的 parse failure 契约。未修改 `BrowserFeedRefreshSource` 的网络请求主流程，也未引入 host-only mock。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - Web
  - wasm32 browser harness
- 额外影响：
  - browser refresh source-side request fallback coverage

## 关键变更

### request fallback 纯函数覆盖

- 在 `feed_request.rs` 中拆出 `should_fallback_web_feed_request_parts()`。
- 原 `should_fallback_web_feed_request()` 仍负责从 `reqwest::Response` 取 status 和 shell 判定结果。
- 新增纯函数测试覆盖：
  - proxy shell 且 direct 仍可尝试时 fallback
  - direct request 不因 shell 判定 fallback
  - final request 不再 fallback
  - 选定错误状态码 fallback
  - `429 Too Many Requests` 不进入当前 fallback 集合

### bad XML body contract

- `wasm_refresh_contract_harness.rs` 新增 bad XML body 测试。
- 断言结果为 `FeedRefreshSourceOutput::Failed`，且 message 前缀为 `解析订阅失败:`。

### 测试计划回写

- `contract-harness-rebuild-plan.md` 更新 source-side 当前进度：
  - bad XML body parse failure 已完成
  - proxy shell / login shell request-level fallback 纯函数覆盖已完成
  - request-level network / CORS 与 non-success status 仍待实现

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --lib --no-run`：通过
- `bash scripts/run_wasm_refresh_contract_harness.sh`：通过，10 tests passed
- `git diff --check`：通过

### 手工验收

- 审查文件：
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`

## 结果

- 本次交付可合并。
- browser refresh source-side 已进一步覆盖 request fallback 与 bad XML parse failure。

## 风险与后续事项

- 仍未覆盖：
  - network / CORS failure
  - non-success status -> `Failed`
- 下一步如果继续，应优先看是否能不碰网络层地抽出 non-success status classification helper；network / CORS failure 可能仍需要真实 wasm fetch harness 或更明确的环境约束。

## 给下一位 Agent 的备注

- 当前不要再拆 `BrowserFeedRefreshSource` 主流程。
- 若继续推进，先看：
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `docs/testing/contract-harness-rebuild-plan.md`
