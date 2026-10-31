# PR #53 合并、main CI 与工作区保全

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`main`；复用 `E:\gitclone\RSS-Reader-worktrees\native-back-policy`
- 合并前 main：`0c5e31a6cca607c5ba6f8783b2fc3923c0eb1160`
- 产品合并 HEAD：`ed0714d3ca11365d34ed41ea174284d4eb86ee83`
- 相关 commit：PR head `21f3468a2bd6e0233f9bc1d985ad83a4cb86631f`；本记录所在提交仅补文档与证据
- 相关 tag / release：无新 tag 或版本发布；Pages 沿用 main 自动流程，Docker 非发布验证
- 状态：`validated`（合并提交的主线检查通过；后续文档 head 按精确 SHA 单独核对）

## 工作摘要

按用户授权合并 [PR #53](https://github.com/Develata/RSS-Reader/pull/53)，将 Reader 图片访问隔离和同种标记写入门禁纳入 main。合并后核对新触发的主线 CI，并更新维护状态、文档入口和本记录；不继续下一批正文模块化，也不修改产品代码。

## 影响范围

- 合并内容：Reader 规则、session、图片事件及回归脚本；具体复现、修复和候选三端验收见[原实现交接](./2026-10-30-reader-race-fixes.md)。
- 本轮编辑：`docs/` 下的维护状态、契约头部状态、交接和证据摘要。
- 平台验证：新执行 main 的 CI、Docker 和 Pages 工作流；没有重新安装或操作 Windows / Android 应用。
- 主 checkout `E:\gitclone\RSS-Reader` 仅作只读身份与保全检查，不切分支、不清理、不提交其既有改动。

## 合并与审查核对

- 合并前复核 head 为 `21f3468a`，base 为 `0c5e31a6`，GitHub 为 `CLEAN / MERGEABLE`。PR 的 CI 36/36、Pages build 成功；reviews、inline threads 和 comments 均为空，没有将自审称为外部 reviewer approval。
- 使用 merge commit 方式，并通过 `expected_head_sha` 固定已验收的 head。实际合并提交为 `ed0714d3`，两个父提交分别为上述 base 和 head。
- `git rev-parse ed0714d3^{tree}` 与 `git rev-parse 21f3468a^{tree}` 均为 `1b7fda4c6abfec4035be6d4b9c7e5171cee5a5cd`。没有冲突解决或未验证的运行代码改写。
- 仅接续工作树执行 `git fetch origin main`、`git switch main`、`git merge --ff-only origin/main`。保留全部 9 个本地分支、8 个远端分支和 2 个工作树，不删除分支或历史证据。

## 验证与验收

本轮原始证据位于 `target/reader-race-merge-20261031/`；提交内摘要见[主线验收回执](../testing/evidence/2026-10-31-reader-race-merge/receipt.json)。检查按 source SHA 区分合并提交和后续文档提交，不沿用 PR 的绿色状态代替 main 运行。

| 命令 / 流程 | 结果与边界 |
| --- | --- |
| main `ed0714d3` [CI 41184569359](https://github.com/Develata/RSS-Reader/actions/runs/41184569359) | 36/36 成功；完整 jobs 列表核对，不只读取第一页 |
| CI 的 Reader 正式 Web 回归 | 下载归档并验证 SHA-256，6/6；`serverBuild=dev-ed0714d3ca11`，两个视口均覆盖图片激活后切文、标记慢响应门禁与真实存储一致性，无页面/控制台错误 |
| main [Docker 41184569498](https://github.com/Develata/RSS-Reader/actions/runs/41184569498) | 成功；下载回执为容器 9 项、Linux 包 6 项，均绑定 `git-ed0714d3ca11365d34ed41ea174284d4eb86ee83`；GHCR 登录、tag、push 步骤跳过，非发布检查成功 |
| main [Pages 41184569454](https://github.com/Develata/RSS-Reader/actions/runs/41184569454) | build / deploy 成功；下载 smoke 回执 `pass`，演示仍使用内存资料库、重载重置 |
| `python target/reader-race-merge-20261031/check_docs.py`、`git diff --check` | 提交前检查 Markdown 本地链接/锚点、JSON 与空白错误；统计见本地 `documentation-check.json` |
| `python target/reader-race-merge-20261031/verify-primary.py` | HEAD、branch、status、index 与 41 个变更／缺失路径和开工快照相同 |

候选的本地 app 428 项（含独立规则 27 项）、正式 Web 6/6、Windows WebView2 8/8、Android 真机 4/4 继续归属 `21f3468a` 的[原验收证据](../testing/evidence/2026-10-30-reader-race-fixes/receipt.json)。合并树相同说明产品内容一致，不表示本轮重新执行了真机验收。

后续提交仅改变 `docs/`。推送后将按最终 head 核对 CI/Docker，并在本地证据根写入 `final-main-receipt.json`；不在提交前预报成功，不为将提交 SHA 写入自身而循环提交。Pages 的 `docs/**` 路径过滤不会因这次纯文档提交重新部署。

## 工作区保全

主 checkout 仍为 `feat/reading-workspace-20261024` / `020f9d4031febc709cf1fdca2e7360698ba19f26`。开工时以 `GIT_OPTIONAL_LOCKS=0` 读取状态，保存 index SHA-256、完整 porcelain 状态及 41 个变更／缺失路径的散列；结束时再次比较。另一任务的 JavaScript 审计文档未纳入提交。

## 结果、风险与后续

PR #53 已合并，合并提交的 main CI、Docker 和 Pages 均通过。本轮没有发布新版本、打 tag 或推送 Docker 镜像。物理键盘、系统 IME、辅助技术、macOS/Linux GUI、正式签名 Android 升级等未覆盖范围沿用原记录，不能从主线 CI 推定通过。

下一批仍是 `ui/runtime/reader.rs → pages/reader_page/support.rs` 的正文选择依赖整理，保留现有候选顺序和唯一清洗政策；见[正文渲染边界](../design/reader-rendering-boundary.md#下一批建议)。

## 给下一位 Agent 的备注

先读[当前维护状态](../current-status.md)，再核对 main 与 dirty 状态；工作树目录名不等于当前分支。`verify-primary.py` 只检查并记录主 checkout，不修复或覆盖差异。旧 `target/reader-bugcheck-20261030/` 保存缺陷复现和候选宿主记录，本轮目录保存合并后的远端证据，两者均保留。
