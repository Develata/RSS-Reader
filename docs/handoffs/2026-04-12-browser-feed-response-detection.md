# Browser Feed Response Detection Split

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：1366ecd
- 相关 commit：1366ecd
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成 `browser/feed.rs` 第二阶段收敛：把 proxy login / SPA shell 响应识别与 HTML body 识别从 request policy / parse 路径中拆到独立 response helper。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_response.rs`
  - `crates/rssr-infra/src/application_adapters/browser/mod.rs`
- 平台：
  - Web
  - `rssr-web` browser feed smoke
- 额外影响：
  - browser feed response diagnostics

## 关键变更

### Browser Feed Response Helper

- 新增 `feed_response.rs`，集中承载：
  - `looks_like_proxy_login_or_spa_shell`
  - `looks_like_html_response_body`
- 新增纯函数测试覆盖：
  - HTML / XHTML / `/login` 路径识别
  - BOM + HTML body 识别
  - XML body 不应误判为 HTML

### Browser Feed Request / Parse Wiring

- `feed_request.rs` 改为依赖 `feed_response.rs` 做 proxy shell 判定，不再自己维护响应识别规则。
- `feed.rs` 改为依赖 `feed_response.rs` 做 HTML body 检测，自身继续保留 fetch orchestration 与 parse normalization。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8369 --web-port 18869 --log-dir target/release-ui-regression/20260412-codex-browser-feed-response-detection`：通过

### 手工验收

- 未执行独立手工点击验收；本次依赖 release UI 自动门禁与 `rssr-web` browser feed smoke。

## 结果

- 本次变更已验证，可合并。
- `browser/feed.rs` 的 request policy 与 response diagnostics 已经分离，第三阶段若继续推进，就只剩 parse normalization 收敛。
- release UI regression summary：`target/release-ui-regression/20260412-codex-browser-feed-response-detection/summary.md`

## 风险与后续事项

- release UI summary 记录的是执行命令时的基线 `1050267`；命令运行时包含本次未提交 worktree，随后提交为 `1366ecd`，提交后未重复整轮回归。
- `feed_response.rs` 的纯函数测试位于 wasm-only browser module 中；native workspace test 不会执行它们，本轮主要依赖 wasm 编译与 release UI regression 覆盖行为。
- 下一阶段如果继续做 parse normalization，优先目标应是避免再把 request / response helper 重新耦回同文件。

## 给下一位 Agent 的备注

- 若继续第三阶段，入口文件仍是 `crates/rssr-infra/src/application_adapters/browser/feed.rs`。
- 继续拆 parse normalization 时，建议先比较 `crates/rssr-infra/src/parser/feed_parser.rs` 与 browser 版本是否已有足够多的重复，避免一边拆边引入新的双份语义。
