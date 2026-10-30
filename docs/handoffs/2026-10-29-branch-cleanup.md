# 已合并闲置分支清理

> 历史回执：本文件及证据于 2026-10-29 从 `codex/parse-image-performance` 补收。下文的分支数量、SHA、授权和执行结果均属于当时快照，不是本轮新执行的删除；现状见[整合记录](./2026-10-29-consolidation.md)。

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/parse-image-performance`
- 当前 HEAD（清理前）：`a92ed16c08eec5815600211cad612c063bf06b8c`
- 相关 commit：`6e99706c`（PR #38 合并）；清理记录为本文件所属的 `docs: record approved branch cleanup` 本地文档提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

用户确认按已列出的 13 个本地分支、9 个远端分支执行清理。执行前重新核对原提交、主线包含关系和工作树占用，保存并验证可恢复的 Git bundle，再删除全部 22 个目标引用。没有扩大到其他分支或工作树。

## 影响范围

- 模块：本地 Git 分支、`origin` 上的指定分支、交接索引及清理证据。
- 平台：Windows 本地仓库及 GitHub `Develata/RSS-Reader`。
- 额外影响：没有产品源码、构建配置、应用资料库或发布产物变更。
- 原始清单、执行日志和备份：当前工作树的 `target/branch-cleanup-20261029/`，不纳入版本库。
- 已提交的精简证据：[result.json](../testing/evidence/2026-10-29-branch-cleanup/result.json)，包含全部被删除分支的名称与原 SHA。

## 关键变更

### 删除范围与执行约束

- 本地删除 13 个分支，远端删除 9 个分支，跳过 0 个。执行时每个原提交均可从 `origin/main` 到达，且分支名没有关联任何登记中的工作树。
- 远端先 dry-run，再使用 `git push --atomic` 和每个引用的 `--force-with-lease=refs/heads/<name>:<expected-sha>` 删除。这里的 lease 用于阻止误删同期变化的分支；没有改写其他分支的提交历史。
- 本地逐个重新核对提交、主线包含关系与占用，再执行 `git branch -d -- <name>`，没有使用 `-D`。
- 保留所有未合并历史、备份分支、占用中的分支及缺失目录对应的登记记录；没有执行工作树清理或文件删除。

### 可恢复备份

- 文件：`target/branch-cleanup-20261029/before-cleanup.bundle`。
- 大小：12,266,899 字节。
- SHA256：`F458250398DB4A5763A712487D8E18AC6BA6EAC607D03E6CB8A3BC2843C1787A`。
- `git bundle create ... --all` 后，`git bundle verify` 通过；全部 22 个目标引用及原 SHA 均在 bundle 中。
- bundle 只保存 Git 对象和引用，不包含未提交、未跟踪的工作树文件。本次没有修改这些用户文件，也没有对其逐文件计算哈希。
- 恢复时先从证据中选定名称和原 SHA。本地使用 `git branch <name> <sha>`；远端使用 `git push origin <sha>:refs/heads/<name>`。若对象已不在仓库，先从 bundle 获取对应原引用到明确命名的恢复分支并核对 SHA。恢复前重新检查目标名称是否已被其他工作使用。

### 并行工作的保留

- 删除后核对时，另一个任务将 `feat/reading-workspace-20261024` 从 `075569b3` 推进至 `906035ef`。reflog 记录时间为 2026-10-29 10:10:44 UTC，提交主题为 `fix(native): preserve BOM in WebView mutation transport`。
- 主工作区的状态条目从 48 个变为 40 个；减少的 8 个文件路径与该提交的文件清单完全一致。其他已有可用工作树的 HEAD 和状态在本次写入交接记录前均未改变。
- 10:12:09 UTC 另一个任务从 `origin/main` 新建 `codex/directory-toolbar-sticky-header`，工作树位于 `E:/gitclone/RSS-Reader-directory-ui-20261029`。该分支和工作树保留。
- 初次验证误把一个缺少 `.git` 的旧工作树目录当成有效工作树，Git 因而读取了上级主仓库的状态。最终验证检查实际仓库根目录，并将该路径仅作为保留的旧登记记录核对。初次结果保存在 `verification.json`，纠正后的结果为 `verification-final.json`。

## 验证与验收

### 自动化验证

- `git fetch --no-tags origin`、`git ls-remote --heads origin`：执行前刷新并核对远端状态；全部目标原 SHA 与用户确认清单一致。
- `git merge-base --is-ancestor <sha> origin/main`、`git worktree list --porcelain`：全部目标已完整合入主线，且未被任何登记中的工作树占用。
- `git bundle verify`、`git bundle list-heads`、`Get-FileHash -Algorithm SHA256`：备份完整，22 个目标引用可恢复，执行后哈希未变化。
- 远端 dry-run 与实际原子删除、本地安全删除：全部通过，删除日志保存在本地证据目录。
- 删除后 `git ls-remote --heads origin`、`git for-each-ref`：22 个目标均已消失；所有保留远端分支与标签未改变。
- `origin/main` 始终为 `6e99706ce90026c70c9538eb35faf572f02781fd`；本地 `main` 始终为 `d9ebe35460b5a0975a2c8fff0db7b1233160c0c0`。没有顺带更新本地 main。
- 原有工作树登记全部保留。已识别上节所述并行提交和新工作树，没有无法解释的剩余差异。
- 文档链接、JSON 清单与计数校验、`git diff --check`：通过；检查 76 个相对链接，13 + 9 条原 SHA 与执行结果逐项一致。

### 手工验收

- 本轮没有变更应用代码，未执行产品界面验收、Rust 构建或应用测试，也不把此前 CI 结果作为本次新增验证。

## 结果

- 用户确认的分支清理范围已全部完成。
- 最终核对时有 15 个本地分支、27 个远端分支。本地为原有 27 个减去 13 个，再加上并行任务新建的 1 个。
- 当前 `codex/parse-image-performance` 仍保留：清理前已有 2 个文档提交尚未合入 main，本记录另作 1 个本地文档提交。
- 主工作区的未合并提交和剩余用户改动保留；没有替其他任务提交、重置或清理文件。

## 风险与后续事项

- 本地 bundle 位于被忽略的 `target/` 内；后续清理构建目录前，应先另存该文件并复核上述 SHA256。
- 分支清理不等于将所有历史分支合入主线。剩余历史若要继续处理，需单独核实归属与完整性。
- 此次远端操作仅删除已确认的 9 个分支。清理记录按仓库授权做本地提交，不包含新建 PR、合并或推送到 main。

## 给下一位 Agent 的备注

- 先读本记录及精简证据；恢复时使用其中完整 SHA，不要仅凭分支名猜测原提交。
- 共享仓库中还有活跃任务。后续操作前重新读取分支、工作树和改动状态，不要把本次快照当成持续有效的授权或现状。
