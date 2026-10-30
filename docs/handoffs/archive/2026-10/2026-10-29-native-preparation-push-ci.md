# 原生界面前置分支推送与 PR CI

> 历史推送回执，2026-10-30 从 `020f9d40` 补收归档，内容对应初始 head `58157a1a`。PR #40 后续已完成冲突处理并合并，见[最终回执](../../2026-10-29-pr40-final-review-merge.md)。本记录中的草稿、冲突及旧架构状态只在当时成立；本次没有新增推送、CI 或设备验收。

- 日期：2026-10-29
- 作者 / Agent：Codex
- 本地分支：`feat/reading-workspace-20261024`
- 开始 HEAD / 本次推送：`58157a1a90796f8a17624bf9958b0be75bb15bf4`
- 远端新分支：`codex/native-ui-preparation-20261029`
- PR：[草稿 #40](https://github.com/Develata/RSS-Reader/pull/40)，base 为 `main`
- 相关 commit：本交接为推送后的本地记录，生成时 `commit: pending`；不在本次已推送 SHA 中。最终 SHA 记入本机 `final-receipt.json`。
- 相关 tag / release：无
- 状态：`validated`（已推送分支 CI 全部通过；PR 仍为草稿，存在主线冲突）

## 工作摘要

用户授权推送一次，并在 PR 中检查 CI。推送前发现远端同名分支已经随 [PR #32](https://github.com/Develata/RSS-Reader/pull/32) 合并到 v0.2 主线，当前本地工作与它分叉。因此将本地成果推到独立 `codex` 分支，建立草稿 PR；原远端分支没有被覆盖。

## 影响范围

- 远端：新分支、草稿 PR #40、一次 `CI` 手动运行。
- 本地：新增本交接和 `target/native-prep-pr-ci-20261029/` 中的检查记录；没有新增生产代码修改。
- 平台：CI 按已推送分支自己的 workflow 执行；不代表当前 main 合并结果或完整设备验收。

## 关键操作与边界

### 分支和 PR 身份

- 推送前远端 `feat/reading-workspace-20261024` 为 `30321e5b0643fbeabb90d4db14cde52220305ddb`，其 PR #32 已合并、关闭。双方共同基线是 `6109380b1563ddb1a8812a654980729a9eb9ac3b`；旧远端分支独有 405 个提交，本地独有 17 个提交。
- 当前 main 核对为 `6a201d5a83efa730c48a966e002d2f76025cfcda`；相对本地为 436 / 17 个独有提交。PR 三点差异为 75 文件、7667 行新增、441 行删除；它与两个分支 tip 的直接差异不是同一概念。
- 只执行一次普通推送：`git push origin 58157a1a90796f8a17624bf9958b0be75bb15bf4:refs/heads/codex/native-ui-preparation-20261029`。随后核对远端 ref 与该完整 SHA 相同。
- PR 创建为 draft，并附加到当前 Codex 对话。既有 dirty / staged / untracked 内容不进入推送。

### 冲突与 CI 类型

`git merge-tree --write-tree --messages HEAD origin/main` 只分析合并，没有应用到工作树或 index。发现 20 个文件冲突：15 个 Rust 页面/装配文件、3 个设计文档和 2 个浏览器验收脚本；GitHub 返回 `mergeable: false`。

主线现用 `rssr-api::Backend`、rusqlite 单库、正式 Web 服务器存储及 `SettingsPatch` 逐字段编辑/保存；当前分支仍沿用七 crate 中的 host、SQLx 双库、BrowserStore 和整份草稿保存规则。它们存在实质语义差异，不能机械选择某一侧来消冲突。本次没有实施迁移或改写这些边界。

PR 创建后当前 head 的 checks 为空。调用分支已声明的 `workflow_dispatch` 手动触发 `CI`：

```powershell
gh workflow run ci.yml --repo Develata/RSS-Reader --ref codex/native-ui-preparation-20261029
```

运行：[40864200130](https://github.com/Develata/RSS-Reader/actions/runs/40864200130)，事件 `workflow_dispatch`，head 为 `58157a1a90796f8a17624bf9958b0be75bb15bf4`。检查已经出现在 PR #40 的相同 head 上。这是**分支验证**，不是 `pull_request` 合并提交验证；Pages 的 PR 自动构建未执行。

## 验证与验收

### 自动化与远端检查

- 用户授权、Git remote、HEAD、推送范围、旧 PR 合并状态及远端 ref：已核对。
- 本批新 CI：`40864200130` 在 2026-10-29 11:17:10–11:24:16 UTC 执行，最终 `completed / success`，24/24 job 全部成功。包括 7 个原生模块、3 个 WASM 浏览器契约、5 个 Web 主题、Windows/Linux 验收运行器、Android smoke、Web smoke、格式/工具检查及 `lint-and-test` 聚合门禁。
- PR 检查读取绑定完整 head `58157a1a90796f8a17624bf9958b0be75bb15bf4`，返回 24 个成功检查、`hasMore: false`；完整 run 回执也核对为 24 项，避免只检查第一页或旧提交。CI 链接、手动事件类型与未完成的主线整合边界已补入 PR 描述。
- 源码本地验收见 [Settings 表单交接](../../2026-10-29-settings-form-feedback.md) 与[审查交接](../../2026-10-29-settings-form-review.md)；这些既有结果不算本轮重新执行。
- 本机证据目录：`target/native-prep-pr-ci-20261029/`。保存初始状态、17 个提交、合并分析、推送 ref 与精确 head 的 CI 回执。缺失时不得仅凭本记录补写通过。

### 手工与设备验收

未执行。本次操作是推送和 CI 检查，没有新增桌面/Android/真实输入法验收。

## 结果

独立分支与草稿 PR 已建立，精确推送 head 的 CI 24/24 全部通过。PR 因主线冲突不能合并，不应把分支 CI 结果称作主线整合通过。

## 风险与后续事项

- 若要合入 main，应从当前 main 在隔离分支上迁移有价值的页面纯 Rust 规则，保留主线 Backend、单库和逐字段保存语义，重新适配测试；不能恢复已退役的 BrowserStore 或整份设置覆盖来消冲突。
- 这涉及本批原先明确未覆盖的主线语义与适配工作，按 `AGENTS.md` 的架构护栏，需先明确范围再开展。本次仅把成果推送为可检查的草稿 PR。
- 本交接在推送后本地提交，不追加第二次推送；CI 对应的提交始终以上述完整 SHA 为准。

## 给下一位 Agent 的备注

- 当前本地 checkout 仍是原分支；新 PR 的远端分支名称如上，不要向已合并的旧远端分支 force push。
- 任务外技能/spec 删除、两个 10/03 handoff 修改及已有 untracked 资料保留。后续不能把它们连同迁移暂存。
- 阅读 CI 时绑定完整 head SHA，区分手动分支检查、PR 合并提交检查和发布检查。
