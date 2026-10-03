# 文章列表目录交互意图澄清与设计记录

- 日期：2026-10-03
- 作者 / Agent：Codex
- 分支：main（upstream：origin/main）
- 当前 HEAD：e8c6ee165761f2e89baa6612b96324310def187a
- 相关 commit：pending（commit: pending）
- 相关 tag / release：N/A
- 状态：`draft`（设计已确认，实施与运行验收未执行）

## 工作摘要

在用户指定的 Windows 主目录 `E:/gitclone/RSS-Reader/` 记录文章列表右侧目录与左侧主列表的交互设计，纠正“当前组始终不可折叠”和“跨组才恢复跟随”的误解。本轮仅编辑对应 docs，没有创建 worktree、额外任务或 PR。

## 影响范围

- 模块：文章列表目录交互设计；未修改产品代码。
- 平台：设计涉及 Desktop / Web 右侧目录，以及后续共享 Rust 状态的手机顶部目录；未在任何平台运行验收。
- 额外影响：设计入口、UI 边界文档和本交接；无 workflow 改动。

## 关键变更

### 产品规则与实施边界

- 新增 [entries-directory-follow.md](../design/entries-directory-follow.md)：范围、完整规则、两段用户原话、Rust 状态方向、性能目标、实施疑问、历史证据及未来验收契约。
- 目录内滚动或展开/收起允许独立浏览，包括折叠当前组；主列表同组内滚动也恢复跟随，清除手动展开状态，收起其他组，仅展开当前位置路径，无需跟随开关。
- 未来统一 Rust 目录状态，各目录消费者独立订阅；JS 保留必要 DOM 能力，避免每帧全量 IPC，不污染文章 presenter。本轮不实施。
- 性能不退化和降低内存仅为目标；CSS 折叠保留 DOM 不保证内存收益，卸载方案需验证首次展开和跟随流畅度。
- [设计索引](../design/README.md)与 [UI 边界文档](../design/ui-shell-bus-page-facade.md)增加入口，明确设计待实施。

### 主目录同步与未提交内容保护

- pull 前 HEAD：`1b5f8ebf8f7b50d576df2dc1e2ae4eec0ede4d23`。
- 初次 `git pull --ff-only` 因沙箱无法写入 `.git/FETCH_HEAD` 失败；按用户已授权的主目录 pull 范围申请工具提权后重试成功，Fast-forward 至 `e8c6ee165761f2e89baa6612b96324310def187a`，无冲突或分叉。
- pull 带入上游代码、文档等更新；它们属于已授权主线同步，不是本轮本地编辑。未 checkout PR19 分支，也未将其内容覆盖到 main。
- 原有 22 项 tracked 删除（9 项 `.agents/skills/` 与 13 项 `.specify/`）及未跟踪 `.workbuddy/` 保留；初始暂存区为空。缺失的 checkout 技能未恢复。
- 未 stash / reset / clean，未切分支，未 commit / push / merge；无新 tag 或 release。

## 验证与验收

### 文档与工作区检查

- `Get-Location`、`git branch --show-current`、`git rev-parse --abbrev-ref '@{upstream}'`、`git rev-parse HEAD`：核对主目录、main、origin/main 及同步前后 SHA。
- `git status --short`、`git diff --name-only`、`git diff --cached --name-only`：检查既有删除与未跟踪状态，区分本轮 docs 改动；暂存区保持为空。
- `rg` 检索 docs 中目录/跟随/PR19/活动组等记录：找到 2026-04-14 历史方案，未找到 PR19 专项文档。PR19 四个旧契约键盘失败仅按本次任务交接注明来源，未声称独立复核。
- `git diff --check -- docs`：通过；新增文档另做尾部空白检查和相对链接存在性核对。
- 手工核对完整规则与任务原意：目录内自由展开/收起、同组滚动恢复、重置其他组、无开关、Rust 后续方向与未决事件来源均已记录。

### 运行验收

- 编译、单元测试、浏览器/键盘交互、性能和内存测试：全部未执行，遵循本轮仅文档授权。
- 任何历史通过/失败均不作为本次设计已经实现或运行验收通过的证据。

## 结果

文档可供下一轮实现和契约评审使用；本轮无产品行为或发布变更。所有新增/修改文档保持未提交。

## 风险与后续事项

- 程序化滚动、目录事件冒泡、嵌套滚动、惯性、键盘默认行为、目录导航与跨页加载的事件归属仍待设计验证。
- 当前位置锚点、路径失效、跨端订阅生命周期及 DOM 保留/卸载成本仍待细化。
- 后续根据新规则与事件来源更新鼠标/键盘契约，保留 PR19 旧失败背景，不得只为 CI 变绿修改预期。

## 给下一位 Agent 的备注

先读[目录交互设计](../design/entries-directory-follow.md)。用户已选择将 Rust 状态统一与修复一起推进，不先单独加 JS 拦截。历史交接的“激活强制展开”与常驻 DOM 决策不能覆盖新设计；实施前重新检查工作区并保护上述既有改动。
