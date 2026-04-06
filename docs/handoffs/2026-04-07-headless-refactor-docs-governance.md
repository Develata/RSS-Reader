# Headless 重构文档与治理落地

- 日期：2026-04-07
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：b49b6ad
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

为“完全 headless active interface”重构建立正式文档和 specify 治理基础：补齐设计文档、
模块级 Chrome MCP 视觉等价验收规则、`.specify` 宪章与模板同步，以及本次重构的独立规格
与计划文档。

## 影响范围

- 模块：
  - `docs/design/`
  - `docs/testing/`
  - `docs/README.md`
  - `.specify/memory/constitution.md`
  - `.specify/templates/`
  - `specs/002-headless-active-interface/`
- 平台：
  - Windows
  - macOS
  - Linux
  - Web
  - Android
  - CLI
- 额外影响：
  - docs
  - governance
  - specify

## 关键变更

### 设计文档

- 新增 `docs/design/headless-active-interface.md`，定义命令层、查询层、视图壳、迁移目标与门禁。
- 更新 `functional-design-philosophy.md` 与 `frontend-command-reference.md`，把 headless 命令面演进写入长期设计。

### 测试门禁

- 新增 `docs/testing/headless-refactor-equivalence.md`，将“每完成一个模块就做 Chrome MCP 前后对照验证”定义为强制验收流程。

### specify 治理

- `.specify/memory/constitution.md` 升级到 `1.2.0`，新增“Headless 命令面，视觉等价交付”原则。
- 同步更新 `spec-template.md`、`plan-template.md`、`tasks-template.md` 和 `constitution-template.md`。

### 本次重构规格

- 新增 `specs/002-headless-active-interface/spec.md`
- 新增 `specs/002-headless-active-interface/plan.md`
- 新增 `specs/002-headless-active-interface/tasks.md`

## 验证与验收

### 自动化验证

- 文档与模板一致性人工复核：通过
- 代码构建 / 运行验证：未执行（本次为文档与治理交付）

### 手工验收

- 设计文档、testing 文档与 `.specify` 宪章的约束一致性检查：通过
- 新 spec / plan / tasks 与本次重构目标对齐检查：通过

## 结果

- 当前已具备按规范推进 headless 重构的治理基础。
- 后续任何模块实现都应以这份 spec 和新的 Chrome MCP 门禁为准。

## 风险与后续事项

- 当前只是建立治理和计划，尚未开始真正的命令层代码重构。
- 工作区内仍存在上一轮未提交的代码变更，需要与本轮文档变更一起谨慎整理。

## 给下一位 Agent 的备注

- 先读 `docs/design/headless-active-interface.md` 与 `docs/testing/headless-refactor-equivalence.md`。
- 开始任何模块实现前，先对照 `specs/002-headless-active-interface/` 建立模块基线和 Chrome MCP 对照路径。
