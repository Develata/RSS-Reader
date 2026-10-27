# 自然目录回归与阅读工作区的主线接入

- 日期：2026-10-27
- 作者 / Agent：Codex
- 工作区：Codex managed worktree `integrate-directory-prs`
- 分支：`codex/integrate-directory-scroll-20261027`
- main 基线：`936930f7384fa91ebeb3acae1ec1513fc1cff097`
- 原 PR #24 head：`d9e157762b76cb254d4fb0c3d239cac3eeb674bb`
- 原 PR #32 head：`6109380b1563ddb1a8812a654980729a9eb9ac3b`
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`draft`，分阶段验证与合并中。

## 工作摘要

用户授权按合并评估建议推进：先将 #24 测试接入 main，再将 #32 的阅读工作区功能接入
现有 v0.2 架构，解决冲突并在验证通过后合并。原工作目录的任务外修改保持不动。

## 影响范围

- #24：Web / 原生目录测试入口与独立合成 fixture，无生产代码变更。
- #32：后续阶段，沿用 main 的 Backend / application / 单资料库边界。
- 本轮不打 tag、不创建正式 release；PR 合并触发仓库既有主线检查。

## 关键变更

### PR #24

- 合并 main 历史，保留两个原始测试 / 证据提交，不重写已推送历史。
- 保持 main 已删除的旧静态 BrowserStore 服务和双库 Python seeder 处于删除状态。
- 自然目录用例通过现有 `ServerPage` / `browser_fixture_library` 使用真实服务器和单资料库。
- 同时保留 main 新增的阅读位置、元信息和本地时间检查。
- 在 `run_server_web_regression.sh` 提供 `--directory-scroll-only`，接入原有 regression suite，
  默认主题和 directory-only 仍执行新增用例。

## 验证与验收

- 两个目录 `.mjs` 的 `node --check`：通过。
- `bash -n scripts/run_server_web_regression.sh`：通过。
- `git diff --check origin/main`：待提交前复核。
- 合并 index 对旧 PR 父提交的 `--check` 报告两项 main 已有的末尾空行；没有为此改写主线历史证据。
- 远端主线架构的完整 CI / Web 回归：推送后执行，结果待补。
- 本阶段不把 2026-10-07 的 Windows 手工证据当作新主线版本的原生验收。

## 结果

- #24 三处文本冲突已解决；等待当前集成提交的回归通过后合并。
- #32 尚未开始集成。

## 风险与后续事项

- 新增滚动用例沿用真实 wheel / scrollbar drag 与逐帧证据，不能用合成 reducer 调用替代。
- #32 的旧 BrowserStore、sqlx 双库、已退役 host 路径不得重新带回主线。
- 本记录随各阶段更新；最后按远端完整 SHA 核对 CI 和 PR 合并状态。

## 给下一位 Agent 的备注

- 工作目录在 `C:/Users/QQ/.codex/worktrees/integrate-directory-prs/RSS-Reader`。
- 生产架构以 main `AGENTS.md`、`docs/design/v0.2-architecture.md` 为准。
