# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

本地优先、面向个人的极简 RSS 阅读器（Rust Edition 2024 + Dioxus 0.7.3），覆盖桌面（Windows/Linux/macOS）、Web、Android 与 CLI。

产品边界长期固定为四类能力：**订阅、阅读、基本设置、基础配置交换**（`docs/design/functional-design-philosophy.md`）。超出边界的功能（AI 总结/分析、推荐流、社交、标签树/文件夹系统、文章库跨设备同步等）默认拒绝，不要实现。

## 常用命令

```bash
# 桌面端运行
cargo run -p rssr-app

# CLI
cargo run -p rssr-cli -- --help

# Web 端开发（需 rustup target add wasm32-unknown-unknown 和 dioxus-cli 0.7.3）
dx serve --platform web --package rssr-app

# 提交前验证（与 CI 一致）
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p rssr-app --target wasm32-unknown-unknown   # 涉及 UI/共享代码时必查
cargo check -p rssr-app --target aarch64-linux-android    # 涉及移动端交互时

# 单个集成测试（集成测试集中在 crates/rssr-infra/tests/）
cargo test -p rssr-infra --test test_entry_state_and_search
# 按名称过滤单元测试
cargo test -p rssr-application <test_name>

# wasm 浏览器契约测试（CI 三个 harness；需 wasm-bindgen-cli 0.2.114 + Chrome for Testing）
bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness
# 其余：wasm_subscription_contract_harness / wasm_config_exchange_contract_harness

# Web 部署态验证（登录 + /feed-proxy，用于 CORS 受限源）
dx bundle --platform web --package rssr-app --release --debug-symbols false --out-dir target/web-e2e
cargo run -p rssr-web -- --print-password-hash adminadmin
# 然后按 README「发布与交付」一节设置 RSS_READER_WEB_* 环境变量启动 rssr-web
```

## 架构

六个 crate 组成分层架构，依赖方向严格向内（UI → application → domain；infra 实现 domain/application 定义的接口）：

- **`rssr-domain`** — 实体（`Feed`、`Entry`、`UserSettings`、`AppStateSnapshot`）、仓储 trait（`FeedRepository`、`EntryIndexRepository`、`EntryContentRepository` 等）、`DomainError`。无任何 I/O。
- **`rssr-application`** — 用例服务（`RefreshService`、`ReaderService`、`EntriesListService`、`ImportExportService`、`SubscriptionWorkflow` 等）。`composition.rs` 中的 `AppUseCases::compose(AppCompositionInput)` 用端口 trait（`FeedRefreshSourcePort`、`RefreshStorePort`、`OpmlCodecPort`、`ClockPort`、`AppStatePort`）组装全部服务，平台无关。
- **`rssr-infra`** — 端口适配器，按编译目标二选一（`lib.rs` 中 `#[cfg(not(target_arch = "wasm32"))]` 门控）：
  - **原生（桌面/Android）**：sqlx SQLite，**索引库与正文库是两个独立数据库**（迁移分别在 `migrations/` 与 `migrations_content/`）；reqwest 抓取 + `BodyAssetLocalizer` 正文图片本地化；feed-rs/quick-xml 解析；OPML 编解码；WebDAV 配置同步。入口 `compose_native_sqlite_use_cases(index_pool, content_pool)`。
  - **wasm32**：`application_adapters/browser/` 下的适配器包一个 `Arc<Mutex<BrowserState>>`（序列化到 `localStorage`）；`db`/`fetch`/`parser`/`config_sync` 模块在 wasm 上不编译。入口 `compose_browser_use_cases`。
- **`rssr-app`** — Dioxus UI。`bootstrap/native.rs` 与 `bootstrap/web.rs` 按平台选择组装并暴露 host capabilities（刷新、剪贴板、图片本地化、远程配置）；`ui/` 分 shell / commands / runtime；`pages/` 只调用 `AppServices`。
- **`rssr-cli`** — 复用同一 application 层的自动化入口。
- **`rssr-web`** — 仅用于 Web 部署的薄 axum 服务：Argon2 登录、HttpOnly 会话 cookie、静态包托管 + SPA 回退、`/feed-proxy` 服务端代抓（绕过浏览器 CORS）。桌面端、CLI、本地开发都不依赖它。

关键约束：**平台差异只允许存在于 infra / adapter / host capability 层**，application/domain 的语义必须跨平台统一。UI 组件里禁止直接写 SQL、HTTP 或 feed/OPML/JSON 解析。

桌面端数据库在首次启动时自动创建于可执行文件同目录 `RSS-Reader/rss-reader.db` 并执行迁移。

## 仓库规则（来自 AGENTS.md，必须遵守）

- **交接记录**：每完成一次可交付工作，必须在 `docs/handoffs/` 新增/更新 `YYYY-MM-DD-<slug>.md` 记录（模板 `docs/handoffs/TEMPLATE.md`），含工作摘要、受影响模块、验证命令与结果、commit 状态。未补记录不算完整交付。
- **架构护栏**：出现代码严重分叉、平台差异回流到 application/domain 语义、前后端大规模职责重分配、或违背功能设计哲学的方案时，必须先做保守分析并向用户明确提出，默认持保守/负面倾向，不得静默推进。
- **就近 AGENTS.md**：`crates/rssr-infra/src/db/AGENTS.md` 与 `crates/rssr-app/src/pages/AGENTS.md` 对各自目录有更细的修改约束（如改 `entry_repository.rs` 的排序/筛选必须同步考虑列表页、阅读导航、搜索一致性，并跑 `test_entry_state_and_search`）。改这些目录前先读。
- **页面层改动默认同时影响 Web / 桌面 / Android**，不要只按桌面视口判断；改完至少跑 wasm target check。
- 行为边界、用户流程、配置方式变化时同步更新 `README.md`、`docs/`、`specs/`。

## 平台差异提醒

- Web 端受浏览器 CORS 限制：部分 feed 桌面端可刷、Web 直连失败，需经 `rssr-web` 的 `/feed-proxy` 验证。
- Web 端持久化是 `localStorage` 序列化状态，不是 SQLite；不要把两套语义混写。
- Android 本地打包需在 `dx bundle` 后运行 `python3 scripts/prepare_android_bundle.py`，再用生成的 Gradle 工程出 APK。

## 代码风格

- rustfmt：`max_width = 100`、`use_small_heuristics = "Max"`（`rustfmt.toml`）。
- clippy 以 `-D warnings` 为基线。
- 性能敏感路径（仓储查询、刷新流程）优先减少 clone、分配与全表扫描。
