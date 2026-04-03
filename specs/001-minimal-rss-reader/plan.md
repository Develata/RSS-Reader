# 实施计划：极简个人 RSS 阅读器 MVP

**分支**：`001-minimal-rss-reader-followup-2` | **日期**：2026-03-26 | **规格**：[spec.md](/home/develata/gitclone/RSS-Reader/specs/001-minimal-rss-reader/spec.md)
**输入**：来自 `/specs/001-minimal-rss-reader/spec.md` 的功能规格

## 概要

构建一个面向个人使用的极简 RSS 阅读器 MVP，优先覆盖订阅添加、订阅刷新、文章浏览、
文章阅读、已读/收藏、标题搜索、本地持久化，以及订阅源和偏好设置的导入导出与远端
配置交换。实现采用单一 Rust workspace，按 UI、应用服务、领域模型、基础设施四层
划分，确保首版保持本地优先、性能敏感、边界清晰，并且不引入完整同步系统。界面基于
Dioxus 0.7.3，桌面端与 Web 端共用同一套 Rust 行为逻辑；补充设计原则记录在
`docs/design/functional-design-philosophy.md` 与
`docs/design/frontend-command-reference.md`，作为后续 CLI、样式扩展和界面接口演进的
补充约束，但不改变本 feature 的 MVP 边界。产品功能收敛在订阅、阅读、基本设置和基础
配置交换之内，不引入 AI 文本加工或其它偏离阅读器本质的能力。正文缓存边界限定为 feed
已提供的 HTML / 文本内容，不默认抓取站点原网页；阅读体验增强优先通过正文静态资源本
地化实现。设置页额外承载一组克制的主题切换能力：用户可编辑或导入自定义 CSS、导出当前
CSS，并通过预置主题按钮、主题下拉与主题卡片快速切换样式；不引入实时预览模式。整体
导航采用紧凑头部，不保留大面积品牌展示块，并在设置页提供一个轻量 GitHub 仓库入口，
方便用户回到开源项目主页。若需要 Web 登录门禁，它应被视为部署包装层而不是阅读器核心
功能；同时，任何暴露在设置页中的偏好项都应对应实际生效的运行时行为，而不是仅作持久化
占位。阅读组织能力应继续围绕最直接的两类维度演进：来源与时间；OPML / 配置包中的
`folder` 信息可继续保留用于互操作保真，但不演化为 GUI 主线能力；对于超过阈值的旧文章，
允许通过自动归档降低当前阅读面噪音，但归档不等于删除。界面风格可以借鉴 VitePress /
Vue 生态在排版、节奏和阅读层级上的优点，但不将产品演化为文档站或 CMS 壳。文章页的筛选
和目录增强也应服从这个边界：允许提供更完整的阅读状态筛选、来源多选筛选，以及“按时间时
月份 → 日期 → 来源 → 文章、按来源时来源 → 月份”的轻量目录层级，但不演化为复杂规则引擎或站
点式树状导航系统。

## 技术上下文

**语言/版本**：Rust 稳定版（Edition 2024）  
**主要依赖**：Dioxus 0.7.3、dioxus-router 0.7.3、tokio、sqlx、reqwest、feed-rs、quick-xml、serde、serde_json、thiserror、anyhow、tracing、time、url、浏览器本地持久化状态适配层
**存储**：桌面端使用本地 SQLite；Web 当前使用浏览器本地持久化状态（`localStorage` 序列化）；配置交换使用本地配置文件与 OPML/JSON 导入导出文件
**测试**：`cargo test --workspace`、仓储/解析集成测试、导入导出测试、桌面/Web 手工验证、Android target smoke check 与本地 Debug APK 构建验证  
**目标平台**：Windows、macOS、Web；移动端当前以 Android 为首个稳定落地点；同时保留 CLI 入口与 Docker Compose / GHCR Web 部署形态；Android 正式签名发布仍不作为本次 MVP 的交付门禁
**项目类型**：单用户客户端应用、共享 Rust workspace  
**性能目标**：快速启动、顺滑列表滚动、订阅增量刷新、10,000 篇文章规模下搜索和筛选保持响应  
**约束**：本地优先、仅配置同步、单用户、无服务端依赖、Web 必须具备本地持久化、UI 保持克制、避免过度异步复杂度、部署包装层不得膨胀为核心产品功能、设置项应避免无效占位
**正文缓存策略**：缓存边界保持在 feed 已提供的正文 HTML / 文本之内；默认不抓取站点原网页；正文中的静态图片资源应优先考虑本地化缓存与本地引用改写  
**规模/范围**：个人订阅规模（几十到数百个 feed）、本地文章规模约 10,000 篇、核心页面 3 个

## 宪章检查

*门禁：必须在 Phase 0 研究前通过，并在 Phase 1 设计后重新检查。*

- `Rust 核心，Dioxus 界面`：通过。所有生产代码保持为 Rust；UI 仅负责视图与交互，业务逻辑落在应用层与基础设施层。
- `本地优先，单用户数据所有权`：通过。桌面端直接使用 SQLite；Web 当前通过浏览器本地持久化状态保留同一套阅读器核心数据与偏好，远端仅交换配置文件。
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

docs/
├── README.md
├── README.en.md
├── roadmaps/
│   └── android-release-roadmap.md
├── testing/
│   └── manual-regression.md
└── design/
    ├── frontend-command-reference.md
    ├── functional-design-philosophy.md
    └── theme-author-selector-reference.md
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
│       └── import_export_service.rs
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
`rssr-infra` 实现 SQLite、HTTP、RSS/Atom 解析、
OPML 与配置交换。样式与命令系统的长期演进原则单独记录在 `docs/design/` 目录，但实现仍
以这些 crate 边界为准：`functional-design-philosophy.md` 负责说明产品范围、
交互边界和设计原则，`frontend-command-reference.md` 负责说明当前稳定命令面与公开界面
接口。正文缓存与静态资源本地化也应沿这些 crate 边界演进：资源抓取、缓存落盘和 HTML 引
用改写由 Rust 应用层与基础设施层负责，而不是在 UI 层临时处理。主题切换的公开 hook、
class 和 CSS 变量接口由 `docs/design/theme-author-selector-reference.md` 约束。导航头保
持工具化与低干扰风格，仓库入口作为设置页中的次级动作暴露，而不是在主阅读区域占据版
面。Android 当前已补齐 Dioxus 移动端入口、`Dioxus.toml` 配置和 Debug APK 构建链路，
用作移动端首个稳定落地点；正式签名发布能力仍单独记录在
`docs/roadmaps/android-release-roadmap.md`，避免和当前 MVP 的桌面/Web 交付范围混淆。

## 复杂度追踪

无宪章违规项，无需额外说明。
