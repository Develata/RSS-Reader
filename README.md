# RSS Reader

一个本地优先、面向个人使用的极简 RSS 阅读器。  
它优先解决“顺手订阅、稳定刷新、舒服阅读、可离线查看、易于迁移”这些实际问题，而不是做成一个厚重的平台。

[English README](./docs/README.en.md) | [文档索引](./docs/README.md) | [MIT License](./LICENSE)

## 项目定位

RSS Reader 是一个以 Rust 为核心、基于 Dioxus 构建的跨平台阅读器，当前重点覆盖：

- 桌面端日常阅读
- Web 端浏览器验证与静态部署
- Android Debug APK 构建与后续移动端演进
- 本地 SQLite 持久化
- JSON / OPML 配置交换
- 可选 WebDAV 配置同步
- CLI 自动化与自定义 CSS 主题

## 当前能力

### 阅读与订阅

- 添加 / 删除 RSS feed
- 刷新单个订阅或全部订阅
- 文章列表、阅读页、已读 / 收藏 / 搜索
- 阅读页支持：
  - 返回上一页
  - 上一篇未读 / 下一篇未读
  - 上一篇同订阅文章 / 下一篇同订阅文章

### 本地优先

- 桌面端使用本地 SQLite
- Web 使用 wasm SQLite，并持久化到浏览器存储
- feed 提供多少正文，就缓存多少正文
- 桌面端 / Android 会尽量把正文中的图片资源本地化进缓存 HTML
- Web 端正文也会缓存，但图片本地化受浏览器 CORS 约束

### 配置与迁移

- 导入 / 导出配置包 JSON
- 导入 / 导出 OPML
- WebDAV 上传 / 下载配置
- 自定义 CSS 主题、主题卡片、预置主题切换

### 自动化

- `rssr-cli` 可用于：
  - 列出订阅
  - 添加 / 删除 feed
  - 刷新订阅
  - 导入 / 导出配置
  - 导入 / 导出 OPML
  - 查看 / 保存设置
  - 推送 / 拉取 WebDAV 配置

## 平台状态

| 平台 | 当前状态 |
| --- | --- |
| Windows Desktop | 可发布 |
| Linux Desktop | 可发布 |
| macOS Desktop | 可发布 |
| Web | 可发布 |
| Android Debug APK | 已接入 |
| Android Signed Release APK / AAB | 已有 workflow，待 secrets 与正式验收 |

## 快速开始

### 本地开发

```bash
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.3 --locked
cargo run -p rssr-app
```

### Web 运行

```bash
dx serve --platform web --package rssr-app
```

注意：

- Web 端远端 feed 是否能抓取，取决于目标站点是否允许跨域请求
- 有些 feed 在 desktop / Android 正常，在 Web 会被浏览器 CORS 限制拦住

### 验证

```bash
cargo fmt --all
cargo test --workspace
cargo check -p rssr-app --target wasm32-unknown-unknown
```

## 发布与交付

### GitHub Release 产物

当前 release workflow 会发布：

- Windows desktop
- Linux desktop
- macOS desktop
- Web 静态包
- Android debug APK

如果配置了 Android signing secrets，还会额外发布：

- Android release APK
- Android release AAB

### Docker / Compose

仓库包含 Web 版本的容器化部署支持：

```bash
docker build -t rss-reader-web .
docker compose up --build
```

默认访问：

```text
http://127.0.0.1:8080
```

## 文档导航

更完整的说明放在 [`docs/`](./docs/README.md)：

- [英文 README](./docs/README.en.md)
- [设计文档索引](./docs/design/README.md)
- [Android 发布路线图](./docs/roadmaps/android-release-roadmap.md)
- [测试与回归索引](./docs/testing/README.md)

## 仓库结构

```text
crates/
├── rssr-app/
├── rssr-cli/
├── rssr-application/
├── rssr-domain/
└── rssr-infra/

assets/
docs/
migrations/
specs/
tests/
```

## 开源协议

本项目使用 [MIT License](./LICENSE)。
