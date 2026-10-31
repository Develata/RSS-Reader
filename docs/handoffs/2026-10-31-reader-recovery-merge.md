# PR #57 合并与 Reader 读取恢复主线验收

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`main`，复用 E 盘接续工作树
- 合并 HEAD：`df531fccd67012fa6d114f8cbf926dd428065f0a`
- 相关 commit：PR head `6a39570e079a1b6cd687ed6b2286e62797bbae4a`；本记录所在提交仅更新交接文档
- 相关 PR：[PR #57](https://github.com/Develata/RSS-Reader/pull/57)
- 相关 tag / release：无新 tag 或版本发布
- 状态：`validated`

## 工作摘要与影响范围

按用户授权推送已有分支、创建 PR、完成代码复核和 CI 后合并，再核对新 main。P2 缺陷的复现与有界修复见[读取恢复交接](./2026-10-31-reader-reread-recovery.md)：有效读取恢复后清除过期读取提示，保留当前写入失败或结果未知，不替换正文。

本次 PR 同时收录此前 main 补充回归文档，共 15 个文件、2 个提交；历史 1149 项 workspace 结果仍归修复前基线，不冒充新候选验收。合并后仅补充状态、契约与交接记录，没有继续修改产品实现、数据库、宿主或 workflow。

## 关键变更与 review

- 固定 head `6a39570e`、base `60b4c6f1` 完成代码自审，未发现新的可操作问题。核对提示来源、读取/访问代次、写入门禁、全局恢复信号、正文保持以及浏览器断言的完成条件；自审记录保存在 `review.json`。
- 合并前 GitHub review submissions 和 review threads 均为空，主线无额外 rules。这里记录代码自审和无待处理意见，不表述为外部独立审批通过。
- PR [CI 41204796310](https://github.com/Develata/RSS-Reader/actions/runs/41204796310) 为 36/36 成功；[Pages PR 构建](https://github.com/Develata/RSS-Reader/actions/runs/41204796456)成功，deploy 按 PR 规则跳过。下载验收 ZIP `13043878538` 并核对 GitHub SHA-256，Linux Chrome `156.0.8078.80` 的 Reader 8/8，2 次预期 503、页面异常和非预期控制台异常均为 0。
- PR 实测合并引用 `17eceb98` 与 head、最终合并提交的 tree 都为 `1190f9f23940c0d90b55dd5737b9c9d97127e9d2`。通过 `expected_head_sha` 采用 merge commit 合并，实际父提交为上述 base 和 head，无冲突处理或额外代码改写。
- 仅切换 E 盘接续工作树到既有 main 并快进；主 checkout 仍在 `feat/reading-workspace-20261024` / `020f9d40`。没有清理任何分支、工作树或证据。

## 验证与验收

本轮原始资料保存于 `target/reader-recovery-merge-20261031/`，提交内为[精简回执](../testing/evidence/2026-10-31-reader-recovery-merge/receipt.json)。以下工作流均绑定合并提交 `df531fcc`，首轮成功。

| 检查 | 合并后结果 |
| --- | --- |
| [main CI 41205551977](https://github.com/Develata/RSS-Reader/actions/runs/41205551977) | 36/36 成功，完整 jobs 分页核对 |
| 下载 CI 工作区产物 `13043533662` | Linux Chrome `156.0.8078.80`；适配器 13/13、工作区 18/18、Reader 8/8，构建为 `dev-df531fccd670`；Reader 2 次预期 503，页面和非预期控制台异常均为 0 |
| [Docker 41205551983](https://github.com/Develata/RSS-Reader/actions/runs/41205551983) | 容器 9 项、Linux 包 6 项通过，绑定 `git-df531fccd67012fa6d114f8cbf926dd428065f0a`；镜像推送跳过，非发布验证成功 |
| [Pages 41205551964](https://github.com/Develata/RSS-Reader/actions/runs/41205551964) | 构建、部署成功；下载 smoke 回执为 `pass`，内存 SQLite、重载重置 |
| 四份合并后验收 ZIP | 核对 GitHub SHA-256、所属工作流与 head；ID、摘要与具体结果见精简回执 |
| `python target/reader-recovery-merge-20261031/verify-evidence.py --label main` | 合并父提交、tree、完整 jobs、产物身份、读取恢复与取消/EOF 断言均通过 |
| `python target/reader-recovery-merge-20261031/check_docs.py`、`git diff --check` | 文档链接/锚点、回执 JSON 与差异检查通过 |
| `python target/reader-recovery-merge-20261031/verify-primary.py` | 主 checkout 的 HEAD、分支、status、index 及 41 个变更/缺失路径与开工快照相同 |

GitHub 工作流和 jobs 通过 `gh api --paginate --slurp` 核对完整分页，按精确 SHA 分开记录 PR、合并提交和最终文档提交，不将连接器的首分页当作完整结果。

本记录所在文档提交的推送后检查保存为同目录的 `final-ci.json`、`final-summary.json`、`final-state.json`；不为将自身 SHA 写回文档而循环提交。Pages 按现有 `paths-ignore` 不为仅文档推送重复触发。

未重新执行 Windows WebView2、Android 实际宿主、物理 IME、辅助技术、正式包升级或候选原生渲染器验收。此前 Windows 8 项绑定修复候选，Android 只有编译检查；合并后 CI 不能改写这些历史覆盖边界。

## 当前状态、风险与后续

- PR #57 已合并，合并提交的新 main CI、Docker、Pages 和上述产物验收均通过。没有发布新版本或推送镜像。
- 后续原生界面前置候选是核对 Settings 草稿、逐字段保存和异步回填规则能否独立调用，只处理实际复用阻碍。本批没有启动 Settings 重构，也没有确认其存在新缺陷。
- 主 checkout 的 HEAD、分支、status、index 和 41 个变更/缺失路径与开工快照保持一致；最终提交后继续核对。

## 给下一位 Agent 的备注

先看[维护状态](../current-status.md)与[Reader 渲染契约](../design/reader-rendering-boundary.md)。本轮继续使用 `E:/gitclone/RSS-Reader-worktrees/native-back-policy`，无需新建工作树。保留主 checkout 的既有改动、所有本地验收资料及原分支引用。
