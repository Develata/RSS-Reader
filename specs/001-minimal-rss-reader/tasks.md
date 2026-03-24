---

description: "极简个人 RSS 阅读器 MVP 的剩余实现任务列表"

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
- **测试**：各 crate `tests/` 与 `tests/manual/`

## Phase 1：初始化（共享基础设施）

**目的**：确认当前分支已具备 workspace、crate 结构、格式化和基础测试能力。

当前分支已完成初始化工作，本阶段无剩余任务。

---

## Phase 2：基础能力（阻塞性前置条件）

**目的**：补齐所有用户故事共享的核心基础设施，使后续功能都建立在真实仓储、抓取和解析能力之上。

**⚠️ 关键**：本阶段完成前，不得开始任何新的用户故事 UI 接线。

- [ ] T001 实现订阅源 SQLite 仓储 `crates/rssr-infra/src/db/feed_repository.rs`
- [ ] T002 [P] 实现文章 SQLite 仓储与去重更新逻辑 `crates/rssr-infra/src/db/entry_repository.rs`
- [ ] T003 [P] 实现用户偏好 SQLite 仓储 `crates/rssr-infra/src/db/settings_repository.rs`
- [ ] T004 [P] 在数据库模块导出仓储实现 `crates/rssr-infra/src/db/mod.rs`
- [ ] T005 [P] 实现条件请求抓取客户端与请求头处理 `crates/rssr-infra/src/fetch/client.rs`
- [ ] T006 [P] 在抓取模块导出客户端与抓取结果模型 `crates/rssr-infra/src/fetch/mod.rs`
- [ ] T007 [P] 实现 RSS/Atom 解析与字段归一化 `crates/rssr-infra/src/parser/feed_parser.rs`
- [ ] T008 [P] 在解析模块导出解析入口与归一化结果 `crates/rssr-infra/src/parser/mod.rs`
- [ ] T009 [P] 定义应用装配与共享运行时上下文 `crates/rssr-app/src/bootstrap.rs`
- [ ] T010 添加订阅刷新流程集成测试 `crates/rssr-infra/tests/test_feed_refresh_flow.rs`
- [ ] T011 [P] 添加解析与去重回归测试 `crates/rssr-infra/tests/test_feed_parse_dedup.rs`
- [ ] T012 [P] 添加设置仓储持久化测试 `crates/rssr-infra/tests/test_settings_repository.rs`

**检查点**：本地仓储、抓取、解析和应用装配已经具备，用户故事实现可开始。

---

## Phase 3：用户故事 1 - 高效阅读我的订阅（优先级：P1）🎯 MVP

**目标**：用户能够添加订阅、刷新 feed、浏览文章列表并进入阅读页。

**独立测试**：添加一个有效 feed URL，刷新本地库，打开文章列表并阅读正文，不依赖除抓取
feed 之外的远端服务。

### 用户故事 1 的验证

- [ ] T013 [P] [US1] 添加订阅、刷新和文章入库端到端测试 `crates/rssr-infra/tests/test_feed_refresh_flow.rs`
- [ ] T014 [P] [US1] 补充列表与阅读主流程手工验证说明 `tests/manual/us1-reading-checklist.md`

### 用户故事 1 的实现

- [ ] T015 [US1] 在订阅服务中实现添加订阅与刷新用例 `crates/rssr-application/src/feed_service.rs`
- [ ] T016 [US1] 在文章服务中实现文章列表与阅读详情查询 `crates/rssr-application/src/entry_service.rs`
- [ ] T017 [P] [US1] 定义列表与阅读页 DTO `crates/rssr-application/src/dto.rs`
- [ ] T018 [P] [US1] 扩展路由以支持订阅页、文章列表页和阅读页 `crates/rssr-app/src/router.rs`
- [ ] T019 [P] [US1] 实现订阅栏页面 `crates/rssr-app/src/pages/feeds_page.rs`
- [ ] T020 [P] [US1] 实现文章列表页面 `crates/rssr-app/src/pages/entries_page.rs`
- [ ] T021 [P] [US1] 实现阅读页 `crates/rssr-app/src/pages/reader_page.rs`
- [ ] T022 [P] [US1] 实现空状态与错误提示组件 `crates/rssr-app/src/components/status_banner.rs`
- [ ] T023 [US1] 接线应用启动、原生/Web 后端选择与初始加载流程 `crates/rssr-app/src/app.rs`
- [ ] T024 [US1] 更新页面导出入口以纳入 MVP 页面结构 `crates/rssr-app/src/pages/mod.rs`
- [ ] T025 [US1] 记录 MVP 刷新与阅读性能检查结果 `tests/manual/us1-performance-checklist.md`

**检查点**：此时用户故事 1 应完整可用，并可独立演示为 MVP。

---

## Phase 4：用户故事 2 - 管理阅读进度（优先级：P2）

**目标**：用户能够标记已读/未读、收藏/取消收藏，并按状态筛选和按标题搜索。

**独立测试**：基于已有本地文章库切换多篇文章的已读和收藏状态，并验证状态持久化、筛选和
标题搜索可用。

### 用户故事 2 的验证

- [ ] T026 [P] [US2] 添加已读、收藏与标题搜索集成测试 `crates/rssr-infra/tests/test_entry_state_and_search.rs`
- [ ] T027 [P] [US2] 添加桌面快捷键与筛选交互手工验证说明 `tests/manual/us2-interaction-checklist.md`

### 用户故事 2 的实现

- [ ] T028 [US2] 扩展文章仓储以支持状态更新、筛选和标题搜索 `crates/rssr-infra/src/db/entry_repository.rs`
- [ ] T029 [US2] 在文章服务中实现已读、收藏、筛选和搜索用例 `crates/rssr-application/src/entry_service.rs`
- [ ] T030 [P] [US2] 实现文章筛选与搜索组件 `crates/rssr-app/src/components/entry_filters.rs`
- [ ] T031 [P] [US2] 在文章列表页接入已读/收藏和筛选搜索交互 `crates/rssr-app/src/pages/entries_page.rs`
- [ ] T032 [P] [US2] 在阅读页接入已读/收藏切换交互 `crates/rssr-app/src/pages/reader_page.rs`
- [ ] T033 [US2] 实现桌面快捷键 hook `crates/rssr-app/src/hooks/use_reader_shortcuts.rs`
- [ ] T034 [US2] 记录 10,000 篇文章规模下的状态切换与搜索性能结果 `tests/manual/us2-performance-checklist.md`

**检查点**：此时用户故事 1 和 2 都应可独立工作。

---

## Phase 5：用户故事 3 - 携带我的订阅源和偏好设置（优先级：P3）

**目标**：用户能够导入导出 OPML、导入导出配置包，并通过远端位置上传下载配置。

**独立测试**：导出订阅源与偏好设置后，在另一个环境导入或从远端拉取，恢复配置但不恢复
文章库与阅读状态。

### 用户故事 3 的验证

- [ ] T035 [P] [US3] 添加配置包导入导出集成测试 `crates/rssr-infra/tests/test_config_package_io.rs`
- [ ] T036 [P] [US3] 添加 OPML 互操作测试 `crates/rssr-infra/tests/test_opml_interop.rs`
- [ ] T037 [P] [US3] 添加配置交换手工验证说明 `tests/manual/us3-config-exchange-checklist.md`

### 用户故事 3 的实现

- [ ] T038 [P] [US3] 实现配置包文件读写与导入辅助逻辑 `crates/rssr-infra/src/config_sync/file_format.rs`
- [ ] T039 [P] [US3] 实现 WebDAV 配置上传下载客户端 `crates/rssr-infra/src/config_sync/webdav.rs`
- [ ] T040 [P] [US3] 实现 OPML 导入导出逻辑 `crates/rssr-infra/src/opml/mod.rs`
- [ ] T041 [US3] 在导入导出服务中实现配置导入、导出和远端交换用例 `crates/rssr-application/src/import_export_service.rs`
- [ ] T042 [US3] 在设置服务中实现偏好设置读写与主题设置持久化 `crates/rssr-application/src/settings_service.rs`
- [ ] T043 [P] [US3] 实现设置页面 `crates/rssr-app/src/pages/settings_page.rs`
- [ ] T044 [US3] 实现浅色、深色和跟随系统主题逻辑 `crates/rssr-app/src/theme/mod.rs`
- [ ] T045 [P] [US3] 在订阅页接入 OPML 与配置包导入导出入口 `crates/rssr-app/src/pages/feeds_page.rs`
- [ ] T046 [P] [US3] 在设置页接入 WebDAV 配置交换入口 `crates/rssr-app/src/pages/settings_page.rs`
- [ ] T047 [US3] 记录配置交换边界与主题生效手工验证结果 `tests/manual/us3-boundary-checklist.md`

**检查点**：此时所有用户故事都应可独立工作。

---

## Phase 6：打磨与横切关注点

**目的**：补齐文档、体验验证和发布前整理。

- [ ] T048 [P] 更新快速开始与实际实现保持一致 `specs/001-minimal-rss-reader/quickstart.md`
- [ ] T049 清理跨 crate 公共接口与无用抽象 `crates/rssr-application/src/lib.rs`
- [ ] T050 [P] 补充高风险回归冒烟测试 `crates/rssr-infra/tests/test_regression_smoke.rs`
- [ ] T051 [P] 验证配置包 schema、导入器和导出器保持一致 `specs/001-minimal-rss-reader/contracts/config-package.schema.json`
- [ ] T052 运行并记录完整手工验收结果 `tests/manual/final-acceptance-checklist.md`

---

## 依赖与执行顺序

### 阶段依赖

- **初始化（Phase 1）**：当前分支已完成，无阻塞项。
- **基础能力（Phase 2）**：阻塞所有新的用户故事实现。
- **用户故事 1（Phase 3）**：依赖基础能力完成，是 MVP 的最小交付。
- **用户故事 2（Phase 4）**：依赖用户故事 1 提供列表和阅读基础设施。
- **用户故事 3（Phase 5）**：依赖基础能力，以及用户偏好和订阅源持久化能力。
- **打磨阶段（Phase 6）**：依赖所有目标用户故事完成。

### 用户故事依赖

- **用户故事 1（P1）**：无其他故事依赖，是 MVP 核心。
- **用户故事 2（P2）**：依赖用户故事 1 的文章列表和阅读视图，但应保持独立可验证。
- **用户故事 3（P3）**：依赖基础订阅源和偏好设置能力，但不依赖已读/收藏功能本身。

### 每个用户故事内部顺序

- 先补自动化验证任务，再补仓储或基础设施。
- 再实现应用服务用例。
- 最后实现 UI 接线与手工验证。
- 每个故事在进入下一优先级前必须达到可独立演示状态。

### 并行机会

- Phase 2 中的仓储、抓取客户端和解析器可以并行推进。
- 用户故事 1 中的订阅栏、文章列表页和阅读页可并行实现。
- 用户故事 2 中的筛选组件、快捷键和阅读页状态操作可并行。
- 用户故事 3 中的 OPML、配置包和 WebDAV 模块可并行。

---

## 并行示例：用户故事 1

```bash
# 一起启动用户故事 1 的基础实现：
Task: "T015 [US1] 在订阅服务中实现添加订阅与刷新用例 `crates/rssr-application/src/feed_service.rs`"
Task: "T018 [US1] 扩展路由以支持订阅页、文章列表页和阅读页 `crates/rssr-app/src/router.rs`"

# 一起启动相互独立的支撑任务：
Task: "T019 [US1] 实现订阅栏页面 `crates/rssr-app/src/pages/feeds_page.rs`"
Task: "T020 [US1] 实现文章列表页面 `crates/rssr-app/src/pages/entries_page.rs`"
Task: "T021 [US1] 实现阅读页 `crates/rssr-app/src/pages/reader_page.rs`"
```

---

## 实施策略

### 先做 MVP（仅用户故事 1）

1. 完成 Phase 2：真实仓储、抓取、解析和应用装配。
2. 完成 Phase 3：实现订阅添加、刷新、文章列表和阅读页。
3. 停下来执行 `T013`、`T014`、`T025`，验证 MVP 已可独立交付。

### 增量交付

1. 先交付用户故事 1，形成最小可用阅读器。
2. 在不破坏列表和阅读体验的前提下补入用户故事 2。
3. 最后补入用户故事 3，提供配置迁移与配置交换能力。

### 并行团队策略

1. 开发者 A：仓储与 SQLite 主路径。
2. 开发者 B：抓取、解析、OPML 和 WebDAV。
3. 开发者 C：Dioxus 页面与交互接线。

---

## 备注

- 当前任务列表聚焦**剩余待完成任务**，已完成的初始化和部分基础能力不再重复列出。
- 共 52 个任务，其中 US1 为 13 个、US2 为 9 个、US3 为 13 个。
- 所有任务均符合 `- [ ] Txxx ...` 格式，并包含明确文件路径。
- 推荐 MVP 范围为 Phase 2 到 Phase 3。
