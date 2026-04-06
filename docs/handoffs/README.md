# Agent 交接记录

这个目录保存**按日期持续追加**的工程交接记录，用来承接每次 agent 工作后的真实上下文。

它和 [agent-handoff.md](/home/develata/gitclone/RSS-Reader/docs/agent-handoff.md) 的分工不同：

- [agent-handoff.md](/home/develata/gitclone/RSS-Reader/docs/agent-handoff.md)
  - 长期稳定的项目总览
  - 适合新 agent 快速建立整体认知
- `docs/handoffs/`
  - 按日期和版本滚动的交接日志
  - 适合了解“最近到底改了什么、验到了哪里、还有什么风险”

## 命名规则

- 默认文件名：`YYYY-MM-DD-<slug>.md`
- 如果一次工作跨多个提交但属于同一批交付，可以合并到同一份记录
- 如果当天已有同主题记录，可以更新原文件，但必须补齐新的验证与结果

示例：

- `2026-04-06-android-window-resume.md`
- `2026-04-06-release-workflow-smoke-checks.md`

## 固定格式

所有记录都 MUST 基于 [TEMPLATE.md](/home/develata/gitclone/RSS-Reader/docs/handoffs/TEMPLATE.md) 编写，并至少包含以下部分：

1. 元数据
   - 日期
   - 作者 / Agent
   - 分支
   - 当前 HEAD
   - 相关 commit / tag / release
   - 状态
2. 工作摘要
3. 影响范围
   - 模块
   - 平台
   - workflow / 文档
4. 关键变更
5. 验证与验收
   - 具体命令 / 手工路径
   - 验收结果
6. 风险与后续事项
7. 给下一位 agent 的备注

## 编写要求

- 记录必须是**事实型**的，不写空泛总结。
- 验证项必须写明：
  - 跑了什么
  - 结果怎样
  - 没跑的也要写清楚原因
- 如果工作尚未提交，必须明确写出 `commit: pending`。
- 如果修复依赖新 tag / 新 release 才会生效，也必须写清楚。
- 涉及跨端行为时，优先写清：
  - desktop
  - Android
  - Web
  - Docker / `rssr-web`

## 历史交接说明

在本目录建立之前的历史上下文，继续保留在：

- [agent-handoff.md](/home/develata/gitclone/RSS-Reader/docs/agent-handoff.md)

后续新工作统一落到 `docs/handoffs/`。
