# 文章页分组树改为携带条目引用，切换已读/收藏不再重建

- 日期：2026-07-26
- 作者 / Agent：Claude (math-architect)
- 分支：main
- 当前 HEAD：pending（本记录随同一批改动提交）
- 相关 commit：pending
- 相关 tag / release：`v0.1.10` 之后，尚未发布
- 状态：`validated`

## 工作摘要

接 `2026-07-26-presenter-memo-and-server-session-recovery.md` 里记为「本轮未做」的第 1 项：
让分组树不再携带带标记的条目对象，从而消除读/收藏切换引发的分组树重建。

上一轮把 presenter 的 memo 依赖收窄后，一次切换从「建树 2 次」降到「建树 1 次」——
剩下的那 1 次来自 `PatchEntryFlags` 本身：它改的是条目自身的 `is_read` / `is_starred`，
而分组树的叶子直接持有这些条目（`Vec<Arc<EntrySummary>>`），`Arc::make_mut` 换掉一个 `Arc`
就让投影不相等。本轮把叶子换成「指向条目的引用」，标记与标题因此彻底退出分组树的输入，
无筛选条件下一次切换的建树次数降到 **0**。

## 影响范围

- 模块：`crates/rssr-app/src/pages/entries_page/{groups,presenter,facade,cards,mod}.rs`
- 平台：全平台（页面层改动）
- 额外影响：无迁移、无 workflow 变更、无用户可见行为变化（纯内部表示与缓存粒度）

## 关键变更

### 1. 分组树叶子：条目对象 → `EntryCardRef { index, id }`

`groups.rs` 现在完全不依赖 `rssr_domain`，输入是新的 `EntryGroupKey<'a>`：

```rust
pub(crate) struct EntryGroupKey<'a> {
    pub(crate) index: usize,          // 完整可见列表中的绝对下标
    pub(crate) id: i64,
    pub(crate) feed_title: &'a str,
    pub(crate) published_at: Option<OffsetDateTime>,
}
```

只含分组真正依赖的字段，**结构上取不到** `title` / `is_read` / `is_starred`。
叶子（`EntryDateSourceGroup` / `EntrySourceMonthGroup`）从 `Vec<Arc<EntrySummary>>`
变成 `Vec<EntryCardRef>`。

顺带省掉的开销（不是本轮目标，是副产品）：分组过程原先每层都 `Arc::clone`
（时间模式 3 层 ⇒ 每条 3 次原子加减），来源分组与日期内来源分组还各 `clone` 一次
`feed_title` 作 `BTreeMap` 键；现在键是 `&'a str`，叶子只装两个整数，
`paged_entries.to_vec()` 那次分配也随之消失。`BTreeMap<&str, _>` 与
`BTreeMap<String, _>` 的顺序一致，分组顺序不变。

### 2. 投影：`GroupingEntries` 与刻意变粗的相等性

`EntriesPresenterInput.entries` 换成 newtype `GroupingEntries(Vec<Arc<EntrySummary>>)`，
手写 `PartialEq`：比较数量、顺序，以及每个位置的 `id` / `published_at` / `feed_title`
（未改动的条目共享 `Arc`，先走 `Arc::ptr_eq` 短路）；**不比较** `title` / `is_read` /
`is_starred`。它只暴露 `len()` 与 `grouping_keys()`，没有任何途径读到标记。

newtype 是刻意的：如果直接把 `Vec<Arc<EntrySummary>>` 放在投影里，日后往分组标题加一句
「未读 N 篇」就会读到 memo 缓存里那份过期标记，且不会有任何编译错误。

由此得到本轮的核心不变量：

> 两份投影相等 ⇒ 条目数量相同，且每个下标处仍是同一条目。

所以 presenter memo 复用旧树时，树里的下标依然有效。

### 3. 渲染树与全量树共用同一套绝对下标

`from_input` 只建一次全量键，分页树用它的**子切片**：

```rust
let grouping_keys = input.entries.grouping_keys();
let paged_keys = &grouping_keys[page_start_index..page_end_index];
```

这样「相对下标」这种表示根本不存在——它会让第 2 页的卡片解析到第 1 页的条目上。
`find_active_{time,source}_anchors` 相应从「按条目 id 查」改成「按下标查」，
入参是当页首条的绝对下标。

### 4. 卡片渲染时解析条目，且以 id 为判据

`EntriesPageFacade::entry_at(card)` 从每帧新建的 snapshot 里取条目：
`entries.get(card.index)` 是 O(1) 快路径，但**必须** `entry.id == card.id` 才采用，
否则退化为按 id 线性查找，找不到则返回 `None`（卡片这一帧不渲染）。

为什么不只用下标：分组树取自 memo 缓存，而「投影 memo 重算」与「组件重渲染」是调度器里
两件独立的工作。已核对 dioxus 0.7.9 源码，正常次序是安全的：

- `dioxus-signals/src/memo.rs` 的 `try_read_unchecked` 会 `swap` 掉 `dirty` 标记并
  **同步重算**（注释引用了 issue 2416），因此读 memo 链本身具有自愈性；
- `dioxus-core/src/scheduler.rs:203-222`：脏 scope 与脏 task 的 `ScopeOrder` 相等时，
  `Work::PollTask` 优先于 `Work::RerunScope`，所以投影 memo 的重算任务先于组件重跑。

但这条次序一旦在某个路径上不成立，只按下标解析就会把**另一篇文章**渲染到该位置上
（`key` 也会跟着错）。校验 id 的代价是每张卡片一次整数比较，换来的是
「最坏情况少渲染一张卡片、下一帧自愈」而不是「渲染错文章」。按照
correctness > performance 的优先级，这个交换是划算的。

## 复核意见与落地情况

一次独立正确性复核（子代理，读了 vendored 的 dioxus 源码而非猜测）逐条核对了相等关系是否覆盖
presenter 全部输出字段、`find_active_*` 语义变更的五种边界、分页树 `target_page` 的消费方、
`default_expanded_directory_sections` 的锚点匹配、`BTreeMap<&str,_>` 与 `BTreeMap<String,_>`
的顺序等价、以及条目 id 唯一性（原生 `entries.id` 是主键且查询是一对一 join；浏览器端按 id 建
`HashMap`）。**结论：未发现会渲染出错误条目或错误分组的正确性缺陷**，无 CRITICAL / HIGH。

以下为它提出的两项以及处理：

- **（不接受，但据此改进了注释）复核认为「分组树不会过期」的真正依据是
  `Memo::try_read_unchecked` 的读时同步重算，调度器次序只是旁证，即使次序反过来结论也成立。**
  重新核对 `dioxus-signals-0.7.9/src/memo.rs:166-196` 后确认这个判断不成立：`needs_update`
  为假时该函数走 `else` 分支直接返回缓存值，**不会去看上游**。因此「状态信号 → 投影 memo」
  这一跳只能靠调度器把投影 memo 的重算任务排在组件重渲染之前（`scheduler.rs:203-222`），
  读时自愈只能保证「投影 memo → presenter memo」那一跳。两跳的保证强度不同，
  也正因为第一跳依赖调度器内部次序，`entry_at` 的 id 校验**不是**可以省掉的防御性代码。
  已把 `entry_at` 与 `EntryCardRef` 的注释改写成按两跳分别说明，并写明「没被标脏的 memo
  直接返回缓存值」这一点，避免后续维护者误判其中任一跳的强度。
- **（接受）`from_input` 过长且两个 match 分支高度重复**（约 117 行，超过项目「函数 < 50 行」
  准则）。已抽出 `GroupingOutcome` 与 `GroupingOutcome::by_time` / `by_source`，
  顺带消掉了原来那个 8 元组的解构。`from_input` 降到约 62 行，其中 17 行是
  `EntriesPagePresenter` 的 16 字段字面量——要再降需要改 presenter 的字段布局并连带改动
  facade 的 8 个访问器，本轮**未做**。抽出的两个构造函数结构仍然平行（两种树类型不同，
  这是固有的），但「改了一条忘了改另一条」现在是两个具名函数的差异，看得见。

另外复核在过程中观察到工作区文件出现过一个把分页树下标改回**相对下标**的中间变量 `mutant`，
并提示提交前确认工作区干净。那是我为验证测试非空转而**故意注入**的变异（见下「变异测试」一节），
注入后立即还原；`grep` 计数与最终 fmt/clippy/test 全量重跑均已确认残留为零。

## 收益（按实际可断言的口径）

- **无读/收藏筛选时**：一次切换的分组树重建次数 0（上一轮是 1，上一轮之前是 2）。
  依据：`only_grouping_relevant_entry_fields_invalidate_the_projection` 断言标记变化后
  投影仍相等，因此 presenter memo 不重算。
- **有筛选且切换后条目不再匹配时**：`PatchEntryFlags` 的 `retain` 会移除该条目，
  数量变化 ⇒ 投影失效 ⇒ 重建 1 次。这是必要的重建，不是回归。
- 单次重建本身也更便宜（见「关键变更 1」），但未做基准测量，不给具体倍数。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（0 输出）
- `cargo test --workspace`：通过（33 个测试二进制，224 passed / 0 failed）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-infra --target wasm32-unknown-unknown --all-targets`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：**未能执行**，
  本机缺 NDK 的 `clang.exe`（`cc-rs: failed to find tool "clang.exe"`），
  与本次改动无关；本轮未触碰移动端交互代码。

### 新增/调整测试

`groups.rs`：

- `leaf_refs_come_from_the_key_not_the_slice_position`：叶子里的 `(下标, id)` 必须来自
  传入的键，而不是切片内位置。
- `groups_entries_by_time_in_descending_month_order` 等改为断言 `entry_cards`。

`presenter.rs`：

- `only_grouping_relevant_entry_fields_invalidate_the_projection`：标记与标题变化 ⇒ 投影相等；
  `feed_title` / `published_at` / `id` / 数量变化 ⇒ 投影失效。**替代**了上一轮那条
  「已读标记变化必须被看见」——该保证已改由「卡片按引用从最新 snapshot 解析」提供。
- `reusing_the_cached_tree_keeps_indices_pointing_at_the_same_entries`：`entry_at` 快路径的前提，
  即投影相等时旧树的 `(下标, id)` 在新列表上依然自洽。
- `paged_group_leaves_carry_absolute_indices`：两种分组模式下第 2 页的叶子都必须是 `[2, 3]`。
- `empty_entry_list_produces_an_empty_presenter`：空列表时 `&keys[0..0]` 必须合法不 panic，
  越界的当前页夹回第 1 页，锚点为 `None`；两种分组模式各跑一遍。

`cards.rs`：`marks_list_edges_by_position`（`list_edge_state` 从 `mod.rs` 移来后补的覆盖）。

### 变异测试（确认新测试非空转）

手工注入两处缺陷并确认测试确实失败，随后完整还原：

1. 让 `GroupingEntries::eq` 一并比较 `is_read`
   ⇒ `only_grouping_relevant_entry_fields_invalidate_the_projection` FAILED。
2. 让分页树改用页内相对下标
   ⇒ `paged_group_leaves_carry_absolute_indices` FAILED（`left: [0, 1] right: [2, 3]`），
   且既有的 `presenter_marks_directory_item_for_current_page_first_entry` 也 FAILED
   （当页首条下标错位导致活动锚点从 beta 变成 alpha）——两条测试独立地卡住了同一个缺陷。

### 手工验收

- 未执行。建议回归：文章页大量文章时连续切换已读/收藏的流畅度；分页跳转后右侧目录的
  高亮项是否仍对应当页首条；时间/来源两种分组模式都要过一遍。

## 风险与后续事项

1. `session.snapshot()` 仍在每次渲染深拷贝整份 `EntriesPageState`（`feeds`、`status`、
   `selected_feed_urls`）。本轮让卡片解析依赖它，但没有加重它——这是上一轮就记下的既有开销，
   要去掉需要让 facade 改持信号而非快照。
2. **列表 SQL 分页**仍未做，依旧需要先设计分组聚合接口。
3. 全量分组树的叶子现在只为 `find_active_*_anchors` 提供下标，可改成按下标区间直接推导，
   连叶子都不必建；Source 模式仍会构建随即丢弃的 date 层。
4. `group_date_buckets` 仍对每个条目做一次 `format_entry_date_utc`（`time` 格式化 + `String`
   分配）来当 `BTreeMap` 键，是单次建树里最贵的部分。本轮刻意不动，以保持分组行为逐字节不变。
5. 页面层其余已确认未改的问题见 `2026-07-26-audit-remediation-round-2.md`。

## 给下一位 Agent 的备注

- 往 `EntriesPresenterInput` 加字段前先想清楚：加进去意味着该字段变化会重建两棵分组树。
  往 `GroupingEntries::eq` 的比较集合里加字段同理，而且要同步问一句：
  这个字段是不是本来就该由「渲染时解析」提供，而不是由缓存提供。
- 分组树的叶子**不要**再塞回条目对象。真需要某个条目字段参与分组，就把它加进
  `EntryGroupKey` 并同步加进 `GroupingEntries::eq` 的比较集合，两处必须一致。
- `entry_at` 的 id 校验不是防御性冗余，是 `(下标, id)` 这套表示的判据。
  删掉它就等于把「渲染错文章」的可能性重新引入——注意「状态信号 → 投影 memo」这一跳
  依赖 dioxus 调度器次序，不是靠 memo 读时自愈兜住的（读时自愈只覆盖第二跳，
  且**不会**让未标脏的 memo 去看上游）。升级 dioxus 版本时这一段值得重新核对。
- `from_input` 的两条分组流程在 `GroupingOutcome::by_time` / `by_source` 里，结构平行；
  改其中一条前先确认另一条是否需要同样的改动。
