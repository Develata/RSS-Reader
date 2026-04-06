# 建立标准化交接记录机制

- 日期：2026-04-06
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：f0b2979
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：draft

## 工作摘要

为项目建立长期可维护的 agent 交接机制：新增按日期滚动的 `docs/handoffs/` 目录、固定模板，并把“每次 agent 工作后必须补交接记录”写入根级 `AGENTS.md` 和 `specify` 宪章。

## 影响范围

- 模块：
  - `docs/handoffs/`
  - `docs/agent-handoff.md`
  - `AGENTS.md`
  - `.specify/memory/constitution.md`
  - `.specify/templates/constitution-template.md`
- 平台：
  - Windows
  - macOS
  - Linux
  - Android
  - Web
  - Docker
- 额外影响：
  - docs
  - governance
  - specify

## 关键变更

### 交接记录目录

- 新增 `docs/handoffs/README.md`，定义目录职责、命名规则、固定格式和编写要求。
- 新增 `docs/handoffs/TEMPLATE.md`，作为后续每次 agent 工作后的标准模板。
- 新增当前这份记录，作为机制落地后的首条正式交接记录。

### 治理与约束

- 根级 `AGENTS.md` 增加“Agent 交接记录要求”，明确每次可交付工作后必须补交接记录。
- `specify` 宪章将新增同样的强制性要求，使其从“建议”升级为“治理要求”。

### 文档分工

- 保留 `docs/agent-handoff.md` 作为长期稳定总览。
- 后续滚动变更统一追加到 `docs/handoffs/`，避免总览文档被开发日志淹没。

## 验证与验收

### 自动化验证

- `git diff --check`：pending

### 手工验收

- 审阅目录结构与模板格式：pending
- 审阅根级 `AGENTS.md` 和 `specify` 宪章要求是否一致：pending

## 结果

- 当前机制已设计完成，待本次修改保存并验证后即可作为后续 agent 的强制流程。
- 从这次工作开始，项目会同时具备“稳定总览”和“按日期滚动的交接日志”两层交接资料。

## 风险与后续事项

- 历史改动不会自动补全到 `docs/handoffs/`，仍需继续参考 `docs/agent-handoff.md`。
- 后续如需更强的发布纪律，可以考虑再把 release note 与 handoff 记录建立映射。

## 给下一位 Agent 的备注

- 先看 `docs/handoffs/README.md`，再决定是否需要新建记录或更新同日记录。
- 长期项目背景仍以 `docs/agent-handoff.md` 为入口。
