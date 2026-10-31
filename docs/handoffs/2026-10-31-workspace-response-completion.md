# 偏好保存响应完成信号的独立诊断

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/workspace-response-completion`
- 基线：PR #54 合并提交 `465fead902f1f5cea423aeeaf1cbba623c9082b1`
- 相关 commit：被动诊断 `08a5a291`；PR #54 主线交接 `280d55d7`；延迟对照 `c79af82d`；本记录所在提交补验收并移除 CI 临时实验
- 相关 tag / release：无
- 状态：`validated`（诊断工具和有界实验已验证；历史不稳定点仍未结案）

## 工作摘要

PR #54 已按授权合并。随后单列调查 `preferenceOrdering` 在 HTTP 200 后收到 `ERR_ABORTED`、等不到 `requestfinished` 的历史 CI 失败。正文选择提取与本问题分开处理，产品源码不变。

## 影响范围与关键变更

仅浏览器测试及 CI 工具检查：为 workspace PUT 的实际 fetch 响应流增加被动观察；不另外读取或克隆响应，不替换应用收到的 promise，不更改现有断言和 15 秒超时。失败报告补充读取的字节、EOF、读取错误和 abort 时是否已读完，成功记录也保留读取事实。每组场景完成后恢复原函数；普通 Node 测试约束观察器不提前消费、取消不等于读完及清理行为。review 补充了诊断读取/清理失败时保留原场景错误的处理。

## 验证与验收

- Windows Chrome `155.0.8059.40`、隔离正式 Web：原偏好场景及被动观察版本均通过。150 ms 受控延迟的记录明确显示四笔 PUT 都先读到完整 `true` 和 EOF，后触发 abort；本机仍有正常 `requestfinished`，因此尚不能解释 Linux CI 的缺失。
- `reqwest 0.12.28` 的 wasm `AbortGuard::drop` 调用 abort，`bytes_stream` 持有该 guard 到流释放；本项目 `read_body` 正常路径读取到 EOF。该调用链是已核对源码，是否导致历史 CI 失败仍待同环境证据。
- 独立本地 fetch 探针在无/有拦截及 0/150 ms 延迟下未复现完成事件缺失；不能用未复现断言 CI 无问题。
- `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs scripts/browser/workspace_response_probe.test.cjs`：13/13 通过；`actionlint .github/workflows/ci.yml` 与 `git diff --check` 通过。
- 被动诊断 `08a5a291` 的 [CI 41190658010](https://github.com/Develata/RSS-Reader/actions/runs/41190658010)：首轮 36/36；Linux Chrome 156 原场景通过，未复现历史失败。Pages PR build 成功、deploy 按条件跳过。
- `node scripts/browser/rssr_preference_response_diagnostic.cjs`：在相同原断言下各跑一次 150 ms / 500 ms CDP 网络延迟。Windows Chrome 155 与 Linux Chrome `156.0.8078.80` 两组均通过；Linux 的 8 笔 PUT 都读到 `true`、EOF，随后 abort，仍收到正常完成事件。各组保留最终 PUT 身份、阻塞期间的初值反例、真实服务器持久化及重载后状态断言。
- Linux 实验对应 `c79af82d` 的 [CI 41191391042](https://github.com/Develata/RSS-Reader/actions/runs/41191391042)。工作区 artifact `13039560374` 已下载并核对 SHA-256 `69588c64256c7300b9878b363c6059871dae875b5732841ab56fdbd21d1b89df`；精简事实见[诊断回执](../testing/evidence/2026-10-31-workspace-response-completion/receipt.json)。临时 CI 调用已移除；保留脚本供显式诊断使用，不增加常驻 job。
- 本地复用的 server、fixture 与 29 个 Web 文件均重新核对散列，匹配正文提取阶段的已验收产物；产品源码与 main `465fead9` 相同，不把旧构建标识冒充新构建。详见 `local-inputs.json`。
- 原始证据：`target/workspace-response-20261031/`。最终 head 推送后的 CI 单独记录于该目录，不用实验提交的结果代替。

## 结果、风险与后续

已完成观察器、清理保护、有界对照与 review，没有修改生产保存逻辑。本轮没有复现历史失败，因此没有依据放宽 `requestfinished` 门禁或认定浏览器/请求库就是根因。新增 promise 回调也可能改变竞态时序；通过结果不能证明偶发问题已消失。保留原失败证据与待办，后续同签名失败可直接对照实际 EOF、读取错误和取消顺序，再单列修正。

## 给下一位 Agent 的备注

继续使用 E 盘接续工作树；主 checkout 的 HEAD、index 和 41 个变更/缺失路径保留。主线合并检查记录位于 `target/reader-body-merge-20261031/`，本问题证据独立保存。未重跑 Windows WebView2 或 Android 实际宿主；浏览器测试工具改动不冒充原生宿主验收。
