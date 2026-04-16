# index.db + content.db 双库改造

- 日期：2026-04-16
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：30d03a2
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按已确认方案完成了 `index.db + content.db` 双库改造的首个可运行版本：application 层显式拆分索引/正文仓储，native 路径落地真实双库，纯静态 Web 保持同样的逻辑分层但不强求相同的物理存储实现。

随后补做了两项回归修复：

- 修正纯静态 Web `count_entries()` 被 `limit` 污染的问题，保证总数/总页数语义与 native SQL 保持一致。
- 优化 native SQLite 刷新写路径，把“每篇正文一次回查 `entry_id`”改成“整批索引写入后一次性批量解析 `entry_id`”，避免正文批量刷新时的额外逐条 `SELECT`。
- 修正纯静态 Web 的 wasm 编译断点：补齐 browser `state` 的转换函数导出，并修复 wasm harness 对 `EntryIndexRepository` trait 的导入。

## 影响范围

- 模块：
  - `crates/rssr-domain`
  - `crates/rssr-application`
  - `crates/rssr-infra`
  - `crates/rssr-app`
  - `crates/rssr-cli`
  - `migrations/`
  - `migrations_content/`
- 平台：
  - Windows / Desktop
  - Android（沿 native SQLite 路径）
  - Web（browser/static 逻辑分层）
  - CLI
- 额外影响：
  - migration
  - 测试夹具与 contract harness

## 关键变更

### Domain / Application 边界

- 在 `rssr-domain` 中新增 `EntryRecord`、`EntryContent`、`EntryIndexRepository`、`EntryContentRepository`。
- 保留兼容性的 `EntryRepository::get_entry()` 组合入口，但 application 不再依赖单一“大而全”文章仓储。
- `EntriesListService` 只依赖索引仓储。
- `ReaderService` 改为组合索引仓储和正文仓储，并新增“索引存在但正文缺失”降级路径。
- `FeedService` 和 `ImportExportService` 在清理文章时同时触发 index/content 删除。

### Native SQLite 双库

- `SqliteEntryRepository` 改为内部持有 `index_pool + content_pool`。
- 新增 `entry_contents` 正文存储逻辑、正文哈希写入、正文图片本地化回写的双库实现。
- 刷新写入路径改为：
  - 先写 feed metadata
  - 再写 index entries
  - 再写 content entries
  - 最后回写 `has_content`
- `NativeSqliteBackend` 新增 `content.db` 路径推导、连接与迁移方法。
- Desktop / CLI 初始化已切到 `index.db + content.db`。

### Browser / Static Web 逻辑分层

- `BrowserState` 拆分为：
  - `core.entries`（索引）
  - `entry_content`（正文）
  - `entry_flags`
  - `app_state`
- browser query / refresh / entry adapter 已改为逻辑双仓储实现。
- localStorage 新增独立 `ENTRY_CONTENT_STORAGE_KEY`。
- `count_entries()` 已与 `list_entries()` 解耦，不再继承 `limit` 裁切语义。

### Migration / Tests

- 新增 `migrations/0002_split_entry_content.sql`：
  - 迁移旧 `entries` 表到不含正文列的新结构
  - 计算并保留 `has_content`
  - 不迁移旧正文缓存
- 新增 `migrations_content/0001_initial.sql` 创建 `entry_contents`。
- 批量更新 application / infra / wasm harness 测试夹具，适配新仓储边界和双库初始化。
- `test_sqlite_bootstrap` 已扩展为同时断言 `rss-reader.db` 与 `rss-reader-content.db` 启动成功。
- 新增回归测试：
  - browser query 的 `count_entries_ignores_limit`
  - repository 批量正文写入后的 `entry_id` 解析与正文读取
- 修复 `test_entry_large_dataset_performance` 的旧 schema 直接灌数逻辑，使其适配 `index.db + content.db`。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo check --workspace`：通过
- `cargo test --workspace --no-run`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-infra --test test_sqlite_bootstrap --test test_feed_refresh_flow --test test_application_refresh_store_adapter --test test_entry_state_and_search --test test_config_exchange_contract_harness --test test_subscription_contract_harness --test test_webdav_local_roundtrip --test test_config_package_io`：通过
- `cargo test -p rssr-infra --test test_entry_state_and_search --test test_entry_large_dataset_performance`：通过
- `cargo test -p rssr-infra`：通过
- `cargo check -p rssr-infra --target wasm32-unknown-unknown --tests`：通过

### 手工验收

- Desktop 手工启动与页面路径：未执行
- Android / Web 手工交互：未执行

## 结果

- 当前改动已达到“可编译、关键测试通过、native 真双库落地、browser 逻辑分层对齐”的可继续集成状态。
- 尚未做 Desktop/Android/Web 的人工交互验收，因此更适合先合并到持续开发分支，再做 UI/运行态 smoke。

## 风险与后续事项

- `EntryRepository` 兼容 trait 目前仍保留，后续如果调用面全部切干净，可以再考虑收缩。
- 纯静态 Web 已按逻辑分层拆开，但尚未做专门的浏览器端性能回归与手工验收。
- 当前迁移策略明确接受“旧正文不迁移”，升级后正文依赖后续刷新回填；如果以后要做旧正文迁移，需要单独设计批量搬运与校验流程。
- 目前列表查询仍沿用现有 `LIKE` / 排序索引策略，这次只修正了 `count_entries` 语义，没有同时推进新的分页 SQL / cursor 方案。

## 给下一位 Agent 的备注

- 入口优先看：
  - `crates/rssr-domain/src/repository.rs`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-infra/src/db/entry_repository.rs`
  - `crates/rssr-infra/src/application_adapters/browser/`
- 如果继续推进分页 / cursor / page-count，请先基于新的 `EntryIndexRepository` 做，不要再把正文字段拉回列表路径。
- 如果继续推进 native 存储层，优先保持差异停留在 infra / adapter / host capability 层，不要把平台差异反灌到 application 语义。
