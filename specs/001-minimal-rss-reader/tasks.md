---

description: "极简个人 RSS 阅读器 MVP 的实现任务列表"
---

# 任务列表：极简个人 RSS 阅读器 MVP

**输入**：来自 `/specs/001-minimal-rss-reader/` 的设计文档  
**前置条件**：`plan.md`、`spec.md`、`research.md`、`data-model.md`、`contracts/`、`quickstart.md`

**测试**：本功能涉及解析、持久化、Web 本地存储、导入导出和状态流转，因此包含自动化
测试；涉及 UI 行为，因此包含桌面/Web/Android 的手工验证任务。

**组织方式**：任务按用户故事分组，确保每个故事都可以独立实现、独立验证和独立交付。

## 格式：`[ID] [P?] [Story] Description`

- **[P]**：可并行执行（不同文件、无未完成依赖）
- **[Story]**：任务所属用户故事（US1、US2、US3）
- 每个任务描述都包含精确文件路径

## 路径约定

- **应用层 UI**：`crates/rssr-app/src/`
- **应用服务层**：`crates/rssr-application/src/`
- **领域层**：`crates/rssr-domain/src/`
- **基础设施层**：`crates/rssr-infra/src/`
- **测试**：`tests/` 与各 crate 本地测试

## Phase 1：初始化（共享基础设施）

**目的**：建立 Rust workspace、crate 结构和基础开发工具链。

- [ ] T001 创建 workspace 根配置 `Cargo.toml`
- [ ] T002 创建应用 crate 配置 `crates/rssr-app/Cargo.toml`
- [ ] T003 [P] 创建应用服务 crate 配置 `crates/rssr-application/Cargo.toml`
- [ ] T004 [P] 创建领域 crate 配置 `crates/rssr-domain/Cargo.toml`
- [ ] T005 [P] 创建基础设施 crate 配置 `crates/rssr-infra/Cargo.toml`
- [ ] T006 创建应用入口与基础目录结构 `crates/rssr-app/src/main.rs`
- [ ] T007 [P] 创建共享格式化与检查配置 `.editorconfig`
- [ ] T008 [P] 创建 Rust 工具链与 lint 配置 `rustfmt.toml`

---

## Phase 2：基础能力（阻塞性前置条件）

**目的**：建立所有用户故事共享的核心模型、存储、抓取、解析和配置交换基础。

**⚠️ 关键**：本阶段完成前，不得开始任何用户故事实现。

- [ ] T009 创建领域模型导出入口 `crates/rssr-domain/src/lib.rs`
- [ ] T010 [P] 定义订阅源模型 `crates/rssr-domain/src/feed.rs`
- [ ] T011 [P] 定义文章与状态模型 `crates/rssr-domain/src/entry.rs`
- [ ] T012 [P] 定义用户偏好与配置包模型 `crates/rssr-domain/src/settings.rs`
- [ ] T013 定义仓储 trait 与应用边界 `crates/rssr-domain/src/repository.rs`
- [ ] T014 创建基础设施导出入口 `crates/rssr-infra/src/lib.rs`
- [ ] T015 [P] 创建 SQLite schema 与迁移 `migrations/0001_initial.sql`
- [ ] T016 [P] 实现数据库模块骨架 `crates/rssr-infra/src/db/mod.rs`
- [ ] T017 [P] 实现存储后端抽象 `crates/rssr-infra/src/db/storage_backend.rs`
- [ ] T018 [P] 实现原生 SQLite 持久化后端 `crates/rssr-infra/src/db/sqlite_native.rs`
- [ ] T019 [P] 实现 Web wasm SQLite + IndexedDB 持久化后端 `crates/rssr-infra/src/db/sqlite_web.rs`
- [ ] T020 [P] 实现订阅抓取客户端骨架 `crates/rssr-infra/src/fetch/mod.rs`
- [ ] T021 [P] 实现 RSS/Atom 解析模块骨架 `crates/rssr-infra/src/parser/mod.rs`
- [ ] T022 [P] 实现 OPML 模块骨架 `crates/rssr-infra/src/opml/mod.rs`
- [ ] T023 [P] 实现配置交换模块骨架 `crates/rssr-infra/src/config_sync/mod.rs`
- [ ] T024 创建应用服务导出入口 `crates/rssr-application/src/lib.rs`
- [ ] T025 [P] 创建订阅应用服务骨架 `crates/rssr-application/src/feed_service.rs`
- [ ] T026 [P] 创建文章应用服务骨架 `crates/rssr-application/src/entry_service.rs`
- [ ] T027 [P] 创建设置应用服务骨架 `crates/rssr-application/src/settings_service.rs`
- [ ] T028 [P] 创建导入导出应用服务骨架 `crates/rssr-application/src/import_export_service.rs`
- [ ] T029 配置日志与错误处理基础设施 `crates/rssr-app/src/main.rs`
- [ ] T030 [P] 添加数据库迁移与仓储验证测试 `tests/integration/test_sqlite_bootstrap.rs`
- [ ] T031 [P] 添加解析与去重基础验证测试 `tests/integration/test_feed_parse_dedup.rs`
- [ ] T032 [P] 添加 Web 本地持久化验证测试 `tests/integration/test_web_sqlite_persistence.rs`

**检查点**：基础能力已具备，用户故事实现可以开始。

---

## Phase 3：用户故事 1 - 高效阅读我的订阅（优先级：P1）🎯 MVP

**目标**：用户能够添加订阅、刷新 feed、浏览文章列表并进入阅读页。

**独立测试**：添加一个有效 feed URL，刷新本地库，打开文章列表并阅读正文，不依赖除抓取
feed 之外的远端服务。

### 用户故事 1 的验证

- [ ] T033 [P] [US1] 添加订阅抓取与文章入库集成测试 `tests/integration/test_feed_refresh_flow.rs`
- [ ] T034 [P] [US1] 添加阅读列表与阅读页手工验证说明 `specs/001-minimal-rss-reader/quickstart.md`

### 用户故事 1 的实现

- [ ] T035 [P] [US1] 实现订阅源 SQLite 仓储 `crates/rssr-infra/src/db/feed_repository.rs`
- [ ] T036 [P] [US1] 实现文章 SQLite 仓储与去重更新逻辑 `crates/rssr-infra/src/db/entry_repository.rs`
- [ ] T037 [P] [US1] 实现条件请求抓取逻辑 `crates/rssr-infra/src/fetch/client.rs`
- [ ] T038 [P] [US1] 实现 RSS/Atom 解析与字段归一化 `crates/rssr-infra/src/parser/feed_parser.rs`
- [ ] T039 [US1] 实现添加订阅与刷新用例 `crates/rssr-application/src/feed_service.rs`
- [ ] T040 [US1] 实现文章列表与阅读用例 `crates/rssr-application/src/entry_service.rs`
- [ ] T041 [P] [US1] 实现应用路由与页面骨架 `crates/rssr-app/src/router.rs`
- [ ] T042 [P] [US1] 实现订阅侧栏页面 `crates/rssr-app/src/pages/feeds_page.rs`
- [ ] T043 [P] [US1] 实现文章列表页面 `crates/rssr-app/src/pages/entries_page.rs`
- [ ] T044 [P] [US1] 实现阅读页 `crates/rssr-app/src/pages/reader_page.rs`
- [ ] T045 [US1] 接线应用初始化与原生/Web 数据库启动流程 `crates/rssr-app/src/app.rs`
- [ ] T046 [US1] 添加刷新错误反馈与空状态处理 `crates/rssr-app/src/components/status_banner.rs`
- [ ] T047 [US1] 确认订阅刷新和文章列表性能满足计划目标 `tests/manual/us1-performance-checklist.md`

**检查点**：此时用户故事 1 应完整可用，并可独立演示为 MVP。

---

## Phase 4：用户故事 2 - 管理阅读进度（优先级：P2）

**目标**：用户能够标记已读/未读、收藏/取消收藏，并按状态筛选和按标题搜索。

**独立测试**：基于已有本地文章库切换多篇文章的已读和收藏状态，并验证状态持久化、筛选和
标题搜索可用。

### 用户故事 2 的验证

- [ ] T048 [P] [US2] 添加已读收藏状态与标题搜索集成测试 `tests/integration/test_entry_state_and_search.rs`
- [ ] T049 [P] [US2] 添加桌面快捷键与筛选交互手工验证说明 `tests/manual/us2-interaction-checklist.md`

### 用户故事 2 的实现

- [ ] T050 [P] [US2] 扩展文章仓储以支持状态更新和标题搜索 `crates/rssr-infra/src/db/entry_repository.rs`
- [ ] T051 [US2] 实现已读、收藏、筛选和标题搜索用例 `crates/rssr-application/src/entry_service.rs`
- [ ] T052 [P] [US2] 实现文章列表筛选与搜索组件 `crates/rssr-app/src/components/entry_filters.rs`
- [ ] T053 [P] [US2] 在文章列表中接入已读/收藏交互 `crates/rssr-app/src/pages/entries_page.rs`
- [ ] T054 [P] [US2] 在阅读页中接入已读/收藏交互 `crates/rssr-app/src/pages/reader_page.rs`
- [ ] T055 [US2] 实现桌面快捷键支持 `crates/rssr-app/src/hooks/use_reader_shortcuts.rs`
- [ ] T056 [US2] 确认状态切换、筛选和搜索在 10,000 篇文章规模下保持响应 `tests/manual/us2-performance-checklist.md`

**检查点**：此时用户故事 1 和 2 都应可独立工作。

---

## Phase 5：用户故事 3 - 携带我的订阅源和偏好设置（优先级：P3）

**目标**：用户能够导入导出 OPML、导入导出配置包，并通过远端位置上传下载配置。

**独立测试**：导出订阅源与偏好设置后，在另一个环境导入或从远端拉取，恢复配置但不恢复
文章库与阅读状态。

### 用户故事 3 的验证

- [ ] T057 [P] [US3] 添加配置包导入导出测试 `tests/integration/test_config_package_io.rs`
- [ ] T058 [P] [US3] 添加 OPML 互操作测试 `tests/integration/test_opml_interop.rs`
- [ ] T059 [P] [US3] 添加配置交换手工验证说明 `tests/manual/us3-config-exchange-checklist.md`

### 用户故事 3 的实现

- [ ] T060 [P] [US3] 实现配置包序列化与语义校验 `crates/rssr-infra/src/config_sync/file_format.rs`
- [ ] T061 [P] [US3] 实现 WebDAV 配置上传下载客户端 `crates/rssr-infra/src/config_sync/webdav.rs`
- [ ] T062 [P] [US3] 实现 OPML 导入导出逻辑 `crates/rssr-infra/src/opml/mod.rs`
- [ ] T063 [US3] 实现配置导入导出与远端交换用例 `crates/rssr-application/src/import_export_service.rs`
- [ ] T064 [US3] 实现用户偏好设置读写用例 `crates/rssr-application/src/settings_service.rs`
- [ ] T065 [P] [US3] 实现设置页面 `crates/rssr-app/src/pages/settings_page.rs`
- [ ] T066 [P] [US3] 接入 OPML 与配置包导入导出入口 `crates/rssr-app/src/pages/feeds_page.rs`
- [ ] T067 [P] [US3] 接入 WebDAV 配置交换入口 `crates/rssr-app/src/pages/settings_page.rs`
- [ ] T068 [US3] 确认远端配置交换只包含订阅源和偏好设置 `tests/manual/us3-boundary-checklist.md`

**检查点**：此时所有用户故事都应可独立工作。

---

## Phase 6：打磨与横切关注点

**目的**：补齐文档、体验验证和发布前整理。

- [ ] T069 [P] 更新快速开始与使用说明 `specs/001-minimal-rss-reader/quickstart.md`
- [ ] T070 清理跨 crate 公共接口与无用抽象 `crates/rssr-application/src/lib.rs`
- [ ] T071 [P] 补充高风险回归测试 `tests/integration/test_regression_smoke.rs`
- [ ] T072 [P] 验证配置包 schema 与实现保持一致 `specs/001-minimal-rss-reader/contracts/config-package.schema.json`
- [ ] T073 运行并记录完整手工验收结果 `tests/manual/final-acceptance-checklist.md`

---

## 依赖与执行顺序

### 阶段依赖

- **初始化（Phase 1）**：无依赖，可立即开始。
- **基础能力（Phase 2）**：依赖初始化完成，并阻塞所有用户故事。
- **用户故事 1（Phase 3）**：依赖基础能力完成，是 MVP 的最小交付。
- **用户故事 2（Phase 4）**：依赖基础能力和用户故事 1 的列表/阅读基础设施。
- **用户故事 3（Phase 5）**：依赖基础能力，以及用户偏好设置和订阅源持久化能力。
- **打磨阶段（Phase 6）**：依赖所有目标用户故事完成。

### 用户故事依赖

- **用户故事 1（P1）**：无其他故事依赖，是 MVP 核心。
- **用户故事 2（P2）**：依赖用户故事 1 已提供文章列表和阅读视图，但应保持独立可验证。
- **用户故事 3（P3）**：依赖基础订阅源和偏好设置能力，但不依赖已读/收藏功能本身。

### 每个用户故事内部顺序

- 先补自动化验证任务，再补仓储或基础设施。
- 再实现应用服务用例。
- 最后实现 UI 接线与手工验证。
- 每个故事在进入下一优先级前必须达到可独立演示状态。

### 并行机会

- Phase 1 中的 crate 配置和工具配置可并行。
- Phase 2 中的领域模型、原生 SQLite、Web 持久化、抓取、解析、OPML 和配置交换骨架可并行。
- 用户故事 1 中的订阅页、文章列表页和阅读页页面实现可并行。
- 用户故事 2 中的筛选组件、快捷键和阅读页状态操作可并行。
- 用户故事 3 中的 OPML、配置包和 WebDAV 模块可并行。

---

## 并行示例：用户故事 1

```bash
# 一起启动用户故事 1 的基础实现：
Task: "T035 [US1] 实现订阅源 SQLite 仓储 `crates/rssr-infra/src/db/feed_repository.rs`"
Task: "T037 [US1] 实现条件请求抓取逻辑 `crates/rssr-infra/src/fetch/client.rs`"
Task: "T038 [US1] 实现 RSS/Atom 解析与字段归一化 `crates/rssr-infra/src/parser/feed_parser.rs`"

# 一起启动用户故事 1 的 UI 页面：
Task: "T042 [US1] 实现订阅侧栏页面 `crates/rssr-app/src/pages/feeds_page.rs`"
Task: "T043 [US1] 实现文章列表页面 `crates/rssr-app/src/pages/entries_page.rs`"
Task: "T044 [US1] 实现阅读页 `crates/rssr-app/src/pages/reader_page.rs`"
```

---

## 实施策略

### 先做 MVP（仅用户故事 1）

1. 完成 Phase 1：初始化 workspace 和 crate。
2. 完成 Phase 2：建好领域模型、存储、抓取、解析和应用服务骨架。
3. 完成 Phase 3：实现订阅添加、刷新、文章列表和阅读页。
4. 停下来执行 `T033`、`T034`、`T047`，验证 MVP 已可独立交付。

### 增量交付

1. 先交付用户故事 1，形成最小可用阅读器。
2. 在不破坏列表和阅读体验的前提下补入用户故事 2。
3. 最后补入用户故事 3，提供配置迁移与配置交换能力。

### 并行团队策略

1. 开发者 A：Phase 1、Phase 2 的 workspace、领域模型和数据库。
2. 开发者 B：Phase 2 的抓取、解析、OPML、配置交换基础设施。
3. 开发者 C：从用户故事 1 开始承担 Dioxus 页面与交互实现。

---

## 备注

- 共 73 个任务，全部符合 `- [ ] Txxx ...` 格式。
- 用户故事任务均带 `[US1]`、`[US2]`、`[US3]` 标签。
- 所有实现任务均包含明确文件路径，可直接执行。
- 推荐 MVP 范围为 Phase 1 到 Phase 3。
