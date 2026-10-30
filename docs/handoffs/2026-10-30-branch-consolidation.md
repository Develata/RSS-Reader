# 全部分支审计与历史引用清理

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/native-back-policy`
- 审计开始 HEAD：`bfe29fe64475716ae9c270fcdef0841692c7a61b`
- main / origin/main：`4101520c33266026747db867c33e2b13baea3d23`
- 相关 commit：本记录所在的 `docs: retire audited historical branches` 本地提交；本批只提交文档与审计回执
- 相关 tag / release：无；现有 tag 不变
- 状态：`validated`（分支审计、引用清理和所列本地回归；不包括返回策略的宿主验收）

## 工作摘要

用户明确授权检查其余全部分支、吸纳值得保留的内容，并清理已合并或过时的分支及残留工作树登记。在上一轮[文档整合](./2026-10-29-consolidation.md)基础上，本轮逐项检查了 17 个本地、12 个真实远端引用，共 20 个不同分支名。GitHub 检查时没有打开的 PR。

结果：删除 11 个本地旧分支、11 个远端旧分支和 4 条失效工作树登记，剩余 **6 个本地分支、1 个远端分支（main）、7 个有效工作树**。删除仅作用于已核对的旧引用；没有推送候选提交、改写提交历史、发布、修改 tag 或向 PR #40 追加内容。

## 影响范围

- 文档：补收 3 份历史 handoff 和 2 份原始验收 JSON；更新当前状态、维护入口和归档索引。
- Git：远端引用、本地引用及对应的已退役分支跟踪配置、4 条失效工作树登记。
- 平台：Windows 上的仓库维护及 Rust 回归。生产代码、架构、数据库、渲染器和依赖不变。
- 其他工作：主 checkout 的既有改动及其他有效工作树均保留；旧 C 盘目录不属于本批。

## 全部分支处置

L 为本地，R 为远端；完整 SHA、ahead/behind、独有提交、删除清单及执行回执见[结构化证据](../testing/evidence/2026-10-30-branch-consolidation/result.json)。ahead 仅表示提交身份不同，不等于产品功能遗漏。

| 分支名 | 处置 | 核对依据 |
| --- | --- | --- |
| `audit/architecture-scan` | 删除 L/R | 原始 696 行架构审计已完整保留在当前文档；与旧 blob 比较仅新增 35 行，无删除 |
| `backup/v0.2-architecture-pre-c2-squash` | 删除 L | `bde2502c` 与已在 main 的 `5d4716ad` 文件树完全相同；重排前历史保留在 bundle |
| `backup/v0.2-architecture-pre-resplit` | 删除 L | `09ca47ec` 与已在 main 的 `e8811401` 文件树完全相同；重排前历史保留在 bundle |
| `claude/wonderful-engelbart-fdac91` | 保留 L | 已包含于 main，但由有效工作树 `nostalgic-perlman-51793d` 占用 |
| `codex/integration-final-20261023` | 保留 L，删除 R | `936930f7` 已包含于 main；本地由 `RSS-Reader-integration-20261023` 占用 |
| `codex/native-back-policy` | 保留 L | 当前未合并候选；返回策略实现 `62113c4f` 和后续文档均保留，未推送 |
| `codex/native-main-integration` | 删除 L | PR #40 的另一条旧整合线；规则已由最终主线接入或独立实现，冲突行为按下节保留现行契约；补收独有回执 |
| `codex/native-ui-preparation-20261029` | 删除 R | `adb7c5af` 已在 main；PR #40 已合并 |
| `codex/parse-image-performance` | 删除 L/R | 产品已由 PR #38 合并；额外 4 个本地文档提交的有效内容已于 `bfe29fe6` 补收，远端只有其中前 2 个文档提交 |
| `codex/pr40-finalize-20261029` | 删除 L | `6117a353` 已包含于 main，无工作树占用 |
| `codex/pr40-main-integration` | 删除 L | 与 main 同为 `4101520c`，无工作树占用 |
| `feat/reader-mathml` | 保留 L | `73f8d6ee` 已包含于 main，但由有效工作树 `reader-mathml` 占用 |
| `feat/reading-workspace-20261024` | 保留 L，删除 R | 远端 `30321e5b` 已包含于 main；本地 `020f9d40` 属于有 40 条既有状态的主 checkout，补收遗漏的初次推送回执，不动其文件或 HEAD |
| `fix/feed-proxy-content-isolation` | 删除 L/R | `9dd7517d` 与 PR #25 合并提交 `e4c4a826` 文件树完全相同；后续主线已退役 `/feed-proxy` |
| `fix/migration-opml-validation` | 删除 R | 旧双库锁与 OPML 校验由当前单库事务和整批校验覆盖；严格拒绝重复 OPML URL 与当前 upsert 不同，不恢复该旧行为 |
| `investigate/directory-scroll-20261007` | 删除 L/R | 本地 `d9e15776`、远端 `bfd2f3b6` 都已包含于 main；旧工作树登记失效 |
| `main` | 保留 L/R | 两者均为 `4101520c`，本轮没有移动 |
| `refactor/application-usecases-step1` | 删除 L/R | `89d4c3fe` 的用例集中化已在现行 application 组合和 Backend 路径实现；旧文件布局已退役 |
| `v0.2-architecture` | 删除 L/R | `b7495142` 已包含于 main；旧工作树登记失效 |
| `zheye-mainline-stabilization` | 删除 R | 共享服务、契约 harness、HTML fallback 等已吸收；旧粘贴拦截和 Web 初始化布局不适用于当前实现 |

## 有独有提交的分支如何判定

### 四月用例重构与 zheye 稳定化

检查 `89d4c3fe` 至 `c03c79d2` 的用例、Web 适配、HTML、粘贴和测试变更，并对照当前源码及[四月汇总](./archive/2026-04/2026-04-09-daily-rollup.md)。当前 `composition.rs` 组合共享 `AppUseCases`；UI 经 `Backend`，CLI 和服务端复用 application 服务。刷新、订阅、配置交换分别有当前 `test_library_*_contract_harness.rs`；Reader 保留 HTML-like summary fallback 回归，Entries 的 bootstrap/query 使用独立请求生命周期。

旧 Ctrl/Cmd+V 拦截后来导致原生粘贴被吞，当前 `feeds_page/sections/compose.rs` 已明确恢复输入框原生粘贴；不把旧 ClipboardPort 后备重新带回。旧 BrowserStore、旧 Web 初始化和旧文件组织也不恢复。原始提交完整留在 bundle，不重复把早已汇总的四月过程文档放回当前入口。

### 旧迁移锁与 OPML 分支

当前 `store/migrate.rs` 用 Immediate 事务包住迁移和历史记录；原生资料库创建、旧库导入与锁由当前单库实现负责，不移植旧 SQLx 双库启动锁。

`import_export_service.rs` 在任何写入前完成 OPML 解码及所有 URL 校验，然后调用必需的事务批量端口；`store/feeds.rs` 在同一事务内 upsert。现有回归覆盖截断、根节点外内容、不支持的 scheme、批量中途失败整体回滚、名称与来源标题分离。JSON 配置导入另有归一化重复 URL 校验和配置指纹。

**并非逐行等价：**旧分支额外拒绝同一 OPML 中重复的归一化 URL；当前源码沿用按 URL 的 upsert。将其改成拒绝会改变现有导入行为，本轮没有实际缺陷复现或要求收紧此行为，故不采纳。旧代码用整份当前设置快照调用配置替换，也不适用于当前 OPML 只批量写订阅及设置逐字段保存的边界。没有将这些差异虚报为全部已 cherry-pick。

### PR #40 另一条 Settings 整合线

保留最终 main 的 `draft_edits` / `request_lifecycle` 唯一所有者及 `stored → submitted → pending`。旧来源 `9a6700fc` 的三个建议已对照[现行适配契约](../design/native-control-adapter-contracts.md)和 facade/session 调用链检查：

| 旧建议 | 本轮决定 |
| --- | --- |
| 无效数字输入将最后有效值强行标为 pending | 当前约定不构造补丁；stored 真正供应字段时，等值回填也退役提示；submitted/pending 覆盖则保留提示。保持主线，不制造新的待保存字段 |
| 预设选择 A → B → A 用独立代次保护 | 当前保存回执明确按“当前选择仍等于提交选择”重新识别预设；未编辑 CSS 的加载跟随 stored。旧代次方案会改变已接受的行为，不混入本轮 |
| 保存/无效保存/等值存储投影都使主题导入失效 | facade 已在明确保存及预览尝试前失效身份，包括被拒绝的尝试；下载在准入时失效。stored 投影仅在实际改变 CSS 时失效，保持主线契约 |

当前 app 回归重新执行通过，但本轮没有将这些旧分支差异判定为新的运行缺陷，也没有新增行为修复。旧来源的 Windows/Web 通过数只属于其旧产物，不能转用为当前返回策略的验收。

## 补收文档

- [原生旧整合线及此前分支清理](./archive/2026-10/2026-10-29-native-main-integration.md)、[设置会话职责整理](./archive/2026-10/2026-10-29-settings-cohesion.md)：来自 `2312dd0e`，补足另一条整合线的所有权和历史清理回执。
- [PR #40 初次推送与 CI](./archive/2026-10/2026-10-29-native-preparation-push-ci.md)：来自 `020f9d40` 的独有文档提交 `418801f8`。
- 对应两份 `result.json` 与来源 Git blob 完全一致；handoff 原文只增加历史说明并修复链接，原始 blob 身份见证据中的 `imports`。
- 归档现为 201 份；当前入口只指向本轮结论，不把旧草稿、冲突和测试状态重新列成当前待办。

## 清理执行与保全

先创建工作树之外的 `E:/gitclone/RSS-Reader-backups/2026-10-30-branch-consolidation/before-cleanup.bundle`，`git bundle verify` 通过，逐一确认待删 22 个引用及预期 SHA 均在 bundle 中。文件为 12,290,129 字节，SHA256：`717F6DCE467E5B9BCD8C30AD0D81F1C689BA652A2CD862A71A30B8E5434224C4`。bundle 保存 Git 对象与引用，不包含未提交文件。

4 条失效工作树的元数据另存同目录 `stale-worktree-metadata/`；29 个文件逐一核对 SHA256，4 个索引相对各自 HEAD 都没有暂存差异。`git worktree prune --dry-run --verbose --expire now` 精确匹配以下清单后才执行 prune，只清登记，未删除任何工作树目录中的用户文件：

| 失效登记路径（相对 `E:/gitclone/RSS-Reader/`） | Git 登记 ID |
| --- | --- |
| `target/audit-20261009/b2-baseline` | `b2-baseline` |
| `target/directory-scroll-20261007/worktree` | `worktree` |
| `target/feed-proxy-isolation-20261007/worktree` | `worktree1` |
| `target/audit-20261009/worktree` | `worktree2` |

远端先重新读取 `ls-remote`，确认与审计快照一致；使用 `git push --dry-run --atomic` 预演，再以逐条 `--force-with-lease=<ref>:<expected-sha>` 原子删除 11 个旧 head。只有删除 refspec，没有上传候选分支。

本地排除所有有效工作树占用后，使用 `git update-ref --stdin` 的 start/prepare/commit 事务，逐条携带预期 SHA 删除 11 个引用，并移除这些已删分支的跟踪配置。首次 PowerShell 管道使用 CRLF，Git 在 `start` 处拒绝、没有修改引用；复核所有 SHA 不变后，用无 BOM UTF-8/LF 标准输入完成事务，两次日志均保留。

删除后重新读取真实远端、全部引用及 tags，确认只有清单中的引用消失，main 和所有保留引用未变。保留分支的原有 upstream 配置未改，指向已删远端的旧分支可能显示 `gone`，这不影响本地工作。

主 checkout 保持 `020f9d40`，40 条既有 Git 状态相同，快照中的 18 个现存普通文件 SHA256 相同；未跟踪目录用展开后的 Git 状态复核，没有声称递归散列所有用户文件。其他 5 个有效 checkout 的 HEAD 和清洁状态相同。当前候选工作树独立提交本批文档。未枚举或删除任何工作树的 ignored 产物。

## 验证与验收

- `cargo test -p rssr-domain -p rssr-application --locked`：100 通过。
- `cargo test -p rssr-infra --locked --test test_library_refresh_contract_harness --test test_library_subscription_contract_harness --test test_library_config_exchange_contract_harness --test test_opml_interop --test test_library_import`：48 通过。
- `cargo test -p rssr-infra --locked --lib store::migrate`：10 通过。
- `cargo test -p rssr-app --locked`：373 通过、1 个既有忽略项。
- 合计 **531 通过、0 失败、1 忽略**；本轮没有修改测试或生产代码。
- Git bundle、待删引用身份、保留引用与 tags、工作树状态、主 checkout 已记录散列：通过。
- 321 份 Markdown、1463 个本地链接、15 个锚点、3 份新增 JSON、201 份归档数量及原始回执 blob、`git diff --check`：通过；结果写入本批结构化回执。
- 完整日志位于 `target/branch-consolidation-20261030/`，清理清单、引用快照、执行回执和散列同步保存在工作树外的上述恢复目录。
- 本轮未运行 Playwright、WebView2、adb、构建或远端 CI；本批是文档与引用维护，Rust 回归用于确认旧分支能力映射，不声称完整产品或宿主验收。

## 结果、风险与后续

剩余本地分支是 main、当前候选和 4 个其他有效工作树占用的分支，其中主 checkout 仍有既有改动。即使提交已进 main，也不替其他任务拆除有效工作树或切换其分支；两处有效 detached 工作树也保留。失效登记已清，无需继续为数量而删目录。

本轮未发现应额外移植的产品修复。保留原生界面前置模块化的范围和行为，下一步仍是[返回策略](./2026-10-29-native-back-policy.md)实际 Windows/Android 宿主验收；此前自动执行审批拒绝的场景仍未覆盖，531 个本地 Rust 测试不能代替它。候选进入新 PR 另按后续授权，本批不发布。

## 恢复方法

恢复包不依赖旧工作树。需要旧历史时，从 bundle 精确取到新的 `codex/` 恢复分支，再检查；不要覆盖当前分支或主 checkout。例如下列命令仅为恢复说明，本轮未执行：

```powershell
git bundle verify E:/gitclone/RSS-Reader-backups/2026-10-30-branch-consolidation/before-cleanup.bundle
git fetch E:/gitclone/RSS-Reader-backups/2026-10-30-branch-consolidation/before-cleanup.bundle refs/heads/codex/native-main-integration:refs/heads/codex/recovered-native-main-integration
git show 9a6700fc:crates/rssr-app/src/pages/settings_page/README.md
```

恢复某个原远端独有分支时，bundle 的来源引用使用 `refs/remotes/origin/<原名称>`，目标仍取一个新的本地恢复分支；本地恢复不自动授权重新创建远端。工作树登记备份用于追溯其 HEAD/index，不能把它当作丢失工作目录文件的备份。

## 同日后续：剩余有效工作树盘点（未删除）

用户要求“本地分支和工作树还有不少，你先看看”。检查基线为本地 `cd8e6c36`，本次只检查剩余 checkout 并追加本节，没有移动或删除分支、工作树、缓存或证据。盘点记录对应本节所在的 `docs: inspect remaining local worktrees` 本地文档提交；未推送。

### 已核实事实

当前仍为 6 个本地分支、7 个有效工作树。除主 checkout 和当前候选外，以下 5 个旧工作树的 `status --porcelain=v1 --untracked-files=all` 均为空；`merge-base --is-ancestor` 确认其全部提交已在本地 main `4101520c`，没有独有提交。上一轮的“有效工作树”只说明目录和 Git 登记存在，不代表正在开发。

| 工作树 | HEAD / 分支 | 文件逻辑大小 | Git 忽略内容 |
| --- | --- | --- | --- |
| `E:/gitclone/RSS-Reader/.claude/worktrees/determined-mestorf-560b15` | `e4c4a826`，detached | 1,303,335,751 字节（1.214 GiB） | `target/debug`、flycheck 等构建缓存 |
| `E:/gitclone/RSS-Reader/.claude/worktrees/nostalgic-perlman-51793d` | `73f8d6ee`，`claude/wonderful-engelbart-fdac91` | 8,696,519 字节（8.29 MiB） | `.claude/settings.local.json`，231 字节；只查文件元数据，未读取配置内容 |
| `E:/gitclone/RSS-Reader/.claude/worktrees/reader-mathml` | `73f8d6ee`，`feat/reader-mathml` | 1,308,151,882 字节（1.218 GiB） | `target/debug`、flycheck 等构建缓存 |
| `E:/gitclone/RSS-Reader/.claude/worktrees/youthful-curie-5447de` | `d9ebe354`，detached | 7,990,186 字节（7.62 MiB） | 未发现忽略文件 |
| `E:/gitclone/RSS-Reader-integration-20261023` | `936930f7`，`codex/integration-final-20261023` | 1,635,329,316 字节（1.523 GiB） | `target/handoff` 和空的 `target/test-build-id` |

合计 4,263,503,654 字节，约 3.97 GiB。扫描未跨越 reparse point、无读取错误；逻辑大小不等于实际可释放空间，本轮释放为零。前四处没有发现 `.db` / `.sqlite` / `.sqlite3` 文件，旧整合目录中这些文件属于历史浏览器测试 profile。

旧整合工作树的 `target/handoff` 有 4004 个普通文件，共 1,620,334,144 字节（约 1.51 GiB）：`release-v0.2.0` 841.93 MiB、`public-pages` 468.40 MiB、`review-fixes` 163.07 MiB、`merged-checks` 69.93 MiB，以及只读审查和最终 CI 回执。它包含测试 profile、产物、日志、截图和原始 CI 附件，不能把整个目录当编译缓存删除。另外有 5 个 Chromium Singleton 重解析点，计数、大小与递归扫描均未跟随它们；后续保全应单独记录链接目标，不复制到目标外。

主 checkout 仍在 `020f9d40`，40 条既有状态保留，另有被忽略的 `rss-reader.db`、构建目录和嵌套工作树；不能将其作为普通旧 checkout 清掉。它比 main 多出的两个提交都是文档，内容已在前轮吸纳，未提交文件仍属于原任务。当前 `native-back-policy` 检查时干净，比 main 多 5 个提交，包含尚未合并的返回策略及文档；必须保留。

### 占用检查与建议

Git 没有工作树 `locked` 标记；本次读取的可见 Codex 任务没有以这 5 个旧目录作为 cwd。Windows 进程的可执行路径/命令行中也未发现匹配这 5 个目录的非 shell 进程。以上仅为快照，不能证明其他编辑器、终端或外部工具绝对未持有目录；没有据此终止任何进程或会话。

建议下一批先把旧整合目录的证据及 Claude 本地配置复制到工作树外并逐文件校验，再退役这 5 个已合并、无源码改动的工作树，同时删除其 3 个本地分支。这样可收敛为 **2 个工作树、3 个本地分支**：主 checkout 的 `feat/reading-workspace-20261024`、当前 `codex/native-back-policy` 和未检出的 main。现有 Git bundle 不含上述 ignored 文件，不能跳过保全步骤。

本次不扫描或处理旧 C 盘目录，不改主 checkout 的既有内容。验证仅为 Git 身份/状态、忽略项、文件元数据与逻辑大小、有限进程/任务快照及文档检查；没有执行产品测试、宿主验收或清理操作。
