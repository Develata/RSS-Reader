# 文章列表、归档计数与订阅摘要不再逐行读全部文章

- 日期：2026-11-12
- 作者 / Agent：Claude（Opus 5.5）
- 分支：`claude/storage-linear-queries`（基于 `origin/main` `7d60a59c`）
- 当前 HEAD：见分支
- 相关 commit：本分支首个提交；评审修正另记于下文
- 相关 tag / release：N/A（未发布）
- 状态：`validated`（本地；未推送、未建 PR）

## 工作摘要

同日的只读审查用生产 SQL 实测发现三处随文章总数线性增长、且每次加载都执行的查询：未限订阅的文章列表把全部文章排序后取一页；列表旁的归档计数和订阅页的未读 / 总数逐行读全部文章。本次用一个覆盖索引、固定连接顺序和归档条件的等价改写修复，25 万篇时默认首页列表 34.6 → 3.2 ms、归档计数 195.1 → 8.0 ms、订阅摘要 199.4 → 8.2 ms。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/store/`：`entries.rs`、`feeds.rs`、`migrate.rs`、`migrations/0003_entry_flag_index.sql`、新单测 `query_plans.rs`
  - `crates/rssr-infra/tests/`：新 `test_library_entry_filter_combinations.rs`；`test_library_baseline_schema.rs`、`test_library_store.rs` 的迁移数量
- 平台：全部原生宿主（桌面、Android、CLI、服务器）与 Pages 演示的内存库都会执行迁移 0003；正式 Web 经服务器，不直接受影响。
- 额外影响：资料库迁移。升级后的库不能再由 v0.2.3 及更早版本打开（与 v0.2.1 的 0002 相同），**下一次发布说明必须写明，回退须恢复升级前备份**。

## 关键变更

### 索引（迁移 0003）

- `idx_entries_feed_flags ON entries(feed_id, is_read, is_starred, is_archived, published_at)`：任何读 / 收藏 / 归档筛选下的计数、订阅的未读与总数都只读该索引。只加索引，不改数据。

### 查询（`store/entries.rs`）

- 资料库不跑 `ANALYZE`，连接顺序靠 `CROSS JOIN` 固定：未限订阅的列表从 `entries` 出发，沿 `idx_entries_sort_key` 取到一页即停；限定订阅的列表保持规划器的选择；所有计数从 `feeds` 出发，逐订阅读覆盖索引。
- `OnlyArchived` 改写为 `NOT (<ExcludeArchived 条件>)`，与原 `OR` 写法在 `is_archived ∈ {0,1}` 下逐行等价；原写法在新索引存在时会被拆成多索引并集，部分组合慢 30–50 ms。
- 只调整了 SQL 形状和连接顺序，筛选语义、排序、上限与参数都不变。

### 测试

- `store::query_plans`（4 项）：在捆绑的 SQLite 上钉住上述计划，包括全部 135 种非搜索计数都只读索引、`OnlyArchived` 不出现 `MULTI-INDEX OR`。
- `test_library_entry_filter_combinations`：3 个订阅（含已删除）× 72 篇覆盖全部发布时间与标记组合，360 种筛选形状的计数、无上限列表与 `LIMIT 3` 列表都与 Rust 里按定义求出的集合和顺序一致。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：通过，67 个测试二进制 1204 passed / 0 failed / 5 ignored（macOS arm64）
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown --features pages-demo --locked`：通过
- `cargo check -p rssr-infra --target wasm32-unknown-unknown --features memory-store --tests --locked`：通过
- 性能对比：见[测量证据](../testing/evidence/2026-11-12-storage-queries/README.md)。Python 3.13 自带 SQLite 3.53.1 上的 5 万 / 25 万篇夹具，两版结果逐行一致；计划由 `store::query_plans` 在捆绑的 3.53.2 上确认。

### 手工验收

- 真实界面打开大资料库：未执行（没有用户规模的真实库）
- wasm 契约 harness（`wasm_memory_store`）与 Pages 页面验收：未执行
- Linux / Windows 上的测试：未执行（未改执行器或文件层）

## 结果

- 可提交评审。改动只在 infra 的存储层，application / domain 语义不变。
- 用户可见：大资料库下文章页、订阅页打开更快；升级后不能直接降级到 v0.2.3。

## 风险与后续事项

- 代价：刷新式 upsert 约 +5%，一次标记 2000 篇已读 30.9 → 44.6 ms；25 万篇升级建索引约 0.4 秒，库文件 +2.9%。
- 已知小退化（均 ≤ 6 ms）：“只看收藏”的计数不再走收藏索引；未限订阅的标题搜索列表约 +2%。
- 未修、另行处理：单订阅的“已读”列表走全局未读索引，25 万篇约 177 ms；刷新 upsert 的无变化写入与可能无用的历史索引仍是 M05。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-infra/src/store/entries.rs` 中 `LISTED_FROM_ENTRIES` / `COUNTED_FROM_FEEDS` 的注释，以及 `store/AGENTS.md` 的新条目。
- 改筛选 SQL 或 `entries` 索引时，先跑 `store::query_plans` 与 `test_library_entry_filter_combinations`，再用证据目录里的脚本对全部形状做前后对比。
