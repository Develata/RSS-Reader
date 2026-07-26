# 全仓审计整改（第二轮）：校验收敛、正文处理下沉、归档筛选下推、WebDAV 认证

- 日期：2026-07-26
- 作者 / Agent：Claude (math-architect)
- 分支：main
- 当前 HEAD：98f38a1
- 相关 commit：`ce83443`、`981cc0e`、`8e3ef7a`、`98f38a1`（前置轮次见 `84169c5`）
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
  - `migrations/0003_entry_sort_key_indexes.sql`
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
- `migrations/0003`：按实际排序键 `COALESCE(published_at, created_at) DESC, id DESC`
  建表达式索引（全局 / 按订阅 / 未读三个变体）。此前索引建在裸 `published_at` 上，
  实际排序一条也命中不了。
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

- 可合并。四个 commit 各自独立可回滚。
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
3. **SSRF 的 DNS rebinding TOCTOU**：`/feed-proxy` 先解析域名做校验、再按域名发起请求，
   两次解析之间存在时间窗。彻底修复需要解析一次后直接连校验过的 IP 并覆写 Host/SNI。
   当前该端点在登录之后才可达，影响有限。
4. **`rssr-web` 认证测试**已串行化，但根因是这些测试依赖进程级环境变量；
   若后续新增同类测试，记得一并取 `auth::test_env::lock()`。

## 给下一位 Agent 的备注

- 正文处理的入口现在是 `crates/rssr-infra/src/html/`；页面层不应再出现 HTML 解析或消毒。
- 归档语义的唯一来源是 `rssr_domain::entry`（`ArchiveFilter` + `archive_cutoff_at`），
  改动时必须同步 SQLite 与 browser 两个适配器，并跑
  `cargo test -p rssr-infra --test test_entry_state_and_search`。
- 校验规则只改 `crates/rssr-domain/src/validation.rs`，不要在 application / infra 再写一份。
