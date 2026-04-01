# 快速开始：极简个人 RSS 阅读器

## 1. 准备环境

- 安装 Rust 稳定版工具链
- 准备支持桌面/Web/Android 的 Dioxus 开发环境
- 确保本地可用 SQLite
- 如需验证 Web 目标，安装 `wasm32-unknown-unknown` target

## 2. 初始化与校验

1. 在仓库根目录执行格式化与测试：
   - `cargo fmt --all`
   - `cargo test --workspace`
2. 如需验证 Web 构建，执行：
   - `cargo check -p rssr-app --target wasm32-unknown-unknown`
3. 如需验证 Android 目标与移动端接线，执行：
   - `cargo check -p rssr-app --target aarch64-linux-android`

## 3. 启动应用

- Desktop:
  - `cargo run -p rssr-app`
- Web:
  - `dx serve --platform web --package rssr-app`
- CLI:
  - `cargo run -p rssr-cli -- --help`

## 4. 核心使用流程

1. 在订阅页添加 feed，并使用“刷新此订阅”或“刷新全部”抓取文章。
2. 应用启动后会按设置中的刷新间隔自动后台刷新。
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
5. 在阅读页确认完整 HTML 正文优先展示，且危险脚本不会被直接渲染。
6. 修改设置中的 `list_density`、`startup_view`、`reader_font_scale`，确认都真实影响界面和启动路径。

## 6. 验证配置交换

1. 在订阅页导出配置包 JSON 与 OPML。
2. 在设置页填写 WebDAV endpoint 和 remote path。
3. 上传当前配置包。
4. 清空本地订阅和设置或使用新环境。
5. 下载并导入配置包。
6. 确认订阅源和偏好设置恢复，但文章库和状态不被同步。
7. 如果配置包中带有 `folder` / OPML 分组，确认它们被保留用于互操作保真，但 GUI 阅读组织仍只围绕来源与时间。

## 7. 手工体验检查

- 桌面端快捷键是否可用
- 列表滚动是否顺滑
- 阅读页排版是否清晰
- 浅色/深色/跟随系统主题切换是否正常
- Android 触控导航是否自然
- Web 部署下的登录门禁是否只作为入口保护层存在，而不侵入阅读主流程
