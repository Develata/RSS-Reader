# 任务列表：完全 Headless Active Interface 重构

**输入**：来自 `/specs/002-headless-active-interface/` 的设计文档  
**前置条件**：`plan.md`、`spec.md`

**测试**：每个模块必须包含自动化验证与 Chrome MCP 前后对照验证；凡是影响 UI 的模块，必
须验证桌面与小视口，并确认视觉与体验等价。

## Phase 1：治理与基线

- [ ] T001 更新 `docs/design/` 与 `docs/testing/` 文档，明确 headless 目标与模块级验收规则
- [ ] T002 更新 `.specify/` 宪章与模板，把模块级 Chrome MCP 验证写入强制流程
- [ ] T003 为订阅、阅读、设置三个动作域梳理当前公开动作接口与基线页面路径

## Phase 2：订阅模块

- [ ] T004 抽订阅模块命令层与查询层 `crates/rssr-app/src/pages/feeds_*`
- [ ] T005 保持订阅页视图壳与公开语义标记等价
- [ ] T006 使用 Chrome MCP 对订阅页执行重构前后对照验证并记录结果

## Phase 3：阅读模块

- [ ] T007 抽文章页与阅读页命令层、导航查询与状态更新语义
- [ ] T008 保持文章页与阅读页的视觉层级、目录结构和操作触达等价
- [ ] T009 使用 Chrome MCP 对文章页与阅读页执行重构前后对照验证并记录结果

## Phase 4：设置与配置交换模块

- [ ] T010 抽设置保存、主题、CSS、配置交换和 WebDAV 命令层
- [ ] T011 保持设置页视觉与交互节奏等价
- [ ] T012 使用 Chrome MCP 对设置页和配置交换路径执行重构前后对照验证并记录结果

## Phase 5：CLI / GUI 语义统一

- [ ] T013 梳理 `rssr-cli` 与 GUI 动作域的重复语义
- [ ] T014 让至少一个动作域完成 GUI / CLI 统一命令语义
- [ ] T015 记录统一后的边界与剩余分歧
