# 文档索引

根目录 [README.md](../README.md) 面向首次使用；这里按部署、开发和验收任务提供下一步入口。历史记录说明当时做过什么，当前行为以源码、发布说明及长期文档为准。

## 从哪里开始

如果你要直接使用，先看[根 README 的 30 秒开始阅读](../README.md#30-秒开始阅读)和[最新 Release](https://github.com/Develata/RSS-Reader/releases/latest)。英文入口是 [README.en.md](./README.en.md)。

如果你要部署或开发，按任务进入：

- 部署带登录和 `/feed-proxy` 的 Web 版：[Web / Docker 部署](./deployment/web.md)。
- 查询订阅、阅读、主题和备份细节：[使用指南](./user-guide.md)。
- 构建 Android 或了解实机验收缺口：[Android 构建与验收状态](./roadmaps/android-release-roadmap.md)。
- 修改业务或 UI：[功能设计哲学](./design/functional-design-philosophy.md)、[前端命令与界面接口清单](./design/frontend-command-reference.md)。
- 运行 CI 同等检查：[主线验证矩阵](./testing/mainline-validation-matrix.md)；准备发布 UI 验收：[测试与回归索引](./testing/README.md)。
- 提交贡献：[贡献说明](../CONTRIBUTING.md)；追溯最近实现：[Agent 交接记录](./handoffs/README.md)。

如果你已经知道自己要做什么，可以直接走下面的分流：

- 想追溯 2026-04 架构审查及当时的 application 收敛计划：
  - [架构审查报告（2026-04）](./architecture-review-2026-04.md)
  - [Application Use Case 收敛计划](./design/application-use-case-consolidation-plan.md)
- 想改主题或让 AI 生成 CSS：
  - [主题作者选择器参考](./design/theme-author-selector-reference.md)
- 想理解当前产品边界、缓存策略和样式体系：
  - [功能设计哲学](./design/functional-design-philosophy.md)
- 想确认当前前端命令面和稳定界面接口：
  - [前端命令与界面接口清单](./design/frontend-command-reference.md)
- 想理解 Headless Active Interface 的长期设计方向：
  - [Headless Active Interface 设计目标](./design/headless-active-interface.md)
- 想继续推进 application use case 收敛：
  - [Application Use Case 收敛计划](./design/application-use-case-consolidation-plan.md)
- 想准备 Android 发包或验收：
  - [Android 构建与验收状态](./roadmaps/android-release-roadmap.md)
- 想跑一轮人工验证：
  - [手工回归测试清单](./testing/manual-regression.md)
- 想提交代码或补文档：
  - [贡献说明](../CONTRIBUTING.md)

## 文档分区

### 使用与部署

- [使用指南](./user-guide.md)：日常操作与数据备份边界。
- [Web / Docker 部署](./deployment/web.md)：已发布镜像、认证状态、生产 HTTPS 与本地部署态验证。
- [Android 构建与验收状态](./roadmaps/android-release-roadmap.md)：构建命令、已发布签名包与未完成的真机路径。

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
- 前端如何保持语义化 UI 与 headless 命令面一致
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
- [Android 构建与验收状态](./roadmaps/android-release-roadmap.md)

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

- `docs/deployment/`
  - 运行、部署、认证和发布产物的长期操作说明
- `docs/design/`
  - 长期设计原则、接口边界、样式/交互约束
- `docs/roadmaps/`
  - 尚未完全并入当前稳定交付范围、但已进入规划或部分落地的平台路线
- `docs/testing/`
  - 手工验证、回归记录、测试说明
- `docs/handoffs/`
  - 按日期记录已执行工作及仍未验证的边界；不替代当前使用指南
