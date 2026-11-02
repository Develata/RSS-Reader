# Linux 桌面入口名称与 0.2.2-1 打包修订

- 日期：2026-11-02
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：`claude/linux-desktop-entry`
- 当前 HEAD：`f0db8420`（提交前）
- 相关 commit：本分支提交
- 相关 tag / release：`v0.2.2` 新增附件 `RSS-Reader-linux-x86_64-0.2.2-1.deb`；[Develata/apt-repo](https://github.com/Develata/apt-repo) 发布 `rssr-app 0.2.2-1`
- 状态：`validated`

## 工作摘要

v0.2.2 的 deb 安装后，桌面菜单显示 `RssrApp`，软件包说明也是 `RssrApp`。源码里改用 Dioxus 自带的 desktop 模板与 `short_description` 修正，并在 Release 工作流加断言；已发布的 v0.2.2 按 Develata 的决定不覆盖附件，另发 Debian 打包修订 `0.2.2-1`（程序不变），经 apt 源推给用户。

## 影响范围

- 模块：`Dioxus.toml`、`linux/rssr-app.desktop`（新增）、`.github/workflows/release.yml` 的 Linux smoke 检查。
- 平台：Linux deb。`short_description` 另会进入 Windows 安装器模板，但 Release 不生成 Windows 安装器；macOS 打包不读该字段。
- 外部：v0.2.2 Release 增加第 15 个附件；`Develata/apt-repo` 的 `sources.json` 与 `scripts/fetch.sh`。

## 关键变更

### 原因（对照 Dioxus CLI 0.7.9 源码）

- Linux 打包的 `Name=` 来自 `bundled_app_name()`：可执行文件名 `rssr-app` 转 PascalCase，得到 `RssrApp`；`[application] name` 不参与。
- 软件包 `Description` 取 `bundle.short_description`，否则取 crate `description`，二者都没有设置。
- `bundle.deb.desktop_template` 用 `fs::read_to_string` 直接读取，相对路径按 `dx bundle` 的工作目录（CI 与本地都是仓库根目录）解析，与按 crate 目录解析的图标路径不同；`Dioxus.toml` 注释写明了这一点。

### 源码修正

- `linux/rssr-app.desktop`：`Name=RSS-Reader`、`Comment`、`Categories=Network;Feed;News;`、`Keywords`；`Exec` / `Icon` 仍用打包器变量（渲染为 `rssr-app`），文件名不变，窗口与入口的对应关系不变。
- `Dioxus.toml`：`[bundle] short_description = "Local-first RSS and Atom reader"`；`[bundle.deb] desktop_template`。
- `release.yml`：对重新打包后的正式 deb 断言 `.desktop` 含整行 `Name=RSS-Reader`、`Description` 等于上述说明。

### 0.2.2-1 打包修订

- 由正式 v0.2.2 deb（SHA256 `8bbfe59a…`）`dpkg-deb -R` 解包后修改：`.desktop` 替换为本分支 `dx bundle` 实际渲染的文件（逐字节相同），权限显式设为 0644（本机 umask 077，dx 本地产物为 0600；CI umask 022 不受影响）；control 只改 `Version: 0.2.2-1` 与 `Description`；重算 `md5sums`；`dpkg-deb --build --root-owner-group`。
- `/usr/bin/rssr-app` 与图标的 SHA256 与原包相同（`6a2e4a36…`、`d36036c1…`）。新包 SHA256 `ac9d00aa29d0224a79d6a9706484db73773ee89c840ebfc20378351da85d7082`，已作为新附件上传到 v0.2.2 Release；原 14 个附件未改动。
- 这是手工重打包，不是 CI 产物；以上逐文件比对是其可追溯依据。
- apt 源：`sources.json` 的资产可写成按顺序尝试的名字列表，`["RSS-Reader-linux-x86_64-0.2.2-1.deb", "RSS-Reader-linux-x86_64.deb"]`。下一次正式发布不带修订附件时自动回落到常规 deb；之后可删去第一项。

## 验证与验收

### 自动化验证

- `dx bundle --locked --platform desktop --package-types deb --package rssr-app --release`（本机 dx 0.7.9）：exit 0；产物 `.desktop` 为 `Name=RSS-Reader` 等预期内容，`Description: Local-first RSS and Atom reader`。
- `scripts/prepare_linux_deb.sh` 重新打包该产物后，逐条运行新的 smoke 断言：通过；同一断言对 v0.2.2 正式 deb 失败（`grep` exit 1），确认能拦住旧名称。
- `release.yml` 用 `yaml.safe_load` 解析通过。本分支改动不涉及 Rust 源码，未跑 cargo 门禁；完整 Release 工作流要到下次打 tag 才实际运行。
- Docker `debian:trixie`：从线上 apt 源装 0.2.2（`Name=RssrApp`）后装本地 0.2.2-1：版本与名称更新，`.desktop` 0644 root，`dpkg --verify` 无输出。
- apt-repo `fetch.sh`：首选修订附件、首选缺失时回落常规附件、全部缺失时报错退出，三种情况均按预期。Publish 运行成功，线上 `Packages` 为 `rssr-app 0.2.2-1`。
- Docker `debian:trixie`：先装 v0.2.2 正式 deb，再加线上源，`apt list --upgradable` 显示 `0.2.2-1 [upgradable from: 0.2.2]`，`apt-get upgrade` 后 `Name=RSS-Reader`。

### 未验证

- 真实 GNOME 桌面上的菜单显示、搜索关键词与运行窗口的图标对应（容器内无桌面）。
- 本分支源码修正在正式 Release 工作流中的运行（下次发版时）。

## 结果

- 已装 v0.2.2 的 apt 用户经 `apt upgrade` 得到 `RSS-Reader` 名称；直接下载者可用新附件。之后的正式 deb 由模板直接生成正确名称。

## 风险与后续事项

- 若 GNOME Dock 中运行窗口与入口图标不对应，可在模板加 `StartupWMClass`（未观察到问题，未加）。
- 下次发布后从 apt-repo `sources.json` 删去 `0.2.2-1` 那一项（留着也只会回落，不影响结果）。
- deb 仍不含 `rssr-cli`（CLI 单独 tar.gz），与 [CLI/GUI 分发设计](../design/cli-gui-distribution.md) 中 deb 同装两个程序的描述不一致，本次未改。

## 给下一位 Agent 的备注

- 入口：`linux/rssr-app.desktop`、`Dioxus.toml` 的 `[bundle.deb]`、`release.yml` 的 “Smoke-check Linux artifacts”。
- apt 源与签名见 [Develata/apt-repo](https://github.com/Develata/apt-repo) README。
