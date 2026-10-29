# 剩余分支来源、主线差距与活跃工作冲突核对

> 历史审计：本文件及证据于 2026-10-29 从 `codex/parse-image-performance` 补收。下文记录的是 PR #40 整合前的状态；原生增量随后已通过 PR #40 适配并合入 main，不再待做那次整体接入。当前分支数、工作树与剩余边界见[整合记录](./2026-10-29-consolidation.md)。

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/parse-image-performance`
- 审计前 HEAD：`1d3d1213b0e789d913f686d53846699c99744a45`
- 对照主线：`origin/main` → `6e99706ce90026c70c9538eb35faf572f02781fd`
- 相关 commit：本文件所属的 `docs: audit remaining branches and active work divergence` 本地文档提交
- 相关 tag / release：N/A
- 状态：`validated`（分支与文本合并检查，不是整合后产品验收）

## 工作摘要

用户询问清理后为何还有 15 个本地、27 个远端分支，是否尚未合并、过时，以及会否影响正在进行的工作。本轮刷新远端，逐项核对提交关系、GitHub PR 合并回执、工作树归属与不改变检出的合并模拟。结论是：大部分远端分支已完成合并；真正需要优先处理的是主工作区继续在旧架构基线上开发的新增提交。最终快照为 2026-10-29 10:59:47 UTC：该工作已有 16 个独有提交；目录任务同期创建 PR #39，远端数量增至 28。

此前清理只依据严格的 Git 祖先关系和工作树占用。压缩合并会产生新 SHA，因此“原分支提交不是 main 的祖先”不能直接写成“功能还没合入”。本轮补充这一区分，没有扩大删除范围。

## 影响范围

- Git 元数据读取与模拟、GitHub PR 只读查询、交接索引及本报告。
- Windows 本地仓库与 `Develata/RSS-Reader` 远端。
- 没有切换其他工作树、实际合并、重置、删除分支、推送或修改其他任务的源码。
- 结构化证据：[result.json](../testing/evidence/2026-10-29-remaining-branch-audit/result.json)。完整本地日志在 `target/branch-audit-20261029/`。

## 关键发现

### 需要优先处理的活跃工作

1. **“规划原生 Rust 化前的模块优化”**：`E:/gitclone/RSS-Reader` 的 `feat/reading-workspace-20261024` 在最终快照时为 `ca0861cb`。相对主线有 16 个独有提交，缺少主线的 432 个提交；16 个提交尚不在当前已读取的 origin 远端引用中。其同名远端为 `30321e5b`，已通过 PR #32 合并，二者并不相同。
2. 这里不只是历史计数差异：该本地分支仍包含 `rssr-host`、infra 的 `sqlx` 依赖及 Web 启动路径的 `BrowserStore`；当前 main 已改为 `rssr-api::Backend`、rusqlite 单资料库及正式 Web 的服务器资料库。PR #32 的适配在隔离工作树完成，原本地工作树没有同步这一基线。相关约束见[主线接入记录](./archive/2026-10/2026-10-27-pr-integration.md)。
3. 将 `ca0861cb` 与 `6e99706c` 做文本合并模拟，出现 **20 个冲突文件**，集中在 Entries、Feeds、Reader、Settings 的会话逻辑、应用启动与验收脚本。完整路径在结构化证据中。16 个提交相对共同祖先改动 74 个路径；两端整棵树的差异还包含主线架构演进，不能将整个分支树差异当作新增提交的改动范围。
4. **“调整目录按钮与固定标题”**：工作位于独立的 `E:/gitclone/RSS-Reader-directory-ui-20261029`，`codex/directory-toolbar-sticky-header` 最终读取到 `ec46724a`，领先 main 3 个提交、落后 0 个，工作树干净，与 main 的文本合并模拟通过。它与前述 16 个提交只有 `scripts/browser/rssr_workspace_acceptance.cjs` 一个修改路径重叠；两条分支直接合并仍有 20 个冲突文件，主要来自原生分支尚未吸收 main。文件级检查不保证后续修改或运行行为没有冲突。
5. 本地 `main` 仍指向 `d9ebe354`，落后远端 main 480 个提交、没有独有提交。之后新任务应显式采用刷新后的 `origin/main`，不能将这个本地名称误认为当前版本。

这些旧分支的存在不会自动改变活跃工作树。实际风险在后续整合，以及在旧架构上验证的成果被误认为已经覆盖现行架构。

### 原有 27 个远端分支的实际构成

| 类别 | 数量 | 核实结果 |
| --- | ---: | --- |
| 对应 PR 已压缩合并、原提交不属于主线历史 | 17 | GitHub 标记已合并，当前分支 SHA 与 PR 最终 SHA 一致，合并提交均可从 main 到达，且均只有一个父提交；不是 17 项待合并功能 |
| 原提交已完整包含在 main | 4 | `codex/integration-final-20261023`、`feat/reading-workspace-20261024`、`investigate/directory-scroll-20261007`、`v0.2-architecture`；此前因同名本地分支仍关联有效或过期工作树记录而保留 |
| main 本身 | 1 | 当前主线 |
| 已合并功能之后新增文档 | 1 | `codex/parse-image-performance`：远端只比 main 多 2 个合并回执文档提交，性能实现已经合入 |
| 未证明完整吸收的旧历史 | 4 | 四月的架构审计、application 试验、zheye 整理，以及十月初的 OPML 另一版修复；不应直接整条合回 |

初次查询的 38 条 PR 均已关闭；审计期间目录任务新建了 [PR #39](https://github.com/Develata/RSS-Reader/pull/39)，使远端分支增加到 28 个。最终开放 PR 查询只返回这项新工作。关闭不等于合并；例如旧 OPML PR #8 已关闭但未合并，当前同名分支 SHA 又不同于该 PR 最终 SHA，不能借用它的状态证明现有分支已处理。

### 明显过时的历史与备份

- `audit/architecture-scan`：2026-04-06，领先 1 / 落后 940；独有提交只增加旧架构审计文档。
- `refactor/application-usecases-step1`：2026-04-08，领先 1 / 落后 938；当前直接合并模拟有 15 个冲突文件，应按现行 application 契约核对历史价值。
- `zheye-mainline-stabilization`：2026-04-09，领先 6 / 落后 938；[当时汇总](./archive/2026-04/2026-04-09-daily-rollup.md)记录已吸收与重建高价值内容，但不能据此声称每个历史改动均完全保留。直接合并模拟有 25 个冲突文件。
- `fix/migration-opml-validation`：2026-10-01，领先 4 / 落后 550；与已合并的 PR #6 主题重复，但当前分支的完整等价性未证实，模拟有 4 个冲突文件。
- 十月初已合并的修复分支大多落后 508–554 个提交。旧分支再合并产生冲突不代表修复遗漏，而是旧实现与后续演进不再相同。
- 两个 `backup/v0.2-architecture-*` 是经授权整理提交历史前的保险副本。`09ca47ec` 与已在 main 中的 `e8811401` 整树完全相同；`bde2502c` 与已在 main 中的 `5d4716ad` 整树完全相同。没有等待再次合入的独立功能。
- `feat/reader-mathml`、`claude/wonderful-engelbart-fdac91` 已完整包含在 main，只是各自工作树仍登记在使用它们；两者都落后 472 个提交。

## 验证与验收

### 自动化 / 只读核对

- `git fetch --no-tags origin`、`git ls-remote --heads origin`：初次核对真实远端 27 个分支，最终读取 28 个；不把已不存在的远端跟踪缓存计入。
- `git for-each-ref`、`git rev-list --left-right --count`、`git cherry`：初次覆盖 15 个本地及 27 个远端引用；对同期新增分支和两个活跃任务的新 HEAD 补做核对。领先、落后均按提交图计数，不代表同等数量的独立功能或遗漏。
- GitHub pulls collection：读取全部 38 条记录，逐项匹配分支名称、最终 SHA、`merged_at` 与合并提交；17 个历史差异分支的合并提交均为 main 祖先。
- `git merge-tree --write-tree --name-only --messages`：对需要整合检查的唯一 SHA 模拟合并。只写 Git 对象，不修改分支、索引或现有工作树；自动合并干净不代表行为已验收。
- `git worktree list --porcelain`、实际工作树 HEAD、未提交及未跟踪路径：确认活跃任务分离及路径重叠，其他任务修改继续保留。
- `git rev-parse <sha>^{tree}`、`git merge-base --is-ancestor`：两个历史备份与主线已包含的替代提交整树相同。
- 文档链接、证据 JSON 与引用数量、`git diff --check`：通过；80 个相对链接、最终 15 + 28 个引用及 20 个原生分支冲突路径均与快照一致。

### 未覆盖

- 未执行整合后的构建或运行测试，也未为四个旧历史分支做逐语义的完整吸收审计。
- 两个活跃任务仍在修改文件；未提交内容未冻结，本报告绑定证据时间与 SHA，不能替代正式整合时的复核。

## 结果与建议

- **先处理原生任务的基线差异。** 保留现有提交和未提交内容，在基于最新 main 的隔离整合环境中逐项接入有效增量，继续使用现有 Backend、application/domain 语义、infra 适配与单资料库。不要用旧分支整棵树覆盖 main，也不要在其他任务的 dirty 工作树中直接切换或重置。
- 先迁移这 16 个提交中的 UI 纯策略及边界修复，再针对现行会话所有权、请求回执与原生适配验证；这是较小的接入路径，不是重启一次架构迁移。具体整合尚未执行。
- 已合并旧分支可在下一轮按明确清单归档或删除；占用中的旧工作树需先核对归属。四个未证明等价的历史分支应单独评估，不为减少数量而强行合并。
- 本对话分支审计前只多出 3 个文档提交；本报告另作本地文档提交。产品性能实现已经合入 main。

## 风险与后续事项

当前需要用户决策的是优先安排原生增量接入，而不是合并全部剩余分支。本轮仅回答与存证，没有向其他任务发送指令，也没有继续执行分支清理。

## 给下一位 Agent 的备注

详细清单和冲突路径见结构化证据。继续工作前重新读取 main、活跃任务 HEAD 与工作树状态；按当前 SHA 重做合并模拟，保留用户与其他任务的修改。

## 附：逐分支引用快照

以下为上述审计时间的引用状态；本文档提交会继续推进当前对话的文档分支。领先与落后都相对于 `origin/main`，本地与远端同名分支分别计算。

### 本地

| 分支 | SHA | 领先 | 落后 | 结论 |
| --- | --- | ---: | ---: | --- |
| `audit/architecture-scan` | `b0380771` | 1 | 940 | 旧历史，完整吸收未证实 |
| `backup/v0.2-architecture-pre-c2-squash` | `bde2502c` | 6 | 339 | 备份，整树等价已核实 |
| `backup/v0.2-architecture-pre-resplit` | `09ca47ec` | 5 | 401 | 备份，整树等价已核实 |
| `claude/wonderful-engelbart-fdac91` | `73f8d6ee` | 0 | 472 | 已完整包含，保留工作树 |
| `codex/directory-toolbar-sticky-header` | `ec46724a` | 3 | 0 | 目录新工作，PR #39 |
| `codex/integration-final-20261023` | `936930f7` | 0 | 85 | 已完整包含，保留工作树 |
| `codex/parse-image-performance` | `1d3d1213` | 3 | 0 | 仅合并后文档 |
| `feat/reader-mathml` | `73f8d6ee` | 0 | 472 | 已完整包含，保留工作树 |
| `feat/reading-workspace-20261024` | `ca0861cb` | 16 | 432 | 原生优化，20 个文本冲突 |
| `fix/feed-proxy-content-isolation` | `9dd7517d` | 4 | 508 | PR #25 已合并，保留旧 SHA |
| `fix/native-outbound-request-safety` | `6479ffba` | 3 | 509 | PR #26 已合并，保留旧 SHA |
| `investigate/directory-scroll-20261007` | `d9e15776` | 0 | 507 | 已完整包含，保留工作树 |
| `main` | `d9ebe354` | 0 | 480 | 本地 main 尚未快进 |
| `refactor/application-usecases-step1` | `89d4c3fe` | 1 | 938 | 旧历史，完整吸收未证实 |
| `v0.2-architecture` | `b7495142` | 0 | 154 | 已完整包含，保留工作树 |

### 远端

| 分支 | SHA | 领先 | 落后 | 结论 |
| --- | --- | ---: | ---: | --- |
| `audit/architecture-scan` | `b0380771` | 1 | 940 | 旧历史，完整吸收未证实 |
| `chore/pages-actions-hardening` | `ffb1f1f7` | 2 | 544 | PR #14 已合并，保留旧 SHA |
| `codex/directory-toolbar-sticky-header` | `241bdebf` | 2 | 0 | 目录新工作，PR #39 |
| `codex/integration-final-20261023` | `936930f7` | 0 | 85 | 已完整包含，保留工作树 |
| `codex/parse-image-performance` | `a92ed16c` | 2 | 0 | 仅合并后文档 |
| `feat/reading-workspace-20261024` | `30321e5b` | 0 | 27 | 已完整包含，保留工作树 |
| `fix/feed-guid-identity` | `7be7d6d5` | 2 | 550 | PR #5 已合并，保留旧 SHA |
| `fix/feed-proxy-content-isolation` | `9dd7517d` | 4 | 508 | PR #25 已合并，保留旧 SHA |
| `fix/import-validation-migration-lock` | `dba08059` | 11 | 550 | PR #6 已合并，保留旧 SHA |
| `fix/large-feed-filter-bind-limit` | `d7adc4c8` | 4 | 547 | PR #11 已合并，保留旧 SHA |
| `fix/migration-opml-validation` | `3f95bb3e` | 4 | 550 | 旧历史，完整吸收未证实 |
| `fix/native-outbound-request-safety` | `6479ffba` | 3 | 509 | PR #26 已合并，保留旧 SHA |
| `fix/native-persistence-atomicity` | `52a1db0d` | 22 | 554 | PR #3 已合并，保留旧 SHA |
| `fix/pages-smoke-profile-cleanup` | `620844e4` | 1 | 541 | PR #18 已合并，保留旧 SHA |
| `fix/post-audit-repair` | `dcfbcffe` | 54 | 548 | PR #10 已合并，保留旧 SHA |
| `fix/post-generation-review` | `ea170874` | 28 | 545 | PR #13 已合并，保留旧 SHA |
| `fix/refresh-generation-fence` | `09d627fb` | 55 | 547 | PR #12 已合并，保留旧 SHA |
| `fix/rust-1.99-async-trait-clippy` | `4030cbde` | 4 | 551 | PR #9 已合并，保留旧 SHA |
| `fix/sqlite-large-feed-batching` | `06acbab0` | 4 | 553 | PR #4 已合并，保留旧 SHA |
| `fix/storage-gc-query-contract` | `a964ae65` | 25 | 544 | PR #15 已合并，保留旧 SHA |
| `hotfix/mobile-entry-scroll-snapback` | `3a6427c1` | 9 | 554 | PR #2 已合并，保留旧 SHA |
| `investigate/directory-scroll-20261007` | `bfd2f3b6` | 0 | 82 | 已完整包含，保留工作树 |
| `main` | `6e99706c` | 0 | 0 | 当前主线 |
| `perf/batch-content-upserts` | `096a28db` | 5 | 541 | PR #17 已合并，保留旧 SHA |
| `perf/sqlite-large-feed-scope-json` | `193f6f6f` | 5 | 542 | PR #16 已合并，保留旧 SHA |
| `refactor/application-usecases-step1` | `89d4c3fe` | 1 | 938 | 旧历史，完整吸收未证实 |
| `v0.2-architecture` | `b7495142` | 0 | 154 | 已完整包含，保留工作树 |
| `zheye-mainline-stabilization` | `c03c79d2` | 6 | 938 | 旧历史，完整吸收未证实 |
