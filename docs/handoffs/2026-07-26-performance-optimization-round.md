# 性能优化轮：文章页克隆、渲染期克隆、刷新并发与 WAL

- 日期：2026-07-26
- 作者 / Agent：Claude (math-architect)
- 分支：main
- 当前 HEAD：77c85fe
- 相关 commit：`3a07b94`（索引恢复）、`2be5118`（本轮优化）、`e248098`（对抗式复核修正）、`77c85fe`（正确性复核修正）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按上一轮审计给出的优化优先级落地第 1、3、4、5 项；第 2 项（列表 SQL 分页）仍按原计划保留，
因为它需要先设计分组聚合接口。过程中发现并修掉一个前置问题：数据库其实不是 WAL 模式，
这让「提高刷新并发」这件事本身带有风险。

## 影响范围

- 模块：
  - `crates/rssr-app/`：`pages/entries_page/{state,reducer,presenter}.rs`、`app.rs`、
    `components/entry_filters.rs`、`bootstrap/native.rs`
  - `crates/rssr-cli/`：`main.rs`（并发度与桌面端对齐）
  - `crates/rssr-infra/`：`db.rs`、`db/sqlite_native.rs`、
    `application_adapters/browser/state{,/storage}.rs`
  - `migrations/0005_restore_entry_sort_index.sql`（上一条 commit）
- 平台：
  - Windows / macOS / Linux / Android（WAL 与并发刷新）、Web（删除死代码）
- 额外影响：
  - **磁盘格式相关**：数据库改为 WAL 模式，目录里会多出 `-wal` / `-shm` 附属文件，
    已在 `README.md`「数据存储与缓存 → 桌面端」写明，含备份注意事项。

## 关键变更

### 文章页：去掉每次状态变化的全量深拷贝

`EntriesPageState.entries` 由 `Vec<EntrySummary>` 改为 `Vec<Arc<EntrySummary>>`。

presenter 每次重建都要把可见集合交给分组树，此前 `state.entries.iter().cloned().map(Arc::new)`
会深拷贝每条的 `title` 与 `feed_title` 两个 `String`。卡片组件本来就接收 `Arc<EntrySummary>`，
把 `Arc` 前移到状态里之后，重建只剩指针拷贝。

`PatchEntryFlags` 改用 `Arc::make_mut`：只有被点击的那一条会因为写入而克隆，其余继续共享。
行为不变——状态被写入后 memo 会重新求值，卡片拿到的是新的 `Arc`，因此不会读到旧标记。

### 渲染期冗余克隆

- `app.rs` 根组件把 `settings()` 读一次存进局部变量复用。此前一次渲染读 5 次，
  每次都克隆整份 `UserSettings`（含完整 `custom_css` 文本）。
- `components/entry_filters.rs` 不再为每个来源 chip 预先构造候选选中集合
  （O(来源数 × 已选数) 次克隆，其中只有被点的那一个会被用到），改为点击时才调用新的
  `toggle_source_selection`，并补上两条单元测试覆盖增删与去重。

### 刷新并发，以及落地前发现的 WAL 问题

`REFRESH_ALL_CONCURRENCY` 由 1 提到 4（CLI 用 `CLI_REFRESH_CONCURRENCY` 对齐）。
此前 `RefreshService::refresh_all` 里的 `JoinSet` 并发分支等于从未被使用；配合每个源 30s
的请求超时，N 个源最坏要串行等 N × 30s。

**落地前实测发现 journal 模式其实是 `delete`（回滚日志），不是 WAL。** 并发写之所以没报错，
只是因为 sqlx 设了 busy_timeout 让写者排队。这种模式有两个问题：
写事务会阻塞读，也就是刷新时前台查询会被卡住；批量较大时排队还可能超时并抛
`database is locked`。因此显式启用 WAL 并把 busy_timeout 设为 30s，统一在
`db::connect_options_for_path` / `create_sqlite_pool` 里设置（内存库跳过 WAL，它不支持）。

注意：SQLite 同一时刻仍然只允许一个写者，所以**并发度提升主要作用在网络抓取阶段**
（也正是超时所在的阶段），数据库写入依旧是串行的。不要把它理解成写入也快了 4 倍。

### Web 端

删除从未被调用的 `save_entry_content_patch`（定义 + 再导出，无调用点）。

## 对抗式复核结果（子代理）与随之的更正（`e248098`）

派了一个子代理逐条**尝试反驳**本轮的四个性能声明。结论是四条全部「部分成立」，
其中两条我说过头了，另外查出三处实际问题——都已修。

### 被证伪/需要更正的声明

- **「深拷贝是文章页的主要分配开销」不成立。** 分组树自身的 String 分配量与之相当：
  `groups.rs` 里每条目会为日期键与 `feed_title` 键各分配一个 String，每个分组还有
  标题/副标题/anchor 等约 4-5 个。时间模式下叶子分组是「日期 × 来源」，分组数可能接近
  条目数的一半，于是分组分配约 4.5N-5N，与被去掉的约 6N 同量级。
  **本次改动是约 40-55% 的削减，不是消除主要开销。**
- **「并发度 4 约等于 4 倍」不成立。** 墙钟时间约为 `max(总延迟/并发度, 最大单源延迟)`。
  正是我在注释里举的「有源挂到 30s 超时」的场景下收益最小：20 个 0.5s 源 + 2 个 30s 超时源
  串行 70s，并发 4 只降到 30s（2.3 倍）；只有一个挂住的源时约 1.3 倍。尾部延迟决定下限。
- **「目录路径从不需要条目对象」不成立。** `find_active_time_anchors` /
  `find_active_source_anchors`（`groups.rs:178-216`）确实要下钻到叶子读 `entry.id`，
  它们是全量树叶子的唯一消费者。原则上可以只用索引区间推导，但那是尚未做的重构。

### 查出并已修的三处实际问题

1. **连接池没有余量（我引入的回归）**：`default_sqlite_max_connections` 对文件库返回 4，
   正好等于 `REFRESH_ALL_CONCURRENCY = 4`。四个刷新任务在提交期间可占满整个池，
   界面查询与正文本地化写回只能排队等连接——正是我想用 WAL 解决的问题换了个形式回来。
   池大小提到 8。
2. **`save_state_snapshot` 按值接收**，5 个调用点各自 `state.clone()` 深拷贝整个
   `BrowserState`（含全部正文）。改为按引用，保存设置与每次刷新提交都受益。
3. **`save_entry_flag_patch` 冗余读取**：每次切换已读/收藏都要把全部标记读出来并 JSON
   解析一遍再线性合并，而调用方刚改完的内存状态才是权威值。改为 `save_entry_flags_slice`
   直接写入。标记数量随累计阅读历史单调增长，不受归档窗口限制。

### 复核确认无误的部分

- `Arc::make_mut`（`reducer.rs`）行为正确，是预期的写时复制。
- 图片下载信号量确实通过 `Arc` 共享，且 localizer 全进程只构造一次、`spawn` 时只 clone，
  因此提高刷新并发**不会**放大图片抓取并发（上限仍是 2）。
- 读/收藏切换确实只写 flags 一个 key，不会重写整个语料库。
- 一个我没提到的额外收益：`EntrySummary` 派生 `Eq`，因此 `Arc<EntrySummary>` 的比较走
  指针快路径，presenter 的 `PartialEq` 结构比较从「比两个 String」变成「比指针」。

## 正确性复核结果（第二个子代理）与随之的修正（`77c85fe`）

第二个子代理独立复核了 Arc 改动的可见行为、`toggle_source_selection` 的等价性、以及并发度 4
的安全性。它独立地也定位到了 WAL 与连接池这两点（我已先行修掉），并另外报了两个 HIGH。

### 确认无误

- **Arc 改动不存在跨 mutation 的陈旧读**：`state.entries[i]` 的 `Arc` 始终至少有两个持有者
  （state 自身 + facade 持有的 `Arc<EntriesPageState>` 快照），因此 `Arc::make_mut` 实际总走
  「克隆后替换」，presenter 里的旧 `Arc` 保持旧值——与此前「presenter 深拷贝出独立副本」
  的可见行为一致。重渲染链路也通：置脏 → memo 重算 → `PartialEq` 发现差异 → 传播 →
  `render_entry_card` 是普通函数无 props memo 拦截。
- **`toggle_source_selection` 与原逻辑逐条等价**（含 `String`/`str` 比较语义、不排序的移除
  分支、`push + sort + dedup` 的新增分支）。
- **并发提交不会死锁**：`SqliteRefreshStore::commit` 全程没有跨库嵌套持有——
  `upsert_entries_and_resolve_contents` 先 `tx.commit()` 再做后续查询，`upsert_contents`
  先提交 content 事务再回 index 库跑 `mark_has_content`，`feed_repository` 全是单语句自动提交。
  不存在 index→content 与 content→index 的循环等待。
- 持 `MutexGuard` 跨 localStorage 写入不构成问题：区间内没有 `.await`，`setItem` 不会回调进
  Rust（`storage` 事件只投递给其他文档）。

### 两个 HIGH（都不是新代码，但被并发化从死路径变成主路径）

1. **单个订阅失败会 abort 兄弟任务并丢弃整轮结果**（`refresh_service.rs`）。
   `outcomes.push(outcome?)` 的 `?` 会在 `JoinSet` 仍有在飞任务时返回，`JoinSet::drop` abort
   剩余任务——它们可能正停在写事务里（sqlx 会回滚，但该订阅会留在「元数据已更新、条目未写入」
   的半更新状态）；而且整轮结果连同已成功的订阅一起丢掉，既不做图片本地化也给不出 per-feed
   失败报告。并发度为 1 时这条分支根本不会走，因此这是并发化引入的**新**失败模式，
   而 30s busy_timeout 之后的 `SQLITE_BUSY` 正是最可能的触发源。
   已改为把单个订阅的失败收敛成该订阅的 `Failed`，`Err` 只留给整轮级前置失败；
   任务 panic 按 task id 归到对应订阅；结果按 `feed_id` 排序消除完成顺序造成的行序抖动。
2. **WAL 的启用是尽力而为且无人校验**（`db.rs`）。已改为启动时实测 journal 模式，
   不是 `wal` 就 warn 并把并发度退回串行，并把实际模式写进启动日志。

### 其余按复核意见一并收拾

- 并发度常量收敛到 `rssr_application::DEFAULT_REFRESH_CONCURRENCY`，池大小由它推导，
  桌面端与 CLI 都引用它（此前三处各写字面量，改一处不会让另两处报错）。
- `is_memory_database` 也认 `sqlite://:memory:` 与裸 `:memory:`。
- `entry_filters` 注释此前夸大改进（每个 chip 仍克隆一份选中集合，省掉的只是多余的
  `sort + dedup`），已如实改写并删掉多余的 `url` 克隆。
- browser 端 `save_entry_flags_slice` 注释补上适用范围（单标签页内权威；跨标签页整片覆盖
  会丢另一个标签的标记改动——不是新问题，但覆盖频率变高）。
- 并发写测试的说明改为如实：并发写那一半并不能区分 WAL 与回滚日志（两种模式都有
  busy_timeout，且写事务第一条就是写、不存在读升级），真正起守护作用的是 journal 模式断言；
  去掉重言式断言；两个池都关闭。**清理保持尽力而为、不做断言**——我一度把它写成硬断言，
  结果在整套跑时偶发失败：句柄释放到文件可删之间有 OS 时间差，断言它就是制造 flaky。
- `WebAuthGate` 把 hook 调用移到提前 return 之前，消除 hook 顺序隐雷。
- `app.rs` 恢复「仅在已认证时读 settings」的短路语义：无条件读会让登录门禁也订阅 settings。

### 复核指出、但**未改**的两项

- `content_pool` 也用 8 条连接（每条连接一个 OS 后台线程，两库峰值 16 线程）。
  content 库同样有最多 4 个并发写者（`upsert_contents`）加阅读页读取，判断 8 可接受；
  若要精简可给两个池分别设容量。
- 跨标签页的标记同步需要走 `storage` 事件，属于新增能力，不在本轮范围。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（exit 0）
- `cargo test --workspace`：**连续两次**全绿（各 32 个测试二进制，0 failed）——
  第二次是为了确认清理逻辑改回尽力而为之后不再偶发失败
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- 新增测试：
  - `rssr-infra`：`test_concurrent_refresh_writes` —— 4 个订阅各 60 条并发批量写入必须
    全部成功，并**断言 journal 模式为 WAL**，防止以后改连接选项时静默退回 `delete`。
  - `rssr-app`：`toggle_source_selection` 两条。

### 手工验收

- 未执行。建议手工回归：
  - 桌面端「刷新全部」在多个订阅下的表现，以及刷新期间界面是否还会卡（WAL 的主要收益）
  - 文章页大量文章时切换已读/收藏的响应
  - 首次启动一个**已存在的旧库**：确认 WAL 转换正常、`-wal` 文件出现且数据完好

## 结果

- 可合并。
- 用户可见影响：
  - 刷新全部明显更快（抓取阶段并发化），且刷新期间前台查询不再被写事务阻塞
  - 文章页在文章量大时切换已读/收藏更顺
  - 数据目录多出 `-wal` / `-shm` 文件（正常现象，备份时需一并复制）

## 风险与后续事项

1. **WAL 是磁盘行为变化**：对已有库是就地转换且可逆（`PRAGMA journal_mode=DELETE`），
   但 WAL 依赖目录可写，且在部分网络文件系统上不可用。本项目是本地优先应用，判断为可接受。
2. **列表 SQL 分页仍未做**（上一轮第 2 项），依旧需要先设计分组聚合接口；
   `idx_entries_sort_key` 已恢复，分页落地后它还能让扫描提前终止。
3. **分组树仍然每次状态变化重建两棵**（全量用于目录 + 当页用于渲染）。本轮只消除了
   逐条深拷贝，没有消除重建本身。
   复核还指出一个**比减少每次分配更值钱的杠杆**：presenter 的 `use_memo` 没有字段粒度，
   任何一次对状态信号的写入都会让它失效——`SetStatus`、`SetControlsHidden` 这类与分组
   完全无关的 intent 也会重建两棵树。**降低重建频率优于降低单次重建成本。**
   另外全量树的叶子（每条目 3 次 `Arc::clone` + Vec 槽位）只为给
   `find_active_*_anchors` 扫 id 用，而这可以改成用索引区间推导；
   Source 模式还会构建随即丢弃的 date 层（`dates: Vec::new()`）。
4. 上一轮记录的页面层 6 项「已确认未改」问题仍然待决，见
   `2026-07-26-audit-remediation-round-2.md`。

## 给下一位 Agent 的备注

- 改 `crates/rssr-infra/src/db.rs` 的连接选项前，先看 `test_concurrent_refresh_writes`：
  并发刷新依赖 WAL，退回 `delete` 会让刷新期间界面卡顿并可能报 `database is locked`。
- 文章页的条目现在是 `Arc<EntrySummary>`：新增写入路径时用 `Arc::make_mut`，
  不要把 `Vec<EntrySummary>` 重新塞回状态，否则每次重建的深拷贝会回来。
