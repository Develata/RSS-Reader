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

# Web 端开发（需 rustup target add wasm32-unknown-unknown 和 dioxus-cli 0.7.9）
dx serve --platform web --package rssr-app --locked

# 提交前验证（与 CI 一致）
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p rssr-app --target wasm32-unknown-unknown --locked   # 涉及 UI/共享代码时必查
cargo check -p rssr-app --target aarch64-linux-android --locked    # 涉及移动端交互时

# 单个集成测试（集成测试集中在 crates/rssr-infra/tests/）
cargo test -p rssr-infra --test test_entry_state_and_search --locked
# 按名称过滤单元测试
cargo test -p rssr-application --locked <test_name>

# wasm 浏览器契约测试（CI 三个 harness；需 wasm-bindgen-cli 0.2.126 + Chrome for Testing）
bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness
# 其余：wasm_subscription_contract_harness / wasm_config_exchange_contract_harness

# Web 部署态验证（登录 + 服务端模式；CORS 受限源由服务端抓取）
dx bundle --platform web --package rssr-app --release --debug-symbols false --out-dir target/web-e2e --locked
# 然后按 docs/deployment/web.md 设置 RSS_READER_WEB_* 环境变量启动 rssr-web
```

## 架构

七个 crate 组成分层架构，依赖方向严格向内（UI → host → application → domain；infra 实现 domain/application 定义的接口）：

- **`rssr-domain`** — 实体（`Feed`、`Entry`、`UserSettings`、`AppStateSnapshot`）、仓储 trait（`FeedRepository`、`EntryIndexRepository`、`EntryContentRepository` 等）、`DomainError`。无任何 I/O。
- **`rssr-application`** — 用例服务（`RefreshService`、`ReaderService`、`EntriesListService`、`ImportExportService`、`SubscriptionWorkflow` 等）。`composition.rs` 中的 `AppUseCases::compose(AppCompositionInput)` 用端口 trait（`FeedRefreshSourcePort`、`RefreshStorePort`、`OpmlCodecPort`、`ClockPort`、`AppStatePort`）组装全部服务，平台无关。
- **`rssr-infra`** — 端口适配器，按编译目标二选一（`lib.rs` 中 `#[cfg(not(target_arch = "wasm32"))]` 门控）：
  - **原生（桌面/Android）**：sqlx SQLite，**索引库与正文库是两个独立数据库**（迁移分别在 `migrations/` 与 `migrations_content/`）；reqwest 抓取 + `BodyAssetLocalizer` 正文图片本地化；feed-rs/quick-xml 解析；OPML 编解码；WebDAV 配置同步。入口 `compose_native_sqlite_use_cases(index_pool, content_pool)`。
  - **wasm32**：`application_adapters/browser/` 下的适配器共用 `BrowserStore`，通过 Web Locks 协调 `localStorage` 四片数据，以单一提交头发布；缓存按版本同步，刷新逐订阅持久化，仅正文记录变化时写正文片。`db`/`fetch`/`parser`/`config_sync` 模块在 wasm 上不编译。入口 `compose_browser_use_cases`。
- **`rssr-host`** — 宿主能力（刷新、自动刷新、正文图片本地化、远程配置）的 trait、结果类型、自动刷新调度与 `RefreshFlight` 单飞，两个目标都编译；`native` 模块是不依赖界面框架的原生宿主 `NativeHost::open(NativeSqliteBackend)`（SQLite 双库 + 宿主能力），供桌面、Android 与 `rssr-web` 服务端模式共用（`docs/design/rssr-web-server-mode.md`）；`NativeHostOptions` 选择出站策略（`rssr_infra::outbound`，服务端默认只访问公网）。`HostServices` 是界面运行时全部能力的 UI 无关实现（`rssr-app` 的 `UiServices` 端口委托给它）；`protocol` 定义服务端模式的端点（路径 + 输入输出类型 + 协议版本），`server` 负责分派。
- **`rssr-app`** — Dioxus UI。`bootstrap/native.rs` 在默认数据目录上打开 `NativeHost`，`bootstrap/web.rs` 组装浏览器宿主，二者按平台暴露 host capabilities；`ui/` 分 shell / commands / runtime；`pages/` 通过 facade/session 派发命令，runtime 调用统一用例或 host capability。
- **`rssr-cli`** — 复用同一 application 层的自动化入口。
- **`rssr-web`** — Web 部署的 axum 服务：Argon2 登录、HttpOnly 会话 cookie、静态包托管 + SPA 回退；服务端模式在 `RSS_READER_WEB_DATA_DIR` 打开与桌面相同的 `NativeHost`，经 `/api/v1` 提供 `HostServices`（读请求体前校验会话 + JSON + 协议头 + 同源，调用在独立任务里执行并有并发上限）；浏览器界面在 rssr-web 托管下经 `rssr_host::client::Backend::Remote` 调用它，不用 `BrowserStore`。桌面端、CLI 都不依赖它。

关键约束：**平台差异只允许存在于 infra / adapter / host capability 层**，application/domain 的语义必须跨平台统一。UI 组件里禁止直接写 SQL、HTTP 或 feed/OPML/JSON 解析。

Linux `/usr/bin` 安装版的数据库首次启动时创建于 `$XDG_DATA_HOME/rss-reader/`（未设置时为 `~/.local/share/rss-reader/`）；其它桌面/便携版仍在可执行文件同目录 `RSS-Reader/`。路径规则集中在 `rssr-infra::db::sqlite_native`，索引库、正文库及界面偏好共用它。

## 仓库规则（来自 AGENTS.md，必须遵守）

- **交接记录**：每完成一次可交付工作，必须在 `docs/handoffs/` 新增/更新 `YYYY-MM-DD-<slug>.md` 记录（模板 `docs/handoffs/TEMPLATE.md`），含工作摘要、受影响模块、验证命令与结果、commit 状态。未补记录不算完整交付。
- **架构护栏**：出现代码严重分叉、平台差异回流到 application/domain 语义、前后端大规模职责重分配、或违背功能设计哲学的方案时，必须先做保守分析并向用户明确提出，默认持保守/负面倾向，不得静默推进。
- **就近 AGENTS.md**：`crates/rssr-infra/src/db/AGENTS.md` 与 `crates/rssr-app/src/pages/AGENTS.md` 对各自目录有更细的修改约束（如改 `entry_repository.rs` 的排序/筛选必须同步考虑列表页、阅读导航、搜索一致性，并跑 `test_entry_state_and_search`）。改这些目录前先读。
- **页面层改动默认同时影响 Web / 桌面 / Android**，不要只按桌面视口判断；改完至少跑 wasm target check。
- 行为边界、用户流程、配置方式变化时同步更新 `README.md`、`docs/`、`specs/`。

## 平台差异提醒

- 静态 Web 包只直连抓取，受浏览器 CORS 限制：部分 feed 桌面端可刷、静态包刷不了，需在 `rssr-web` 服务端模式下验证（`/feed-proxy` 已于 2026-10-20 退役）。
- 静态 Web 包（不经 rssr-web）的持久化是 `localStorage` 序列化状态；rssr-web 托管时是服务端 SQLite。两套存储不要混写，模式判定只在 `ui/runtime/services.rs` 的 `server_mode`。
- Android 本地打包需在 `dx bundle` 后运行 `python3 scripts/prepare_android_bundle.py`，再用生成的 Gradle 工程出 APK。发布流水线额外传 `--release-tag`，把 tag 转成 `versionCode` / `versionName`（`Cargo.toml` 版本号长期停在 0.1.0，不能作为版本来源）。
- Android 发布产物的断言只认资源名（`aapt2 dump resources` 里的 `mipmap/rssr_launcher`），不认文件路径：release 会跑 `optimizeReleaseResources` 压缩资源路径，按路径断言的写法只在 debug 上成立。

## 代码风格

- rustfmt：`max_width = 100`、`use_small_heuristics = "Max"`（`rustfmt.toml`）。
- clippy 以 `-D warnings` 为基线。
- 性能敏感路径（仓储查询、刷新流程）优先减少 clone、分配与全表扫描。
