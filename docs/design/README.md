# 设计文档

这个目录收纳 RSS Reader 的长期设计原则、公开样式接口与主题编写约定。

## 当前文档

- [前端命令与样式设计哲学](./frontend-command-and-styling-philosophy.md)
- [主题作者选择器参考](./theme-author-selector-reference.md)

## 先读哪一份

如果你在做这些事，可以直接按下面选：

- 想理解界面为什么这样分层、Rust 和 CSS 的边界怎么划：
  - [前端命令与样式设计哲学](./frontend-command-and-styling-philosophy.md)
- 想手写主题，或把一份文档直接丢给 AI 生成 CSS：
  - [主题作者选择器参考](./theme-author-selector-reference.md)

## 两份文档分别解决什么问题

### 前端命令与样式设计哲学

关注：

- 行为由 Rust 控制、样式由 CSS 控制
- CLI / UI 共用应用服务
- 稳定样式接口如何定义
- 正文缓存与图片本地化的边界

适合：

- 改交互边界
- 改主题系统
- 判断某类改动应该落在 UI、应用服务还是基础设施层

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
