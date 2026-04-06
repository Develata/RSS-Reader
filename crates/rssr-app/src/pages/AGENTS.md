# 页面层模块说明

本目录负责 `rssr-app` 的页面组件与页面装配。

## 职责边界

- 页面负责：
  - 组织 UI
  - 调用 `AppServices`
  - 管理页面局部状态
  - 绑定快捷键、目录、筛选、表单交互
- 页面不负责：
  - 直接访问数据库
  - 直接发 HTTP
  - 直接解析 RSS / OPML / JSON 配置包

## 设计原则

- 保持 RSS 阅读器边界：
  - 订阅
  - 文章浏览
  - 阅读
  - 基础设置
  - 基础配置交换
- 不把页面重新做成：
  - 文档站
  - CMS
  - 标签树 / 文件夹系统
  - AI 内容平台

## 修改约束

- 共用行为优先抽到：
  - `components/`
  - `hooks/`
  - 页面子模块文件
- 不把复杂业务规则塞进页面组件闭包里
- 桌面端和移动端都要考虑：
  - 顶部导航占用
  - 阅读页连续阅读
  - 目录与筛选的触达方式
- 解释性文案保持克制，优先把说明留在 README / docs

## 特别注意

- 页面层变更默认会同步影响：
  - Web
  - Windows / macOS
  - Android
- 所以不要只按桌面端 Web 视口做判断
- 目录跳转、返回行为、键盘快捷键这类功能，修改后要考虑：
  - Web
  - desktop
  - Android

## 变更后建议检查

- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- 涉及移动端交互时：
  - `cargo check -p rssr-app --target aarch64-linux-android`

