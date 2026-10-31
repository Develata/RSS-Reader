# 偏好保存响应完成信号的独立诊断

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/workspace-response-completion`
- 基线：PR #54 合并提交 `465fead902f1f5cea423aeeaf1cbba623c9082b1`
- 相关 commit：被动诊断 `08a5a291`；延迟对照为本记录所在提交
- 相关 tag / release：无
- 状态：`draft`（正在诊断，不宣称根因已修复）

## 工作摘要

PR #54 已按授权合并。随后单列调查 `preferenceOrdering` 在 HTTP 200 后收到 `ERR_ABORTED`、等不到 `requestfinished` 的历史 CI 失败。正文选择提取与本问题分开处理，产品源码不变。

## 影响范围与关键变更

仅浏览器测试及 CI 工具检查：为 workspace PUT 的实际 fetch 响应流增加被动观察；不另外读取或克隆响应，不替换应用收到的 promise，不更改现有断言和 15 秒超时。失败报告补充读取的字节、EOF、读取错误和 abort 时是否已读完。每组场景完成后恢复原函数；普通 Node 测试约束观察器不提前消费、取消不等于读完及清理行为。

## 验证与验收

- Windows Chrome `155.0.8059.40`、隔离正式 Web：原偏好场景及被动观察版本均通过。150 ms 受控延迟的记录明确显示四笔 PUT 都先读到完整 `true` 和 EOF，后触发 abort；本机仍有正常 `requestfinished`，因此尚不能解释 Linux CI 的缺失。
- `reqwest 0.12.28` 的 wasm `AbortGuard::drop` 调用 abort，`bytes_stream` 持有该 guard 到流释放；本项目 `read_body` 正常路径读取到 EOF。该调用链是已核对源码，是否导致历史 CI 失败仍待同环境证据。
- 独立本地 fetch 探针在无/有拦截及 0/150 ms 延迟下未复现完成事件缺失；不能用未复现断言 CI 无问题。
- `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs scripts/browser/workspace_response_probe.test.cjs`：13/13 通过；`actionlint .github/workflows/ci.yml` 与 `git diff --check` 通过。
- 被动诊断 `08a5a291` 的 [CI 41190658010](https://github.com/Develata/RSS-Reader/actions/runs/41190658010)：首轮 36/36；Linux Chrome 156 原场景通过，未复现历史失败。Pages PR build 成功、deploy 按条件跳过。
- 新增有界对照脚本，在相同原断言下各跑一次 150 ms / 500 ms CDP 网络延迟。Windows Chrome 155 两组均通过；暂挂现有 reading-workspace job 收取相同 Linux 环境证据，实验完成后移除 CI 的临时调用，不增加常驻 job。
- 原始证据：`target/workspace-response-20261031/`。延迟对照的 CI 结果将在完成后补充。

## 结果、风险与后续

本提交先补同环境观测，不改变生产保存逻辑。必须区分实际响应未读完与 DevTools 完成事件缺失；保留最终请求身份、真实存储和刷新后状态断言。不得将重复运行成功或扩大超时写成修复。

## 给下一位 Agent 的备注

继续使用 E 盘接续工作树；主 checkout 的 HEAD、index 和 41 个变更/缺失路径保留。主线合并检查记录位于 `target/reader-body-merge-20261031/`，本问题证据独立保存。未重跑 Windows WebView2 或 Android 实际宿主；浏览器测试工具改动不冒充原生宿主验收。
