# Agent 架构护栏补充

- 日期：2026-04-15
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：0f32c52
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

为后续 agent 的设计与计划阶段补充显式架构护栏，要求在出现严重代码分叉、污染 infra 边界、前后端大规模迁移或违背设计哲学时，先做保守分析并立即向交互人员提出风险。

## 影响范围

- 模块：
  - `crates/rssr-application/src/composition.rs`
  - `AGENTS.md`
- 平台：
  - Windows
  - macOS
  - Linux
  - Android
  - Web
  - Docker
- 额外影响：
  - `docs/handoffs/`

## 关键变更

### 代码侧护栏

- 在 application 组合入口 `crates/rssr-application/src/composition.rs` 增加核心注释。
- 注释明确要求：当设计/计划开始把结构性代价推进到 application 组合层时，必须先停下来做保守分析并向交互人员提示风险。

### Agent 指令补充

- 在根 `AGENTS.md` 的手工补充区新增“Agent 架构护栏”。
- 将需要显式拦截的四类风险固定下来：
  - 代码严重分叉
  - infra 架构污染
  - 前后端大规模迁移或职责重分配
  - 明显违背设计哲学
- 明确 agent 对上述方案默认持保守甚至负面倾向，并要求先说明统一语义、差异边界、迁移性质与更小替代路径。

## 验证与验收

### 自动化验证

- `git status --short`：通过，修改前工作区无待确认输出。
- 静态代码复核：通过，补充仅涉及注释与仓库级 agent 指令，没有改动运行时逻辑。

### 手工验收

- 检查 `AGENTS.md` 手工补充区是否包含新护栏：通过
- 检查 `crates/rssr-application/src/composition.rs` 是否包含核心架构提醒注释：通过

## 结果

- 本次交付已满足“补充架构护栏提醒”的目标，可继续集成。
- 变更不会影响现有编译和运行时行为，主要用于约束后续 agent 的设计与计划决策。

## 风险与后续事项

- 当前护栏是仓库级规则，不会自动替代具体设计评审；后续仍需在高风险架构讨论中逐案展开技术论证。
- 如果未来新增更明确的 server-backed Web 主线或存储分层路线，可在设计文档中再补一份更细的架构决策记录。

## 给下一位 Agent 的备注

- 涉及跨平台数据层、server-backed Web、纯静态 Web、native/Android 分层时，先读根 `AGENTS.md` 的“Agent 架构护栏”。
- 如需判断某个方案是否越界，优先对照 `docs/design/functional-design-philosophy.md` 与 `crates/rssr-application/src/composition.rs` 的组合边界注释。
