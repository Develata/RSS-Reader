# PR #54 合并、main 核验与接续边界

- 日期：2026-10-31
- 作者 / Agent：Codex
- 接续分支：`codex/workspace-response-completion`
- 产品合并 HEAD：`465fead902f1f5cea423aeeaf1cbba623c9082b1`
- 相关 commit：PR head `b4253a82c6a6d7d6d4d67d92752363f751336687`；合并回执所在提交仅更新文档
- 相关 tag / release：无新 tag 或版本发布
- 状态：`validated`

## 工作摘要

按用户“按建议继续”的授权合并 [PR #54](https://github.com/Develata/RSS-Reader/pull/54)，完成 Reader 正文选择归属的主线整合；核对合并后新触发的 main 检查，另开 [PR #55](https://github.com/Develata/RSS-Reader/pull/55) 调查偏好保存测试的完成信号。两个问题分开留证。

## 影响范围与关键变更

- 合并前再次固定 head `b4253a82`、base `801f6337`，PR CI 36/36，通过 `expected_head_sha` 采用 merge commit 合并。
- 合并 tree 与已验收 head 完全相同：`15c106c497b04ab812dd23c0be242eed837c7726`。没有冲突解决或额外产品改写；正文规则与候选验证见[原交接](./2026-10-31-reader-body-boundary.md)。
- 仍使用 `E:\gitclone\RSS-Reader-worktrees\native-back-policy`；从实际合并提交新建偏好诊断分支，不修改已合并 PR。
- 本记录同步当前状态与长期文档的整合状态，没有扩展原生渲染器、Host 接口、crate 或数据库。

## 验证与验收

完整资料在 `target/reader-body-merge-20261031/`，提交内为[精简回执](../testing/evidence/2026-10-31-reader-body-merge/receipt.json)。各工作流均绑定合并 SHA，首轮成功。

| 检查 | 实际结果 |
| --- | --- |
| [main CI 41190053548](https://github.com/Develata/RSS-Reader/actions/runs/41190053548) | 36/36 成功；完整 jobs 分页核对 |
| 下载 CI Reader Web 产物 | Chrome `156.0.8078.80`，6/6，页面/控制台错误均为 0；`serverBuild=dev-465fead902f1` |
| [Docker 41190053564](https://github.com/Develata/RSS-Reader/actions/runs/41190053564) | 工作流成功；容器 9 项、Linux 包 6 项通过，均绑定 `git-465fead902f1f5cea423aeeaf1cbba623c9082b1` |
| [Pages 41190053455](https://github.com/Develata/RSS-Reader/actions/runs/41190053455) | 构建和部署成功；下载 smoke 回执 `pass`，内存 SQLite、重载重置 |
| 四份 ZIP 证据 | 下载后逐一核对 GitHub SHA-256；ID、摘要与运行身份见精简回执 |
| `python target/reader-body-merge-20261031/verify-primary.py` | 主 checkout 的 HEAD、分支、status、index 和 41 个变更/缺失路径与开工快照相同 |

没有重跑 Windows WebView2、Android 实际宿主、物理键盘、IME、辅助技术或候选原生渲染器。此前候选的原生结果保留原提交归属，不写成合并后重新验收。

## 结果、风险与后续

PR #54 已合并，主线 CI、Docker 和 Pages 均通过。正文选择提取在此收口。偏好测试历史失败没有因本次 main 通过而结案；实际响应读取与 DevTools 事件的后续诊断见[独立交接](./2026-10-31-workspace-response-completion.md)。

## 给下一位 Agent 的备注

继续以[维护状态](../current-status.md)和[正文渲染契约](../design/reader-rendering-boundary.md)为入口。当前有 11 个本地分支、10 个远端分支和 2 个工作树；本批没有清理分支或证据。主 checkout 仍为 `feat/reading-workspace-20261024` / `020f9d4031febc709cf1fdca2e7360698ba19f26`，其中另一任务的改动不纳入任何提交。
