# Agent 交接记录

先读[当前维护状态](../current-status.md)，再按下面的主题查最近实现。handoff 是指定日期、源码与环境的执行记录，不能代替长期契约，也不能把旧的通过结果当作新版本验收。

## 当前入口

| 主题 | 优先阅读 |
| --- | --- |
| PR #40 主线整合 | [逐字段保存、原生适配与验收](./2026-10-29-pr40-main-integration.md)、[最新源码原生复验](./2026-10-29-pr40-native-retry.md)、[CI 等待边界](./2026-10-29-pr40-ci-wait-boundaries.md) |
| 主题预览与设计细节 | [设计审查改进与验收](./2026-10-29-design-refinements.md)、[原始审查依据](./2026-10-29-design-audit.md) |
| 订阅与设置顶部层级 | [导航与独立主面板统一](./2026-10-29-secondary-page-surfaces.md) |
| 2026-10-29 | [目录入口、固定标题与 Penpot 同步](./2026-10-29-directory-toolbar-sticky-header.md) |
| 正文性能与后台图片调度 | [PR 与 CI 验收](./2026-10-29-parse-image-pr-ci.md)、[实现及证据](./2026-10-28-parse-image-performance.md)、[长期预算契约](../design/reader-processing-budgets.md) |
| 文档维护与归档 | [2026-10-28 整理](./2026-10-28-docs-maintenance.md)、[历史索引](./archive/README.md) |
| 历史待办核实与 PR 收尾 | [本轮核实](./2026-10-28-historical-followup-audit.md)、[当前待办](../maintenance-backlog.md) |
| 最新发布与验收 | [v0.2.1 正式发布](./2026-10-28-v0.2.1-release.md)、[此前原生、设计与升级检查](./2026-10-28-release-readiness.md) |
| 主线合并与交叉复核 | [PR #33 合并](./2026-10-27-workspace-merge.md)、[工作区复测](./2026-10-27-workspace-recheck.md) |
| 会话职责与生命周期 | [职责拆分](./2026-10-27-workspace-cohesion.md)、[生命周期修复](./2026-10-27-workspace-review-fixes.md) |
| 目录交互与视觉 | [右侧箭头与缩进](./2026-10-27-directory-right-disclosure.md)、[窗口边界](./2026-10-27-directory-window-boundaries.md) |
| Penpot 覆盖 | [当前设计基准](../design/penpot-code-baseline.md)、[覆盖完成](./2026-10-26-penpot-coverage-completion.md) |
| 原生设备证据 | [Windows / Android 目录](./2026-10-26-native-directory-validation.md)、[Android 返回与持久目录](./2026-10-25-android-back-and-persistent-storage.md) |
| 已发布版本 | [v0.2.1](./2026-10-28-v0.2.1-release.md)、[v0.2.0](./2026-10-23-v0.2.0-release.md) |

## 近期记录

这里保留 v0.2.0 发布及之后的记录。发布前 147 份交接见[按月份整理的历史索引](./archive/README.md)；更早的整体背景见[2026-04 总览](../agent-handoff.md)。

| 日期 | 记录 |
| --- | --- |
| 2026-10-29 | [PR #40 CI 等待边界与诊断](./2026-10-29-pr40-ci-wait-boundaries.md) |
| 2026-10-29 | [PR #40 最新源码 Windows 原生验收重试](./2026-10-29-pr40-native-retry.md) |
| 2026-10-29 | [解析与图片队列 PR 创建及 CI 验收](./2026-10-29-parse-image-pr-ci.md) |
| 2026-10-28 | [正文解析性能与后台图片队列](./2026-10-28-parse-image-performance.md) |
| 2026-10-28 | [v0.2.1 正式发布与产物核验](./2026-10-28-v0.2.1-release.md) |
| 2026-10-28 | [PR 收尾与历史待办核实](./2026-10-28-historical-followup-audit.md) |
| 2026-10-28 | [原生交互、Penpot 抽查与补丁发布准备](./2026-10-28-release-readiness.md) |
| 2026-10-28 | [文档维护入口、状态校正与历史交接归档](./2026-10-28-docs-maintenance.md) |
| 2026-10-27 | [阅读工作区复查：切页期间的偏好基线与文章写入](./2026-10-27-workspace-review-fixes.md) |
| 2026-10-27 | [工作区复测：恢复读取、离页写入与目录截图修复](./2026-10-27-workspace-recheck.md) |
| 2026-10-27 | [阅读工作区修复：Claude 复核与 PR #33 合并](./2026-10-27-workspace-merge.md) |
| 2026-10-27 | [文章工作区会话的高内聚、低耦合重构](./2026-10-27-workspace-cohesion.md) |
| 2026-10-27 | [Windows Web release 的 wasm-opt 异常修复与完整构建验收](./2026-10-27-wasm-opt-release-build.md) |
| 2026-10-27 | [自然目录回归与阅读工作区的主线接入](./2026-10-27-pr-integration.md) |
| 2026-10-27 | [原生界面前置整理：下一步优先级评估](./2026-10-27-native-preparation-next-step.md) |
| 2026-10-27 | [Entries 初始化与查询生命周期规则提取](./2026-10-27-entries-request-lifecycle.md) |
| 2026-10-27 | [目录窗口化的筛选、分页与返回边界](./2026-10-27-directory-window-boundaries.md) |
| 2026-10-27 | [目录箭头移至右侧与文章缩进收紧](./2026-10-27-directory-right-disclosure.md) |
| 2026-10-27 | [目录与 Web release 推送、CI 回归修复及验证](./2026-10-27-directory-push-ci.md) |
| 2026-10-27 | [目录展开箭头与条目间距修正](./2026-10-27-directory-disclosure-size.md) |
| 2026-10-27 | [目录模块内聚性与 agent 可读性整理](./2026-10-27-directory-cohesion-readability.md) |
| 2026-10-26 | [默认纯色背景与暗色登录文字修正](./2026-10-26-solid-background.md) |
| 2026-10-26 | [Penpot 阅读、设置与搜索继续对齐](./2026-10-26-penpot-reader-settings-alignment.md) |
| 2026-10-26 | [Penpot 登录、异常、内容与主题继续对齐](./2026-10-26-penpot-coverage-completion.md) |
| 2026-10-26 | [Penpot 按当前代码同步与分类整理](./2026-10-26-penpot-code-alignment.md) |
| 2026-10-26 | [原生界面前置整理：目录控制器与阅读快捷键](./2026-10-26-native-ui-preparation.md) |
| 2026-10-26 | [目录可靠性、时间文章层级与最新原生验收](./2026-10-26-native-directory-validation.md) |
| 2026-10-26 | [历史 SQL 换行兼容与目录按需挂载](./2026-10-26-migration-directory-rendering.md) |
| 2026-10-26 | [文章工具栏对齐与目录间距微调](./2026-10-26-entries-toolbar-alignment.md) |
| 2026-10-26 | [大目录文章行窗口化](./2026-10-26-directory-windowing.md) |
| 2026-10-26 | [目录窗口化交互复查](./2026-10-26-directory-windowing-review.md) |
| 2026-10-26 | [紧凑顶部导航与文章工作区间距](./2026-10-26-compact-app-navigation.md) |
| 2026-10-25 | [来源目录文章层级与来源筛选 Rust 草稿](./2026-10-25-source-directory-native-prep.md) |
| 2026-10-25 | [阅读分支独立 CI 与原生宿主性能基线](./2026-10-25-reading-native-baseline.md) |
| 2026-10-25 | [界面改版后的持久化保留检查](./2026-10-25-persistence-retention-check.md) |
| 2026-10-25 | [原生界面前置工作：阅读位置与下拉刷新模块](./2026-10-25-native-interaction-modules.md) |
| 2026-10-25 | [Android 连续阅读返回修复](./2026-10-25-android-repeated-back.md) |
| 2026-10-25 | [Android 安装自动化与实机验收](./2026-10-25-android-device-access-and-qa.md) |
| 2026-10-25 | [Android 系统返回与持久数据目录修复](./2026-10-25-android-back-and-persistent-storage.md) |
| 2026-10-24 | [阅读工作区第三轮审查：来源身份与写入互斥](./2026-10-24-reading-workspace-source-write-review.md) |
| 2026-10-24 | [阅读工作区实现审查与修复](./2026-10-24-reading-workspace-review-fixes.md) |
| 2026-10-24 | [阅读工作区第二轮审查：查询生命周期与导入原名](./2026-10-24-reading-workspace-query-import-review.md) |
| 2026-10-24 | [阅读工作区第四轮审查：偏好保存顺序与来源弹窗高度](./2026-10-24-reading-workspace-preference-review.md) |
| 2026-10-24 | [阅读工作区模板落地、显式多选与订阅编辑](./2026-10-24-reading-workspace-implementation.md) |
| 2026-10-24 | [阅读工作区第五轮审查：输入法 Esc 与分组文字对齐](./2026-10-24-reading-workspace-ime-alignment-review.md) |
| 2026-10-24 | [原生化准备复核：下拉刷新释放与阅读加载状态](./2026-10-24-pull-refresh-release-review.md) |
| 2026-10-24 | [原生 Rust 前置模块化：盘点与待评审方案](./2026-10-24-native-preparation-assessment.md) |
| 2026-10-23 | [v0.2.0 发布](./2026-10-23-v0.2.0-release.md) |
| 2026-10-23 | [独立复审、历史整合与公网 Pages 验收](./2026-10-23-integration-merge-pages.md) |

## 写入规则

- 文件名使用 `YYYY-MM-DD-<slug>.md`；同日同主题可以更新已有记录，补充结果时标记更新日期。
- 使用 [TEMPLATE](./TEMPLATE.md)，至少包含元数据、摘要、影响范围、关键变更、验证、风险与后续、下一位 Agent 备注。
- 元数据记录分支、代码 HEAD、相关提交 / tag / release 和状态。未提交时明确写 `commit: pending`；文档提交不冒充产品源码提交。
- 验证逐项写命令、结果、实际平台和未覆盖范围。自动化、模拟器、真机、视觉截图和性能证据分开记录。
- 变更当前契约时同步长期文档；合并或发布结果更新[维护状态](../current-status.md)。
- 发布里程碑之后整理历史记录时，先将仍有效的约束归入长期文档、将未解决问题列入当前状态，再归档并修复仓库内链接。原始证据不因“过时”而删除。
- 归档后的旧命令、路径、状态属于其记录日期，不能直接作为新任务的实施步骤。
