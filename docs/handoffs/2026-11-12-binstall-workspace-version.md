# cargo binstall 支持与 workspace 版本跟随发布

- 日期：2026-11-12
- 作者 / Agent：Claude Code（Opus）
- 分支：claude/binstall-version
- 当前 HEAD：6a863b8d（分支基点）
- 相关 commit：见本分支提交
- 相关 tag / release：v0.2.2（当前 workspace 版本）；发布规则从下一次发布生效
- 状态：`validated`（本机；Release 工作流的新检查未在 GitHub 上运行）

## 工作摘要

CLI 可以用 `cargo binstall --git https://github.com/Develata/RSS-Reader rssr-cli` 直接安装 Release 附件。前提是 workspace 版本等于最新已发布版本，所以本次结束了版本号停在 0.1.0 的状态：现在设为 0.2.2，并由 Release 工作流检查 tag 与版本一致。

## 影响范围

- 模块：
  - `Cargo.toml`、`Cargo.lock`（七个成员 0.1.0 → 0.2.2，依赖未变）
  - `crates/rssr-cli/Cargo.toml`（`repository` 继承与 `[package.metadata.binstall]`）
  - `.github/workflows/release.yml`（新 job `check-version`，全部构建 job 依赖它）
- 平台：
  - CLI：macOS（Intel / Apple Silicon）、Linux x86_64、Windows x86_64
- 额外影响：
  - 发布规则（CLAUDE.md）、README 中英文安装说明、几处“版本停在 0.1.0”的注释与文档

## 关键变更

### 为什么曾停在 0.1.0（已核实）

- 0.1.0 在 2026-03-24 搭工作区骨架时写入（759d8412），之后从未修改。v0.1.21（Android）、v0.1.22（Windows PE / CLI）、2026-10-05（macOS bundle）每次修版本显示，都选择读 `RELEASE_TAG`、“不修改 workspace 版本”，以缩小当次改动。归档 handoff 中没有固定 0.1.0 的理由，属于累积下来的路径依赖。

### binstall 配置

- binstall 的 `{ version }` 取自 crate 清单。版本保持 0.1.0 时实测它不会报错，而是下载 `releases/download/v0.1.0/rssr-cli-macos-aarch64.tar.gz`，即 4 月的旧 CLI。所以版本号修正与 binstall 配置必须在同一提交。
- `rssr-cli` 原先没有继承 `repository`，`{ repo }` 会为空；已补上。
- 下载地址按现有附件命名：`rssr-cli-{ os-name }-{ target-arch }.tar.gz`，Windows 为 `.zip`。二进制在归档根目录。
- 禁用 quick-install（第三方构建）。musl 目标指向一个不发布的附件名，让它退回源码编译，而不是拿到 glibc 构建。没有附件的平台（如 Linux aarch64）同样退回编译。
- `rssr-cli` 不在 crates.io（名称未被占用；工作区依赖 `[patch.crates-io]` 的 vendor 补丁，发布到 crates.io 不现实），所以安装命令带 `--git`，读默认分支的清单。

### 发布规则

- workspace 版本必须等于最新已发布版本：发版准备提交里改成新版本，再打 tag；main 上不写“下一个开发版本”。
- `check-version` 用 `cargo metadata --no-deps` 检查每个 workspace 成员的版本都等于 tag 去掉 `v`，在任何构建之前失败。
- 代价：从版本提交合入 main 到 Release 发布完成之间，`--git` 安装会 404（附件尚未上传）。
- `RELEASE_TAG` 注入保留不动。构建标识仍不用 `CARGO_PKG_VERSION`，理由改为“它只在发版时变，区分不了两次开发构建”。

## 验证与验收

### 自动化验证

本机为 macOS 27.2 arm64，rustc 1.99.1。

- `cargo update --workspace`：`Cargo.lock` 只有七个成员版本 0.1.0 → 0.2.2。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：唯一错误是 `crates/rssr-infra/tests/test_library_store.rs:954` 的 `i32 == i16`。macOS 的 `libc::F_UNLCK` 是 `c_short`，Linux 是 `c_int`；这是本次之前就有的 macOS 专属问题，与本次改动无关，Linux CI 不受影响。`--lib --bins` 范围通过。
- `cargo test --workspace --exclude rssr-infra --locked`：除 rssr-web 外全部通过。rssr-web 在默认 `TMPDIR` 下有 66 个失败，原因是 macOS 的 `/var` 是符号链接，认证库拒绝经符号链接打开（见 `docs/deployment/web.md`）。改用真实路径作为 `TMPDIR` 后，`cargo test -p rssr-web --locked` 103 passed / 0 failed。
- `cargo test -p rssr-infra --locked --lib` 加除 `test_library_store` 以外的全部集成测试：466 passed / 0 failed。`test_library_store` 因上面的编译错误在 macOS 上无法构建。
- `python3 scripts/test_prepare_macos_bundle.py`：通过。
- wasm target check 未执行：本次没有改动 UI 或共享代码，只改了版本号。

### 手工验收

- 用 v0.2.2 已发布附件做 binstall 实测（scratch 工作区，版本 0.2.2，同一份配置）：
  - `--manifest-path` dry-run，`--targets` 依次为 aarch64-apple-darwin、x86_64-apple-darwin、x86_64-unknown-linux-gnu、x86_64-pc-windows-msvc：四个都找到附件并解析出 `rssr-cli` / `rssr-cli.exe`。
  - 对照：版本 0.1.0 时下载的是 v0.1.0 附件（见上）。
  - 仓库 scratch clone 加同样改动后，`cargo binstall --git file://<clone> rssr-cli --disable-strategies compile` 真实安装，`rssr-cli --version` 输出 `rssr 0.2.2`；macOS 上直接运行，未被 Gatekeeper 拦截。
- `check-version` 的脚本在本机以 `RELEASE_TAG=v0.2.2` 运行通过；`RELEASE_TAG=v0.2.3` 时列出七个 crate 并以 1 退出。

## 结果

- 合并后即可用 `cargo binstall --git` 安装 v0.2.2 CLI。
- 下一次发版必须先把 workspace 版本改成新 tag 的版本，否则 Release 在 `check-version` 失败。

## 风险与后续事项

- Windows 与 Linux 的 CLI 默认把数据放在可执行文件旁，binstall 安装后即 `~/.cargo/bin/RSS-Reader/`，与桌面版不是同一个资料库。README 已说明用 `--data-dir` / `RSSR_DATA_DIR`；数据目录规则本身未改。
- `check-version` 未在 GitHub runner 上实际运行；第一次发版时确认。
- 没有 Linux aarch64、musl 附件；需要时在 release.yml 加构建，binstall 配置无需改。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-cli/Cargo.toml` 的 binstall 段、`release.yml` 的 `check-version`。
- 改 CLI 附件命名或归档结构时同步改 binstall 的 `pkg-url` / `bin-dir`，并用 `cargo binstall --manifest-path <工作区> rssr-cli --targets <triple> --dry-run` 对已发布附件核对。
