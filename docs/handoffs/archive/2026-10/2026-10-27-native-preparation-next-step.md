# 原生界面前置整理：下一步优先级评估

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 评估基线 HEAD：`c808ff4284d233984a42224f69264329db0e3ff7`
- 相关 commit：本文同批本地提交；记录生成时 `commit: pending`，用 `git log -1 -- docs/handoffs/2026-10-27-native-preparation-next-step.md` 定位
- 相关 tag / release：N/A
- 状态：`draft`（源码评估已完成，建议尚未实施）

## 工作摘要

用户询问继续优化、模块化及原生 Rust 界面前置工作时，应优先推进哪个部分。核对当前源码及 10 月 24–26 日交接后，建议优先独立文章工作区的异步会话规则，第一批只覆盖初始化和文章查询的结果归属、接纳及重查决策。已存在的纯 reducer、交互控制器和业务用例继续复用。

## 影响范围

- 本次仅新增评估交接，没有修改生产代码、测试、配置、数据或设计契约。
- 候选模块：`crates/rssr-app/src/pages/entries_page/session.rs`；后续按收益再评估 `mod.rs` 的查询触发条件与 `reader_page/session.rs`。
- 平台：候选属于共享前端内部，涉及 Web、Windows、Android、Linux、macOS；`rssr-web` 托管模式继续使用同一前端和原有服务契约。
- 无 workflow、依赖、迁移、tag 或发布变化。

## 关键发现与建议

### 当前源码确认的事实

- 阅读位置、下拉刷新手势、来源筛选草稿、目录状态、目录窗口和阅读快捷键已有普通 Rust 规则模块，边界见 `docs/design/native-ui-preparation.md`。
- Entries 的 `state.rs`、`presenter.rs` 和 `reduce_entries_page_intent(&mut EntriesPageState, ...)` 已承接大量可复用逻辑；不应把它们描述为尚未 Rust 化。
- `EntriesPageSession` 仍以 Dioxus `Signal` 持有页面状态及两种请求代次，在 `spawn` 闭包内决定旧初始化/查询结果是否作废，以及查询期间写入完成后是否触发重查。单条与批量写入互斥、偏好写入资格和旧偏好快照过滤也在此处。
- 这些规则已有场景测试，但 session 测试通过 `VirtualDom`、`Signal` 和异步调度驱动。提取的价值是让规则可以独立重放并供未来界面复用，不是发现当前规则不存在或没有测试。
- `mod.rs` 还组织查询范围、归档时钟、刷新 revision、首次就绪、返回页码与偏好保存的 effect；这一层的触发次序需要保留，不能在提取策略时另造一套自动查询机制。
- 列表集合已使用 `Arc<Vec<Arc<EntrySummary>>>`，卡片闭包已捕获 session 与当前文章；Reader 正文已使用 `Arc<str>`。历史深克隆诊断不再直接适用于当前源码。
- Reader 已有纯 reducer 和文章访问代次；正文选择、清洗和地址处理已在 Rust。正文最终仍由 HTML/文本视图呈现；图片打开状态在 Rust，DOM 适配负责同步默认动作、焦点、模态和滚动。

### 第一优先：文章工作区的异步会话规则

第一批范围建议限定为初始化及查询的生命周期策略：

1. 初始化与文章查询使用独立身份，旧成功和旧失败都不能覆盖新状态。
2. 查询与已读、收藏、批量写入交错时，利用现有 `flags_revision` / `reload_revision` 决定接纳、丢弃或请求重查；避免重复重查和旧列表复活。
3. 保持偏好 Pending 时等待、Unavailable 时允许阅读的既有条件；结果到达后再恢复并约束返回页码。

实现应是当前页面模块内的具体 Rust 策略及明确输入/输出，不依赖 `Signal`、DOM 或 `spawn`。Dioxus session 继续负责任务执行、生命周期与状态发布；页面规则仍留在 UI 层，业务动作仍经 facade/session → `UiCommand` → runtime → application/host。

继续使用唯一一份 `EntriesPageState` 和完整目录投影。先迁移上述两个实际入口，证明收益后再处理单条/批量写入准入和偏好保存策略。当前不需要新增 crate、通用页面框架、全局 store 或另一套状态镜像。

### 后续顺序

1. 第一批完成后，再按同一边界收口 Entries 的写入与偏好规则；保留现有失败解锁、选择范围、写入生命周期和持久化约束。
2. Reader 的异步结果归属作为下一候选，复用其现有 reducer 和访问代次。只有发现明确重复或复用需求时才抽共用机制，不强迫各页面同构。
3. 真正接入原生渲染器前，单独准备正文兼容样本及能力验收：HTML/MathML、表格、图片、链接、文字选择/复制、自定义 CSS、输入法和返回。原生呈现如何承接现有 CSS 与正文能力尚未决定；这一步涉及公开体验契约，应形成可评审方案再实施。

DOM 同步事件和测量已有明确适配职责，继续削减 JS 行数本身不是优先级依据。性能工作应跟随当前候选的冷展开、滚动、查询和内存测量；目录节点减少及历史样本不能推导本次策略抽取会更快。

## 验证与验收

### 本次已执行

- `git status --short`、`git branch --show-current`、`git log`、`git rev-parse HEAD`：确认当前分支、基线与任务外改动。
- `rg` / 源码阅读：核对 root/pages 的 AGENTS、功能哲学、headless、shell/facade、前端命令和原生前置边界；直接检查 Entries / Reader 的 state、reducer、session、页面装配及图片适配。
- `.codegraph` 不存在，本次按仓库规则使用 CLI 检索，没有初始化索引。
- `git diff --cached --check -- docs/handoffs/2026-10-27-native-preparation-next-step.md`：通过；暂存区仅包含本文。
- Cargo、浏览器、Windows/Android 实机和性能测试：未执行，本次仅评估及文档变更。已有交接中的通过记录只作历史证据，没有作为本轮重跑结果。

### 建议实施时的验收

- 普通 Rust 场景重放不创建 `VirtualDom`：旧初始化/查询的成功与失败、查询和写入交错、两类请求独立、偏好失败可读不可覆写、返回页码恢复与收缩。
- 保留必要的 Dioxus 集成测试，验证适配器的状态发布和生命周期；不能只把旧集成测试移走。
- 同一基线与候选运行现有浏览器路径：快速筛选/刷新、已读/收藏、批量归档、目录浏览/选择、跨页/返回及偏好重开。勾选、目录操作不增加查询或分组重算。
- 运行相称的 Cargo test、fmt、Clippy 和 Web/Android 编译；涉及实际交互时分别记录 Web、Windows WebView2、Android 的覆盖。
- 若报告性能收益，须测量同一数据集、同一产物身份的前后结果，分别说明查询次数、分组次数、布局/交互耗时与内存；不从模块化推断收益。

## 结果

下一步建议已具体化为 Entries 初始化/查询策略的有界提取。它服务于原生界面复用与可验证性；本次没有实施提取，也没有新增正确性缺陷或性能回退的实测结论。

## 风险与后续事项

- 主要风险是迁移后请求身份失配、重复发布/查询、失败路径漏解锁或旧偏好覆盖新选择；验收应围绕事件交错，而非只测正常返回。
- 现有源码已有完整保护，实施必须保留这些保护，不能为了接口简洁合并 bootstrap/query generation。
- 真实系统 IME、Linux/macOS 和当前候选的完整性能范围仍未在本轮验证。原生控件/框架与 CSS 承接方案也未确定。
- 未 push、打 tag、发布或触发远端工作流。

## 给下一位 Agent 的备注

- 首先阅读 `entries_page/session.rs`、`reducer.rs`、`mod.rs` 与 `docs/design/ui-shell-bus-page-facade.md`，以其现有契约为基线。
- 目录窗口化最新修复是 `bc2aedbc`，当前基线 `c808ff42` 是源码身份补记。继续前先核对 HEAD，不能把前一轮产物当新候选。
- 保留任务外 `.agents` / `.specify` 删除、两份 10 月 3 日交接修改及未跟踪设计材料；检查期间另出现未跟踪 `scripts/browser/rssr_directory_window_boundaries.cjs`，同样保留，只提交本文。
