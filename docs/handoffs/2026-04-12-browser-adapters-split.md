# Browser Adapters Split

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：036c82d
- 相关 commit：036c82d
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按职责拆分 browser application adapters，降低单文件多职责风险；本次保持 `browser::adapters::{...}` 外部导入路径不变，不改变运行行为。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/browser/adapters.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/app_state.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/config.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/entry.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/feed.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/refresh.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/settings.rs`
  - `crates/rssr-infra/src/application_adapters/browser/adapters/shared.rs`
- 平台：
  - Web
  - wasm32 browser local state path
  - Linux 验证环境
- 额外影响：
  - release UI regression 记录：`target/release-ui-regression/20260412-codex-browser-adapters-split/summary.md`

## 关键变更

### Browser Adapter Boundary

- `adapters.rs` 缩减为子模块声明和 re-export，保留现有外部 API。
- Feed repository、entry repository、app state adapter、settings repository、OPML/remote config adapter、refresh source/store 分别拆入独立文件。
- 新增 `shared.rs` 承载 browser adapter 内部共享的 persistence error 映射。

### 行为保持

- `BrowserFeedRepository`、`BrowserEntryRepository`、`BrowserAppStateAdapter`、`BrowserSettingsRepository`、`BrowserOpmlCodec`、`BrowserRemoteConfigStore`、`BrowserFeedRefreshSource`、`BrowserRefreshStore` 的公开构造和 trait 实现路径保持可用。
- 未修改 browser localStorage 状态模型、refresh commit 行为、query 行为或 config exchange 行为。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过
- `cargo check -p rssr-infra`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-cli`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test -p rssr-infra`：通过
- `cargo test --workspace`：通过
- `cargo clippy --workspace --all-targets`：通过，仍有既有 warning
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8352 --web-port 18852 --log-dir target/release-ui-regression/20260412-codex-browser-adapters-split`：通过

### 手工验收

- 未执行独立手工 UI 点击验收；本次依赖 release UI 自动门禁覆盖 web bundle 和 rssr-web browser feed smoke。

## 结果

- 本次交付可合并；browser adapter 不再是单个 650+ 行多职责文件。
- `rssr-web browser feed smoke` 本轮通过，未复现超时。

## 风险与后续事项

- 拆分只是边界整理，browser localStorage 状态模型仍是 MVP 型持久化结构；大量文章时仍可能需要 IndexedDB 或更细粒度索引。
- `cargo clippy --workspace --all-targets` 仍提示 `test_browser_state_seed_contracts` fixture dead_code warning，以及 `crates/rssr-app/src/ui/shell.rs` 的 `Navigator` clone_on_copy warning。
- settings 远端 pull 后重新 load settings 仍在 UI runtime 编排，可后续收敛为 settings sync use case。
- push 仍依赖 GitHub HTTPS 凭据；本地验证通过不代表远端已更新。

## 给下一位 Agent 的备注

- 阅读入口：`crates/rssr-infra/src/application_adapters/browser/adapters.rs`。
- 如果继续清理 browser 侧结构，下一步更适合处理 `state.rs` 的持久化切片边界，而不是重新合并 adapter 文件。
