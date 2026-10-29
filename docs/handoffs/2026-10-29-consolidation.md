# 当前阶段、分支核对与文档整合

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/native-back-policy`
- 起点 HEAD：`62113c4fc181e245bc077dcd46eda56a969d7b96`
- 核对主线：`4101520c33266026747db867c33e2b13baea3d23`
- 相关 commit：本记录与文档整理同次本地提交，精确身份由本文件 Git 历史定位；提交回执另存 `target/consolidation-20261029/final-receipt.json`
- 相关 tag / release：无；不 push、不发布、不修改 PR #40
- 状态：`validated`（仅分支事实、文档整合与归档；返回策略实际原生验收仍未完成）

## 工作摘要

用户要求先说明阶段、待办与分支数量，再整合分支和过时文档。核对结果是：当前 main 已包含 PR #38–#45，原生界面仍在接入控件前的规则模块化阶段；本地返回策略已经实现，但实际 Windows / Android 宿主验收未完成。当前工作树已基于最新 main，无需再合旧产品分支。先快进闲置的本地 main、补收遗漏文档、统一当前状态与历史入口。

## 影响范围

- 工作目录：`E:/gitclone/RSS-Reader-worktrees/native-back-policy`。
- 修改：文档、文档证据及本地 `main` 引用；不改 Rust、CSS、依赖、工作流、资料库或发布产物。
- 主 checkout `E:/gitclone/RSS-Reader` 的 40 条未提交状态与其他有效工作树保留；不切换它们的分支。
- 不删除本地/远端分支，不清理工作树登记，不处理旧 C 盘目录。
- 结构化快照与归档清单：[result.json](../testing/evidence/2026-10-29-consolidation/result.json)。本地详细日志在 `target/consolidation-20261029/`。

## 关键变更

### 主线与候选

- 刷新及回读远端后，main 仍为 `4101520c`；GitHub 当前打开的 PR 查询为空。
- 本地 main 原为 `9b17a4d5`，落后 41 个提交、无独有提交，且未被任何登记工作树检出。检查祖先关系后，用包含旧 SHA 条件的 `git update-ref` 将它快进到 `4101520c`；未切换任何 checkout。
- 返回策略实现 `62113c4f` 继续留在当前候选分支。此前 Rust / Web 验证及原生构建不等于实际宿主通过；本轮文档核对没有补跑这些场景，见[本批原始记录](./2026-10-29-native-back-policy.md)。
- 两个历史备份的文件树分别与 main 已包含的 `e8811401`、`5d4716ad` 相同，本轮重新验证了整树身份与祖先关系；不是两项待合并功能。

### 补收遗漏文档

- `codex/parse-image-performance` 比 main 多的 `4c1956a8`、`a92ed16c`、`1d3d1213`、`22c5d44b` 均仅改文档。补入最终 [PR #38 合并回执](./2026-10-29-parse-image-pr-ci.md)、[历史清理记录](./2026-10-29-branch-cleanup.md)、[历史审计](./2026-10-29-remaining-branch-audit.md)及两份 JSON，导入时五个 Git blob 均与来源相同。
- 两份历史分支报告随后增加历史说明、本页链接并修复归档引用，保留原始事实、数字和证据；当前状态和索引按最新 main 重写，没有回退到旧分支的产品树。
- `codex/native-main-integration` 是 PR #40 的另一条接入历史。其 Settings 方案及移交须与主线逐字段方案区分；当前接入依据为[主线整合记录](./2026-10-29-pr40-main-integration.md)及最终 review，不把独有 SHA 自动视作遗漏。旧移交文档仍可由本地 `git show 2312dd0e:docs/handoffs/2026-10-29-native-main-integration.md` 回读，本轮未声称整条分支逐行等价。
- 主 checkout 的两个独有提交为文档。`020f9d40` 的测试约定与交接已由 PR #44/#45 覆盖；`418801f8` 是早期原生分支推送/CI 的历史记录，后续整合和最终 CI 有独立回执，保留原分支供追溯。未合入其旧架构 AGENTS。

### 文档职责收口

- [维护状态](../current-status.md)区分已发布、最新 main、本地候选，集中回答当前阶段与近期顺序。
- [维护待办](../maintenance-backlog.md)把返回策略收口与 M01–M07 持续维护分开；PR #38/#40 不再作为待合并工作。
- 设计与测试索引修正“性能工作分支尚未合并”；初期 Use Case 计划明确作为历史语境，现行边界清单改正 UI 直接调用 `AppUseCases`、独立 `SettingsSyncService` 和旧 `/feed-proxy` 的过时说明，并对照实际源码。
- 当前交接索引不再罗列 51 份 10 月 23–28 日过程记录，改由[历史索引](./archive/README.md)集中检索。发布、最终验收、当前批次与全部原始证据保留。
- 用户确认“按清单归档，保留原文和证据”后，将精确清单的 51 份记录移入 `docs/handoffs/archive/2026-10/`。搬移前后 SHA256 全部一致；随后通过明确路径的文件补丁修复引用。与原始 Git 内容逐一比较，51 份正文及解析后的链接目标均保留，现有归档总数为 198。
- 全量文档链接检查发现另一份既有交接仍引用已退役源码，将两条链接固定到该交接审查提交 `1d27eae4`；本轮确认该提交可从 main 到达且文件存在。路线图索引也改为当前 v0.2.1 / 维护状态入口。
- 自动审批两次拒绝运行 Python 链接修正程序（第二次已改为只读生成计划），均返回 `approval required by policy, but AskForApproval is set to Never`，命令未执行，也未返回更具体原因。停止运行该程序，改用逐项审查的文档文件补丁，仅修改已明确的引用；最终核验使用只读 PowerShell/Git。没有修改执行审批配置。

## 分支逐项处置

本轮共 **17 个本地、12 个真实远端引用**；按名称去重为 20 个。下表同名引用合并展示，`—` 表示该端不存在。本轮没有删除任何引用。

| 分支 | 本地 | 远端 | 处置 |
| --- | --- | --- | --- |
| `main` | `4101520c` | `4101520c` | 本地已快进；主线唯一基准 |
| `codex/native-back-policy` | `62113c4f` 起点 | — | 当前候选；本轮再加文档，原生验收待补 |
| `codex/parse-image-performance` | `22c5d44b` | `a92ed16c` | 产品已由 #38 合入；遗漏文档本轮补收 |
| `codex/native-main-integration` | `2312dd0e` | — | 另一整合历史，保留，禁止整树覆盖当前主线 |
| `codex/native-ui-preparation-20261029` | — | `adb7c5af` | 已是 main 祖先，#40 已完成 |
| `codex/pr40-finalize-20261029` | `6117a353` | — | 已是 main 祖先，未被登记工作树占用，可另行确认删除 |
| `codex/pr40-main-integration` | `4101520c` | — | 与 main 相同，未被登记工作树占用，可另行确认删除 |
| `codex/integration-final-20261023` | `936930f7` | `936930f7` | 已包含；保留占用中的工作树 |
| `feat/reading-workspace-20261024` | `020f9d40` | `30321e5b` | 远端已包含；本地旧基线有文档及其他任务 dirty，保持原样 |
| `feat/reader-mathml` | `73f8d6ee` | — | 已包含；保留占用中的工作树 |
| `claude/wonderful-engelbart-fdac91` | `73f8d6ee` | — | 已包含；保留占用中的工作树 |
| `investigate/directory-scroll-20261007` | `d9e15776` | `bfd2f3b6` | 两端已包含，本地仍有残留工作树登记 |
| `v0.2-architecture` | `b7495142` | `b7495142` | 已包含，本地仍有残留工作树登记 |
| `fix/feed-proxy-content-isolation` | `9dd7517d` | `9dd7517d` | #25 已合并，旧代理后来退役；保留原历史与登记 |
| `backup/v0.2-architecture-pre-resplit` | `09ca47ec` | — | 整树等价于 main 中 `e8811401`；备份保留 |
| `backup/v0.2-architecture-pre-c2-squash` | `bde2502c` | — | 整树等价于 main 中 `5d4716ad`；备份保留 |
| `audit/architecture-scan` | `b0380771` | `b0380771` | 四月审计文档，需单独判断历史保留价值 |
| `refactor/application-usecases-step1` | `89d4c3fe` | `89d4c3fe` | 四月试验，完整语义吸收未证明，不直接合入 |
| `fix/migration-opml-validation` | — | `3f95bb3e` | 旧 OPML 另一版历史，完整等价未证明 |
| `zheye-mainline-stabilization` | — | `c03c79d2` | 四月整理历史，完整等价未证明 |

引用多不等于未完成产品多。未证明完整吸收的四类旧历史需单独判断保留价值；压缩合并、替代实现和文档补记都会影响 ahead/behind 数字。

## 工作树核对

11 条登记中有 7 个有效工作树、4 条残留。有效工作树包括主 checkout、四个 `.claude/worktrees/` checkout、`RSS-Reader-integration-20261023` 和当前 E 盘工作树。主 checkout 有 40 条既有状态，其余起点干净。

残留为主 checkout 下的 `target/audit-20261009/b2-baseline`、`target/audit-20261009/worktree`、`target/directory-scroll-20261007/worktree`、`target/feed-proxy-isolation-20261007/worktree`。其中 audit 的 `worktree` 目录虽存在，已无自身 `.git`；直接在其下执行 Git 会发现父仓库，不能把父仓库 dirty 误当它的状态。最终快照通过 `.git` 与 `--show-toplevel` 核对实际根目录。本轮不运行 `git worktree prune` 或删除这些路径。

## 验证与验收

### 自动化 / 只读核对

- `git fetch --no-tags origin`、`git ls-remote --heads origin`、`git for-each-ref`、`git rev-list`、`git merge-base --is-ancestor`：核实最新 main、引用数量、祖先关系；不把 `origin/HEAD` 算入分支。
- GitHub 打开 PR 查询与 #38 合并回执：核对为空及已合并；本轮无 GitHub 写操作。
- `git worktree list --porcelain`、有效目录根路径与状态检查：通过；不将残留目录向上发现的主仓库当作独立 checkout。
- `git hash-object --path` 与来源 blob：五份导入材料一致；两份历史 JSON 保留来源 Git blob，不重写内容。
- 文档本地链接与锚点：遍历 317 份 Markdown，检查 1434 条本地链接及 15 个标题锚点，缺失文件与锚点均为 0；两条退役源码引用固定到已核实存在的历史提交。外部站点和原生运行未由此验证。
- 三份新增 JSON 解析、`git diff --check`：通过；51 份归档正文及解析后链接目标与原 Git 版本比较全部一致。
- 产品路径无变化：`git diff --exit-code -- crates assets scripts .github build-support Cargo.toml Cargo.lock Dioxus.toml` 通过。另 6 个有效工作树的状态与起点逐一比较一致，主 checkout 仍为 40 条既有状态。
- 收尾再次 `git ls-remote --heads origin`：12 个远端引用及 SHA 与起点完全相同，没有远端操作。

### 未覆盖

- 本轮只做文档与引用整合，不重跑 Cargo、Web、原生或 CI；不将旧验收当新结果。
- 没有逐语义审计所有旧分支；未确认清理旧工作树内 ignored 文件或用户资料的安全性，故保留。
- 返回策略的原生验收仍受上一轮执行审批限制；本轮没有重试被拒绝命令或绕过限制。

## 结果与后续

先结束本批返回策略的实际宿主验收，再按用户后续授权进入新的 PR。当前 docs-only 整合可以独立审查，不要求重新启动架构迁移。文档物理归档已按确认清单完成；分支删除、工作树清理和远端操作均需另行明确范围，不能借“整合”一并处理。

下一位 Agent 先读维护状态、本记录及返回策略交接；继续使用当前 E 盘分支，重新读取 main 和工作树状态，不把本快照视为持续有效的删除授权。
