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
抽成不依赖界面框架的 `rssr-host`，桌面与 Android 改为依赖它。行为不变；它是 P1 服务端复用同一宿主的前提。

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

- P0 完成，可合并。行为不变。

## 风险与后续事项

- 发现一处既有问题，本次未修：WAL 降级警告的字符串字面量里夹着一段多余空格
  （`native.rs` 中"…文件系统），<空格>刷新退回串行…"），只影响日志文案。
- 下一步 P1：在 `rssr-web` 中用 `NativeHost::open(NativeSqliteBackend::with_path(<RSS_READER_WEB_DATA_DIR>/rss-reader.db))` 打开文章库（正文库路径由 `with_path` 自动派生），并实现 `/api/v1`。
  P1 依赖 5070d71（抓取超时、`feed_http_client`），该提交还在 `feat/reader-mathml`，开工前需要先合入 main。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-host/src/lib.rs`、`crates/rssr-host/src/native.rs`。
