# 文档索引

根目录 [README](../README.md) 面向首次使用；这里提供维护、设计和验收入口。核对日期：2026-10-29。当前 main、已发布 v0.2.1 和尚未合入的返回策略是三个不同基线，统一在[维护状态](./current-status.md)列明。

正式 Web 使用服务器上的 SQLite 资料库；Pages 使用每次加载重建的内存 SQLite 和预抓取 RSS 快照。当前状态与验证边界见[维护状态](./current-status.md)，带日期的记录只证明其指定版本和场景。

## 按任务进入

| 要做的事 | 先读 | 需要时继续读 |
| --- | --- | --- |
| 使用、升级、备份 | [使用指南](./user-guide.md) | [英文入口](./README.en.md)、[原生旧库升级](./testing/native-upgrade-verification.md) |
| 部署服务器 / Docker | [Web 部署](./deployment/web.md) | [服务包与容器验收](./testing/web-distribution.md) |
| 修改业务或界面 | [功能边界](./design/functional-design-philosophy.md) | [设计索引](./design/README.md)、[前端命令契约](./design/frontend-command-reference.md) |
| 修改主题或同步 Penpot | [主题选择器](./design/theme-author-selector-reference.md) | [Penpot 代码基准](./design/penpot-code-baseline.md) |
| 排查正文解析与图片缓存 | [处理预算与职责](./design/reader-processing-budgets.md) | [解析与队列实测](./testing/evidence/2026-10-28-parse-image/README.md) |
| 检查主线 / 准备发布 | [测试索引](./testing/README.md) | [主线矩阵](./testing/mainline-validation-matrix.md)、[发布 UI 清单](./testing/release-ui-regression-checklist.md)、[v0.2.1 说明](./releases/v0.2.1.md) |
| 构建或验收 Android | [Android 状态](./roadmaps/android-release-roadmap.md) | [原生阅读验收](./testing/native-reading-baseline.md) |
| 接手当前工作 | [维护状态](./current-status.md) | [当前待办](./maintenance-backlog.md)、[近期交接](./handoffs/README.md) |
| 整合分支与整理旧文档 | [本轮整合与分支清单](./handoffs/2026-10-29-consolidation.md) | [历史交接索引](./handoffs/archive/README.md) |
| 追溯旧决定或旧实现 | [历史交接索引](./handoffs/archive/README.md) | [2026-04 总览](./agent-handoff.md)、[2026-10 审计](./engineering-audit-2026-10.md) |

## 文档职责

- `design/`：现行行为、架构和公开接口；历史实施计划在索引中单列，不作为待办列表。
- `deployment/`、`user-guide.md`：可实际执行的使用、升级与部署说明。
- `testing/`：持续维护的验证入口、覆盖矩阵和环境边界；`evidence/`、`baselines/` 保存指定版本的证据。
- `roadmaps/`：尚未完成的平台交付和验收范围。
- `handoffs/`：近期执行记录；历史记录按月份放入 `handoffs/archive/`。归档不表示其中所有风险已消失，当前待办不从旧记录的“下一步”自动生成。
- `reviews/` 和带日期的审计：当时的发现，是否仍成立须对照当前代码复核。

## 维护规则

1. 改变用户行为、命令、存储或平台边界时，同步修改对应长期文档；仅写 handoff 不算完成文档维护。
2. 当前状态只在[维护状态](./current-status.md)汇总，经源码核实的后续工作维护在[当前待办](./maintenance-backlog.md)。必须区分主线提交、发布 tag、构建成功和实际交互验收。
3. 每次交付按[交接模板](./handoffs/TEMPLATE.md)记录命令、源码身份、结果、未覆盖项及提交状态；不要把旧的“通过”复制成新版本结论。
4. 归档保留原始事实和证据，修复仓库内引用；已退役入口链接到明确的历史版本，不能改指到语义不同的新命令。
5. 不存在或无法回读的证据明确标记“未找到 / 未核对”，不补造结果。外部证据路径、Penpot 版本和下载文件都要注明验证边界。
6. 修改文档后检查相对链接与 `git diff --check`。文档审计不代替产品测试；发布前仍按覆盖矩阵补足本次变更涉及的场景。

贡献流程见 [CONTRIBUTING](../CONTRIBUTING.md)，仓库约束见 [AGENTS](../AGENTS.md)。
