# PR 收尾与历史待办核实

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`codex/handoff-current-audit-20261028`
- 代码基线：`da8521e23b16079294dd43f4cfe9a1f7d5024cdc`
- 相关 commit：`commit: pending`；提交后以本记录所在提交为准
- 相关 tag / release：v0.2.0 已发布；v0.2.1 草稿，未创建 tag
- 状态：`validated`（文档、PR 收尾及跨平台服务包范围）

## 工作摘要

用户批准按建议继续收尾两个 PR、核实历史待办，再确定发布。已合并文档 PR #35 与依赖 PR #34，从合并后的 main 在当前干净 worktree 建立本分支；未操作主检出的任务外文件。

## 影响范围

- 文档：当前维护状态、历史待办清单、设计入口、发布候选和本次交接。
- 远端：合并两个已通过检查的 PR，触发 main 工作流；另运行既有的四平台 Web 服务包验证入口。
- 产品：本轮未修改 Rust、CSS、依赖锁文件或工作流实现；历史正文保留。

## PR 核查与合并

- [PR #35](https://github.com/Develata/RSS-Reader/pull/35)：head `5e44fed68f319b7aa43dc5e53315b0e64f8cd6cd`，37 success / 1 skipped，无冲突；使用 head 校验合并，merge `781a7360278af3b285f79c8997cb039cf6c560ba`。此前 Claude 的只读文档复核及之后索引校正分别保留在原交接，不能写成 Claude 审过所有后续提交。
- [PR #34](https://github.com/Develata/RSS-Reader/pull/34)：head `416f8c07673ddd8bbf1daef8af080552e928b9bf`，37 success / 1 skipped；仅将 `web-packages.yml` 的 setup-python 从 v6 改为 v7。使用 head 校验合并，merge `da8521e23b16079294dd43f4cfe9a1f7d5024cdc`。
- 兼容性：对照上游 [v7.0.0 发布说明](https://github.com/actions/setup-python/releases/tag/v7.0.0)和 [v7 action.yml](https://github.com/actions/setup-python/blob/v7/action.yml)，`pip-install` 已删除，但仓库未使用；当前只传 `python-version: '3.13'`。v6 / v7 均使用 Node 24，目标为 GitHub 托管 Windows / macOS runner。未发现需要修正的输入或运行器配置。
- 普通 PR CI 没有执行这个仅在全平台服务打包时用到的 action，不能将 37 项通过当作该路径的实测。为此运行 `gh workflow run docker.yml --ref main -f all_platforms=true`，固定 source `da8521e2`，run [40677169921](https://github.com/Develata/RSS-Reader/actions/runs/40677169921)。该入口的 `use_existing_package=false`，`SHOULD_PUSH_IMAGE=false`，只验证服务包和容器，不发布 GitHub Release 或 GHCR。

## 历史核实结果

- 扫描 `docs/handoffs/archive/YYYY-MM/*.md` 的 147 份记录，定位 293 个匹配风险、后续或交接标题的段落；该数字是文本检索范围，包含重复/嵌套段落，不是 293 个独立缺陷。
- 优先核对架构、迁移、资源边界、发布，以及相应源码和回归测试，新增[当前维护待办](../maintenance-backlog.md)：7 类待办、10 类已关闭或被替代的旧事项、保留的契约与暂缓方向。
- 确认旧参数上限已有 JSON 参数回归，刷新乱序/重新激活已有票据与 generation，旧双库孤儿窗口由单库原子写替代，当前迁移换行规范化、JSON Feed 模型预检与本地化输出预算均已实现。
- 仍需跟进：HTML 预算盲区、后台图片任务公平性及排队资源、真实设备旧库导入、索引与写成本测量、正式包验收及上游补丁维护。区分源码支持、旧实测与本轮未复现，不把所有历史 TODO 改成“已解决”。
- 上游实时只读核查：rust-url #1163、feed-rs #301 仍为 open；url 标签列表首项 v2.5.8。首次把 #1163 当 pull request 查询返回 404，随后按 issue 正确读回；没有据此判断其不存在。
- B2 文档标题仍写“待确认”，与其正文“已确认”矛盾；改为确认与实施记录，并注明 BrowserStore 是阶段 B 的历史说明，当前平台组合以 v0.2 架构为准。

## 验证与验收

- PR #34 / #35 的确切 head 检查、合并回执：通过；合并后 da8521e2 的 CI 36 success、Pages 2 success、常规 Docker 4 success / 1 skipped，额外四平台包工作流 7 success。回读结果见 [PR 与工作流回执](../testing/evidence/2026-10-28-maintenance/pr-and-ci.json)。
- 已下载四平台服务包验收结果，各 6 checks pass，容器 9 checks pass，均为 `git-da8521e23b16079294dd43f4cfe9a1f7d5024cdc`；三种 native job 的 setup-python@v7 步骤均成功。见[包检查回执](../testing/evidence/2026-10-28-maintenance/package-checks.json)。GitHub artifact digest 作为远端元数据记录，未将 gh 解压的结果说成本机校验过原 ZIP 的字节哈希。
- 本地文档审计：313 份 Markdown、1300 个本地链接，缺失目标 0；标题锚点检查通过，147 份历史正文除归档提示与链接外保持原文；`git diff --check` 通过。
- 本轮未在本机重跑产品单测、原生交互或恶意输入性能测试；远端 main CI 如上单列，源码与已有测试的阅读不冒充新执行结果。
- 扫描明细在本机 `E:/gitclone/RSS-Reader/target/docs-readiness-20261028/historical-followups.json`；不是随仓库交付的完整历史缺陷审计。

## 当前状态、风险与后续事项

- 两个原 PR 已合并；维护清单按当前源码收敛，尚非 147 份历史记录的逐句结案。
- 当前 main 的 Rust / CSS 产品代码与原生验收基线相同；变化为文档和 setup-python 版本。正式包仍须按最终 tag 身份验证。
- v0.2.1 草稿保留发布检查条件。没有通过 Release 的手动入口试跑，因为该入口也会公开发布。

## 给下一位 Agent 的备注

从 `docs/current-status.md` 和 `docs/maintenance-backlog.md` 进入。实施 M01/M02 前先有可重现输入和资源测量，不根据旧的时长或“优化”措辞扩大范围；原始 handoff 与失败证据继续保留。
