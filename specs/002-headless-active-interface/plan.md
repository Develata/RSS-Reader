# 实施计划：完全 Headless Active Interface 重构

**分支**：`002-headless-active-interface` | **日期**：2026-04-07 | **规格**：[spec.md](E:/gitclone/RSS-Reader/specs/002-headless-active-interface/spec.md)
**输入**：来自 `/specs/002-headless-active-interface/spec.md` 的功能规格

## 概要

本次工作将把 RSS-Reader 前端从当前的“语义化 UI + 稳定选择器”推进到“完全 headless
active interface”方向。重构目标不是改变产品边界或视觉风格，而是让前端所有非显示性动作
逐步收敛到统一的 Rust 命令语义与稳定查询接口，并把现有页面收敛为视图壳。整个过程按模
块推进：每重构完一个模块，都必须用 Chrome MCP 对重构前后的真实页面执行同路径基线验
证，并证明视觉与体验没有偏离。GUI 与 CLI 最终将共享同一命令语义，但当前阶段仍保留各
自入口程序。

## 技术上下文

**语言/版本**：Rust 稳定版（Edition 2024）  
**主要依赖**：Dioxus 0.7.3、dioxus-router 0.7.3、tokio、sqlx、reqwest、serde、tracing  
**存储**：桌面端 SQLite；Web 端浏览器本地持久化状态；配置交换使用 JSON / OPML / WebDAV  
**测试**：`cargo test --workspace`、目标模块自动化验证、Chrome MCP 模块级前后对照、必要时 `cargo check -p rssr-app --target wasm32-unknown-unknown`  
**目标平台**：Windows、macOS、Web、Android、CLI  
**项目类型**：单用户客户端应用、共享 Rust workspace  
**性能目标**：命令层抽象不得引入可感知交互延迟；列表、筛选、导航与设置保存保持当前响应级别  
**约束**：本地优先、仅配置同步、功能边界不扩张、视觉与体验默认等价、逐模块收口、每模块 Chrome MCP 验证  
**规模/范围**：前端四大动作域（订阅、阅读、设置、配置交换）与 GUI/CLI 语义统一

## 宪章检查

*门禁：必须在 Phase 0 研究前通过，并在 Phase 1 设计后重新检查。*

- `Rust 核心，Dioxus 界面`：通过。命令层、查询层和视图壳仍完全位于 Rust / Dioxus 体系内。
- `本地优先，单用户数据所有权`：通过。重构不改变本地权威数据源。
- `仅配置同步`：通过。重构不扩大配置交换边界。
- `性能是产品特性`：通过。计划要求命令层抽象不得扩大状态广播和重渲染范围。
- `分层边界，简单演进`：通过。重构按模块推进，先抽命令与查询，再调整视图壳，不引入服务端或推测性插件系统。
- `Headless 命令面，视觉等价交付`：通过。计划明确要求每个模块完成后必须执行 Chrome MCP 视觉等价验证。

## 项目结构

### 文档（本功能）

```text
specs/002-headless-active-interface/
├── spec.md
├── plan.md
└── tasks.md

docs/
├── design/
│   ├── functional-design-philosophy.md
│   ├── frontend-command-reference.md
│   └── headless-active-interface.md
└── testing/
    └── headless-refactor-equivalence.md
```

### 源代码（仓库根目录）

```text
crates/
├── rssr-app/
│   └── src/
│       ├── app.rs
│       ├── router.rs
│       ├── pages/
│       ├── components/
│       └── hooks/
├── rssr-application/
├── rssr-domain/
└── rssr-infra/
```

**结构决策**：本次重构优先以 `rssr-app` 为主要落点，逐步引入跨页面复用的命令层与查询层。
`rssr-application` 保持应用语义汇聚点，`rssr-domain` 继续承载稳定模型与规则，`rssr-infra`
不承担前端布局语义。CLI 不删除，而是逐步向统一命令面收敛。

## 实施阶段

### Phase 0：文档与治理先行

- 更新设计文档，定义 headless active interface 目标与非目标。
- 更新 `.specify` 宪章与模板，把模块级 Chrome MCP 验证写成硬门禁。
- 建立本次重构的规格、计划和任务基线。

### Phase 1：订阅模块命令面

- 先抽订阅页动作的统一命令语义与查询接口。
- 保持订阅页现有视觉结构与路由不变。
- 完成后用 Chrome MCP 对订阅页执行前后对照验证。

### Phase 2：阅读模块命令面

- 抽文章页、阅读页的动作命令与导航查询。
- 统一已读、收藏、导航和筛选语义。
- 完成后执行文章页与阅读页前后对照验证。

### Phase 3：设置与配置交换模块命令面

- 抽设置保存、主题、CSS、WebDAV、配置导入导出命令。
- 统一确认流、状态提示与失败反馈。
- 完成后执行设置页和配置交换路径验证。

### Phase 4：UI 壳与 CLI 对齐

- 抽导航壳和通用查询层。
- 逐步把 CLI 收敛到同一动作语义，而不是保留平行实现。

## 验证策略

- 每个模块开始前，先建立 Chrome MCP 基线。
- 每个模块完成后，复跑相同路径：
  - 路由进入
  - 核心按钮
  - 状态反馈
  - Console
  - 桌面视口
  - 小视口
- 若视觉或体验出现偏离，视为模块未完成。

## 复杂度追踪

| 违规项 | 为什么需要 | 更简单方案被拒绝的原因 |
|--------|------------|------------------------|
| 增加前端命令层与查询层 | 这是把 UI 从 DOM 结构绑定中解放出来的必要抽象 | 继续把业务动作留在页面闭包里，无法得到真正的 headless 命令面 |
