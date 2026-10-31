# 偏好保存完成门禁的复现与修正

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/workspace-response-completion`
- 产品基线：main `465fead902f1f5cea423aeeaf1cbba623c9082b1`
- 缺陷复现提交：`afb57356602475ca4ac31204fe7b5c32935ce8d9`
- 相关 commit：本文件所在修复提交（`test(workspace): wait for the application to consume the final save`）
- 相关 PR：[PR #55](https://github.com/Develata/RSS-Reader/pull/55)
- 相关 tag / release：无
- 状态：`validated`（本地通过；远端按最终 head 单独核对）

## 工作摘要

先完成[被动观测与有界对照](./2026-10-31-workspace-response-completion.md)，最终 CI 捕获原失败：应用已读完最终保存的 `true` 和 EOF，DevTools 却只报告 `ERR_ABORTED`。单独修正测试将 `requestfinished` 等同于应用完成的假阴性，不修改生产保存行为。

## 影响范围与复现

- 仅浏览器测试、测试观察器和既有 CI 作业；Rust、产品 JS、样式、API、数据库和依赖锁文件不变。
- [CI 41192079327](https://github.com/Develata/RSS-Reader/actions/runs/41192079327) 首轮 34 成功 / 2 失败，工作区作业 `124511266545` 在“来源 → 时间”组超时，汇总门禁随之失败。
- 最终 PUT 为请求 2：190 ms 收到 HTTP 200，191 ms 报告 `net::ERR_ABORTED`，没有 `requestfinished`；原 reader 已读取 4 字节 `true`、`done=true`，取消时 `doneWhenAborted=true`。
- `reqwest 0.12.28` 在响应流释放后调用 AbortGuard 的 abort；普通成功场景也观察到此行为。反例证明 DevTools 事件不能承担这里的应用完成门禁，不宣称已完整解释 Chromium 内部调度或发现产品保存缺陷。
- 原日志、HTML、截图和 artifact `13038982256` 保留于 `target/workspace-response-20261031/captured-failure/`；ZIP SHA-256 为 `499c634140972394840a203005c719c993a1865dc5a4ae0f8859af53cbbec5e0`。

## 关键变更

- 观察器从请求副本获取完整 payload，与最终 Playwright PUT 的原始 payload 精确匹配；不依赖网络事件到达顺序，不另读或克隆响应，也不替换应用收到的 promise。
- 完成条件为唯一匹配请求、成功 HTTP、原 reader 的 EOF 和完整 JSON 布尔值；读取/发送错误、过早取消、截断的诊断文本、非法 JSON 或重复 payload 均不通过。允许已读完后的取消，DevTools 终结事件继续保留为诊断。
- 保留最终请求必须实际出现、阻塞期间存储初值已经等于目标的反例、真实服务器持久化和重载后的状态断言，15 秒上限不变。
- 新增真实 Chromium/HTTP 的 10 个适配器反例与正例，复用现有 reading-workspace 作业；没有新增常驻 job 或产品调试状态。
- review 发现默认 TextDecoder 会吞掉 BOM，使非法 JSON 看起来像布尔值；先在新适配器用例复现 `bom-body: true !== false`，再保留 BOM 字节语义并通过回归。该修正只涉及本轮测试观察器。

## 验证与验收

原始资料在 `target/workspace-response-20261031/`，摘要见[修复回执](../testing/evidence/2026-10-31-workspace-response-fix/receipt.json)。

| 命令 / 场景 | 实际结果与边界 |
| --- | --- |
| `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs scripts/browser/workspace_response_probe.test.cjs` | 14/14；BOM 保真调整后观察器 4 项再次通过 |
| `node scripts/browser/workspace_response_probe.browser.cjs` | Windows Chrome 155，10/10：未发送、仅响应头、缺少 EOF、读到首块后实际断开连接、HTTP 错误、非法 JSON、BOM、读完后取消、错误 payload、重复请求；页面异常 0 |
| `node scripts/browser/rssr_workspace_acceptance.cjs` | Windows Chrome 155、隔离正式 Web，18/18，页面异常 0；随后 BOM 保真差异由上述 10 项覆盖，最终 CI 再执行完整脚本 |
| `actionlint .github/workflows/ci.yml`、JS 语法、文档链接、`git diff --check` | 通过 |
| 本地构建输入身份 | server、fixture 与 29 个 Web 文件散列仍匹配已验收产物，产品源码与 main 相同，详见 `local-inputs.json` |

最终 head 的 CI、Pages 和工作区产物在推送后核对，精确 SHA 结果保存在 `target/workspace-response-20261031/final-ci.json`、`final-local-remote-state.json` 和 PR 描述，不用复现提交或对照实验的结果代替，也不为将提交 SHA 写入自身而循环提交。

## 结果、风险与后续

该签名的假阴性已有明确反例和有界测试修正；不能扩大为所有偏好保存问题均不存在。没有放宽为“HTTP 200 即完成”或“任何取消都算成功”。未来请求协议或读取方式变化时，观察器需同步调整并继续保留反例门禁。

没有重新执行 Windows WebView2、Android 实际宿主、系统 IME、辅助技术或候选原生渲染器验收。观察器验证属于测试适配层，不作为新的原生 UI 能力证明。

## 给下一位 Agent 的备注

保留主 checkout 的 HEAD、index 与 41 个变更/缺失路径，继续使用指定 E 盘工作树。该工作树含大量忽略的验收证据，不属于自动清理对象。PR #54 已合并；PR #55 的最终 head 应核对绿色检查后再决定整合，不将未修复前的绿色重跑当作此次修复依据。
