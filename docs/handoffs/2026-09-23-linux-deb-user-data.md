# Linux .deb 普通用户数据目录修复

- 日期：2026-09-23
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：1053f35（工作开始时）
- 相关 commit：本交接记录所在的本地提交（写作时 pending）
- 相关 tag / release：已发布 v0.1.16 未包含修复
- 状态：validated（本机规则和打包适配验证；安装后 GUI 验收待 release runner）

## 工作摘要

已发布的 v0.1.16 `.deb` 将程序安装到 `/usr/bin`，而旧版把 SQLite 数据写到可执行文件旁的 `RSS-Reader/`，普通用户无写权限。旧包的 Debian `Version` 也仍为 `0.1.0`。本次只更改 Linux 系统安装位置的数据路径和打包元数据，保留便携版与其它平台的旧路径。

## 影响范围

- 模块：`rssr-infra::db::sqlite_native`、`rssr-app::ui::shell_prefs`。
- 平台：Linux `/usr/bin` 安装版；Windows、macOS、Android、Web 和 Linux 便携版路径保持原语义。
- 额外影响：Linux release job、CI 工具测试、README / 使用指南 / `CLAUDE.md`。

## 关键变更

### 数据路径

- `/usr/bin` 下的原生程序使用 `$XDG_DATA_HOME/rss-reader/`，变量缺失、为空或为相对路径时回退到 `$HOME/.local/share/rss-reader/`；没有可用绝对路径则显式报错，不回退到共享临时目录。
- 索引库、正文库和 `shell-prefs.json` 调用 infra 的同一目录入口。新建目录权限为 `0700`，现有目录权限不改。
- 不自动迁移旧 `/usr/bin/RSS-Reader/`：该目录若存在，可能为 root 所有。文档要求先退出并备份，再由管理员手动复制完整目录并调整所有权。便携版旧数据仍在原位。

### 打包和验收

- `prepare_linux_deb.sh` 从 release tag 写入 Debian `Version`，重打包时将包内文件所有权归一为 `root/root`；`test_prepare_linux_deb.sh` 检查版本、内容保留与错误 tag 拒绝。
- release job 安装 `.deb` 后，以普通 runner 用户、隔离的中文空格 HOME/XDG 路径和 Xvfb 启动两次，检查两个数据库、目录权限与首次写入的复用。该 job 只有新 tag / 手动触发时才执行，本次未触发远端流程。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo test --locked -p rssr-infra`：通过，包含路径、权限、双库打开/重开测试。
- `cargo test --locked -p rssr-cli`：通过。
- `cargo clippy --locked -p rssr-infra --all-targets -- -D warnings`：通过。
- `cargo clippy --locked -p rssr-cli --all-targets -- -D warnings`：通过。
- `bash scripts/test_prepare_linux_deb.sh`、`bash -n`、`shellcheck`：通过。
- `actionlint .github/workflows/ci.yml .github/workflows/release.yml`：通过。
- 对已下载 v0.1.16 `.deb` 执行重打包适配：测试输出 Version 为 `0.1.17`，`usr/bin/rssr-app` 保留且为 `root/root`。这是工具验证，**不是新版本应用包**。
- `cargo clippy --locked -p rssr-app --all-targets -- -D warnings`：未通过，本机缺 `gdk-pixbuf-2.0`、ATK、Cairo、Pango 等 GTK 开发包；错误发生在依赖的 build script，未到应用源码检查。

### 手工验收

- 本机未安装新的 `.deb`，也未运行 GUI：本机缺 Xvfb 和 GTK/WebKit 开发包，且无免密 sudo。release runner 新增的普通用户安装启动验收待首次新 tag 运行。
- v0.1.16 的已发布附件没有修改，不能视为已修复。

## 结果

- 本地路径规则与打包适配可 review；发布前仍须在 release runner 看到普通用户启动检查通过。
- 本次未 push、未打 tag、未发布。

## 风险与后续事项

- 用户升级已发布旧 `.deb` 后，如曾以 root 启动并在 `/usr/bin/RSS-Reader/` 建库，需要手动迁移完整数据目录。
- Linux 安装路径识别以现有 `.deb` 中真实的 `/usr/bin/rssr-app` 为边界；若未来改变安装位置，应同时更新路径规则和安装 smoke。
- 首次新 release 应检查 CI 真实日志及普通用户安装启动结果，再更新 README 的发布状态。

## 给下一位 Agent 的备注

- 路径权威入口在 `crates/rssr-infra/src/db/sqlite_native.rs`；不要在 app/CLI 再拼一次目录。
- 先看 release job 的 `scripts/smoke_linux_deb_unprivileged.sh` 输出，再判断能否解除旧版安装包警示。
