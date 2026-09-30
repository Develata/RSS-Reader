# 当前文档与源码契约对齐

- 日期：2026-09-30
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：`dbe68e1`（开始核对时）
- 相关 commit：`379ac22`（文档阶段独立提交，随后进入已授权的 UI 优化）
- 相关 tag / release：本次不发布；发布状态引用仓库 `v0.1.20` 交接记录
- 状态：`validated`

## 工作摘要

按用户要求先完整核对并更新 repo 文档，再开始视觉代码优化。本阶段只修改 Markdown；保留历史设计与验收结论的日期语境，不把旧运行结果当成当前通过。

## 影响范围

- 模块：根 README/贡献与 Agent 指南、`docs/` 当前设计/使用/测试/路线图、`specs/` 历史入口和 quickstart。
- 平台：Web、桌面、Android、CLI 的说明；没有运行时变更。
- 额外影响：修复历史 handoff 的本机绝对链接，不修改其当时结论；没有修改 workflow、依赖或存储格式。

## 关键变更

### 当前实现与使用契约

- 修正六 crate、Dioxus 0.7.9、索引/正文双库、BrowserStore 和已迁至 infra 的浏览器适配器说明。
- 修正不存在的 `UiRuntime` 类型、旧页面直调服务表述及锁外持久化建议；锁内读取、修改和发布为一个事务，HTTP 在锁外。
- 整理网站首页发现、批量已读、阅读元信息/本地时区、来源未读数、运行内位置和导航收起、刷新新增计数及 Web 多标签语义。
- 对齐中英文入口、设置保存行为和 Android CSS 文件导入未实现的能力边界。

### UI 与验证文档

- 修正阅读页路由、退役/缺失选择器、primary 默认按钮约定；记录不同响应式断点的职责及对比度/焦点验收要求。
- 将已完成的 harness 重建计划整理为当前入口、覆盖与限制，区分编译、实际浏览器测试、fixture 和真实远端验收。
- 真实浏览器可用项目已有 Playwright/CDP 或 Chrome MCP；保留验收强度，不再把某个工具可用性写成通用前提。
- 环境故障要求实际日志和复验证据，修正“所有 WSL 都无法运行 driver”及“普通跨源图片必然受 CORS 阻断”的错误推断。
- 早期 spec 保留日期、依赖版本与任务勾选，加历史说明；quickstart 更新到当前可执行入口。

### 历史链接

- 132 处旧工作区绝对 Markdown 链接改为可移植引用。其中两份已删除文件链接到本地 Git 已核对的历史 commit，其他链接使用当前相对路径。

## 验证与验收

### 自动化验证

- `python3 target/docs-refresh-validation/check_docs.py`：通过。检查 Markdown 本地链接/标题锚点和两份当前 UI 文档的静态 selector；产物为 `target/docs-refresh-validation/report.json`。首次收口扫描 148 份文件、439 个本地引用，108 / 138 个 selector 均有源码匹配。
- `git diff --check`：通过。
- `git diff --name-only`：本阶段所有已跟踪变更均为 `.md`，确认未开始 Rust/CSS/JS 优化。
- `git cat-file -e <commit>:<path>`：两份退役文件的历史链接目标在本地 Git 中存在。

### 人工核对

- 对照路由、workspace/锁文件、composition/runtime、bootstrap/browser adapter、设置/CLI 参数及现有回归脚本核对当前说明。
- 未运行：Rust 构建、浏览器和实机交互。本阶段无运行时代码变化，文档核对不能宣称新 UI 已验收；下一阶段执行与改动相称的验证。
- 未重新查询远端 Release / CI；发布事实明确引用仓库已记录的 `v0.1.20` 证据。

## 结果

文档阶段完成，可开始用户已授权的 UI 优化：禁用反馈、标题层级、Reader 语义与焦点、共享刷新忙态/状态通知、原生 checkbox 配色。主题契约已先补齐。

## 风险与后续事项

- 主题对比度条款为验收目标，不是全量 WCAG 符合性认证。
- Android/macOS 设备交互与真实远端源验证仍按已有边界记录，不从 Web 自动化推定通过。
- 历史文档中的旧名称/版本保留为历史证据，当前行为从 `docs/README.md` 的长期文档进入。

## 给下一位 Agent 的备注

- 文档源码基线为 `dbe68e1`。UI 优化后需要补充新状态接口和真实验收结果，并单独记录代码提交。
- 同批包含前一轮只读 UI 复核记录 `2026-09-30-ui-review-reassessment.md`。
