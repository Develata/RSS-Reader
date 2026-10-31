# 已合并本地分支清理

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`main`，E 盘既有接续工作树
- 清理基线：`958632740f53c51290337e2062ea85ae2910306a`
- 相关 commit：清理记录 `09bb4bbf`；推送补记见本记录最新提交
- 相关 tag / release：无
- 状态：`validated`

## 工作摘要

用户确认上一轮列出的 13 个本地旧分支可以清理。本次逐一复核提交身份、main 祖先关系、独有提交和工作树占用后，使用 `git branch -d` 删除这 13 个引用；本地分支由 15 个减为 2 个。

## 影响范围与关键变更

只删除下列本地分支及 Git 随之清除的分支跟踪配置。13 个 tip 均完整包含于基线 main，独有提交为 0，无工作树占用。

| 已删除本地分支 | 删除前 tip |
| --- | --- |
| `codex/android-theme-import` | `b03ebc4e` |
| `codex/main-regression-20261031` | `6a39570e` |
| `codex/native-back-policy` | `a3da94d0` |
| `codex/reader-body-boundary` | `b4253a82` |
| `codex/reader-host-boundary` | `6a5b91b4` |
| `codex/reader-race-fixes` | `21f3468a` |
| `codex/reader-rule-boundary` | `ff628ed0` |
| `codex/reader-shortcut-definition` | `4d0aab02` |
| `codex/readme-media-20261031` | `0c4f9e00` |
| `codex/settings-rule-boundary` | `950c9665` |
| `codex/shell-settings-lifecycle` | `8ace1781` |
| `codex/workspace-response-cancel` | `ac15b77b` |
| `codex/workspace-response-completion` | `bf42dc32` |

完整 tip 和结果见[结构化回执](../testing/evidence/2026-10-31-local-branch-cleanup/result.json)。原始引用、跟踪配置、删除输出及保全快照位于 `target/local-branch-cleanup-20261031/`。产品代码、依赖、workflow、资料库和历史验收文件均未修改；没有使用强制删除、GC 或文件系统清理。

## 验证与验收

- 每个 tip 的 `git merge-base --is-ancestor <tip> 95863274` 通过；`git rev-list --count 95863274..<tip>` 均为 0。
- `git worktree list --porcelain` 确认这 13 个分支均无占用；`git branch -d` 全部成功。
- 删除阶段前后 main HEAD、工作树文件状态、两个工作树登记、本地 tag、远端跟踪引用及 `git ls-remote --heads origin` 完全相同；14 个真实远端分支保留。
- 主 checkout 的 HEAD、分支、index、状态与 41 个文件/缺失路径逐项核对不变，见 `primary-preservation.json`。
- 文档本地链接、JSON 解析和 `git diff --check` 通过。此为 Git 元数据及文档维护；清理阶段没有重新运行产品测试或触发 CI，后续推送见下节。
- 手工 UI / 原生宿主验收：未执行，本次没有产品变化。

## 当前状态与后续

- 本地仅保留 `main` 与 `feat/reading-workspace-20261024`。后者仍由主 checkout 使用，有 2 个不在 main 提交历史中的文档提交记录和 41 项既有改动，未清理或整合。
- 两个工作树都保留：`E:/gitclone/RSS-Reader` 与 `E:/gitclone/RSS-Reader-worktrees/native-back-policy`。后者检出 main，目录名称不表示还在使用同名分支；`target/` 的历史验收资料保留。
- 清理交付阶段仅本地提交；后续推送见下节。没有删除远端引用、打 tag 或发布。
- 如需恢复某个本地分支，可从仍保留的同名远端引用创建跟踪分支，例如 `git branch --track codex/settings-rule-boundary origin/codex/settings-rule-boundary`；完整历史也仍被 main 引用。

## 清理记录推送补记

- 用户随后明确授权将本次清理记录推送到 `origin/main`；本补记随原记录 `09bb4bbf` 一并推送。
- 推送前重新获取远端 main，确认仍为 `95863274`，待推送的原提交只涉及 4 个文档/回执文件；本补记只更新维护状态与本交接。
- 推送使用普通 `git push origin main:main`；远端提交身份与主 checkout 保全核对记录在 `target/local-branch-cleanup-push-20261031/`。原清理回执的 `push: false` 描述清理完成时的状态，保留原始证据。
- 本次不重跑产品测试；文档检查与推送回执不能替代新提交的远端 CI 结果。

## 给下一位 Agent 的备注

以[维护状态](../current-status.md)中的本次快照接续，不再使用 PR #57 时的旧分支数量。其他 session 的工作树和主 checkout 改动继续按原归属保留；远端分支清理需另行明确范围。
