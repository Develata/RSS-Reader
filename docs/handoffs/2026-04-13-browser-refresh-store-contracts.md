# Browser Refresh Store Contracts

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：c502707
- 相关 commit：c502707
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续 browser contract hardening，但这次不再试图在 host 目标上硬测 `BrowserFeedRefreshSource`。结论是 `rssr-infra::application_adapters::browser` 整体只在 `wasm32` 编译，因此当前最稳的推进路径是补强 `wasm_refresh_contract_harness` 中 `BrowserRefreshStore` 的契约覆盖，并把这条 wasm harness 明确写入主线验证矩阵。

## 影响范围

- 模块：
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/mainline-validation-matrix.md`
- 平台：
  - Web
  - wasm32 browser harness
- 额外影响：
  - browser refresh store contract coverage
  - 主线 refresh 验证入口

## 关键变更

### 补齐 BrowserRefreshStore 契约

- 新增 `get_target` 契约测试：
  - active feed 会返回 target
  - deleted feed 会被跳过
  - URL 会按 domain 规则去掉默认端口和 fragment
- 新增 `commit(Updated)` 契约测试：
  - 成功刷新会清理旧的 `fetch_error`
- 新增 `commit(Failed)` 契约测试：
  - 失败会更新 `last_fetched_at`
  - 失败不会清空已有 `last_success_at`
  - 失败信息会稳定落入 `fetch_error`

### 主线验证矩阵回写

- `docs/testing/mainline-validation-matrix.md` 的 `refresh` 行新增：
  - `bash scripts/run_wasm_refresh_contract_harness.sh`
- 通过标准同步写明 browser refresh store 的 target / commit 语义也属于 refresh 主线门禁的一部分。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`：通过
- `bash scripts/run_wasm_refresh_contract_harness.sh`：通过
- `git diff --check`：通过

### 手工验收

- 审查文件：
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/mainline-validation-matrix.md`

## 结果

- 本次交付可合并。
- browser refresh store 的核心 contract 现在更完整了，尤其是：
  - `get_target`
  - failed commit 对已有 success 状态的保持
  - updated commit 对旧错误状态的清理

## 风险与后续事项

- `BrowserFeedRefreshSource` 的 source-side 契约仍未被同等粒度覆盖；当前困难点不是实现逻辑，而是它只能在 `wasm32` 下编译，且需要真实 browser fetch 环境。
- 若继续推进下一步，建议优先做：
  - source-side failure triage 设计
  - `rssr-web` 部署壳 / `/feed-proxy` / browser refresh path 的契约说明

## 给下一位 Agent 的备注

- 若要继续 browser refresh contract hardening，先读：
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `docs/testing/mainline-validation-matrix.md`
- 下一步不建议为了测试 `BrowserFeedRefreshSource` 去强行引入 host-only mock 结构；先把 wasm/browser 环境下可稳定验证的 source-side 契约设计清楚再动。
