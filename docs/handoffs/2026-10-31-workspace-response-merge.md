# PR #55 合并与 main 验收

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`main`，复用 E 盘接续工作树
- 合并 HEAD：`79d8711d5d0752f631ff33da95d1ca86b9ef518f`
- 相关 commit：PR head `bf42dc325cf5c19053a51dae2bebc4373d5b67f4`；本记录所在提交仅更新交接文档
- 相关 PR：[PR #55](https://github.com/Develata/RSS-Reader/pull/55)
- 相关 tag / release：无新 tag 或版本发布
- 状态：`validated`

## 工作摘要与影响范围

将偏好保存测试的完成门禁修正整合到 main，核对合并提交触发的检查并更新当前状态。复现与实现细节见[修复交接](./2026-10-31-workspace-response-fix.md)。本次合并限于浏览器测试、既有 CI 作业和文档；Rust、产品 JS、样式、API、数据库和锁文件保持不变。

## 关键变更

- 合并前固定 head `bf42dc32`、base `465fead9`；PR CI 36/36，下载的工作区产物中浏览器适配测试 10/10、工作区 18/18、Reader 6/6。没有待处理 review 或 review thread，不将此表述为外部审批通过。
- 使用 `expected_head_sha` 与 merge commit 合并。实际父提交为上述 base 和 head；合并 tree `1a5f0276aa1b83fe45ce10a7fa46ac113b56d5d4` 与已验证 PR head 完全一致，无冲突解决或额外改写。
- 继续使用 `E:\gitclone\RSS-Reader-worktrees\native-back-policy`，将其中已有本地 main 快进到合并提交。保留原分支引用、全部历史证据和主 checkout 的既有改动。
- 当前状态、待办和正文边界文档同步整合结果；没有继续扩大模块拆分或候选原生渲染器的范围。

## 验证与验收

本轮原始资料保存于 `target/workspace-response-merge-20261031/`；提交内为[精简回执](../testing/evidence/2026-10-31-workspace-response-merge/receipt.json)。下列工作流均绑定合并提交 `79d8711d`，首轮成功。

| 检查 | 实际结果 |
| --- | --- |
| [main CI 41194804755](https://github.com/Develata/RSS-Reader/actions/runs/41194804755) | 36/36 成功，完整 jobs 分页核对 |
| 下载 CI 工作区产物 `13040251560` | Linux Chrome `156.0.8078.80`；真实 HTTP/Chromium 适配器 10/10，完整工作区 18/18（含 `preferenceOrdering`），Reader 6/6；适配器/工作区无页面异常，Reader 页面/控制台错误均为 0，构建为 `dev-79d8711d5d07` |
| [Docker 41194804777](https://github.com/Develata/RSS-Reader/actions/runs/41194804777) | 容器 9 项、Linux 包 6 项通过，均绑定 `git-79d8711d5d0752f631ff33da95d1ca86b9ef518f`；未推送镜像 |
| [Pages 41194804733](https://github.com/Develata/RSS-Reader/actions/runs/41194804733) | 构建/部署成功；下载 smoke 回执为 `pass`，内存 SQLite、重载重置 |
| 四份验收 ZIP | 下载后逐一核对 GitHub SHA-256、产物所属运行和 head；摘要见精简回执 |
| `python target/workspace-response-merge-20261031/write-receipt.py` | 校验合并父提交、tree、改动范围、完整 jobs、产物身份和具体断言结果后生成精简回执 |
| `python target/workspace-response-merge-20261031/check_docs.py`、`git diff --check` | 文档链接/锚点、回执 JSON 与 diff 检查通过 |
| `python target/workspace-response-merge-20261031/verify-primary.py` | 主 checkout 的 HEAD、分支、status、index 和 41 个变更/缺失路径与开工快照相同 |

本记录所在文档提交的推送后检查另存 `target/workspace-response-merge-20261031/final-ci.json`、`final-summary.json` 和 `final-state.json`，与上述合并提交的运行结果分开保存；不为将自身 SHA 写入文档而循环提交。Pages 按既有 `paths-ignore` 不为仅文档推送重复触发。

未重新执行 Windows WebView2、Android 实际宿主、系统 IME、辅助技术或候选原生渲染器验收。普通浏览器、编译和 CI 结果不代替这些覆盖；此前宿主回执保留原提交归属。

## 结果、风险与后续

PR #55 已合并，main CI、Docker、Pages 及下载产物的具体验收均通过。已复现的是 DevTools 终结事件不适合作为应用读取完成门禁的测试假阴性；没有确认新的产品保存缺陷，也不宣称所有偏好问题均已排除。未来请求协议或读取方式变化时，测试观察器须同步维护并保留反例门禁。

Reader 正文选择提取与这一测试问题已收口。后续实现仍由具体复用阻碍驱动；完整原生正文渲染器需要独立验证 HTML/CSS、主题、资源、选择复制、IME 与无障碍兼容。

## 给下一位 Agent 的备注

以[维护状态](../current-status.md)和[正文渲染契约](../design/reader-rendering-boundary.md)为入口。本批不删除分支、工作树或证据；主 checkout 位于 `E:\gitclone\RSS-Reader`，保留在 `feat/reading-workspace-20261024` / `020f9d4031febc709cf1fdca2e7360698ba19f26`。
