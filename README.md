# RSS-Reader

> 打开订阅，直接阅读。

<p align="center">
  <a href="https://github.com/Develata/RSS-Reader/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/Develata/RSS-Reader?display_name=tag"></a>
  <a href="https://github.com/Develata/RSS-Reader/actions/workflows/ci.yml?query=branch%3Amain"><img alt="CI" src="https://github.com/Develata/RSS-Reader/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="./LICENSE"><img alt="MIT License" src="https://img.shields.io/github/license/Develata/RSS-Reader"></a>
  <a href="#30-秒开始阅读"><img alt="Release targets: Windows, Linux, macOS, Android, Web" src="https://img.shields.io/badge/Targets-Windows%20%7C%20Linux%20%7C%20macOS%20%7C%20Android%20%7C%20Web-0078D4"></a>
</p>

![RSS-Reader Web 版文章首页：文章列表、阅读导航与时间目录](assets/readme/rss-reader-overview.png)

_Web 版实际界面；订阅与文章使用演示数据。_

RSS-Reader 是用 Rust 和 Dioxus 构建的本地优先 RSS 阅读器。它把订阅、刷新、筛选和阅读放在一条简短的使用路径上；桌面、Web、Android 和 CLI 复用同一套核心能力。文章库留在当前设备，配置可以通过 JSON、OPML 或可选的 WebDAV 迁移。

[下载最新版本](https://github.com/Develata/RSS-Reader/releases/latest) · [发布说明](https://github.com/Develata/RSS-Reader/releases) · [English](./docs/README.en.md) · [文档](./docs/README.md) · [报告问题](https://github.com/Develata/RSS-Reader/issues)

## 30 秒开始阅读

1. 在 [Releases](https://github.com/Develata/RSS-Reader/releases/latest) 选择适合设备的附件：

   | 设备 | 桌面 / 安装附件 | 开始使用 |
   | --- | --- | --- |
   | Windows x64 | `RSS-Reader-windows-x86_64.zip` | 解压到可写目录，运行 `RSS-Reader.exe`；系统需有 WebView2 Runtime |
   | Linux x64 | `RSS-Reader-linux-x86_64.deb` | 已发布；普通账户启动的目录写权限问题见下方说明 |
   | macOS Intel / Apple Silicon | `RSS-Reader-macos-x86_64.tar.gz` / `RSS-Reader-macos-aarch64.tar.gz` | 解压到可写目录，打开 `RSS-Reader.app` |
   | Android ARM64 | `RSS-Reader-android-arm64-v8a-release.apk` | 安装 APK；AAB 是应用商店产物，不能直接安装 |
   | Web | `RSS-Reader-web.tar.gz` | 静态站点包；需要受保护的登录和跨域 feed 代抓时使用 [Web 部署指南](./docs/deployment/web.md) |

2. 打开应用，在 **S**（Subscribe）页输入 RSS / Atom 地址并添加订阅。
3. 点击 **R**（Read / Home）查看文章；已在首页时再次点击 **R** 可手动刷新全部订阅。

上表按已发布的 `v0.1.15` 产物核对。`main` 上的后续修复只有在新版本发布后才会进入下载包；具体变更和验收范围请以对应 Release 说明为准。Android 已有正式签名 APK / AAB，但系统返回、长按选择、下拉刷新和图片手势尚未完成真机验收；macOS 也尚未完成实机交互验收。

**Linux 安装包限制：** `v0.1.15` 的 `.deb` 把程序装在 `/usr/bin`，当前程序却尝试在可执行文件目录下创建数据目录。普通账户通常无权写入 `/usr/bin`，因此这份包尚不能视为普通用户可正常启动的安装包；发布流水线只检查了包结构，未覆盖安装后的首次启动。修复需要单独处理数据目录与既有数据迁移。

## 日常使用

- **导航与刷新：** R 始终可见。从阅读页或其他页面点击 R 只返回全部文章页；已在首页再次点击才刷新全部订阅。连续触发复用进行中的刷新，手机首页还可在滚动到顶部后下拉刷新。
- **搜索与筛选：** 放大镜展开标题搜索框，Enter 搜索，Esc 收起。文章列表可按来源、未读、收藏等条件筛选；来源名完整显示，多页时分页按钮保持可触达。
- **阅读：** 左上角返回；可切换已读和收藏、跳转相邻文章、点击正文图片放大。正文保留原生文字选择与复制行为。刷新不会突然替换正在看的正文。
- **设置与迁移：** 齿轮进入设置，支持内置主题和自定义 CSS；JSON / OPML 用于配置交换，可选 WebDAV 同步配置。
- **自动化：** `rssr-cli` 支持订阅管理、刷新、设置和配置导入导出；运行 `cargo run --locked -p rssr-cli -- --help` 查看命令。

Reader 快捷键：`M` 切换已读、`F` 切换收藏、`←` / `→` 跳转上一篇 / 下一篇未读。输入框中与带 Ctrl / Cmd 等修饰键的原生编辑操作不应被阅读快捷键接管。

## 本地数据与边界

桌面端在可执行文件同目录的 `RSS-Reader/` 下创建 `rss-reader.db`（索引）和 `rss-reader-content.db`（正文）。SQLite 的 `-wal`、`-shm` 是正常附属文件；备份时先退出应用，或连同 WAL 一起复制。Android 使用应用沙箱内的本地 SQLite；卸载应用会清除本地数据，请先导出配置。Web 将状态序列化保存到当前浏览器的 `localStorage`，它与桌面数据库互不共享；清除站点数据也会清除本地文章库。

RSS-Reader 缓存 feed 提供的正文，不主动抓取原网站补全全文。桌面端和 Android 会尽量把正文图片本地化；Web 受浏览器 CORS 限制。JSON / OPML / WebDAV 交换的是订阅与设置，**不是文章库、已读状态或收藏的跨设备同步**。直接运行静态 Web 包时，某些 feed 会因 CORS 无法刷新；`rssr-web` 提供带登录的同源 `/feed-proxy`，见 [Web 部署指南](./docs/deployment/web.md)。

项目的长期范围是订阅、阅读、基本设置和基础配置交换；不做推荐流、社交平台或 AI 内容加工。设计依据见[功能设计哲学](./docs/design/functional-design-philosophy.md)。

## 从源码运行与验证

需要 Rust 稳定版；Web 开发还需要 `wasm32-unknown-unknown` target 与 Dioxus CLI `0.7.9`。

```bash
# 桌面端
cargo run --locked -p rssr-app

# Web 开发
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.9 --locked
dx serve --platform web --package rssr-app

# CLI
cargo run --locked -p rssr-cli -- --help
```

提交前的基础检查：

```bash
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo check --locked -p rssr-app --target wasm32-unknown-unknown
```

页面或移动端交互改动还应按环境补充 Android target、Web/原生 UI 验收。CI 分模块并行执行 Rust 检查、Web 浏览器契约及 Android 构建；每条路径与环境限制见[主线验证矩阵](./docs/testing/mainline-validation-matrix.md)。编译通过不等于 Android 或 macOS 实机已验收。

## 发布与文档

- [使用指南](./docs/user-guide.md)：订阅、搜索、阅读、主题、WebDAV 和本地备份。
- [Web / Docker 部署与登录配置](./docs/deployment/web.md)：GHCR、Compose、生产环境、`/feed-proxy`。
- [Android 构建与验收状态](./docs/roadmaps/android-release-roadmap.md)：本地 APK、签名与待测设备行为。
- [文档索引](./docs/README.md)：设计、测试、交接记录。
- [贡献说明](./CONTRIBUTING.md) · [MIT License](./LICENSE)。

常见问题：Windows 运行通常需要 WebView2 Runtime；Web 直连 feed 受目标站点 CORS 策略影响；CLI 附件面向脚本和高级用户，普通阅读只需应用附件。若 Android 安装遇到签名不匹配，先确认当前包与已安装版本的签名身份；卸载前务必导出配置，避免丢失本地数据。
