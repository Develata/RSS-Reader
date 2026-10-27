# 目录与 Web release 推送、CI 回归修复及验证

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前产品 HEAD：`5ab77f5afdd07a29b1de36def300b9b1ce6d7eed`
- 相关 commit：`c22a58d9..de7c54af` 共 8 个已推送提交；CI 修复 `5ab77f5a` 已推送。
  本次结果补记随其后的仅文档提交保存，可用 `git log -- docs/handoffs/2026-10-27-directory-push-ci.md` 定位。
- 相关 tag / release：N/A；未合并、打 tag 或发布。
- 状态：`validated`，本地验证及修复提交的 24 项远端 CI 均通过。

## 工作摘要

用户授权推送并检查 CI。已正常 fast-forward 推送当前分支的 8 个提交到 origin，
通过远端引用回读确认 head 为 `de7c54af`。没有提交其他任务的 dirty / deleted / untracked 内容。

首轮 CI 的五套主题均停在同一个下拉刷新测试。定位后修正了测试的固定触摸起点；
随后完整本地回归暴露出旧版 Atlas Reader 搜索框宽度不足，也按既有换行契约修复。

## 影响范围

- 推送内容：目录布局、目录模型与渲染整理、相关设计记录、Web release 构建参数及优化失败守卫。
- CI 修复：`scripts/browser/pull_refresh_contract.mjs`、`rssr_small_viewport_assertions.mjs`、
  `assets/styles/shell.css`；没有改变 Rust 下拉刷新的触发与排除规则。
- 平台：CI 中实际选择的 Linux 原生模块、Web、Windows/Linux release runner、Android 构建。
- workflow：使用既有 `ci.yml`，没有修改 workflow。

## 关键操作

- `git fetch origin feat/reading-workspace-20261024`：与远端没有分叉，本地领先 8 个提交。
- `git push origin HEAD:refs/heads/feat/reading-workspace-20261024`：成功。
- `git ls-remote --heads origin refs/heads/feat/reading-workspace-20261024`：回读完整 SHA 与本地一致。
- [PR #32](https://github.com/Develata/RSS-Reader/pull/32) 为 draft，目标 `main`；GitHub 回读
  `mergeable=false`，因此没有自动创建本次 pull_request 检查。没有为触发 CI 擅自解决合并冲突。
- 手动运行既有 CI：`gh workflow run ci.yml --repo Develata/RSS-Reader --ref feat/reading-workspace-20261024`。

### CI 失败根因及修复

- 原测试在 360px 视口固定从 `(180, 145)` 开始触摸。现场命中测试确认它落在“仅已读”按钮上，
  属于正确被产品排除的控件操作。五套主题均在 `pull_refresh_contract.mjs:140` 等待 armed 超时。
- 共用的 `dragPull` 从真实可见标题或文章元信息计算坐标，并检查实际命中元素不属于交互控件；
  刷新提示可能短暂遮挡标题，因此按命中条件等待，仍通过 CDP 发出真实触摸事件。
  保留多指、取消、80px 阈值及 DOM 释放边界测试；短拖动必须进入 pulling，避免 idle 假通过；
  非顶部测试确认页面确实滚离顶部。
- 旧 Atlas Reader 的搜索框只有 130px。窄导航原来的 `100%` flex basis 配合 `-48px` margin
  使搜索框能与已换行的图标挤在同一行。basis 改为 `calc(100% + 48px)`，让搜索独占完整行宽，
  对齐既有主题契约，不修改旧主题身份或放宽 140px 验收阈值。

## 验证与验收

- [CI 40522262551](https://github.com/Develata/RSS-Reader/actions/runs/40522262551)
  - event：`workflow_dispatch`
  - head SHA：`de7c54af85dc611388e383b61361db4bd5a4472a`
  - 最终状态：failure；24 个 job 中 18 个通过，五套 Web UI 主题及汇总 job 失败。
  - 通过项包含七个原生 crate 的检查与测试、Web release 构建、Android 构建、
    三套 Wasm 浏览器契约、Windows/Linux release runner、格式及工具检查。
- 下载首轮 `ci-ui-default` 与 `ci-web-public` artifacts 至 `target/ci-push-20261027/run1/`。
- 原坐标命中复现：`run_static_web_small_viewport_smoke.sh --release --skip-build --port 18110`
  加临时命中诊断，报告 `BUTTON` / `aria-label="仅已读"`；诊断代码已由正式定位 helper 替代。
- 默认与 Atlas 本地完整回归（修正触摸起点后）：下拉刷新断言通过；继续运行时均在旧版 Atlas
  Reader 130px 搜索框失败。证据：`target/ci-push-20261027/local-fixed-{default,atlas}-v2/`。
- 搜索行宽修复后的 Web release：`target/release-ui-tools/dioxus-0.7.9/dx.exe build
  --platform web --package rssr-app --release --locked --debug-symbols false` 通过，67.27 秒，
  日志没有 ERROR；`target/ci-push-20261027/release-after-ci-fixes.log`。
  当前 index 引用的 Wasm 为 `rssr-app_bg-dxhc19a0d6265d0f.wasm`，4,723,657 字节，
  SHA-256 `106a0f5aa66533435bb0cdd05aee0e1d6121b31d741e32dd09322238f3094533`。
- `bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build --port 18110
  --log-dir target/ci-push-20261027/local-final-default`：通过，337 项断言。
- 同一命令使用 `--port 18111 --preset atlas-sidebar --log-dir target/ci-push-20261027/local-final-atlas`：
  通过，178 项断言。两轮均涵盖旧 Atlas 的 Entries / Reader 搜索；搜索输入实际为 174×44px。
- 两个修改的 `.mjs` 通过 `node --check`；任务范围内 `git diff --check` 通过。
- [第二轮 CI 40528971172](https://github.com/Develata/RSS-Reader/actions/runs/40528971172)：
  `workflow_dispatch`，attempt 1，head `5ab77f5afdd07a29b1de36def300b9b1ce6d7eed`，
  `completed / success`；24/24 个 job 全部通过，包含默认、Atlas、Newsprint、Midnight Ledger、
  Amethyst Glass 五主题及最终 `lint-and-test` 汇总。
- `git ls-remote` 回读的分支 SHA 与上述修复提交一致。
- 上一轮本地优化构建和 172 项浏览器断言见
  [wasm-opt 修复记录](2026-10-27-wasm-opt-release-build.md)；它们不代替本次远端 CI。
- 本次不新增人工 UI / Android 实机验收。

## 结果

- 原有八个提交和后续修复均推送完成；首轮 CI 暴露的问题已经闭环，修复提交的 24 项 CI 全通过。
- PR #32 仍为 draft，GitHub 回读 `mergeable=false`；CI 成功不代表可直接合并。

## 风险与后续事项

- 本次手动 CI 验证分支 head，不包含与当前 main 合并后的结果。PR 冲突仍需单独处理。
- 未触发 Release 或 Pages 部署，没有扩大为发布操作。

## 给下一位 Agent 的备注

- 以 run ID、attempt 与完整 SHA 绑定结果，不使用旧分支检查替代本次。
- 工作区原有删除、修改与未跟踪文件均保留。
