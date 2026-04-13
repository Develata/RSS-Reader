# Refresh Empty Cache Recovery

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：ab2d738
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

修复 native / CLI refresh 在本地 feed 已无任何文章缓存但仍保留 `etag` / `last_modified` 时，会持续走条件请求并被 `304 Not Modified` 锁死为 0 篇的回归；同时放宽 native SQLite 连接池，缓解后台刷新和图片本地化并发时的慢连接告警。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/application_adapters/refresh.rs`
  - `crates/rssr-infra/src/db.rs`
  - `crates/rssr-infra/src/db/sqlite_native.rs`
  - `crates/rssr-infra/src/db/entry_repository.rs`
  - `crates/rssr-infra/tests/test_application_refresh_store_adapter.rs`
- 平台：
  - Linux
  - desktop native
  - CLI
- 额外影响：
  - docs

## 关键变更

### refresh target recovery

- `SqliteRefreshStore::get_target` 在 feed 本地无任何 entries 时忽略 `etag` / `last_modified`，强制下一次 refresh 走全量抓取。
- `SqliteRefreshStore::list_targets` 使用一次 `SELECT DISTINCT feed_id FROM entries` 收集已有文章的 feed，避免 `refresh_all` 对所有空 feed 继续发条件请求。
- 新增适配器测试覆盖“空 entries 但仍持有 validators”时的强制全量抓取行为。

### sqlite pool sizing

- `create_sqlite_pool()` 对文件型 SQLite 默认使用 `max_connections = 4`，保留 `sqlite::memory:` / `mode=memory` 的单连接行为，避免测试库被多连接拆开。
- native 文件路径 backend 同步切到相同的默认连接数，缓解桌面端后台 refresh、UI 查询、正文图片本地化并发时的 `sqlx::pool::acquire` 慢连接告警。

## 验证与验收

### 自动化验证

- `cargo fmt --check`：通过
- `cargo test -p rssr-infra --test test_application_refresh_store_adapter -- --nocapture`：通过
- `cargo test -p rssr-infra --test test_feed_refresh_flow -- --nocapture`：通过
- `cargo test -p rssr-infra`：通过
- `cargo test -p rssr-app`：通过
- `cargo check --workspace`：通过
- `git diff --check`：通过

### 手工验收

- `cargo run --quiet -p rssr-cli -- --database-url "sqlite:///tmp/rssr-nvidia-regression.db" add-feed https://blogs.nvidia.com/feed/`：通过，空库首次导入写入 18 篇 NVIDIA 文章
- 人工构造 `delete from entries` 但保留 feed validators 后再执行 `cargo run --quiet -p rssr-cli -- --database-url "sqlite:///tmp/rssr-empty-cache-recovery.db" refresh --feed-id 1`：通过，坏状态可自愈并重新写入 18 篇
- 对当前本地库 `/home/develata/gitclone/RSS-Reader/target/release/RSS-Reader/rss-reader.db` 清空 NVIDIA feed validators 后执行两次 `refresh --feed-id`：通过，`feed_id=1` 与 `feed_id=9` 均恢复为 18 篇

## 结果

- 本次修复可合并。
- 既有“空缓存 + 304”坏状态在下一次 refresh 时将自动恢复，不需要用户手动删除订阅重加。

## 风险与后续事项

- `rssr_infra::feed_normalization` 当前仍会对 OpenAI 这类稀疏 feed 逐条打印“缺少 summary 和 content”警告；这不影响导入，但日志噪声较大，后续应收敛成聚合日志或降级到 debug。
- SQLite 多连接只能缓解慢连接等待，不能替代更细的后台任务限流；如果图片本地化继续扩张，后续仍应审查 worker 并发上限和写回节流。

## 给下一位 Agent 的备注

- 先看 `crates/rssr-infra/src/application_adapters/refresh.rs`，这里定义了 refresh 是否携带条件请求 validators。
- 如果还要继续收敛日志噪声，下一步入口是 `crates/rssr-infra/src/feed_normalization.rs` 的逐条 warning。
