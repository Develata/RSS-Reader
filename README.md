# RSS-Reader

一个本地优先、面向个人使用的极简 RSS 阅读器。  
它优先解决“顺手订阅、稳定刷新、舒服阅读、可离线查看、易于迁移”这些实际问题，而不是做成一个厚重的平台。

[English README](./docs/README.en.md) | [文档索引](./docs/README.md) | [贡献说明](./CONTRIBUTING.md) | [MIT License](./LICENSE)

## 为什么做这个项目

RSS-Reader 是一个以 Rust 为核心、基于 Dioxus 构建的跨平台阅读器，当前重点覆盖：

- 桌面端日常阅读
- Web 端浏览器验证与静态部署
- Android Debug APK 构建与后续移动端演进
- 本地 SQLite 持久化
- JSON / OPML 配置交换
- 可选 WebDAV 配置同步
- CLI 自动化与自定义 CSS 主题

如果你需要的是这样一种工具，这个项目会比较合适：

- 不想把订阅数据交给第三方平台
- 希望桌面端优先、浏览器也能跑
- 需要离线阅读能力，而不只是在线看摘要
- 希望订阅配置能导出、迁移、脚本化
- 想自己写 CSS 主题，或者让 AI 帮你生成主题

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
- 桌面端 / Android 会在 feed 刷新完成后，尽量把正文中的图片资源在后台本地化进缓存 HTML
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

## 下载安装

### 方式一：直接下载 GitHub Release

如果你只是想使用，不想自己编译，最简单的方式是直接下载 GitHub Release 附件。

当前会发布这些产物：

- `RSS-Reader-windows-x86_64.zip`
- `RSS-Reader-linux-x86_64.tar.gz`
- `RSS-Reader-macos-x86_64.tar.gz`
- `RSS-Reader-macos-aarch64.tar.gz`
- `RSS-Reader-web.tar.gz`
- `RSS-Reader-android-arm64-v8a-debug.apk`

另外还会附带 CLI 和部分 Android release 产物：

- `rssr-cli-windows-x86_64.zip`
- `rssr-cli-linux-x86_64.tar.gz`
- `rssr-cli-macos-x86_64.tar.gz`
- `rssr-cli-macos-aarch64.tar.gz`
- `RSS-Reader-android-arm64-v8a-release.apk`
- `RSS-Reader-android-arm64-v8a-release.aab`

说明：

- Android 默认发布的是适用于真机的 `arm64-v8a` Debug APK
- Android release APK / AAB 只有在仓库配置签名 secrets 后才会一起出现
- 如果你本地自己执行 `dx bundle --platform android ...`，记得随后运行 `python3 scripts/prepare_android_bundle.py target/dx/rssr-app/release/android/app/app/src/main`，再进入生成的 Gradle 工程执行一次 `./gradlew assembleDebug` 或 `assembleRelease`，这样 Android 启动图标和应用显示名才会真正打进 APK / AAB
- Windows 桌面端通常需要系统已安装 WebView2 Runtime

### 方式二：本地编译

如果你要改代码、调试、或自己出包，可以本地编译。

## 快速开始

### 桌面端本地开发

```bash
cargo run -p rssr-app
```

### Web 端本地开发

```bash
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.3 --locked
dx serve --platform web --package rssr-app
```

注意：

- Web 端远端 feed 是否能抓取，取决于目标站点是否允许跨域请求
- 有些 feed 在 desktop / Android 正常，在 Web 会被浏览器 CORS 限制拦住
- 如果你通过 `rssr-web` 部署 Web 版本，服务端会代抓 feed，所以像 `https://www.ruanyifeng.com/blog/atom.xml` 这类会被浏览器 CORS 拦住的源也能正常订阅
- Web 端为避免浏览器缓存导致“刷新看起来没生效”，会在刷新 feed 时附加 cache-busting 参数
- `https://blogs.nvidia.com/feed/` 这类源在浏览器里通常可直接使用；`https://www.ruanyifeng.com/blog/atom.xml` 这类源请优先通过 `rssr-web` / Docker 部署方式验证

### 验证

```bash
cargo fmt --all
cargo test --workspace
cargo check -p rssr-app --target wasm32-unknown-unknown
```

### 常用命令

```bash
# 桌面端
cargo run -p rssr-app

# CLI
cargo run -p rssr-cli -- --help

# 仅检查 web target
cargo check -p rssr-app --target wasm32-unknown-unknown

# Android smoke check
cargo check -p rssr-app --target aarch64-linux-android
```

## 如何使用

### 1. 添加订阅

打开“订阅”页，输入一个 RSS / Atom feed URL，然后点击“添加订阅”。

建议优先用桌面端测试真实远端 feed，因为：

- 桌面端不会受浏览器 CORS 限制
- Web 端只有目标站点允许跨域时才能直接刷新远端 feed

### 2. 阅读文章

“文章”页支持：

- 标题搜索
- 仅未读
- 仅收藏
- 进入阅读页

阅读页支持：

- 返回上一页
- 标记已读 / 未读
- 收藏 / 取消收藏
- 上一篇未读 / 下一篇未读
- 上一篇同订阅文章 / 下一篇同订阅文章

### 3. 切换主题

“设置”页支持：

- 直接编辑自定义 CSS
- 导入主题文件
- 导出当前 CSS
- 预置主题按钮
- 主题下拉
- 主题卡片切换

如果你想自定义外观但不想手写 CSS，可以先读：

- [功能设计哲学](./docs/design/functional-design-philosophy.md)
- [前端命令与界面接口清单](./docs/design/frontend-command-reference.md)
- [主题作者选择器参考](./docs/design/theme-author-selector-reference.md)

### 4. 导入导出

当前支持：

- 配置包 JSON 导入 / 导出
- OPML 导入 / 导出
- WebDAV 配置同步

配置交换的原则是：

- 迁移订阅与设置
- 不把它做成一个云端数据库同步平台
- 本地阅读库仍然以本地为主

## 数据存储与缓存

### 桌面端

桌面端使用本地 SQLite。

默认数据库会自动创建在可执行文件同目录下的 `RSS-Reader/rss-reader.db`。首次启动时程序会自动：

- 创建数据目录
- 创建 SQLite 数据库
- 执行迁移

### Web 端

Web 端使用 wasm SQLite，并把数据库持久化到浏览器存储中。

### 正文缓存策略

当前缓存边界是：

- feed 提供多少正文，就缓存多少正文
- 不默认抓取站点原网页去补全文

另外：

- 桌面端 / Android 会在刷新成功后，于后台尽量把正文里的图片资源本地化进缓存 HTML
- 这样已经成功缓存过的图片，在远端删除后仍然可读
- 图片本地化不会再阻塞“新增订阅 / 刷新订阅”的主流程；即使图片抓取失败或超时，刷新本身仍然成功
- Web 端正文也会缓存，但图片本地化受浏览器 CORS 限制，可能保留远端 URL

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

仓库包含 **Web 版本** 的容器化部署支持。

当前镜像运行的是一个很薄的 `rssr-web` 服务进程，它负责：

- 显示用户名 / 密码登录页
- 在服务端校验凭据
- 签发 `HttpOnly` 会话 cookie
- 登录后再提供 Dioxus Web 静态资源与 SPA 路由回退

它 **不是** 桌面端、CLI、Android 或本地开发运行所需的依赖；只有 Docker / GHCR 的 Web 部署镜像才会用到它。

如果你本地开发或运行原生版本，完全不需要这层部署服务：

- 桌面端：`cargo run -p rssr-app`
- Web 开发：`dx serve --platform web --package rssr-app`

如果你要验证 Web 部署态的完整能力，尤其是：

- 用户名 / 密码登录
- 同源 `/feed-proxy`
- CORS 受限源（例如 `https://www.ruanyifeng.com/blog/atom.xml`）

推荐直接本地运行 `rssr-web`：

```bash
dx bundle --platform web --package rssr-app --release --debug-symbols false --out-dir target/web-e2e

cargo run -p rssr-web -- --print-password-hash adminadmin

RSS_READER_WEB_BIND=127.0.0.1:8060 \
RSS_READER_WEB_STATIC_DIR=target/web-e2e/public \
RSS_READER_WEB_USERNAME=admin \
RSS_READER_WEB_PASSWORD_HASH='<把上一步输出粘贴到这里>' \
RSS_READER_WEB_SESSION_SECRET=01234567890123456789012345678901 \
cargo run -p rssr-web
```

然后访问 `http://127.0.0.1:8060/login`，用 `admin / adminadmin` 登录后测试 feed 导入与刷新。

### 手动修改 Web 登录用户名 / 密码

`rssr-web` 的登录账号由环境变量控制，最常用的就是这三项：

- `RSS_READER_WEB_USERNAME`
- `RSS_READER_WEB_PASSWORD_HASH`
- `RSS_READER_WEB_SESSION_SECRET`

推荐按下面的顺序手动修改：

1. 先生成一个新的 Argon2 密码哈希：

```bash
cargo run -p rssr-web -- --print-password-hash '请换成你自己的强密码'
```

2. 然后在启动 `rssr-web` 或 `docker compose` 之前，替换环境变量：

```bash
export RSS_READER_WEB_USERNAME='请换成你的用户名'
export RSS_READER_WEB_PASSWORD_HASH='把上一步输出的 Argon2 哈希粘贴到这里'
export RSS_READER_WEB_SESSION_SECRET='至少32字符的随机长串'
```

3. 重新启动服务：

```bash
cargo run -p rssr-web
```

或者：

```bash
docker compose up -d
```

补充说明：

- `RSS_READER_WEB_PASSWORD_HASH` 变了以后，旧密码会立即失效
- `RSS_READER_WEB_USERNAME` 变了以后，登录页就必须使用新用户名
- `RSS_READER_WEB_SESSION_SECRET` 变了以后，旧会话 cookie 会失效，用户需要重新登录
- 部署环境不要再保存明文密码，优先只保留 Argon2 哈希

### 直接拉取 GitHub 镜像运行

默认的 [docker-compose.yml](./docker-compose.yml) 是“直接拉取 GitHub Container Registry 镜像”的部署模板，不会在本地重新构建镜像：

```bash
export RSS_READER_WEB_USERNAME=admin
export RSS_READER_WEB_PASSWORD_HASH='请替换成 Argon2 密码哈希'
export RSS_READER_WEB_SESSION_SECRET='至少32字符的随机长串'
docker compose up -d
```

默认访问：

```text
http://127.0.0.1:8039
```

也支持通过环境变量覆盖镜像名和端口：

```bash
RSS_READER_WEB_USERNAME=admin \
RSS_READER_WEB_PASSWORD_HASH='请替换成 Argon2 密码哈希' \
RSS_READER_WEB_SESSION_SECRET='至少32字符的随机长串' \
RSS_READER_IMAGE=ghcr.io/develata/rss-reader:latest \
RSS_READER_PORT=8090 \
docker compose up -d
```

如果你不想用 compose，也可以直接运行镜像：

```bash
docker run --rm \
  -p 8039:8080 \
  -e RSS_READER_WEB_USERNAME=admin \
  -e RSS_READER_WEB_PASSWORD_HASH='请替换成 Argon2 密码哈希' \
  -e RSS_READER_WEB_SESSION_SECRET='至少32字符的随机长串' \
  ghcr.io/develata/rss-reader:latest
```

说明：

- 推荐先生成密码哈希：

```bash
cargo run -p rssr-web -- --print-password-hash '请改成你自己的强密码'
```

- 部署环境请使用 `RSS_READER_WEB_PASSWORD_HASH`，不要继续保存明文密码
- `RSS_READER_WEB_SESSION_SECRET` 请使用长度至少 32 的随机字符串
- 生产环境建议同时设置：
  - `RSS_READER_WEB_ENV=production`
  - `RSS_READER_WEB_SECURE_COOKIE=true`
- 如果启用了 `RSS_READER_WEB_ENV=production`，但没有开启 `RSS_READER_WEB_SECURE_COOKIE=true`，服务会拒绝启动
- 本地 HTTP 测试时可保持 `RSS_READER_WEB_ENV=development`

### 本地构建镜像

```bash
docker compose -f docker-compose.yml -f docker-compose.build.yml up --build
```

这会在保留 compose 默认端口配置的同时，覆盖成“从当前工作区构建镜像”。

如果你只想手动验证 Dockerfile，也可以直接：

```bash
docker build -t rss-reader-web .
```

容器内会带基础健康检查，适合本地部署和简单服务器场景。

### `docker-compose.yml` 模板

下面这个模板适合“直接拉取 GitHub 生成的镜像”：

```yaml
services:
  rss-reader:
    image: ghcr.io/develata/rss-reader:latest
    environment:
      RSS_READER_WEB_USERNAME: admin
      RSS_READER_WEB_PASSWORD_HASH: "$argon2id$..."
      RSS_READER_WEB_SESSION_SECRET: "replace-with-a-random-secret-at-least-32-chars"
      RSS_READER_WEB_ENV: "development"
      RSS_READER_WEB_SECURE_COOKIE: "false"
    ports:
      - "8039:8080"
    restart: unless-stopped
```

如果你希望自定义端口：

```yaml
services:
  rss-reader:
    image: ghcr.io/develata/rss-reader:latest
    environment:
      RSS_READER_WEB_USERNAME: admin
      RSS_READER_WEB_PASSWORD_HASH: "$argon2id$..."
      RSS_READER_WEB_SESSION_SECRET: "replace-with-a-random-secret-at-least-32-chars"
      RSS_READER_WEB_ENV: "production"
      RSS_READER_WEB_SECURE_COOKIE: "true"
    ports:
      - "8090:8080"
    restart: unless-stopped
```

### 什么时候用 Docker，什么时候不用

推荐使用 Docker 的场景：

- 想部署带登录门禁的 Web 版本到服务器
- 想快速拉起一个受保护的浏览器入口
- 不想安装 Rust / Dioxus CLI

不推荐使用 Docker 的场景：

- 想用桌面端离线阅读体验
- 想直接测试 Windows / Linux / macOS 原生二进制
- 想调试桌面端系统行为

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
├── rssr-web/
├── rssr-application/
├── rssr-domain/
└── rssr-infra/

assets/
docs/
migrations/
specs/
tests/
```

## 常见问题

### 1. 为什么 Web 端有些 feed 刷新失败？

因为浏览器会受 CORS 限制。
很多 feed 在桌面端能正常抓取，但在 Web 端会被目标站点阻止跨域请求。

### 2. 为什么桌面端和 Web 端的缓存体验不完全一样？

因为桌面端 / Android 对正文图片本地化更完整；Web 端会受浏览器安全策略限制。

### 3. 为什么 Release 里还有 `rssr-cli`？

CLI 主要给自动化、脚本和高级用户使用。
普通用户只下载 `rssr-app` 即可。

### 4. Windows 双击运行需要额外安装什么吗？

通常需要系统具备 Microsoft WebView2 Runtime。很多 Windows 10/11 机器已经预装。

## 开源协议

本项目使用 [MIT License](./LICENSE)。
