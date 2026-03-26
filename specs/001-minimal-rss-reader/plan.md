# 实施计划：极简个人 RSS 阅读器 MVP

**分支**：`001-minimal-rss-reader-followup-2` | **日期**：2026-03-26 | **规格**：[spec.md](/home/develata/gitclone/RSS-Reader/specs/001-minimal-rss-reader/spec.md)
**输入**：来自 `/specs/001-minimal-rss-reader/spec.md` 的功能规格

## 概要

构建一个面向个人使用的极简 RSS 阅读器 MVP，优先覆盖订阅添加、订阅刷新、文章浏览、
文章阅读、已读/收藏、标题搜索、本地持久化，以及订阅源和偏好设置的导入导出与远端
配置交换。实现采用单一 Rust workspace，按 UI、应用服务、领域模型、基础设施四层
划分，确保首版保持本地优先、性能敏感、边界清晰，并且不引入完整同步系统。界面基于
Dioxus 0.7.3，桌面端与 Web 端共用同一套 Rust 行为逻辑；补充设计原则记录在
`design/frontend-command-and-styling-philosophy.md`，作为后续 CLI 与样式扩展的
补充约束，但不改变本 feature 的 MVP 边界。

## 技术上下文

**语言/版本**：Rust 稳定版（Edition 2024）  
**主要依赖**：Dioxus 0.7.3、dioxus-router 0.7.3、tokio、sqlx、reqwest、feed-rs、quick-xml、serde、serde_json、thiserror、anyhow、tracing、time、url、wasm SQLite 持久化适配层  
**存储**：桌面端使用本地 SQLite；Web 使用 wasm SQLite，并将数据库文件持久化到 IndexedDB；配置交换使用本地配置文件与 OPML/JSON 导入导出文件  
**测试**：`cargo test --workspace`、仓储/解析集成测试、导入导出测试、桌面/Web 手工验证  
**目标平台**：Windows、macOS、Web；Android 保留为架构目标，但不作为本次 MVP 的交付门禁  
**项目类型**：单用户客户端应用、共享 Rust workspace  
**性能目标**：快速启动、顺滑列表滚动、订阅增量刷新、10,000 篇文章规模下搜索和筛选保持响应  
**约束**：本地优先、仅配置同步、单用户、无服务端依赖、Web 必须具备本地持久化、UI 保持克制、避免过度异步复杂度  
**规模/范围**：个人订阅规模（几十到数百个 feed）、本地文章规模约 10,000 篇、核心页面 4 个

## 宪章检查

*门禁：必须在 Phase 0 研究前通过，并在 Phase 1 设计后重新检查。*

- `Rust 核心，Dioxus 界面`：通过。所有生产代码保持为 Rust；UI 仅负责视图与交互，业务逻辑落在应用层与基础设施层。
- `本地优先，单用户数据所有权`：通过。桌面端直接使用 SQLite；Web 通过 wasm SQLite + IndexedDB 持久化保持同一份 SQLite schema 作为事实来源，远端仅交换配置文件。
- `仅配置同步`：通过。计划明确排除文章、已读、收藏和搜索索引同步。
- `性能是产品特性`：通过。计划采用条件请求、局部状态更新、基于索引的查询与克制渲染。
- `分层边界，简单演进`：通过。采用四层 workspace，不引入服务端、插件系统或推测性同步架构。

## 项目结构

### 文档（本功能）

```text
specs/001-minimal-rss-reader/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── config-package.schema.json
│   └── opml-interop.md
└── tasks.md

design/
└── frontend-command-and-styling-philosophy.md
```

### 源代码（仓库根目录）

```text
crates/
├── rssr-app/
│   └── src/
│       ├── app.rs
│       ├── main.rs
│       ├── router.rs
│       ├── pages.rs
│       ├── components.rs
│       ├── hooks.rs
│       └── theme.rs
├── rssr-application/
│   └── src/
│       ├── lib.rs
│       ├── feed_service.rs
│       ├── entry_service.rs
│       ├── settings_service.rs
│       ├── import_export_service.rs
│       └── dto.rs
├── rssr-domain/
│   └── src/
│       ├── lib.rs
│       ├── feed.rs
│       ├── entry.rs
│       ├── settings.rs
│       └── repository.rs
└── rssr-infra/
    └── src/
        ├── lib.rs
        ├── db.rs
        ├── fetch.rs
        ├── parser.rs
        ├── opml.rs
        └── config_sync.rs

assets/
migrations/
tests/
```

**结构决策**：采用 4 个 crate 的单一 workspace。`rssr-app` 负责 Dioxus UI；
`rssr-application` 编排用例与状态；`rssr-domain` 定义核心模型与 trait；
`rssr-infra` 实现 SQLite、Web 端 wasm SQLite 持久化、HTTP、RSS/Atom 解析、
OPML 与配置交换。样式与命令系统的长期演进原则单独记录在 `design/` 目录，但实现仍
以这些 crate 边界为准。

## 复杂度追踪

无宪章违规项，无需额外说明。
