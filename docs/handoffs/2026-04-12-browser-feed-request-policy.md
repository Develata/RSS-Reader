# Browser Feed Request Policy Split

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：6918e90
- 相关 commit：6918e90
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成 `browser/feed.rs` 第一阶段收敛：把 feed proxy URL 构造、request 列表生成与 fallback 判定从解析逻辑中拆到独立 request policy helper。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `crates/rssr-infra/src/application_adapters/browser/mod.rs`
- 平台：
  - Web
  - `rssr-web` browser feed smoke
- 额外影响：
  - browser feed request policy

## 关键变更

### Browser Feed Request Helper

- 新增 `feed_request.rs`，集中承载：
  - `WebFeedRequest` / `WebFeedRequestKind`
  - proxy URL 构造
  - direct request `_rssr_fetch` query 标记
  - request 列表生成
  - fallback status 判定
  - proxy login / SPA shell 响应识别

### Browser Feed Module

- `feed.rs` 现在只保留：
  - `web_fetch_feed_response` orchestration
  - `parse_feed`
  - feed / entry normalization
  - content hashing
- 请求策略与解析语义不再同文件交错。

### 测试补充

- `feed_request.rs` 新增针对 request policy 的纯函数单测：
  - proxy + direct request 顺序
  - origin 缺失时跳过 proxy
  - 非 HTTP scheme 不追加 `_rssr_fetch`
  - HTML / login shell 判定
  - fallback status 判定

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test -p rssr-infra`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --lib --no-run`：通过；确认 wasm-only browser module 与新增单测可编译为 test artifact
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8368 --web-port 18868 --log-dir target/release-ui-regression/20260412-codex-browser-feed-request-policy`：通过

### 手工验收

- 未执行独立手工点击验收；本次依赖 release UI 自动门禁与 `rssr-web` browser feed smoke。

## 结果

- 本次变更已验证，可合并。
- `browser/feed.rs` 的第一层耦合已经拆开，后续可以更安全地继续做 response shell detection 与 parse normalization 收敛。
- release UI regression summary：`target/release-ui-regression/20260412-codex-browser-feed-request-policy/summary.md`

## 风险与后续事项

- release UI summary 记录的是执行命令时的基线 `fb32d97`；命令运行时包含本次未提交 worktree，随后提交为 `6918e90`，提交后未重复整轮回归。
- 新增的 request policy 单测位于 wasm-only browser module 中，当前 native `cargo test` 不会执行它们；本轮只额外验证了 wasm test artifact 可编译。
- 第二阶段建议继续拆 `response shell detection`，第三阶段再评估是否把 parse normalization 单独抽离。

## 给下一位 Agent 的备注

- 下一步优先文件仍是 `crates/rssr-infra/src/application_adapters/browser/feed.rs`。
- 继续拆时保持顺序：
  1. response shell detection
  2. parse normalization
- 不建议在第二阶段同时改 request policy，当前这一层已经稳定并有测试覆盖。
