# P0：抽出原生宿主 crate `rssr-host`

- 日期：2026-10-20
- 作者 / Agent：Claude Code
- 分支：refactor/rssr-host（基于 design/rssr-web-server-mode，后者基于 main e4c4a82）
- 当前 HEAD：见 `git log refactor/rssr-host`
- 相关 commit：本记录同次提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按已确认的 `docs/design/rssr-web-server-mode.md` 第 5 节 P0，把宿主能力与原生宿主从 `rssr-app`
抽成不依赖界面框架的 `rssr-host`，桌面与 Android 改为依赖它。功能行为不变，日志有可观察的变化（见「结果」）；它是 P1 服务端复用同一宿主的前提。

## 影响范围

- 模块：
  - `crates/rssr-host/`（新增）：`lib.rs`、`native.rs`、`refresh_flight.rs`
  - `crates/rssr-app/src/bootstrap.rs`、`bootstrap/native.rs`、`bootstrap/web.rs`、`bootstrap/web/refresh.rs`
  - `Cargo.toml`（workspace member）、`Cargo.lock`、`crates/rssr-app/Cargo.toml`
  - `CLAUDE.md`、`AGENTS.md`（crate 清单）
- 平台：
  - Windows / macOS / Linux / Android（原生宿主改由 `rssr-host` 提供）
  - Web（浏览器宿主仍在 `rssr-app`，改用 `rssr-host` 的 trait、`RefreshFlight` 与调度函数）
- 额外影响：
  - N/A（Dockerfile 为 `COPY . .`，无需改动）

## 关键变更

### `rssr-host`

- `lib.rs`：宿主能力 trait（`AutoRefreshPort`、`RefreshPort`、`ReaderAssetPort`、`RemoteConfigPort`）、
  `HostCapabilities`、各 `*Outcome`、自动刷新调度函数，可见性由 `pub(crate)` 改为 `pub`。两个目标都编译。
- `native.rs`（由 `rssr-app/src/bootstrap/native.rs` 经 `git mv` 移来；原路径在同一提交里换成了新的薄包装，所以提交记录为新增而非重命名，`git log --follow` 追不到旧历史）：`AppServices` 改名为
  `NativeHost`，进程级 `OnceCell` 与 `from_default_location()` 移出，改为
  `NativeHost::open(NativeSqliteBackend) -> Arc<NativeHost>`，由调用方决定数据目录。
  其余刷新、单飞、自动刷新、图片本地化、WebDAV 逻辑原样搬移；日志文案"初始化桌面端本地数据库"
  改为"初始化本地数据库"。
- `refresh_flight.rs`：原样搬移，`RefreshFlight` 改为 `pub`。
- 与 domain / application / infra 一致，加了 `#![allow(clippy::double_must_use)]`
  （async_trait 展开触发的 Clippy 误报）。

### `rssr-app`

- `bootstrap/native.rs` 变成薄包装：进程内共享一个 `AppServices { host: Arc<NativeHost> }`，
  在默认数据目录上打开。
- `bootstrap.rs` 只剩模块声明与对 `rssr_host` 类型的 `pub(crate)` 再导出，页面和 runtime 不用改。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：通过（exit 0）；迁移过来的 9 个测试在 `rssr-host` 下全部通过
  （6 个自动刷新调度、2 个 `RefreshFlight`、1 个按需图片本地化）
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过。本机需显式指定 NDK 28 工具链：
  `CC_aarch64_linux_android`、`AR_aarch64_linux_android`、`CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER`
  指向 `android-clt/current/ndk/28.2.13676358/toolchains/llvm/prebuilt/windows-x86_64/bin`
  下的 `aarch64-linux-android24-clang.cmd` / `llvm-ar.exe`；不指定时 `ring` 的构建脚本因找不到 clang 失败。

### 手工验收

- 桌面启动冒烟：`RUST_LOG=info target/debug/rssr-app.exe` 运行 25 秒后由 `timeout` 结束。日志依次出现
  `rssr_host::native: 初始化本地数据库`、`本地数据库就绪 journal_mode=wal refresh_concurrency=4`、
  `触发后台自动刷新全部订阅`。未手动点击界面。

## 结果

- P0 完成，可合并。刷新、单飞、自动刷新、图片本地化、初始化顺序与错误上下文不变。
- 可观察的日志变化（有意为之）：原生宿主日志的 tracing target 从 `rssr_app::bootstrap::imp` 变为 `rssr_host::native`，一条消息从"初始化桌面端本地数据库"改为"初始化本地数据库"。桌面默认过滤器是全局 `info`，输出不受影响；只有自定义 `RUST_LOG=rssr_app=…` 这类按 target 过滤的配置会漏掉这些日志，此时需要加上 `rssr_host`。不恢复旧 target：共享宿主冒用 `rssr_app` 的模块路径，会给服务端日志打错标签。

## 风险与后续事项

- 发现一处既有问题，本次未修：WAL 降级警告的字符串字面量里夹着一段多余空格
  （`native.rs` 中"…文件系统），<空格>刷新退回串行…"），只影响日志文案。
- 下一步 P1：在 `rssr-web` 中用 `NativeHost::open(NativeSqliteBackend::with_path(<RSS_READER_WEB_DATA_DIR>/rss-reader.db))` 打开文章库（正文库路径由 `with_path` 自动派生），并实现 `/api/v1`。
  P1 依赖 5070d71（抓取超时、`feed_http_client`），该提交还在 `feat/reader-mathml`，开工前需要先合入 main。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-host/src/lib.rs`、`crates/rssr-host/src/native.rs`。

## 交叉模型审查（codex-reviewer，2026-10-20）

对 b0b3099 的审查结论：没有阻断项。逐条核实后的处理如下：

- 日志 target 与消息变化：属实。不恢复旧 target，改为在「结果」中如实记录（见上）。
- `HostCapabilities` 的四个 `Arc<dyn …Port>` 在原生端不是 `Send + Sync`，P1 的 axum 共享状态和
  `tokio::spawn` 都用不了：属实，已修。新增 `PortBounds`，原生端等于 `Send + Sync`，wasm 端不加约束；
  四个 trait 都以它为上界；新增编译期断言测试 `native_host_capabilities_can_be_shared_across_threads`。
- 后台任务没有关闭方法，同一数据库重复 `open` 会出现两套自动刷新：属实，是既有的进程单例假设。
  已在 `NativeHost` 文档注释中写明"一个数据库、一个进程、一个宿主"的前置条件；受控关闭留到 P1。
- 检查清单漏掉迁出的测试：属实，`application-use-case-boundary-checklist.md` 已补
  `cargo test --locked -p rssr-host`。
- 缺少 `NativeHost::open` 的集成覆盖：已补 `open_creates_and_migrates_both_databases_at_an_explicit_path`，
  覆盖显式路径打开、双库创建与迁移、用例可用。审查者没有 b0b3099 的构建证据，是因为证据只写在交接里、
  没有随审查包提供；完整验证见上文，修复后又重跑了一次（见下）。
- P1 的提醒：`NativeSqliteBackend::with_path` 不会创建父目录，服务端要先建好 `RSS_READER_WEB_DATA_DIR`；
  出站策略需要通过新增的 `open_with_options` 一类入口注入，覆盖抓取、图片和 WebDAV 三条出站路径。

### 修复后的验证

- E: 剩余空间跌破磁盘闸的 10 GB 硬阈值，按 Develata 的选择执行了 `cargo clean --profile dev`，
  删除 36.6 GiB。dry-run 确认不涉及 `target/audit-20261009`（那里有一个独立的 git worktree）。
  之后为全量冷构建，统一用 `-j 2`。
- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings`：通过
- `cargo test -p rssr-host --locked`：11 passed
- `cargo test --workspace --locked -j 2`：exit 0。第一次运行因写入 `target/debug/.fingerprint` 时报
  `os error 3` 而失败，原因未查明，可能是并发进程动了刚清空的目录；重跑通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check -p rssr-app --target aarch64-linux-android --locked`（NDK 环境变量同上）：通过
- 桌面启动冒烟（25 秒，由 timeout 结束）：日志同前，依次是 `rssr_host::native` 初始化、就绪、触发自动刷新。
