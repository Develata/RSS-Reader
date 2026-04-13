# Native Auto Refresh Logging And Concurrency

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：50722ba
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

收敛 native 端后台自动刷新日志与并发策略：将自动刷新 feed 并发从 4 降到 1，并把自动刷新失败日志改为输出完整 error chain，避免 SQLite 写冲突被包成单一的“更新订阅抓取状态失败”。

## 影响范围

- 模块：
  - `crates/rssr-app/src/bootstrap/native.rs`
- 平台：
  - Linux
  - desktop native
- 额外影响：
  - docs

## 关键变更

### native refresh concurrency

- `AppServices::REFRESH_ALL_CONCURRENCY` 从 `4` 调整为 `1`。
- 原因是 native 当前持久层仍是 SQLite，refresh 提交路径包含多次写库；在文件型 SQLite 连接池放宽后继续使用 4 路并发，会放大写锁竞争与 `update_fetch_state()` 失败概率。

### auto refresh error logging

- `tracing::warn!(error = %error, ...)` 改为 `tracing::warn!(error = ?error, ...)`。
- 现在后台自动刷新失败时会把完整 anyhow 错误链打印出来，便于区分 `database is locked`、`not found` 或其它 SQL/持久化错误。

## 验证与验收

### 自动化验证

- `cargo fmt --check`：通过
- `cargo test -p rssr-app bootstrap::tests -- --nocapture`：通过
- `cargo check -p rssr-app`：通过

### 手工验收

- 基于用户提供日志与当前代码路径复核 `后台自动刷新失败 error=更新订阅抓取状态失败` 来源：通过，确认错误来自 native `auto_refresh -> refresh_all -> store.commit -> update_fetch_state`
- 未执行完整桌面端长时手工运行；本次改动仅收敛日志与并发值

## 结果

- 本次修复可合并。
- 后续若自动刷新仍失败，日志会直接显示底层错误，不再只有抽象 context。

## 风险与后续事项

- `refresh_all` 当前仍是“任一 feed commit 返回 Err 则整批返回 Err”；如果未来还要提升并发，需要先把 per-feed commit failure 下沉成单 feed outcome，而不是批次级失败。
- 当前只调整了 native 自动刷新并发；若后续手动“刷新全部订阅”也复现 SQLite 写竞争，需要继续复核 UI 触发链路的并发策略。

## 给下一位 Agent 的备注

- 先看 `crates/rssr-app/src/bootstrap/native.rs` 的 `AutoRefreshCapability::ensure_started()`。
- 如果后续日志里出现具体 SQLite 错误，再决定是否要加 `busy_timeout` 或进一步拆 refresh 提交边界。
