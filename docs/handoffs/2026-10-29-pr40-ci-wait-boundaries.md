# PR #40 CI：验收脚本的等待边界与诊断

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/pr40-main-integration`；PR 推送目标 `codex/native-ui-preparation-20261029`
- 当前 HEAD / 基线：`417012c7179e6e176abecfa0d89a669199551fc5`
- 相关 commit：`50af7c99` 为未变的产品源码；`417012c7` 记录原生验收；本次脚本修正与本记录同一提交，可由本文件 git log 查询
- 相关 tag / release：无
- 状态：`validated`（本地脚本验证；新 head 的 CI 另在 PR 描述记录）

## 工作摘要

Windows 原生验收重试已通过，见[原生交接](./2026-10-29-pr40-native-retry.md)。文档提交 `417012c7` 的自动 CI 在阅读工作区任务达到 15 分钟上限后取消，汇总门禁失败。本次收紧已确认的脚本无限等待点并增加阶段日志，不改产品保存规则、断言或工作流时限。

## 影响范围

- 模块：`scripts/browser/rssr_workspace_acceptance.cjs`、`rssr_entries_lifecycle_acceptance.cjs`。
- 平台：正式 Web / 真实服务器浏览器验收脚本；本地 Windows Chrome，远端 Linux CI。
- 本次不改 Rust 产品源码、锁文件、构建或 workflow；原生验收产物仍对应产品源码 `50af7c99`。
- 使用当前独立工作树，保留其他工作树、分支和用户数据。

## 事实、定位与未证实项

- [CI 40906329094](https://github.com/Develata/RSS-Reader/actions/runs/40906329094) 的 reading-workspace 超时取消，lint-and-test 失败；其余 35 项成功、Pages 部署 1 项预期跳过。
- 下载并核对 [artifact 12911389821](https://github.com/Develata/RSS-Reader/actions/runs/40906329094/artifacts/12911389821)，ZIP SHA-256：`804edd730ba438207c2ebe591b002c9cea10dd7262822bea81c5c340b518e236`，本地文件 `target/pr40-417012c7-reading-failure.zip`。
- 原报告中 workspace 18 个场景全部通过；Entries 完成 selection、sources、queryStability、pendingWrites 后，没有输出 preferenceOrdering 完成记录。环境安装早已结束，后续 suites 尚未开始。
- 已确认：原 `holdWrites` 的自建 `captured` Promise 无期限，`page.setDefaultTimeout(15000)` 不约束它；最终 `Response.finished()` 和场景内直接 fetch 也缺少显式时限。
- 未证实：现有 CI 日志不足以定位卡住的具体 await，也不能证明一定是“未产生请求”。原脚本的三轮原顺序复验（4 倍 CPU 节流）以及 20 轮正常速度快速切换均通过。不能将本次诊断改进表述为已复现并消除全部偶发原因。

## 关键变更

- `holdWrites` 的捕获参数改为按需调用的等待函数，用已有 expect.poll 在 15 秒内断言路由确实捕获写入；无请求时明确失败，finally 继续释放 gate 并移除自身路由。
- 最终保存仍匹配真实 PUT 和最终 all/time 内容，仍要求 HTTP 成功；提前登记 requestfinished 事件，并在 15 秒内确认完成的是同一个 Request，替代无期限的 Response.finished 等待。已完成的响应不会因晚开始观察而漏掉。
- 读取“初始值已等于最终值”的反例仍保留，其 fetch 现在带 15 秒 AbortSignal；保存回执后仍核对存储并刷新验证。
- Workspace / Entries 输出 START 阶段；快速偏好场景记录开始、首笔写入截获、最终响应完成。失败时保留原截图、HTML / JSON 和清理路径。
- 未增加自动重试、固定等待或忽略失败，也未放宽现有断言。

## 验证与验收

### 自动化验证

- 两个改动脚本的 `node --check`、`git diff --check`：通过。
- `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs`：10/10 通过。
- Workspace 全套：18/18 通过，无 pageerror；日志 `target/pr40-bounded-workspace.log`。
- Entries 全套：8/8 通过，pageErrors / consoleErrors 均为空；报告 `target/pr40-bounded-entries/acceptance.json`，日志 `target/pr40-bounded-entries.log`。
- 上述真实服务器验收继续使用已匹配的 Web/server 产物。为避开其他工作树的 8119 端口，临时 launcher `target/pr40-isolated-suite.cjs` 只在进程内将 fixture 端口替换为 9325；执行命令为 `node target/pr40-isolated-suite.cjs workspace` 和 `node target/pr40-isolated-suite.cjs entries_lifecycle`，设置 NODE_PATH / CHROME_BIN / ARTIFACT_DIR。未修改仓库的 fixture。
- 缺失写请求的故障注入：基线保持 pending，补发写请求后才完成；修正后约 15015ms 明确失败且路由已撤除。命令 `node target/pr40-preference-timeout-probe.cjs`，记录 `target/pr40-preference-timeout-probe.json`。该探针验证等待边界，不冒充 CI 原始触发条件的复现。
- 原脚本探索日志：`target/pr40-preference-repro.log`（三轮原顺序）和 `target/pr40-preference-fast-repro.log`（20 轮快速偏好切换），均通过；仅用于说明未复现的范围。

### 手工验收

- 未新增人工 UI 验收，没有 Computer Use。此轮未改变产品代码，不重复原生全套、Rust 全工作区或设备验收；原生通过记录仍固定在 `50af7c99`。

## 结果

本地脚本验证通过；按用户原有授权提交、push 到 PR #40 并核对新 head 的自动 CI。新 head / run / 结果和报告回执在 [PR #40](https://github.com/Develata/RSS-Reader/pull/40) 描述中固定，不将 `417012c7` 的失败或更旧 head 的成功混为当前结论。

## 风险与后续事项

- 若同类停滞再次出现，优先使用新增阶段日志、明确超时错误和失败截图定位具体请求或渲染状态；不要先提高工作流时限或重跑到绿。
- 本次只为已发现的无限等待添加边界，未完成对所有历史验收脚本的超时审计。
- PR 保持草稿；不合并、发布或清理其他工作树。

## 给下一位 Agent 的备注

- 先读本记录与原生验收记录，再读取 PR 当前 head 的检查。
- `target/` 内保留原始失败 artifact、复现日志、临时端口 launcher 和故障注入证据；不纳入产品或 CI 依赖。
