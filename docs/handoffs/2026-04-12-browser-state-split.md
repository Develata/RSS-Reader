# Browser State Split

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：66e475c
- 相关 commit：66e475c
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按职责拆分 browser `state.rs`，把持久化模型、localStorage 读写、entry 映射与 upsert 逻辑分离，同时保持 `browser::state::{...}` 对外导出面不变。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/browser/state.rs`
  - `crates/rssr-infra/src/application_adapters/browser/state/models.rs`
  - `crates/rssr-infra/src/application_adapters/browser/state/storage.rs`
  - `crates/rssr-infra/src/application_adapters/browser/state/entries.rs`
- 平台：
  - Web
  - wasm32 browser local state path
  - Linux 验证环境
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-browser-state-split/summary.md`

## 关键变更

### Browser State Boundary

- `state.rs` 缩减为子模块声明和 re-export，外部调用路径保持不变。
- `models.rs` 承载 browser persisted state 模型、slice 类型和 storage key 常量。
- `storage.rs` 承载 localStorage 加载、保存、损坏副本备份和切片写入逻辑。
- `entries.rs` 承载 entry flag 查询、domain entry 映射和 browser entry upsert 行为。

### 行为保持

- `load_state`、`save_state_snapshot`、`save_app_state_slice`、`save_entry_flag_patch` 的调用方式保持不变。
- wasm harness、browser bootstrap、browser query 和 adapter 仍通过 `browser::state` 导入既有类型与函数。
- 未修改 browser 状态模型字段、导出格式、feed refresh 写回或 settings 持久化行为。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-infra`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo test -p rssr-infra`：通过
- `cargo test --workspace`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8353 --web-port 18853 --log-dir target/release-ui-regression/20260412-codex-browser-state-split`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 release UI 自动门禁覆盖 web bundle 和 rssr-web browser feed smoke。

## 结果

- 本次交付可合并；browser state 不再是单个多职责文件。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- browser localStorage 状态模型仍然是 MVP 型持久化结构，当前拆分只改善边界，不改变规模瓶颈。
- `cargo test` 期间仍有既有 `test_browser_state_seed_contracts` fixture dead_code warning。
- settings 远端 pull 后 reload settings 仍在 UI runtime 编排，可继续收敛为 application use case。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-infra/src/application_adapters/browser/state.rs`。
- 如果继续清理 browser 侧结构，下一步更适合审查 `query.rs` 与 `state` 的关系，或收敛 settings sync workflow，而不是回头再改 state 导出面。
