# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

本地优先、面向个人的极简 RSS 阅读器（Rust Edition 2024 + Dioxus 0.7.9），覆盖桌面（Windows/Linux/macOS）、Web、Android 与 CLI。

产品边界长期固定为四类能力：**订阅、阅读、基本设置、基础配置交换**（`docs/design/functional-design-philosophy.md`）。超出边界的功能（AI 总结/分析、推荐流、社交、标签树/文件夹系统、文章库跨设备同步等）默认拒绝，不要实现。

## 常用命令

```bash
# 桌面端运行
cargo run -p rssr-app --locked

# CLI
cargo run -p rssr-cli --locked -- --help

# 正式 Web 同源开发（需 wasm32-unknown-unknown 和 dioxus-cli 0.7.9）
bash scripts/run_rssr_web_browser_smoke.sh
# 静态演示另用 --features pages-demo
dx serve --platform web --package rssr-app --features pages-demo --locked

# 提交前验证（与 CI 一致）
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p rssr-app --target wasm32-unknown-unknown --locked   # 涉及 UI/共享代码时必查
cargo check -p rssr-app --target aarch64-linux-android --locked    # 涉及移动端交互时

# 单个集成测试（集成测试集中在 crates/rssr-infra/tests/）
cargo test -p rssr-infra --test test_library_entry_state_and_search --locked
# 按名称过滤单元测试
cargo test -p rssr-application --locked <test_name>

# wasm 浏览器契约测试（CI 三个 harness；需与 Cargo.lock 中 wasm-bindgen 同版本的 wasm-bindgen-cli（当前 0.2.128，脚本会核对）+ Chrome for Testing）
bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness
# 其余：wasm_subscription_contract_harness / wasm_config_exchange_contract_harness

# 正式 Web 浏览器回归（真实服务器、隔离夹具、同一构建）
bash scripts/run_server_web_regression.sh --suite acceptance --long
bash scripts/run_release_ui_regression.sh --with-fixed-smokes
# 同源开发与升级边界见 docs/deployment/web.md
```

## 架构

七个 crate 组成分层架构，依赖方向严格向内（UI → application → domain；infra 实现 domain/application 定义的接口）：

- **`rssr-domain`** — 实体（`Feed`、`Entry`、`UserSettings`、`AppStateSnapshot`）、仓储 trait（`FeedRepository`、`EntryIndexRepository`、`EntryContentRepository` 等）、`DomainError`。无任何 I/O。
- **`rssr-application`** — 用例服务（`RefreshService`、`ReaderService`、`EntriesListService`、`ImportExportService`、`SubscriptionWorkflow` 等）。`composition.rs` 中的 `AppUseCases::compose(AppCompositionInput)` 用端口 trait（`FeedRefreshSourcePort`、`RefreshStorePort`、`OpmlCodecPort`、`ClockPort`、`AppStatePort`、原子写端口与 `LibraryQueryPort`）组装全部服务，平台无关。界面的一次读取需要多条查询时走 `LibraryQueryPort`（`library_queries.rs`），由存储在一个读快照内回答。
- **`rssr-infra`** — 端口适配器，按编译目标二选一（`lib.rs` 中 `#[cfg(not(target_arch = "wasm32"))]` 门控）：
  - **原生（桌面/Android/CLI）**：`store` 模块，rusqlite 单库 `library.sqlite3`（索引、正文、设置、刷新序号；基线 schema 在 `store/migrations/`，根目录 `migrations/` 与 `migrations_content/` 冻结为 v0.1.x 历史，只供 `store::import` 单向导入旧库）；reqwest 抓取 + `BodyAssetLocalizer` 正文图片本地化；feed-rs/quick-xml 解析；OPML 编解码；WebDAV 配置同步。入口 `store::local::open_local_library()`（首次打开时导入同一数据目录中的 v0.1.x 库）+ `compose_library_use_cases(store)`。
  - **wasm32 正式 Web**：`http-backend` feature 下的 `HttpBackend` 经同源 `/api/v1` 调用服务器；不编译 `browser-store` 或 SQLite。
  - **wasm32 演示（`pages-demo`）**：`application_adapters/browser/` 下的适配器共用 `BrowserStore`，通过 Web Locks 协调 `localStorage` 四片数据，以单一提交头发布；缓存按版本同步，刷新逐订阅持久化，仅正文记录变化时写正文片。`store`/`fetch`/`parser`/`config_sync` 模块在 wasm 上不编译。入口 `compose_browser_use_cases`。
- **`rssr-api`** — 界面与用例之间的 `Backend` 接口（界面的全部操作；阶段 C 的 `HttpBackend` 与本机 `LocalBackend` 都实现它），只依赖 application / domain；依赖图门禁见 `crates/rssr-api/tests/dependency_graph.rs`（Web 构建不含 SQLite）。
- **`rssr-app`** — Dioxus UI。`bootstrap/native.rs` 建好 `LocalBackend`；`bootstrap/web.rs` 的正式构建选择 `HttpBackend`，只有 `pages-demo` 选择浏览器 `LocalBackend<WebHost>`。infra 的 `local_backend` 共用添加订阅、刷新编排与自动刷新，差异在 `LocalHost`；演示宿主禁止网络与自动刷新。`ui/` 分 shell / commands / runtime；`pages/` 通过 facade/session 派发命令，runtime 只经 `Arc<dyn Backend>` 调用。界面接缝的特征测试：`ui/runtime/characterization.rs`。
- **`rssr-cli`** — 复用同一 application 层的自动化入口。
- **`rssr-web`** — 仅用于 Web 部署的薄 axum 服务：Argon2 登录与服务器保存的会话（数据目录 `RSSR_DATA_DIR` 里的 `auth.sqlite3`）、写请求的 Origin / CSRF 校验、阅读器接口 `/api/v1`（`api` 模块：同一数据目录里 `library.sqlite3` 上的 `LocalBackend`，线上形式见 `rssr-api::protocol`；写请求要求构建标识一致，标识由 `build-support/build_id.rs` 在构建时计算）、静态包托管 + SPA 回退；订阅抓取、自动刷新、WebDAV 与图片本地化经服务器的 `LocalBackend` 执行。旧 feed 代理与浏览器口令门禁已退役。子命令 `passwd`、`sessions revoke-all`。桌面端与 CLI 不依赖这个 HTTP 服务；正式 Web 的本地开发也需要同源服务器。

关键约束：**平台差异只允许存在于 infra / adapter / host capability 层**，application/domain 的语义必须跨平台统一。UI 组件里禁止直接写 SQL、HTTP 或 feed/OPML/JSON 解析。

数据目录：Linux `/usr/bin` 安装版在 `$XDG_DATA_HOME/rss-reader/`（未设置时为 `~/.local/share/rss-reader/`）；macOS 在 `~/Library/Application Support/RSS-Reader/`；Android 在应用私有目录 `$HOME/RSS-Reader/`；其它桌面/便携版在可执行文件同目录 `RSS-Reader/`。CLI 可用 `--data-dir` / `RSSR_DATA_DIR` 指定。路径规则集中在 `rssr-infra::store::local`，资料库与界面偏好共用它。

## 仓库规则（来自 AGENTS.md，必须遵守）

- **交接记录**：每完成一次可交付工作，必须在 `docs/handoffs/` 新增/更新 `YYYY-MM-DD-<slug>.md` 记录（模板 `docs/handoffs/TEMPLATE.md`），含工作摘要、受影响模块、验证命令与结果、commit 状态。未补记录不算完整交付。
- **架构护栏**：出现代码严重分叉、平台差异回流到 application/domain 语义、前后端大规模职责重分配、或违背功能设计哲学的方案时，必须先做保守分析并向用户明确提出，默认持保守/负面倾向，不得静默推进。
- **就近 AGENTS.md**：`crates/rssr-infra/src/store/AGENTS.md` 与 `crates/rssr-app/src/pages/AGENTS.md` 对各自目录有更细的修改约束（如改文章排序/筛选必须同步考虑列表页、阅读导航、搜索一致性，并跑 `test_library_entry_state_and_search`）。改这些目录前先读。
- **页面层改动默认同时影响 Web / 桌面 / Android**，不要只按桌面视口判断；改完至少跑 wasm target check。
- 行为边界、用户流程、配置方式变化时同步更新 `README.md`、`docs/`、`specs/`。

## 平台差异提醒

- 正式 Web 的资料库在服务器 `library.sqlite3`；浏览器经 `HttpBackend` 访问同源 API，不直连 feed。浏览器只保留设备偏好与 `sessionStorage` 草稿。旧 Web 文章不迁移，正式 Web 不再离线可读。
- 仅 `pages-demo` 在阶段 D 前仍使用 `BrowserStore` / `localStorage` / Web Locks；在线订阅、WebDAV、自动刷新禁用。正式与演示构建分别验证。
- Android 本地打包需在 `dx bundle` 后运行 `python3 scripts/prepare_android_bundle.py`，再用生成的 Gradle 工程出 APK。发布流水线额外传 `--release-tag`，把 tag 转成 `versionCode` / `versionName`（`Cargo.toml` 版本号长期停在 0.1.0，不能作为版本来源）。
- Android 发布产物的断言只认资源名（`aapt2 dump resources` 里的 `mipmap/rssr_launcher`），不认文件路径：release 会跑 `optimizeReleaseResources` 压缩资源路径，按路径断言的写法只在 debug 上成立。

## 代码风格

- rustfmt：`max_width = 100`、`use_small_heuristics = "Max"`（`rustfmt.toml`）。
- clippy 以 `-D warnings` 为基线。
- 性能敏感路径（仓储查询、刷新流程）优先减少 clone、分配与全表扫描。
