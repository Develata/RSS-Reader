# 顶栏偏好跨平台持久化、搜索框失焦修复与死代理清理

- 日期：2026-07-26
- 作者 / Agent：Claude Code
- 分支：main
- 当前 HEAD：2b35d18
- 相关 commit：pending
- 相关 tag / release：N/A（上一版 v0.1.11）
- 状态：`validated`

## 工作摘要

推进上一轮遗留的开放事项里可以安全落地的部分：去掉搜索框「点一下就跳走」的交互缺陷、
把四处重复的时间格式化合并成一处、删掉注册了但走不通的桌面图片代理，并按用户决定把顶栏
搜索词与导航收起状态从「只有 Web 记得住」统一成三端都记得住。

**本轮还有一项做了又撤回**：原计划补一条「切换已读/收藏后单独刷新阅读导航」，评审阶段
证伪了它的前提（见「被撤回的改动」），已完整回退。

未动的两项（列表 SQL 分页、目录展开态双写）仍然开放，各自需要单独一轮设计。

## 影响范围

- 模块：
  - `crates/rssr-app/src/datetime.rs`（新增）
  - `crates/rssr-app/src/ui/shell_prefs.rs`（新增）、`ui/shell_browser.rs`、`ui/shell.rs`、`ui/mod.rs`
  - `crates/rssr-app/src/pages/entries_page/{cards,groups}.rs`、`pages/reader_page/support.rs`、`pages/feeds_page/sections/support.rs`
  - `crates/rssr-app/src/ui/runtime/reader.rs`（仅格式化函数换名）
  - `crates/rssr-app/src/{main,app}.rs`、`crates/rssr-app/Cargo.toml`
  - `crates/rssr-infra/src/db/sqlite_native.rs`
- 平台：
  - Windows / macOS / Linux / Android / Web
- 额外影响：
  - 原生端新增一个偏好文件 `<数据目录>/shell-prefs.json`（见「风险与后续事项」）

## 关键变更

### 顶栏搜索框：`onfocus` 不再跳转

- 此前 `onfocus` 和 `onsubmit` 各自跳一次 `EntriesPage`。点进搜索框会立刻换路由，
  把刚点中的 input 卸载掉，焦点随之丢失——想搜索得点两次。
- 删掉 `AppShellState::focus_search` / `AppNavShell::focus_search` 与 `app.rs` 的 `onfocus`，
  只保留回车提交那条路径。搜索词存在 `AppShellState`（由 `App` 经 context 提供，路由切换不重建）
  而不在页面里，跳转前输入的内容原样带过去。

### 顶栏偏好：三端一致地持久化（用户决定）

- 搜索词与导航收起状态此前只有 Web 端存进 `localStorage`，桌面 / Android 每次启动都回到
  「未筛选、导航展开」。用户明确选择**三端都记住**（含搜索词，因此桌面重启后列表会保持筛选态）。
- 新增 `ui/shell_prefs.rs`，按平台二选一：wasm 沿用原有的两个 `localStorage` 键
  （`rssr-entry-search` / `rssr-nav-hidden`，老用户已存的值继续生效）；原生端落到
  数据库同目录的 `shell-prefs.json`。
- **刻意保持同步读写**：`use_app_shell_state` 首帧就要拿到值。改走 `AppStateSnapshot` 那条
  异步链路的话，首帧会先渲染成空搜索框加展开导航、下一帧才跳成持久化的值——
  Web 端现有体验会退化出一次闪烁。因此这层不进 domain / application，就是宿主能力适配。
- 原生端有进程内缓存，写入不必先读盘做 read-modify-write，两个字段不会互相覆盖；
  值没变则不落盘。路径解析与 `create_dir_all` 各只做一次（`OnceLock`），不在按键路径上；
  锁中毒时走 `into_inner()` 继续用最后一份值，不静默丢掉用户输入。
- **实测写入代价**：本机 200 次小文件写，`target/` 下 0.52 ms/次，被实时扫描的用户目录下
  1.11 ms/次。约占一帧（16.7 ms）的 3~7%，按键速率下不构成可感知卡顿，因此**不引入写入合并**
  ——合并的代价是进程被杀时丢掉最后几个字符，而「记住搜索词」不该记成一个前缀。
- `rssr_infra::db::sqlite_native::local_data_dir()` 由私有改为公开，让「本地数据文件放哪」
  只保留一处权威定义。**已逐字比对确认数据库解析路径三端未变**（`<base>/RSS-Reader/rss-reader.db`，
  base 解析规则、两条错误文案、Android 的 `HOME` → `temp_dir()` 回落、`content_database_path`
  派生全部原样），不会孤立用户已有的库。
- `shell_browser.rs` 收缩为只放 `complete_web_auth_transition`。

### 时间格式化合并

- `format_entry_date_utc`（`cards.rs` 与 `groups.rs` 各一份）、`format_reader_datetime_utc`、
  `format_feed_datetime_utc` 四处的格式串逐字相同，只是函数名不同。
- 合并为 `crate::datetime::{format_date_utc, format_datetime_utc}`，输出逐字节不变。
  因此 `groups.rs` 的 `BTreeMap` 分桶键、`group_anchor_id(format!("{date}-{id}"))` 锚点串、
  以及分桶顺序都不变，滚动定位不会失效。
- 测试钉住两种格式，并覆盖「先转 UTC 再取日期」——少这一步同一篇文章会落进相邻的另一个日期分组。

### 删除 `rssr-img://` 桌面图片代理

- 协议处理器可用且有测试，但没有任何地方产出这种地址（产出侧此前已被清理），
  `sanitize_reader_html` 的白名单也不含这个 scheme，即使产出也会被消毒掉。
- 整体删除：常量、UA、client、响应处理、`simple_proxy_response` 与其测试，以及
  `Config::with_asynchronous_custom_protocol` 的注册；顺带移除桌面 target 下已成死依赖的
  `reqwest`（rssr-app 内仅剩的两处用法都在 wasm 门控下，与 workspace 的 feature 统一结果无关）。
- **删除前确认过能力覆盖**：`BodyAssetLocalizer` 抓图时已经转发 `Referer` 与同一串 `Accept`，
  这正是防盗链检查依赖的部分，所以删除不会让现在能加载的图片失效。
  仅有的差量是代理会伪装成 Chrome 的 `User-Agent` 并带 `Accept-Language`；
  没有一并搬进 localizer 是刻意的——localizer 现在用的是如实自报身份的 UA，
  把它换成 Chrome 伪装是一次没人要求的行为变更。若日后确实遇到只认浏览器 UA 的图床再单独讨论。

## 被撤回的改动：切换标记后刷新阅读导航

先实现了 `ReaderService::navigation()` + `ReaderPageIntent::SetNavigation`，让切换已读/收藏后
单独重取一次「上一篇 / 下一篇」。评审阶段前提被证伪，**已完整回退**（四个文件 `git checkout`，
`ui/runtime/reader.rs` 定点还原）。

证伪依据（两端各一份，均已复核原文）：

- 原生：`crates/rssr-infra/src/db/entry_repository.rs:436-452`，相邻条目谓词是严格不等
  `sort_at > X OR (sort_at = X AND id > current_id)`。当前条目自身满足 `sort_at = X ∧ id = current_id`，
  两个分支都不满足，被精确排除。反向分支同理。
- Web：`crates/rssr-infra/src/application_adapters/browser/query.rs:196-213` 用
  `ordered_entries[..index]` 与 `[index + 1..]`，同样把 `index` 位（当前条目）切掉。

即 `reader_navigation(id)` 的输出对条目 `id` 自身的 `is_read` / `is_starred` **恒定不变**，
「标完已读按下一篇会跳回刚标掉的那一篇」不可能发生。d70b2b5 的提交信息里已经写过这个结论。

代价一侧同样成立：`spawn_projected_ui_command` 会 `await` 完整个 intent 列表再逐条 apply，
所以那次多出来的导航查询会挡在 `SetStatus` + `PatchEntryFlags` 前面，把星标与状态条的反馈
推迟到查询回来之后——恰好抵消上一轮 d70b2b5 想拿到的收益。Web 端每次切换还要多做一次
全量 flag 建索引 + 全量排序。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- `cargo test --workspace`：通过（235 passed / 0 failed，含本轮新增 6 条）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：失败（`cc-rs: failed to find tool "clang.exe"`，
  本机无 NDK，与本次改动无关；上一轮同样现象已由 CI 构建成功验证）

注：中途一次 `cargo test --workspace` 报过 `E0462 found staticlib std` / `can't find crate for rssr_infra`，
是与 android target 检查共用 `target/` 目录造成的产物冲突，串行重跑即恢复，非代码问题。

### 手工验收

- 未执行（改动的行为分支都有单元测试覆盖；界面层三处可见变化需在真机/浏览器确认，见「结果」）

## 结果

- 可合并。
- 用户可见的变化共两处：
  1. 点搜索框不再立刻跳走导致失焦（修缺陷）；
  2. 桌面 / Android 重启后会保持上次的搜索词与导航收起状态（用户明确要求的行为）。
- 阅读体验的其余部分（正文排版、主题、分组、分页、卡片信息、标记切换的反馈时序）逐字节不变。

## 风险与后续事项

- **新增了一个磁盘文件**：`<数据目录>/shell-prefs.json`。不做版本协商也不做迁移——
  缺失、截断、类型不符一律回落默认值（有测试覆盖六种坏输入）。它是本仓库第二类落盘数据，
  卸载清理与配置导出若要覆盖它需另行决定（当前**不**进配置包，属本机界面偏好而非可交换配置）。
- **搜索词持久化的副作用**：在订阅页/设置页往顶栏搜索框打字，现在既不跳转也已落盘；
  没按回车就切走或关掉应用，下次启动文章列表直接带筛选，看起来像「文章少了」。
  这是用户明确选择的语义，未加任何提示。桌面端此前没有这个状态。
- `cargo clippy -p rssr-app --target wasm32-unknown-unknown` 会报
  `refresh_service.rs:284` 的 `needless_return`。属既有问题（本轮未改该文件），
  且不在 CLAUDE.md 记录的验证命令内，未一并处理。
- **阅读导航的真实缺口（仍然开放，仍属已接受的取舍）**：阅读页开着时，后台刷新新插入的条目
  不会被导航目标感知，要换一篇文章才重取。这不是切换标记造成的，因此也不能靠「切换后重取」
  修好——真要修得挂到刷新完成事件上。优先级低。
- **仍然开放的两项**：
  1. 列表 SQL 分页（`EntriesPageState::entry_query` 仍是 `limit: None`）。建议做法：
     只替换目录/导航那棵 `all_groups` 树的数据来源，渲染树继续用分页窗口。聚合需返回
     分组标题、条目数、首条 id（锚点字符串由它拼出，换了会破坏滚动定位）与首条在完整排序中的
     序号（`target_page = 序号 / 每页数量 + 1`，SQLite 用 `ROW_NUMBER() OVER (...)`）。
     属骨架级改动，实现前需先定设计。
  2. 目录展开态由 VDOM 与 `syncGroupState` 双写（`controls.rs` + `browser_interactions.rs`）。
     建议做法：脚本降级为**传感器**（只观察滚动、上报当前锚点到信号），Rust 作为唯一
     **执行器**（所有 `data-open` / `aria-expanded` 由状态渲染）。直接删掉脚本的写入会
     连带杀掉滚动驱动的高亮。

## 给下一位 Agent 的备注

- 入口文件：`crates/rssr-app/src/ui/shell_prefs.rs`（跨平台偏好，注释里写明了为什么必须同步）、
  `crates/rssr-app/src/datetime.rs`（时间格式的唯一来源）。
- **改阅读导航之前先读本文「被撤回的改动」一节**，以及 d70b2b5 的提交信息：
  当前条目不参与自身的邻居计算，这一点已经被误判过一次。
- 继续推进前先读：`docs/handoffs/2026-07-26-reader-toggle-and-boundary-validation.md`、
  `docs/handoffs/2026-07-26-entries-grouping-tree-index-payload.md`（分页那项的前置背景）、
  `crates/rssr-app/src/pages/AGENTS.md`。
- 页面层改动默认同时影响 Web / 桌面 / Android，改完至少跑 wasm target check。
