# 全仓审计：越界设置 panic、抓取超时与跨平台排序一致性

- 日期：2026-07-26
- 作者 / Agent：Claude (math-architect)
- 分支：main
- 当前 HEAD：0287e5e
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

对全仓六个 crate 做逐模块审计，修复其中「小而独立」的一批缺陷：两处由未设上界的用户设置触发的
panic、feed 抓取缺失超时、刷新路径上的重复深拷贝，以及浏览器端与原生端不一致的文章排序键。
架构级问题（前端承担分页/归档过滤、阅读页自带 HTML 解析器、配置校验三处重复）只做记录，
未改动，等待确认。

## 影响范围

- 模块：
  - `crates/rssr-domain/src/entry.rs`、`settings.rs`、`lib.rs`
  - `crates/rssr-application/src/settings_service.rs`、`import_export_service/rules.rs`
  - `crates/rssr-infra/src/fetch/client/feed_http.rs`
  - `crates/rssr-infra/src/feed_normalization.rs`
  - `crates/rssr-infra/src/application_adapters/refresh.rs`
  - `crates/rssr-infra/src/application_adapters/browser/query.rs`
  - `crates/rssr-infra/src/config_sync/file_format.rs`
  - `crates/rssr-app/src/bootstrap.rs`
- 平台：
  - Windows / macOS / Linux / Android / Web
- 额外影响：
  - 无 migration、无 workflow 变更

## 关键变更

### 越界设置值导致的 panic

- `archive_after_months` 此前只校验 `>= 1`。设置页是自由文本输入框，填入例如 `4000000`
  会让 `archive_cutoff` 里的 `Date::from_calendar_date(...).expect("valid cutoff")` 直接 panic；
  且 `archive_after_months as i32` 对超过 `i32::MAX` 的值会回绕成负数，把归档分界推到未来，
  使所有文章都被判为已归档。
- `archive_cutoff` 改为返回 `Option` 并全程用 i64 运算；算不出分界时按「不归档」处理。
- `refresh_interval_minutes` 同样无上界，`last + Duration::minutes(u32::MAX)` 会越过
  `OffsetDateTime` 年份上界并 panic，静默杀死后台自动刷新任务。改用 `checked_add`。
- 新增域常量 `MAX_ARCHIVE_AFTER_MONTHS`（1200）与 `MAX_REFRESH_INTERVAL_MINUTES`（525600），
  在 `SettingsService::save` 与配置包导入校验中同时生效。

### feed 抓取缺失超时

- `FetchClient` 使用的是裸 `reqwest::Client::new()`，既无连接超时也无请求超时。桌面端
  `REFRESH_ALL_CONCURRENCY = 1`，即串行刷新，一个不响应的源可以把整轮刷新和后台自动刷新
  循环永久挂住。现设置 30s 请求超时 + 10s 连接超时。

### 配置包校验三处不一致

- `rssr-infra` 的 `file_format::validate_config_package` 与 application 层规则不一致：
  前者接受 `version >= 1` 且完全不校验 `entries_page_size`，后者要求 `version == 2` 并校验。
  已把 infra 侧对齐到同一套规则，避免同一份配置包在两条路径上一个通过一个被拒。

### 刷新路径上的重复深拷贝

- `SqliteRefreshStore::commit` 里 `map_application_feed` 会把整批条目深拷贝进一个只用于
  更新 feed 元数据（只读 title / site_url / description）的结构，紧接着
  `map_application_entries` 又拷贝一次。改为不再克隆条目。

### 跨平台排序键不一致

- 原生端 SQL 排序键是 `COALESCE(published_at, created_at) DESC, id DESC`；浏览器适配器只按
  `published_at` 排序，缺 `published_at` 的条目在 Web 上被一律排到末尾，与桌面端顺序不同。
  列表顺序与阅读页上一篇/下一篇导航共用这个键，因此两端行为都会分叉。已让浏览器端使用
  同一个键（在仍带 `created_at` 的持久化条目上排序后再投影成 `EntrySummary`）。

### feed 日期解析加固

- `to_offset_datetime` 的 `expect("valid unix timestamp")` 改为返回 `Option`。`chrono` 可表示的
  年份区间比 `time::OffsetDateTime` 宽，日期完全由远端 feed 控制，越界时只丢弃该时间戳。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（exit 0）
- `cargo test --workspace`：通过（exit 0）
- `cargo check -p rssr-infra --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- 新增回归测试：
  - `rssr-domain`：`out_of_range_archive_threshold_never_panics_and_archives_nothing`
  - `rssr-app`：`out_of_range_refresh_interval_never_panics_the_auto_refresh_loop`

### 手工验收

- 未执行（本次为源码级修复，未做桌面/Web 手工回归）

## 结果

- 可合并。修复均为局部、低风险，且各自带回归测试。
- 用户可见影响：设置页填入过大的归档阈值/刷新间隔不再让应用崩溃或让自动刷新静默停止；
  Web 端缺 `published_at` 的文章排序与桌面端一致；不响应的订阅源不再挂住整轮刷新。

## 风险与后续事项

以下问题已确认但**未改动**，需要先讨论：

1. **文章页把分页/归档过滤/分组全放在前端**：`EntriesPageState::entry_query` 固定
   `limit: None`，每次查询把全部匹配文章读进内存，再由 `EntriesPagePresenter` 做归档过滤、
   分页切片和分组树构建（且每次渲染都会对全量和当页各构建一次分组树）。这是「薄壳前端」
   要求的主要偏离点，也是文章量增长后的主要性能风险。
2. **阅读页自带一套 HTML 解析器**：`pages/reader_page/support.rs`（751 行）里有完整的
   标签/属性解析、HTML 实体解码、WordPress emoji 启发式，与 `infra/fetch/client/image_html.rs`
   的解析器大量重复；其中约 150 行（desktop image proxy 相关）是 `#[allow(dead_code)]` 的
   废弃方案残留。
3. **Web 端不做正文图片地址归一化**：`normalize_html_for_live_display` 在 `rssr-infra::fetch`
   下，而整个 `fetch` 模块被 `#[cfg(not(target_arch = "wasm32"))]` 门控，尽管该函数是纯字符串
   处理、不含 I/O。结果是 Web 阅读页的懒加载图片不会被修正。
4. **配置校验仍有三份实现**：本次只对齐了规则，没有收敛成单一来源。
5. **`ensure_content_schema()` 在每次读取正文时执行一次建表 + 两次建索引 DDL**，与
   `migrations_content/0001_initial.sql` 职责重复。
6. **`entries` 的排序表达式没有可用索引**：现有索引是 `published_at DESC`，而查询按
   `COALESCE(published_at, created_at) DESC, id DESC` 排序，无法命中。
7. **`upsert_entries` / `upsert_contents` 逐条 INSERT 且未包事务**，每条一次隐式事务。
8. **`rssr-web` 的 auth 配置测试通过 `std::env::set_var` 改进程级环境变量且并行执行**，
   本次审计中偶发失败一次、重跑通过，属于既有的测试隔离缺陷。
9. **`BodyAssetLocalizer` 先把图片整体读入内存再判大小**；`/feed-proxy` 同样无超时与体积上限。
10. **WebDAV 同步没有任何认证入口**（`WebDavConfigSync` 用裸 `Client::new()`），
    对需要登录的 WebDAV 服务不可用。

## 给下一位 Agent 的备注

- 入口文件：`crates/rssr-app/src/pages/entries_page/{state,presenter,facade}.rs` 是第 1 项的核心；
  `crates/rssr-app/src/pages/reader_page/support.rs` 是第 2、3 项的核心。
- 第 1 项若要推进，需要同时改 `EntryQuery`（加 offset）、`EntryIndexRepository` 契约、
  两套适配器（SQLite 与 browser）以及 `crates/rssr-infra/src/db/AGENTS.md` 要求的
  `test_entry_state_and_search`，属于跨层改动，先出设计再动手。
