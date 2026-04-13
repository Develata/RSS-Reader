# 文档索引

这里是 RSS-Reader 的细化文档入口。
根目录 [README.md](../README.md) 面向仓库首页与快速使用，这里则收纳长期设计、平台路线图和测试说明。

## 从哪里开始

如果你是第一次进入仓库，建议按这个顺序阅读：

1. [根 README](../README.md)
2. [英文 README](./README.en.md)
3. [功能设计哲学](./design/functional-design-philosophy.md)
4. [贡献说明](../CONTRIBUTING.md)

如果你已经知道自己要做什么，可以直接走下面的分流：

- 想看 2026-04 架构审查结论和当前 application 收敛进度：
  - [架构审查报告（2026-04）](./architecture-review-2026-04.md)
  - [Application Use Case 收敛计划](./design/application-use-case-consolidation-plan.md)
- 想改主题或让 AI 生成 CSS：
  - [主题作者选择器参考](./design/theme-author-selector-reference.md)
- 想理解当前产品边界、缓存策略和样式体系：
  - [功能设计哲学](./design/functional-design-philosophy.md)
- 想确认当前前端命令面和稳定界面接口：
  - [前端命令与界面接口清单](./design/frontend-command-reference.md)
- 想理解即将进行的完全 headless active interface 重构：
  - [Headless Active Interface 设计目标](./design/headless-active-interface.md)
- 想继续推进 application use case 收敛：
  - [Application Use Case 收敛计划](./design/application-use-case-consolidation-plan.md)
- 想准备 Android 发包或验收：
  - [Android 安装包落地清单](./roadmaps/android-release-roadmap.md)
- 想跑一轮人工验证：
  - [手工回归测试清单](./testing/manual-regression.md)
- 想提交代码或补文档：
  - [贡献说明](../CONTRIBUTING.md)

## 文档分区

### 设计文档

- [设计文档索引](./design/README.md)
- [功能设计哲学](./design/functional-design-philosophy.md)
- [Headless Active Interface 设计目标](./design/headless-active-interface.md)
- [前端命令与界面接口清单](./design/frontend-command-reference.md)
- [主题作者选择器参考](./design/theme-author-selector-reference.md)
- [Application Use Case 收敛计划](./design/application-use-case-consolidation-plan.md)

这组文档主要回答：

- 行为与样式边界怎么划分
- 产品功能边界为什么只围绕订阅、阅读、基本设置和基础配置交换
- 当前前端命令面和界面接口有哪些稳定约束
- 前端如何从语义化 UI 演进到完全 headless 的命令面
- 哪些 selector / hook 可以长期依赖
- 怎样在不碰 Rust 逻辑的前提下自定义主题
- 怎样把这套接口直接交给 AI 生成 CSS

### 架构审查与收敛

- [架构审查报告（2026-04）](./architecture-review-2026-04.md)
- [Application Use Case 收敛计划](./design/application-use-case-consolidation-plan.md)
- [Agent 交接记录](./handoffs/README.md)

这组文档主要回答：

- 2026-04 这轮架构审查最初指出了哪些边界失真
- 哪些 application façade 已经删除，哪些 use case 明确保留
- 当前 application naming baseline 和后续收敛顺序是什么
- 最近一轮 agent 工作已经做到哪里，还有哪些边界仍在收敛中

### 路线图

- [路线图索引](./roadmaps/README.md)
- [Android 安装包落地清单](./roadmaps/android-release-roadmap.md)

这组文档主要回答：

- 哪些平台能力已经落地
- 哪些还在持续推进
- 下一步发布链和验收重点在哪里

### 测试与回归

- [测试与回归索引](./testing/README.md)
- [手工回归测试清单](./testing/manual-regression.md)
- [Headless 重构视觉等价验收](./testing/headless-refactor-equivalence.md)

这组文档主要回答：

- Web / desktop 应该如何做手工回归
- 回归结果怎么记录
- 模块级 headless 重构如何做 Chrome MCP 视觉等价验收
- 当前哪些交互最值得重点观察

## 文档组织约定

- `docs/design/`
  - 长期设计原则、接口边界、样式/交互约束
- `docs/roadmaps/`
  - 尚未完全并入当前稳定交付范围、但已进入规划或部分落地的平台路线
- `docs/testing/`
  - 手工验证、回归记录、测试说明
