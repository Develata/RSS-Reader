# Agent 交接记录

先读[当前维护状态](../current-status.md)，再按下面的主题查最近实现。handoff 是指定日期、源码与环境的执行记录，不能代替长期契约，也不能把旧的通过结果当作新版本验收。

## 当前入口

| 主题 | 优先阅读 |
| --- | --- |
| PR #53 合并与 main 核验 | [合并身份、主线 CI 与工作区保全](./2026-10-31-reader-race-merge.md) |
| Reader 并发问题测试与修复 | [旧图片消息、标记写入乱序的复现及三端回归](./2026-10-30-reader-race-fixes.md) |
| Reader 三批整合与正文渲染边界 | [PR #50–#52 合并、主线核验与兼容契约](./2026-10-30-reader-integration.md) |
| Reader 快捷键模块归属 | [共享规则、页面私有装配与依赖方向复核](./2026-10-30-reader-shortcut-cohesion.md) |
| Reader 快捷键定义与同步输入边界 | [单一 Rust 定义、输入过滤、捕获顺序及三端回归](./2026-10-30-reader-shortcut-definition.md) |
| Reader 宿主入口与生命周期 | [图片、dialog、快捷键桥接收敛及三端实际回归](./2026-10-30-reader-host-boundary.md) |
| Reader 规则独立性与 PR #49 整合 | [无 Dioxus 编译、读取归属及宿主回归](./2026-10-30-reader-rule-boundary.md) |
| Android 主题文件导入与 PR #48 整合 | [实现和实际系统选择器验收](./2026-10-30-android-theme-import.md)、[首 BOM 呈现问题的独立复现与修复](./2026-10-30-css-leading-bom-render.md) |
| 列表密度退役、0.3.x 参数删除与 `.agents/` 清理 | [实现、兼容与实际宿主验收](./2026-10-30-density-retirement.md) |
| 当前阶段、分支与文档收口 | [全部分支审计与清理](./2026-10-30-branch-consolidation.md)、[前一轮文档整合](./2026-10-29-consolidation.md)、[当前待办](../maintenance-backlog.md) |
| 当前原生控件评估与 PR #47 整合 | [主题选择实际宿主评估](./2026-10-30-native-theme-selector-assessment.md)、[App 设置读取生命周期与 PR #46](./2026-10-30-shell-settings-lifecycle.md) |
| 原生返回策略 | [Windows / Android 实际宿主验收](./2026-10-30-native-back-validation.md)、[返回导航与退出提示提取](./2026-10-29-native-back-policy.md)、[E 盘接续与背景交接](./2026-10-29-pr40-worktree-handoff.md)、[本批范围评估](./2026-10-29-native-preparation-next-batch.md) |
| PR #40 主线整合 | [main CI 与 Docker 核验](./2026-10-29-pr40-main-ci-docker.md)、[最终审查与合并回执](./2026-10-29-pr40-final-review-merge.md)、[逐字段保存、原生适配与验收](./2026-10-29-pr40-main-integration.md)、[最新主线与原生复验](./2026-10-29-pr40-directory-order-integration.md)、[CI 等待边界](./2026-10-29-pr40-ci-wait-boundaries.md) |
| 目录滚动突跳与顺序模型 | [分页与分组一致性修正](./2026-10-29-directory-reading-order.md)、[Windows 原生复测](./2026-10-29-directory-native-verification.md) |
| 主题预览与设计细节 | [设计审查改进与验收](./2026-10-29-design-refinements.md)、[原始审查依据](./2026-10-29-design-audit.md) |
| 订阅与设置顶部层级 | [导航与独立主面板统一](./2026-10-29-secondary-page-surfaces.md) |
| 2026-10-29 | [目录入口、固定标题与 Penpot 同步](./2026-10-29-directory-toolbar-sticky-header.md) |
| 正文性能与后台图片调度 | [PR 与 CI 验收](./2026-10-29-parse-image-pr-ci.md)、[实现及证据](./2026-10-28-parse-image-performance.md)、[长期预算契约](../design/reader-processing-budgets.md) |
| 文档维护与归档 | [2026-10-30 分支回执补收](./2026-10-30-branch-consolidation.md)、[2026-10-29 整合](./2026-10-29-consolidation.md)、[历史索引](./archive/README.md) |
| 历史待办核实与 PR 收尾 | [本轮核实](./2026-10-28-historical-followup-audit.md)、[当前待办](../maintenance-backlog.md) |
| 最新发布与验收 | [v0.2.1 正式发布](./2026-10-28-v0.2.1-release.md)、[此前原生、设计与升级检查](./2026-10-28-release-readiness.md) |
| 已发布版本 | [v0.2.1](./2026-10-28-v0.2.1-release.md)、[v0.2.0](./2026-10-23-v0.2.0-release.md) |

## 近期记录

这里优先展示当前批次、最终验收与发布记录。10 月 23–28 日的 51 份过程记录已按确认清单归档，与更早的 147 份一起由[历史索引](./archive/README.md)检索。更早的整体背景见[2026-04 总览](../agent-handoff.md)。

| 日期 | 记录 |
| --- | --- |
| 2026-10-31 | [PR #53 合并、main CI 与交接更新](./2026-10-31-reader-race-merge.md) |
| 2026-10-30 | [Reader 并发问题复现、修复与三端验收](./2026-10-30-reader-race-fixes.md) |
| 2026-10-30 | [Reader 三批整合与正文、资源、锚点边界](./2026-10-30-reader-integration.md) |
| 2026-10-30 | [Reader 快捷键模块归属与依赖方向整理](./2026-10-30-reader-shortcut-cohesion.md) |
| 2026-10-30 | [Reader 快捷键定义去重与同步宿主回归](./2026-10-30-reader-shortcut-definition.md) |
| 2026-10-30 | [Reader 宿主入口、卸载清理与基线/候选验收](./2026-10-30-reader-host-boundary.md) |
| 2026-10-30 | [Reader 规则与装配边界、PR #49 整合](./2026-10-30-reader-rule-boundary.md) |
| 2026-10-30 | [Android 主题文件导入与 PR #48 整合](./2026-10-30-android-theme-import.md)、[首 BOM 呈现修复](./2026-10-30-css-leading-bom-render.md) |
| 2026-10-30 | [列表密度退役与旧 Agent skill 清理](./2026-10-30-density-retirement.md) |
| 2026-10-30 | [PR #47 整合与主题模式选择宿主评估](./2026-10-30-native-theme-selector-assessment.md) |
| 2026-10-30 | [PR #46 整合与 App 设置读取生命周期](./2026-10-30-shell-settings-lifecycle.md) |
| 2026-10-30 | [返回策略复核与 Windows / Android 宿主验收](./2026-10-30-native-back-validation.md) |
| 2026-10-30 | [全部分支审计、历史资料保全与旧工作树退役](./2026-10-30-branch-consolidation.md) |
| 2026-10-29 | [当前阶段、分支核对与文档整合](./2026-10-29-consolidation.md) |
| 2026-10-29 | [返回导航与退出提示策略提取](./2026-10-29-native-back-policy.md) |
| 2026-10-29 | [PR #40 收尾与 E 盘工作树接续](./2026-10-29-pr40-worktree-handoff.md) |
| 2026-10-29 | [PR #40 后的原生前置模块化建议](./2026-10-29-native-preparation-next-batch.md) |
| 2026-10-29 | [PR #40 合并后 main CI 与 Docker 核验](./2026-10-29-pr40-main-ci-docker.md) |
| 2026-10-29 | [PR #40 最终审查与合并回执](./2026-10-29-pr40-final-review-merge.md) |
| 2026-10-29 | [PR #40 接入目录顺序主线与原生复验](./2026-10-29-pr40-directory-order-integration.md) |
| 2026-10-29 | [PR #40 CI 等待边界与诊断](./2026-10-29-pr40-ci-wait-boundaries.md) |
| 2026-10-29 | [PR #40 最新源码 Windows 原生验收重试](./2026-10-29-pr40-native-retry.md) |
| 2026-10-29 | [目录突跳修复的 Windows 原生复测](./2026-10-29-directory-native-verification.md) |
| 2026-10-29 | [解析与图片队列 PR 创建及 CI 验收](./2026-10-29-parse-image-pr-ci.md) |
| 2026-10-28 | [正文解析性能与后台图片队列](./2026-10-28-parse-image-performance.md) |
| 2026-10-28 | [v0.2.1 正式发布与产物核验](./2026-10-28-v0.2.1-release.md) |
| 2026-10-28 | [PR 收尾与历史待办核实](./2026-10-28-historical-followup-audit.md) |
| 2026-10-28 | [原生交互、Penpot 抽查与补丁发布准备](./2026-10-28-release-readiness.md) |
| 2026-10-28 | [文档维护入口、状态校正与历史交接归档](./2026-10-28-docs-maintenance.md) |
| 2026-10-23 | [v0.2.0 发布](./2026-10-23-v0.2.0-release.md) |

## 补收的历史分支记录

- [当时已授权的分支清理](./2026-10-29-branch-cleanup.md)和[当时的剩余分支审计](./2026-10-29-remaining-branch-audit.md)：从旧性能分支补收，保留原始数量、SHA 与证据；当前数量和处置以本轮整合清单为准。

## 写入规则

- 文件名使用 `YYYY-MM-DD-<slug>.md`；同日同主题可以更新已有记录，补充结果时标记更新日期。
- 使用 [TEMPLATE](./TEMPLATE.md)，至少包含元数据、摘要、影响范围、关键变更、验证、风险与后续、下一位 Agent 备注。
- 元数据记录分支、代码 HEAD、相关提交 / tag / release 和状态。未提交时明确写 `commit: pending`；文档提交不冒充产品源码提交。
- 验证逐项写命令、结果、实际平台和未覆盖范围。自动化、模拟器、真机、视觉截图和性能证据分开记录。
- 变更当前契约时同步长期文档；合并或发布结果更新[维护状态](../current-status.md)。
- 发布里程碑之后整理历史记录时，先将仍有效的约束归入长期文档、将未解决问题列入当前状态，再归档并修复仓库内链接。原始证据不因“过时”而删除。
- 归档后的旧命令、路径、状态属于其记录日期，不能直接作为新任务的实施步骤。
