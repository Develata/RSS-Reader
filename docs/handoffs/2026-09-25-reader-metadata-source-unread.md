# 任务 1：阅读元信息、来源全部未读数与设备本地时间

- 日期：2026-09-25
- 作者 / Agent：Codex
- 分支：main
- 实施前 HEAD：5e0d8e52d28d835580255f8cb9a921690fa74ff0
- 相关 commit：本记录所在的 `feat(reader): organize metadata and show source unread counts` 本地提交；用户在后续消息中明确授权 commit，未 push / tag / release。
- 相关 tag / release：N/A
- 状态：`draft`（实现与 Web 专项验收完成；后续已补齐原生全量测试、Android ARM64 check 和浏览器契约测试，仍缺 Android 运行与桌面系统浏览器打开验收）

后续验证以 [跨平台补验记录](2026-09-25-task1-platform-validation.md) 为准。本文以下命令表保留实施提交时的原始结果，不覆盖当时的失败证据。

## 工作摘要

按已确认任务 1 实现阅读页订阅名、可选作者、发布时间及独立原文入口；来源筛选展示订阅全部未读数；显示时间和日期分桶使用设备本地历史时区规则。未实现滚动恢复、批量已读、订阅发现、刷新新增计数或自动已读新功能。

实施前再次确认 HEAD 为原始基线、工作树干净，读取任务要求文档与页面指令。没有 `.codegraph/`，使用 rg 与源码查询；未建立索引或安装工具。`.handoff/` 排除状态未改。

## 影响范围

- 模块：application ReaderService、共享 UI reader / entries / feeds、宿主时区适配器、Web 已读持久化。
- 平台：Web、Linux / Windows / macOS、Android；CLI 无命令行为变更。
- 额外影响：README、使用指南、命令与主题接口、专项浏览器回归脚本；无数据库 migration、无 workflow 改动。

## 关键变更

### 加载契约与显示

```rust
pub struct ReaderEntrySnapshot {
    pub entry: Option<Entry>,
    pub navigation: EntryNavigation,
    pub feed_title: Option<String>, // 新增
}

impl ReaderService {
    pub fn new(
        entry_index_repository: Arc<dyn EntryIndexRepository>,
        entry_content_repository: Arc<dyn EntryContentRepository>,
        feed_repository: Arc<dyn FeedRepository>, // 新增注入
    ) -> Self { /* 实际实现见 reader_service.rs */ }
}
```

使用 `FeedRepository::get_feed`，空标题回退订阅 URL；无法取得订阅返回 None，读取错误记录 warning，不阻断正文。
原生与浏览器均经 `AppUseCases::compose` 装配；`ReaderPort::load_entry(&self, entry_id: i64) -> Result<ReaderEntrySnapshot>` 签名保持不变，完整快照自然透传。
没有新增 repository 方法或 trait；FeedSummary、Entry、schema、交换格式、CLI 命令不变。

UI 内部 LoadedContent / State 新增 `author: Option<String>`、`original_url: Option<String>`；来源改为快照订阅标题。
原文入口只接受 HTTP(S)，避免将 feed 中的其它协议当作可执行页面链接。
新公开选择器：`data-action="open-original"`、`data-slot="entry-filters-source-unread-count"`。
`entry-filters-source-chip` 保持不变；checkbox 名称仍为来源名，`aria-describedby` 指向“未读 N 篇”。
`EntryFilters.available_sources` 从 `Vec<(i64, String, String)>` 扩展为 `Vec<(i64, String, String, u32)>`，第四项为全部未读数；内部 presenter / facade 同步透传。

正文链接实际路径：ammonia 清理正文链接，Web 的普通正文链接依旧是普通页面导航；Dioxus desktop/mobile 0.7.9 的 `webview.rs` navigation handler 对 HTTP(S)/mailto 调用 webbrowser 并阻止 WebView 导航。
新原文入口在原生使用 `_self` 进入该既有外部处理器，在 Web 用 `_blank` 和 `noopener noreferrer`；不新增内嵌浏览器，不更改正文链接语义。

### 时间与未读状态

- `bootstrap/local_time.rs` 隔离平台能力：桌面使用仓库已有 chrono（Unix 使用线程局部时区数据库缓存），Web 同步 js-sys Date 计算目标时间戳偏移，Android JNI `TimeZone.getOffset(timestamp_ms)`；不修改进程 TZ。
- 原生无法解析时区时 chrono 的 UTC 回退、Web 非有限偏移、Android 获取失败均显示 UTC；零偏移也统一写 UTC，其余完整时间标数值偏移。
- 列表 runtime 对每条时间戳转换一次；卡片、月份和日期分桶保留并共用同一偏移值，不逐卡片启动异步宿主调用。存储时间与 application/domain 不变。
- presenter 比较包含偏移，避免同一瞬间更换本地偏移后仍复用旧分组。设备运行中切换时区后需重新加载页面。
- 全部未读数直接取 FeedSummary，与订阅页共用仓储汇总；成功标记后通过已有 bootstrap generation 机制重查，失败不预减。返回列表重新 bootstrap。
- Web `set_read` 保存失败时恢复原 flag 或移除新插入 flag，防止内存与持久化分裂导致计数漂移；锁内回滚只复制一个 flag，不复制全状态。
- 全仓检索和调用链未发现基线存在自动标记已读路径：Reader load 只读，写入仅来自显式 ToggleRead。故“覆盖已有自动已读”在本基线不适用，未新增该功能。

### 逐文件清单

| 文件 | 改动 |
| --- | --- |
| `Cargo.lock` | 仅补已有 chrono / tracing 的 crate 依赖关联，无新增包版本。 |
| `crates/rssr-app/Cargo.toml` | 桌面时区适配复用 chrono workspace 依赖。 |
| `crates/rssr-application/Cargo.toml` | 增加已有 tracing，用于非致命元信息读取诊断。 |
| `crates/rssr-application/src/reader_service.rs` | 注入 FeedRepository、扩展快照，覆盖标题回退、缺订阅及读取失败测试。 |
| `crates/rssr-application/src/composition.rs` | 共享装配点传入 FeedRepository。 |
| `crates/rssr-app/src/bootstrap.rs` | 注册本地时间与外部链接宿主模块。 |
| `crates/rssr-app/src/bootstrap/local_time.rs` | 各平台目标时刻偏移与原生多线程 DST 测试。 |
| `crates/rssr-app/src/bootstrap/external_link.rs` | 隔离 Web / 原生链接 target 差异。 |
| `crates/rssr-app/src/datetime.rs` | 本地完整时间、已转换日期格式化及偏移测试。 |
| `crates/rssr-app/src/ui/runtime/reader.rs` | 从快照投影来源、作者与 HTTP(S) 原文 URL。 |
| `crates/rssr-app/src/ui/runtime/entries.rs` | 列表加载时集中转换显示时间。 |
| `crates/rssr-app/src/pages/reader_page/state.rs` | 保存并在换文章时清除新增元信息。 |
| `crates/rssr-app/src/pages/reader_page/reducer.rs` | 应用作者和原文 URL，更新测试构造。 |
| `crates/rssr-app/src/pages/reader_page/session.rs` | 更新测试快照构造。 |
| `crates/rssr-app/src/pages/reader_page/facade.rs` | 暴露作者与原文 URL 给视图。 |
| `crates/rssr-app/src/pages/reader_page/mod.rs` | 按固定顺序显示元信息与独立原文入口。 |
| `crates/rssr-app/src/pages/entries_page/cards.rs` | 使用已转换时间输出日期。 |
| `crates/rssr-app/src/pages/entries_page/groups.rs` | 月份、日期分桶保留本地偏移，更新固定 UTC 测试。 |
| `crates/rssr-app/src/pages/entries_page/presenter.rs` | 透传未读数，分组比较包含时区偏移。 |
| `crates/rssr-app/src/pages/entries_page/facade.rs` | 来源选项透传四元组。 |
| `crates/rssr-app/src/pages/entries_page/session.rs` | 已读写入成功后重查权威来源汇总。 |
| `crates/rssr-app/src/pages/feeds_page/sections/support.rs` | 刷新时间同步使用本地格式化。 |
| `crates/rssr-app/src/components/entry_filters.rs` | 弱化数字与可访问未读描述，保留原选择行为。 |
| `assets/styles/entries.css` | 来源计数的弱化行内样式。 |
| `assets/styles/reader.css` | 元信息长文本换行和原文入口 44px 高度。 |
| `crates/rssr-infra/src/application_adapters/browser/adapters/entry.rs` | 已读持久化失败时回滚单条内存 flag。 |
| `crates/rssr-infra/tests/test_entry_state_and_search.rs` | SQLite 阅读来源及全局未读、重复、失败、零计数测试。 |
| `crates/rssr-infra/tests/wasm_subscription_contract_harness.rs` | 浏览器快照、全局未读与存储失败回滚契约测试。 |
| `scripts/browser/rssr_task1_acceptance.cjs` | 复用现有 SPA fixture server 的 Playwright 专项验收。 |
| `README.md` | 更新日常阅读、计数及本地时间说明。 |
| `docs/user-guide.md` | 补充回退、原文打开和计数更新流程。 |
| `docs/design/frontend-command-reference.md` | 更新稳定接口及行为契约。 |
| `docs/design/theme-author-selector-reference.md` | 更新新增槽位、链接及无障碍契约。 |
| `docs/handoffs/2026-09-25-reader-metadata-source-unread.md` | 本次交接及证据边界。 |

## 验证与验收

### 自动化验证

所有输出位于本机忽略目录 `target/task1-validation/`，命令汇总为 `commands.json`。退出码为实际进程结果。

| 命令 | 退出码 | 结果 |
| --- | ---: | --- |
| `cargo fmt --all --check` | 0 | 通过。 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 101 | 环境阻塞：javascriptcoregtk-4.1 / libsoup-3.0 / WebKitGTK 开发库缺失。 |
| `cargo test --workspace` | 101 | 同上，不能算作 workspace 测试通过。 |
| `cargo check -p rssr-app --target wasm32-unknown-unknown` | 0 | 通过。 |
| `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings` | 0 | 修复 is_multiple_of lint 后通过。 |
| `cargo check -p rssr-app --target aarch64-linux-android` | 101 | 缺 aarch64-linux-android target，E0463，尚未进入应用 Android 代码检查。 |
| `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness` | 1 | 缺 chromedriver，未运行 harness 浏览器测试。 |
| `cargo check -p rssr-infra --test wasm_subscription_contract_harness --target wasm32-unknown-unknown` | 0 | harness 编译通过，不等于执行通过。 |
| `cargo test -p rssr-application -p rssr-infra` | 0 | 148 passed、0 failed、1 ignored（原有忽略项）；wasm-only 测试在原生运行中为 0 项，未算执行通过。 |
| `cargo test -p rssr-infra --test test_entry_state_and_search` | 0 | 最后补充真实 SQLite reader 快照断言后 4 passed。 |
| `cargo check -p rssr-app --tests --target wasm32-unknown-unknown` | 0 | UI 单元测试可编译；不是 UI 测试运行。 |
| `cargo test --offline --manifest-path target/task1-validation/time-check/Cargo.toml` | 0 | 补充独立宿主测试 crate 用路径直接包含当前 local_time/datetime/groups 源码，避开 GTK；11 passed，含四线程历史 DST 边界。 |
| `dx build --platform web --package rssr-app --locked` | 0 | 最终 Web bundle 构建成功；工具报告 dx 0.7.10 与 Dioxus 0.7.9 版本不一致，随后构建成功，真实浏览器验收通过。 |
| `git diff --check` | 0 | 无空白错误。 |

`time-check` 为本机验证辅助产物，未加入产品依赖；正式单元测试仍位于 app 的源码模块中。原生全量测试需要先补齐系统库。
未运行：refresh / config exchange wasm harness，因为本次未改刷新或配置交换适配器；只选 subscription harness。

### 手工 / 真实浏览器验收

使用现有 `scripts/run_web_spa_regression_server.sh --skip-build --port 8097`、已有 Playwright 安装与 Chromium 1234，隔离浏览器上下文，不接触用户站点数据。

```bash
NODE_PATH=/home/deve/.local/share/fnm/node-versions/v24.18.0/installation/lib/node_modules/@playwright/test/node_modules \
CHROME_BIN=/home/deve/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome \
node scripts/browser/rssr_task1_acceptance.cjs
```

浏览器产物：`target/task1-validation/browser/`（JSON 断言记录、360 / 1280 reader 与来源截图）；命令退出码 0。

已验证：

- Web 360×800、1280×800：长订阅名、长作者、长 URL 无横向溢出；原文入口及来源行 ≥44px；已实际查看小屏阅读截图。
- 未先访问列表，直接进入 reader 仍显示正确来源、作者；固定顺序正确。
- 缺标题回退订阅 URL；空作者隐藏；缺时间显示未知发布时间；缺原文 URL 隐藏；订阅不可取得时正文继续显示。
- 真实点击原文创建新标签，`window.opener === null`，原阅读页 URL 不变。
- 阅读页标已读后点击 R 返回列表显示 0；列表标未读后计数为 1；搜索、归档切换不改变来源口径。
- 强制 localStorage flag 写入抛 QuotaExceededError：列表与随后订阅页仍一致显示未读 1。
- 同一纽约上下文 2026-03-08 01:59 UTC-05:00 → 03:00 UTC-04:00；订阅刷新时间也显示正确偏移。
- 纽约负偏移跨月、Kathmandu +05:45、Chatham +13:45：卡片日期与分桶相同；强制偏移获取返回 NaN 后卡片、分桶、reader 一致回退 UTC。

推断（源码支持，非设备验收）：原生链接复用 Dioxus / webbrowser 系统处理器，应保留阅读页；Android JNI 按目标时间获取历史偏移。不能据此宣称桌面打开或 Android 已通过。

未运行：桌面 GUI / 系统浏览器实际打开，缺 GTK / WebKit；Android 实机及完整编译，缺 target / 设备；Windows / macOS 实机未提供。浏览器验证不替代这些平台检查。

## 结果

- 工作树实现完成，Web 核心专项通过；原生 workspace、Android 和 wasm 契约 harness 有明确环境缺口，不宣称完整跨端验收通过或可发布。
- 没有修改存储 schema、配置交换时间语义或 CLI 命令。
- 用户后续明确授权本地 commit；本批提交包含 30 个已跟踪文件修改及 4 个新增文件。未 push / tag / release。

## 风险与后续事项

- 补齐桌面依赖、Android target 后重跑阻塞命令；Android 本次增加 JNI 时区分支，必须完成编译与实机确认。
- 装有 chromedriver 的环境重跑 subscription wasm harness，覆盖新增 flag 回滚分支；真实 Playwright 已覆盖现有 flag 写失败与返回后计数。
- dx / Dioxus 版本警告是现有环境状态，本任务未升级依赖或安装工具。
- 列表显示时间在加载时转换；运行中更换设备时区后重新加载页面以重建日期分组，不新增常驻时区监控。
- 字体/自定义主题可能覆写 CSS；本次验收默认主题，未穷举所有用户主题。

## 给下一位 Agent 的备注

- 先看 `ReaderService::load_entry`、`bootstrap/local_time.rs`、entries session 成功 PatchEntryFlags 后的 bootstrap，以及 browser `set_read` 回滚。
- 使用真实浏览器回归脚本前先构建最新 Web bundle 并启动现有 SPA fixture server；指定本机 Playwright 与 Chrome 路径即可，无须增加 npm 产品依赖。
- 授权更新：用户后续明确表示“可以自动commit”，覆盖此前仅修改工作树的限制；授权不包含 push / tag / release。
- 提交前再次核对文件清单，运行 `cargo fmt --all --check` 与 `git diff --check`，均退出 0。实现代码未在本次提交阶段改变，沿用上述真实测试结果与环境阻塞记录。

## Claude 复核修正（2026-09-25）

- 复核范围：`5e0d8e5..bc0bfce` 全部产品代码 diff，对照本文记录的已确认决定（来源全部未读口径、元信息固定顺序、独立原文入口、设备本地时间）。
- 修正 1：`bootstrap/local_time.rs` 的时区子进程测试改为仅在 `unix` 上编译。原因：测试靠子进程 `TZ` 切换时区，chrono 0.4.45 只在 `offset/local/unix.rs` 读取 `TZ`，Windows 后端走系统 API 会忽略它，在非纽约时区的 Windows 机器上该测试必然失败（CI 只在 Ubuntu 跑测试，因此未暴露）。
- 修正 2：`tokens.css` 全局 `a { color: inherit; text-decoration: none; }` 使“打开原文”在默认主题下与普通文本无法区分；为 `[data-action="open-original"]` 补上与正文链接一致的强调色、下划线与 focus-visible 样式。
- 复核验证（本机 WSL，缺 GTK/WebKit 开发库）：`cargo fmt --all --check` 0；`cargo clippy --locked --workspace --exclude rssr-app --all-targets -- -D warnings` 0；`cargo test --locked --workspace --exclude rssr-app` 186 passed / 0 failed；`cargo check` 与 `cargo clippy -D warnings` 于 `rssr-app --target wasm32-unknown-unknown` 均 0。
- 未验证：修正 1 的 cfg 变更未能在本机编译 rssr-app 原生测试（缺 GTK），需在 Windows 上运行 `cargo test -p rssr-app local_time` 确认该测试被跳过、其余通过；修正 2 未做浏览器截图验收。`wasm_subscription_contract_harness` 未重跑（本机无 chromedriver，wasm-bindgen 为 0.2.128 而非要求的 0.2.126）。
- 后续风险（未修改）：Android 列表加载对每条文章做一次 JNI 时区查询，大列表下的开销未测；`set_starred` 未获得与 `set_read` 对称的持久化失败回滚；`scripts/browser/rssr_task1_acceptance.cjs` 依赖本机 node + Playwright 路径，未接入 CI。
