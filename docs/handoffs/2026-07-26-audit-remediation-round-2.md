# 全仓审计整改（第二轮）：校验收敛、正文处理下沉、归档筛选下推、WebDAV 认证

- 日期：2026-07-26
- 作者 / Agent：Claude (math-architect)
- 分支：main
- 当前 HEAD：88101f7
- 相关 commit：`ce83443`、`981cc0e`、`8e3ef7a`、`98f38a1`、`b5de972`、`c360759`、`88101f7`、`a6e57a5`、`0005 恢复`
  （前置轮次见 `84169c5`）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

承接 `2026-07-26-repo-wide-audit-minor-fixes.md` 里记录的待确认项，在获得确认后逐条落地：
把三份配置校验收敛成一处、把正文 HTML 处理从页面层下沉到 infra 并顺带修好 Web 端裂图、
把归档筛选下推到查询层、给 WebDAV 同步补上认证，以及一批存储与抓取侧的加固。

## 影响范围

- 模块：
  - `crates/rssr-domain/`：新增 `validation.rs`，`entry.rs` 新增 `ArchiveFilter`
  - `crates/rssr-application/`：`settings_service`、`import_export_service`、`entries_list_service`
  - `crates/rssr-infra/`：新增 `html/`（`live_display` + `reader`），`db/entry_repository`、
    `config_sync/{file_format,webdav}`、`application_adapters/browser/query`
  - `crates/rssr-app/`：`pages/entries_page/*`、`pages/reader_page/support.rs`、
    `pages/settings_page/sync`、`ui/runtime/entries.rs`、`main.rs`
  - `crates/rssr-web/`：`proxy.rs`、`auth.rs`（测试隔离）
  - `migrations/0003_entry_sort_key_indexes.sql`、`migrations/0004_drop_unused_entry_sort_index.sql`
- 平台：
  - Windows / macOS / Linux / Android / Web / Docker
- 额外影响：
  - 新增一条索引迁移（只加索引，无数据变更，可回滚）
  - `README.md` 补充 WebDAV 凭据用法

## 关键变更

### 配置校验收敛（`ce83443`）

- 新增 `rssr_domain::validation`，成为设置与配置包校验的唯一来源。
- application 的 `rules.rs` 与 infra 的 `file_format.rs` 改为纯错误类型转换。
  此前 infra 侧接受 `version >= 1` 且完全不校验 `entries_page_size`，与 application 侧
  （要求 `version == 2` 且校验）不一致，同一份配置包在两条路径上结论不同。
- `CONFIG_PACKAGE_VERSION` 常量化，导出与校验共用。

### 存储与抓取加固（`ce83443`）

- `entries` / `entry_contents` 批量 upsert 包进事务：此前逐条 INSERT 各自隐式提交。
- `ensure_content_schema` 改为每实例只执行一次（`OnceCell`）：此前每次读正文都要多跑
  一次建表 + 两次建索引。
- `migrations/0003` + `0004`：按实际排序键 `COALESCE(published_at, created_at) DESC, id DESC`
  建表达式索引。此前索引建在裸 `published_at` 上，实际排序表达式一条也命中不了。

  **实测结论（EXPLAIN QUERY PLAN，20000 条 entries / 20 个 feeds，已 ANALYZE）**：
  - 阅读导航是真正的受益方，两种形态都在排序表达式上做 seek：
    「上一/下一未读」命中 `idx_entries_unread_sort_key`，
    「同订阅上一/下一篇」命中 `idx_entries_feed_sort_key`。
    这两个查询每打开一篇文章要跑 4 次，收益明显。
  - **列表查询的 ORDER BY 仍然走 TEMP B-TREE**，没有被这次索引改造消除：
    `list_entries` / `count_entries` 必须 JOIN feeds 过滤 `is_deleted`，SQLite 因此从
    feeds 侧驱动，无法按 entries 的排序键顺序输出。这一点最初被我说成「列表查询变快」，
    是过度声称，已按实测更正。
  - **关于 `idx_entries_sort_key` 我判断错了两次，最终结论见 `0005`。**
    先说它让列表变快（碰巧对），又用一条只 `SELECT entries.id` 的测试查询「更正」为无用并在
    `0004` 删掉（错），因为那种窄 SELECT 下覆盖索引会胜出，不代表生产查询。
    用真实 SELECT 列表复测：有该索引时计划是
    `SCAN entries USING INDEX idx_entries_sort_key` + feeds 回表，**没有 TEMP B-TREE**；
    删掉后退化为 `SCAN feeds` + `USE TEMP B-TREE FOR ORDER BY`。
    即 `0004` 实际让文章列表变慢，已由 `0005` 恢复该索引。
    教训：评估索引必须用生产查询的真实 SELECT 列表。
- `/feed-proxy` 与正文图片本地化改为边下边累计、超限即放弃，不再先整体读入再比大小；
  `/feed-proxy` 补上连接与请求超时。
- `rssr-web` 认证测试串行化：这些测试改进程级环境变量又并行执行，会互相踩掉对方设置的值。

### 正文 HTML 处理下沉（`981cc0e`）

- 新增 `rssr_infra::html`，**有意不加 `target_arch` 门控**：
  - `live_display`：原 `fetch/client/image_html.rs`，负责懒加载图片地址归一化。
  - `reader`：WordPress emoji 替换 + ammonia 消毒（从页面层迁入）。
- **修复 Web 端裂图**：`normalize_html_for_live_display` 原本埋在
  `#[cfg(not(target_arch = "wasm32"))]` 的 `fetch` 模块下，尽管它是纯字符串处理，
  导致 Web 阅读页拿不到地址归一化，同一篇文章桌面正常、Web 裂图。
- `reader_page/support.rs` 从 751 行降到约 130 行，只保留「显示什么」的选择与时间格式化；
  删除与 infra 重复的标签解析器、HTML 实体解码，以及为已放弃的桌面图片代理留下的死代码。
- 图片本地化相关类型按 target 门控，wasm 构建 0 warning。
- `rssr-app` 去掉 `ammonia` / `regex` / `feed-rs` / `quick-xml` / `chrono` 五个内容处理依赖。

### 归档筛选下推到查询层（`8e3ef7a`）

- domain 新增 `ArchiveFilter`（`All` / `ExcludeArchived` / `OnlyArchived`）并入 `EntryQuery`；
  归档只看 `published_at`，无发布时间的条目永不算归档，与 `is_entry_archived` 一致。
- SQLite 与 browser 两个适配器各自实现同一语义，新增契约测试
  `entry_repository_applies_archive_filter_in_query` 保证「未归档集合 + 已归档计数」互补。
- `EntriesListService::list_entries` 在同一个用例里用一次 COUNT 得出被隐藏的数量，
  页面不再为了显示「已归档 N 篇」而把已归档文章全量读回来。
- presenter 去掉归档过滤与 `now` 参数后成为 state 的纯函数，改用 `use_memo` 缓存：
  分组树只在状态变化时重建一次，而不是每次重绘重建两棵（全量 + 当页）。

### WebDAV 认证（`98f38a1`）

- 支持 endpoint 内嵌 userinfo，取出后走 HTTP Basic 并从 URL 剥掉，不残留在请求地址与
  错误信息里；百分号编码会被解码。
- 选择 URL 而非新增设置项的理由：endpoint 是设置页的**会话内状态**，既不写盘也不进
  `ConfigPackage`，放在这里不会被 `export_config` 导出、也不会被推到远端。
- 补上超时与 4MB 响应上限；401/403 时提示凭据写法。

### 设置输入上下界（`b5de972`）

- 刷新间隔、归档阈值、阅读字号缩放三个输入框改为 `number` 并带 `min`/`max`/`step`，
  上下界直接引用 domain 常量。此前是自由文本框，用户可以直接输入越界值——正是上一轮
  第一个 panic 的来源。现在 UI、application 校验、domain 常量三处对齐。
- 移除 `ArchiveFilter::only_archived`：该变体在 `EntriesListService` 里是就地构造的，
  这个辅助函数从未被调用。

### feed 代理的 DNS rebinding（`c360759`）

- `/feed-proxy` 此前先用 `lookup_host` 校验解析结果，然后仍按域名把请求交给 reqwest，
  客户端会**再解析一次**。两次解析之间，攻击者控制的 DNS 可以换成内网地址绕过校验。
- 改为把校验通过的 `SocketAddr` 一并返回，并用 `ClientBuilder::resolve` 把域名钉到该地址，
  保证「校验的」与「实际连接的」是同一个 IP；仍按域名发请求以保持 Host 头与 TLS SNI 正确。
- 重定向每跳都重新校验并重新钉，不复用上一跳的解析结果。

### 页面层补审后的三个修复（`88101f7`）

补齐第一轮未逐行读过的 `rssr-app` 页面文件，确认并修复：

- **阅读页快捷键会捕获顶栏搜索框的击键**：`onkeydown` 挂在包含 `AppNav {}` 的外层 `article`
  上，keydown 冒泡后，在搜索框里打 `m` 会把当前文章标记已读、打 `f` 切换收藏、方向键直接
  换页；且没有修饰键判断，`Ctrl+F`（浏览器查找）也会命中 `f` 分支。现在处理器挂在不含
  `AppNav` 的容器上，并且带修饰键的组合直接放行。
- **阅读页加载结果无归属校验**：`ApplyLoadedContent` 不带 `entry_id`，快速翻页时先发起的
  慢查询可能在后发起的之后落地——正文停在上一篇，而路由与「标已读」写的是当前这篇。
  现在结果带 `entry_id`，与 `current_entry_id` 不匹配即丢弃；`BeginLoading` 改由 session
  在发起加载前同步派发（加载中态因此真的渲染得出来）。顺带修好「已标记为已读」提示被同一篇
  文章的重载立刻抹掉的问题：只在真的换文章时才清提示。
- **订阅输入框劫持 Ctrl/Cmd+V**：先 `prevent_default` 再走 `ClipboardPort`，但桌面端实现是
  无条件报错，Firefox 上 `navigator.clipboard.readText` 缺失时又静默返回空——原生粘贴被吞，
  用户每次粘贴要么吃错误横幅要么毫无反应，只有 Chromium 系 Web 端可用。现在不再拦截，
  交给输入框原生粘贴；随之整条剪贴板链路成为死代码并一并删除（`ClipboardPort`
  host capability、命令、intent、facade/session 方法、`bootstrap/web/clipboard.rs`）。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（exit 0）
- `cargo test --workspace`：通过（31 个测试二进制全部 ok，0 failed）
- `cargo check -p rssr-infra --target wasm32-unknown-unknown`：通过（0 warning）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- 新增测试：
  - `rssr-domain`：`validation` 模块 5 项
  - `rssr-infra`：`html::reader` 6 项、`config_sync::webdav` 5 项、
    `entry_repository_applies_archive_filter_in_query`

### 手工验收

- 未执行。以下几项建议做一次手工回归：
  - Web 端阅读页图片显示（本轮修复的主要用户可见问题）
  - 需要登录的 WebDAV 端点推送 / 拉取
  - 文章页归档开关与「已归档 N 篇」计数

## 结果

- 可合并。各 commit 独立可回滚。
- 用户可见影响：
  - Web 阅读页的懒加载图片不再裂图
  - WebDAV 同步现在可用于需要登录的服务
  - 文章页在文章量大时不再把已归档文章一并读入内存，翻页与筛选更省
  - 配置包在文件路径与导入路径上的校验结论一致

## 风险与后续事项

1. **文章列表仍未做 SQL 分页**：`entry_query` 仍是 `limit: None`。这是有意保留的——
   目录树（月/日/来源导航）按设计跨越整个结果集，要做真正的 SQL 分页必须先给仓储加一个
   分组聚合能力（每组的标题、计数、首条在全序中的位置），否则目录会退化成只覆盖当前页。
   建议下一步单独设计这个聚合接口，而不是给现有查询硬加 limit/offset。
2. **桌面图片代理链路「注册了但走不通」**：`main.rs` 里的 `rssr-img://` 协议处理器可用
   且有测试，但没有任何地方会产出该 scheme 的地址，且 ammonia 白名单也不含它。
   常量已从页面层移到唯一使用方 `main.rs` 并注明现状，需要决定接通还是整体删除。
3. **`rssr-web` 认证测试**已串行化，但根因是这些测试依赖进程级环境变量；
   若后续新增同类测试，记得一并取 `auth::test_env::lock()`。

### 页面层补审中已确认但**未改**的问题（需要决定）

按严重度排序，均已逐行核对过源码：

1. **服务端会话过期会把用户引到「本地浏览器门禁」**（`web_auth.rs:78-107` 配
   `ui/shell.rs:114-123`）。`auth_state()` 在调用 `local_auth_state()` 前有一道回环主机判断，
   但 `use_authenticated_shell_bus` 的探测失败分支直接调用 `local_auth_state()`，绕过了它。
   `/session-probe` 挂在 `require_auth` 之后，所以服务端会话一过期探测就不是 204——正式部署上
   用户会被引导去创建一组与服务端登录无关的本地凭据。正确行为应是 reload 或跳 `/login`。
   另外 `verify_server_gate` 每次探测新建 `reqwest::Client`，且把网络抖动和会话过期压成同一
   结果且无日志；这也是页面/外壳层唯一直接发 HTTP 的地方，与分层约束相悖。
2. **切换已读/收藏会重跑一次正文图片本地化**（`reader_page/mod.rs:157-173` 配 `state.rs`）：
   `BumpReload` 走整页重载路径，`begin_loading` 把 `asset_localization_requested` 重置，
   于是又发一次 `LocalizeEntryAssets`（原生端可能带网络下载）。根因是 toggle 复用了
   「整页重载」这一条通道，应改为只回写受影响字段。
3. **目录展开态由 VDOM 与外挂脚本双写**（`entries_page/controls.rs` 配
   `browser_interactions.rs` 的 `syncGroupState`）：`data-active`/`data-open`/`aria-expanded`
   等属性两边都在写，Dioxus 不会恢复被脚本改过的值，两个真相源会静默漂移；同一套规则在
   Rust（`directory_section_view_state`）和 JS 里各实现了一遍。
4. **自定义 CSS 校验只在页面层，导入路径完全绕过**（`themes/theme_validation.rs`）：
   只有设置页保存与主题应用两个调用点；通过配置包导入或 WebDAV 拉取进来的 `custom_css`
   直接进入 `<style>`。对照其余设置项都已收敛到 `rssr_domain::validation`，这里要么一起搬进去，
   要么删掉，现状是个不闭合的边界。
5. **数值输入静默吞掉无效输入**（`settings_page/preferences.rs`）：四处 `if let Ok(..)` 没有
   `else`，清空或输入非数字时草稿保持旧值、界面显示用户输入、无任何提示，保存写回的是旧值。
6. 其余较轻：`format_entry_date_utc` 在 `cards.rs` 与 `groups.rs` 各有一份逐字相同的实现；
   `format_feed_datetime_utc` 与 `format_reader_datetime_utc` 同上；`ui/shell.rs` 的
   `submit_search` 与 `focus_search` 函数体逐字相同；顶栏搜索框 `onfocus` 直接导航会把正在
   点击的输入框卸载掉；`shell_browser.rs` 的搜索词/顶栏折叠态只在 Web 端跨会话保留（语义差异
   而非实现差异）；`WebAuthGate` 在 hook 之前提前 return（当前不可达，但是 hook 顺序隐雷）。

## 给下一位 Agent 的备注

- 正文处理的入口现在是 `crates/rssr-infra/src/html/`；页面层不应再出现 HTML 解析或消毒。
- 归档语义的唯一来源是 `rssr_domain::entry`（`ArchiveFilter` + `archive_cutoff_at`），
  改动时必须同步 SQLite 与 browser 两个适配器，并跑
  `cargo test -p rssr-infra --test test_entry_state_and_search`。
- 校验规则只改 `crates/rssr-domain/src/validation.rs`，不要在 application / infra 再写一份。
