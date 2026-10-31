# 合并后审查：响应取消不能充当 EOF

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/workspace-response-cancel`
- 审查基线：main `c2f8cca3802a695690d41aca97fd135b0eb94623`
- 相关 commit：`ac15b77b4e1c8783544e2674b1ff6a061981e4a6`（`test(workspace): reject cancellation before response EOF`）
- 相关 tag / release：无
- 状态：`validated`（最终 PR head 检查通过，PR #56 已合并；新 main 结果见[合并交接](./2026-10-31-workspace-cancel-merge.md)）

## 审查结论

**P2，已复现并修复：测试观察器把提前取消产生的 `done=true` 误判为完整读取。** 服务器声明 20 字节响应，只发送前 4 字节 `true`；应用读到这些字节后调用 `reader.cancel()`，挂起的 `read()` 正常返回 `done=true`。旧观察器没有记录此取消，因此完整布尔响应门禁返回 `true`。释放 reader 后调用 `stream.cancel()` 再重新取得 reader，也能以同样方式制造关闭状态。此问题会削弱偏好保存回归的证据，不是已确认的产品保存缺陷。

除此之外，本轮在审查范围内未发现需要修改的产品问题。正文选择函数与迁移前基线 `801f6337` 逐字相同，runtime 从 `execute` 起的代码也相同；唯一调用方、私有可见性和共享清洗依赖均保持预期，未恢复对页面 `support.rs` 的反向依赖。

## 工作摘要与影响范围

按用户“check, review and fix”复核 PR #54 的正文模块、独立规则测试和 PR #55 的测试观察器、保存完成门禁及其 CI 装配。继续使用指定 E 盘工作树，主 checkout 的其他任务改动保持原样。

变更仅在 `scripts/browser/workspace_response_probe.cjs` 及其两个测试文件，另补交接与当前状态；没有修改 Rust、产品 JS、CSS、保存协议、数据库、依赖或 workflow。

## 关键修正

- 被动记录 reader 和 stream 的 `cancel()` 调用；若当时尚未观察到 EOF，锁定 `cancelledBeforeEof`，后续关闭通知或重复取消不能清除该事实。
- 只有这类提前取消阻止完成判定；实际完整读取后取消仍可通过。仍要求唯一匹配 payload、成功 HTTP、完整 JSON 布尔值及无读取错误。
- 包装继续转发原始参数、`this` 和返回的 promise，不新增响应读取、不克隆响应、不替换请求或页面状态。
- 清理仅恢复当前观察器仍持有的方法，保留后来安装的包装。
- 增加真实 HTTP/Chromium 的 reader 提前取消、stream 提前取消、EOF 后取消三项；工具回归同时约束重复取消、返回 promise 身份和清理归属。

## 验证与验收

原始资料：`target/postmerge-review-20261031/`；[精简回执](../testing/evidence/2026-10-31-workspace-response-cancel-review/receipt.json)保存复现事实、源码和日志散列及实际输入身份。

| 检查 | 实际结果 |
| --- | --- |
| 原有 Node 工具回归 | 基线 14/14，说明旧用例未覆盖这条取消路径 |
| 旧观察器的真实浏览器反例 | Chrome 149 明确复现：声明 20 字节、收到 4 字节、取消后 `done=true`，错误判为完成；Chrome 155 新增 `reader-cancel` 用例同样失败 |
| 新增用例先于修复运行 | Node 6 项中 2 项失败；浏览器先通过原 10 项，再在取消反例上失败 |
| `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs scripts/browser/workspace_response_probe.test.cjs` | 修复后 16/16 |
| `node scripts/browser/workspace_response_probe.browser.cjs` | Windows Chrome `155.0.8059.40`，13/13，页面异常 0；两种提前取消均拒绝，EOF 后取消通过 |
| `node scripts/browser/rssr_workspace_acceptance.cjs` | 同一 Chrome、隔离正式 Web 资料库，18/18，页面异常 0；包含两组偏好写入排序及保存后重载 |
| `cargo test -p rssr-app --test reader_rules --locked` | 35/35，含同一生产正文选择源码和真实 HTML 清洗 |
| `python target/postmerge-review-20261031/verify-source.py` | 正文选择函数、runtime 执行代码与迁移前实现一致；私有模块依赖符合边界 |
| `python target/postmerge-review-20261031/verify-local-inputs.py` | 现有 server、fixture 和 29 个 Web 文件散列匹配原构建；产品源码与本次 main 基线相同，未冒称重新构建了原生包 |
| 三个 JS 文件 `node --check`、文档链接/JSON、`git diff --check` | 通过 |
| `python target/postmerge-review-20261031/verify-primary.py` | 主 checkout 的 HEAD、分支、status、index 和 41 个变更/缺失路径与开工快照相同 |

最终 head `ac15b77b` 的 [CI 41197256449](https://github.com/Develata/RSS-Reader/actions/runs/41197256449) 首轮 36/36，[Pages 41197256425](https://github.com/Develata/RSS-Reader/actions/runs/41197256425) 构建成功、PR 部署按规则跳过。下载并核对 artifact `13041164534` 的 SHA-256：Linux Chrome `156.0.8078.80` 的适配器 13/13、工作区 18/18、Reader 6/6；实际构建 `dev-faa89498c3e7` 对应 tree 与 head 相同的 PR 合成合并提交。精确身份保存在同目录的 `final-ci.json`、`final-workspace-artifact.json` 和 `final-state.json`，不复用 PR #55 的绿色结果代替本批验证。

## 风险与后续

修复仍是测试观察器，对当前请求协议和默认 reader 的使用方式有明确假设；协议或读取方式变化时需同步维护。没有借此引入通用网络探针、全局 UI 框架或第二份业务状态。

没有重跑 Windows WebView2、Android 实际宿主、物理 IME、辅助技术或候选原生渲染器。Chrome 的脚本化工作区验收与 Rust 规则通过不能替代这些覆盖。

本批修复经 PR #56 合并，合并身份和新主线检查见[合并交接](./2026-10-31-workspace-cancel-merge.md)。原始失败、历史验收、主 checkout 和原 E 盘工作树全部保留。
