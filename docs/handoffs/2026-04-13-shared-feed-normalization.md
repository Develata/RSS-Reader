# Shared Feed Normalization Core

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：5a03660
- 相关 commit：5a03660
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成 `browser/feed.rs` 第三阶段收敛：把 native parser 与 browser parser 共有的 feed normalization / dedup / content hash 逻辑抽到 `rssr-infra` 内部共享模块，减少双份语义和后续分叉风险。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/feed_normalization.rs`
  - `crates/rssr-infra/src/parser/feed_parser.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/db/entry_repository.rs`
  - `crates/rssr-infra/src/lib.rs`
- 平台：
  - desktop
  - Web
  - `rssr-web`
- 额外影响：
  - browser feed parsing
  - native feed parsing
  - entry content hash consistency

## 关键变更

### Shared Normalization Module

- 新增 `feed_normalization.rs`，集中承载：
  - `ParsedFeed`
  - `ParsedEntry`
  - XML parse + normalization
  - stable source id / dedup key fallback
  - `hash_content`
- 用 `warn_on_missing_content: bool` 区分 native 与 browser 行为，避免目标特定枚举分支在单目标构建里形成 dead code warning。

### Native / Browser Parser 收敛

- `parser/feed_parser.rs` 改成共享 normalization 的薄封装，保留 native 侧“缺内容条目记录 warning”的行为。
- `application_adapters/browser/feed.rs` 改成共享 normalization 的薄封装，保留 browser 侧“缺内容条目静默跳过”的行为。
- `browser/feed.rs` 继续只负责：
  - request orchestration
  - HTML shell 拦截
  - 调用共享 parse core

### Content Hash 一致性

- `db/entry_repository.rs` 删除本地重复 `hash_content` 实现，改为复用共享 helper。
- 这样 refresh 写入路径与 browser/native parse 路径的内容 hash 规则只保留一份定义。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo test --workspace`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo clippy --workspace --all-targets`：通过
- `git diff --check`：通过
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8370 --web-port 18870 --log-dir target/release-ui-regression/20260412-codex-shared-feed-normalization`：通过

### 手工验收

- 未执行独立手工点击验收；本次依赖 release UI automated gates 与 `rssr-web browser feed smoke`。

## 结果

- 本次变更已验证，可合并。
- `browser/feed.rs` 现已主要保留 browser 特有 orchestration；native/browser 的 feed normalization 语义收敛到单一实现。
- release UI regression summary：`target/release-ui-regression/20260412-codex-shared-feed-normalization/summary.md`

## 风险与后续事项

- release UI summary 记录的基线 commit 是执行脚本时的 `6014611`；脚本运行时包含本次未提交 worktree，随后代码提交为 `5a03660`，本轮未在新 commit 上重复整轮 UI 回归。
- `feed_normalization.rs` 目前仍是 `rssr-infra` crate 内部模块；如果后续 browser/native 之外还要复用这些 DTO，再评估是否公开导出。
- 下一步更值得做的是检查 `rssr-application` 层是否还有“只做聚合却未形成稳定 use case”的服务边界，例如 feeds snapshot / feed service / CLI 调用入口之间的职责是否还需要收敛。

## 给下一位 Agent 的备注

- 若继续 parser 相关工作，先看 `crates/rssr-infra/src/feed_normalization.rs`，这里现在是唯一 normalization 真相源。
- 若转向 application use case 收敛，建议先从 `crates/rssr-application/src/feeds_snapshot_service.rs` 和 `crates/rssr-application/src/feed_service.rs` 入手，判断哪些接口还是“仓储直通 + 少量整形”。
