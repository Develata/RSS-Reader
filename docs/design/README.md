# 设计文档

这个目录收纳 RSS-Reader 的长期设计原则、公开命令与界面接口，以及主题编写约定。

## 当前文档

- [功能设计哲学](./functional-design-philosophy.md)
- [Headless Active Interface 设计目标](./headless-active-interface.md)
- [前端命令与界面接口清单](./frontend-command-reference.md)
- [主题作者选择器参考](./theme-author-selector-reference.md)

## 先读哪一份

如果你在做这些事，可以直接按下面选：

- 想理解界面为什么这样分层、Rust 和 CSS 的边界怎么划：
  - [功能设计哲学](./functional-design-philosophy.md)
- 想确认目前有哪些前端命令、页面接口和 `data-action` 可以长期依赖：
  - [前端命令与界面接口清单](./frontend-command-reference.md)
- 想理解为什么这次要往完全 headless 的活动接口转变，以及迁移门禁：
  - [Headless Active Interface 设计目标](./headless-active-interface.md)
- 想手写主题，或把一份文档直接丢给 AI 生成 CSS：
  - [主题作者选择器参考](./theme-author-selector-reference.md)

## 两份文档分别解决什么问题

### 功能设计哲学

关注：

- 产品功能边界为什么要收敛在订阅、阅读、基本设置和基础配置交换
- 行为由 Rust 控制、样式由 CSS 控制
- GUI / CLI / Docker Compose 形态如何保持同一产品边界
- 正文缓存与图片本地化的边界

适合：

- 改交互边界
- 改主题系统
- 判断某类改动应该落在 UI、应用服务还是基础设施层

### 前端命令与界面接口清单

关注：

- 当前有哪些前端命令应长期保持稳定
- 哪些 `data-page` / `data-nav` / `data-action` 可公开依赖
- 哪些组件 class 和状态接口适合主题和 AI 使用

适合：

- 对齐 UI 命令面
- 检查某次改动有没有越过产品功能边界
- 给 AI 或主题作者提供稳定接口约束

### Headless Active Interface 设计目标

关注：

- 当前前端为什么还不是完全 headless
- 命令层、查询层和视图壳应该如何分离
- 为什么这会支持极端 CSS 重排
- 每完成一个模块后如何用 Chrome MCP 做视觉与体验等价验收

适合：

- 启动这次前端架构重构
- 判断某次 UI 重构是否越过视觉等价边界
- 设计 GUI / CLI 共用的统一命令面

### 主题作者选择器参考

关注：

- 可以长期依赖的 `data-page` / `data-action` / `data-nav`
- 稳定组件 class
- 可覆写变量
- AI 生成主题的约束与提示模板

适合：

- 写新主题
- 让 AI 生成 CSS
- 检查某份主题是否过度依赖内部 DOM 层级
