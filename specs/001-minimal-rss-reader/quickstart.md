# 快速开始：极简个人 RSS 阅读器

操作入口更新于 2026-10-23（v0.2 阶段 D）；历史 spec/plan 的版本上下文保留。更完整的当前说明见 [使用指南](../../docs/user-guide.md)。

## 1. 准备环境

- 安装 Rust 稳定版工具链
- Dioxus CLI 0.7.9；原生桌面需对应 WebView 系统库，Android 工具链见 [构建说明](../../docs/roadmaps/android-release-roadmap.md)
- 原生使用单库 `library.sqlite3`；正式 Web 经同源 API 使用服务器资料库；Pages 演示使用共享 SQL 的内存 SQLite，每次重载从预抓取 RSS 重建，BrowserStore 已退役；不要求单独安装 SQLite 命令行工具
- 如需验证 Web 目标，安装 `wasm32-unknown-unknown` target

## 2. 初始化与校验

1. 在仓库根目录执行格式化与测试：
   - `cargo fmt --all --check`
   - `cargo test --workspace --locked`
2. 如需验证 Web 构建，执行：
   - `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`
3. 如需验证 Android 目标与移动端接线，执行：
   - `cargo check -p rssr-app --target aarch64-linux-android --locked`

## 3. 启动应用

- Desktop:
  - `cargo run -p rssr-app --locked`
- Web:
  - `bash scripts/run_rssr_web_browser_smoke.sh`（正式 Web）；仅演示用 `dx serve --platform web --package rssr-app --features pages-demo --locked`
- CLI:
  - `cargo run -p rssr-cli --locked -- --help`
- Web 部署态：按 [Web / Docker 部署指南](../../docs/deployment/web.md) 构建并配置独立凭据、公开地址（`RSS_READER_WEB_PUBLIC_ORIGIN`）与数据目录；本地验收可使用项目 smoke 脚本生成的临时配置。

## 4. 核心使用流程

1. 在订阅页添加 feed，并使用“刷新此订阅”或“刷新全部”抓取文章。
2. 原生应用启动后会按设置自动刷新；正式 Web 的自动刷新由服务器负责，浏览器不启动周期刷新；Pages 演示禁用刷新。
3. 在文章页按“时间”或“来源”组织文章，并按需切换“包含归档”。
4. 打开文章进入阅读页，验证阅读字号、上下篇导航、已读/收藏和正文排版。
5. 在设置页切换主题、导入或编辑自定义 CSS，并保存阅读偏好。

## 5. 验证核心路径

1. 添加一个有效 feed URL。
2. 刷新订阅并写入本地文章。
3. 在文章页验证：
   - 按时间分组
   - 按来源分组
   - 标题搜索
   - 已读 / 未读筛选
   - 自动归档默认 3 个月生效
4. 进入阅读页，切换已读/收藏状态并重启应用验证持久化。
5. 在阅读页确认feed 提供的 HTML 正文优先展示（不额外抓取原站全文），且危险脚本不会被直接渲染。
6. 修改设置中的 `list_density`、`startup_view`、`reader_font_scale`，确认都真实影响界面和启动路径。
7. 正式 Web 先运行 `bash scripts/run_server_web_regression.sh --suite feed-smoke`；真实远端 feed 另外记录服务器 DNS、出站策略与站点响应，不能用本地夹具代替。

## 6. 验证配置交换

1. 在订阅页导出配置包 JSON 与 OPML。
2. 在设置页填写 WebDAV endpoint 和 remote path。
3. 上传当前配置包。
4. 使用隔离的新数据库 / 浏览器 profile，不清空日常数据。
5. 下载并导入配置包。
6. 确认订阅源和偏好设置恢复，但文章库和状态不被同步。
7. 如果配置包中带有 `folder` / OPML 分组，确认它们被保留用于互操作保真，但 GUI 阅读组织仍只围绕来源与时间。

## 7. 手工体验检查

- 桌面端快捷键是否可用
- 列表滚动是否顺滑
- 阅读页排版是否清晰
- 浅色/深色/跟随系统主题切换是否正常
- Android 触控导航是否自然
- Web 由服务器认证；会话失效后提示重新登录，设置草稿恢复后仍需手动保存
- 旧 Web 升级只交换配置，不迁移文章/已读/收藏；正式 Web 需要服务器在线，不再离线可读
