# 环境限制索引

这份文档记录当前仓库中“已知会影响验证结果，但不应直接判定为代码回归”的环境限制。

它的目标是：

- 帮助后续验证时先区分环境问题和代码回归
- 给每个限制提供可复验、可规避的处理方式
- 避免同一个限制反复写进 handoff 却没有长期索引

## 使用规则

- 先记录实际错误与失败阶段，再判断原因。仅当日志或控制实验支持环境原因时标记 `env-limited`；条目名称相同不构成证据，也不算通过。
- `env-limited` 结论必须同时记录：
  - 日期
  - 验证环境
  - 失败入口
  - 复验方式
- 如果后续发现某限制已经不再成立，应更新本页，而不是只在新的 handoff 里零散备注

## 限制项

### 1. `test_library_webdav_local_roundtrip` 的 loopback 端口权限限制

- 受影响入口：
  - [test_library_webdav_local_roundtrip.rs](../../crates/rssr-infra/tests/test_library_webdav_local_roundtrip.rs)
  - `cargo test --workspace`
  - `cargo test -p rssr-infra --test test_library_webdav_local_roundtrip`
- 触发环境：
  - 受限沙箱
  - 禁止本地 loopback 端口绑定的执行环境
- 现象：
  - 测试在本地临时 HTTP 服务监听阶段失败
  - 全量测试中只剩这一项报错，容易被误认为 infra 或 WebDAV 逻辑回归
- 归因条件：
  - 实际失败发生在 `bind` 并明确返回权限拒绝；允许绑定后相同版本测试可通过。
  - 若已进入 HTTP 或数据断言阶段，不得仅凭旧记录归为环境问题。
- 规避方式 / 复验方式：
  - 在受限环境下，将该项明确标记为 `env-limited`
  - 在允许 loopback 端口绑定的环境中单独复跑 `cargo test -p rssr-infra --test test_library_webdav_local_roundtrip`

### 2. 正式 Web 的同源 API 与服务器网络

正式 Web 不再从浏览器抓取 feed；页面必须与 `rssr-web` 的 `/api/v1` 同源。单独启动静态服务或 `dx serve` 后的 API 404 不能归因为远端 CORS，按 [Web 部署](../deployment/web.md#本地验证部署态) 启动匹配的服务器。

旧版浏览器直连 feed 的 CORS 限制保留在历史记录中。Pages 演示统一禁用在线订阅与 WebDAV，不以开放 CORS 的 feed 作为绕过方式。

普通跨源 `<img>` 显示不必然要求 CORS。正文图片加载失败分别检查响应、HTTPS 混合内容、防盗链和网络策略；不能由跨源 URL 直接推断 CORS 失败。

### 3. WSLg 宿主窗口行为问题

- 受影响入口：
  - Desktop 手工验收
  - `cargo run -p rssr-app` 在 WSL Ubuntu + WSLg 图形宿主下的窗口交互
- 触发环境：
  - WSLg
  - 特定宿主窗口管理行为
- 现象：
  - 标题栏右上角最小化、最大化、关闭按钮无响应
- 归因边界：
  - 主功能可用不能排除窗口层代码问题；需用相同构建在其他宿主复验，或提供窗口管理器侧证据。
- 规避方式 / 复验方式：
  - 记录为宿主环境备注，不单独阻塞主线功能验证
  - 如需确认是否为应用回归，应在非 WSLg 桌面环境复验相同步骤

### 4. ChromeDriver 启动或绑定异常

过去曾在 WSL2 遇到 `bind() failed: Cannot assign requested address (99)` / `driver failed to start`，这是具体运行环境的故障，不是 WSL 的普遍限制。

2026-09-26 的同仓库 WSL/Linux 已通过三个真实 wasm browser harness，使用独立 driver 与 `CHROMEDRIVER_REMOTE=http://127.0.0.1:9518`；见 [集成复验](../handoffs/archive/2026-09/2026-09-26-integration-revalidation.md)。端口是该次记录，不要求其他环境照搬。

复验时检查 Chrome/driver 版本、实际监听地址、端口占用、沙箱权限与代理环境，并在隔离 profile 下重试 `scripts/run_wasm_contract_harness.sh`。若需要独立 driver，只启动并清理本次进程。仍无法启动时记录完整日志，浏览器契约标为未运行；`cargo test ... --no-run` 只证明可编译。也可使用相同版本的 Linux CI 环境验证。

### 5. Fake-IP DNS 与服务器出站策略

- 受影响入口：正式 Web 的服务器订阅抓取、发现、图片本地化与 WebDAV；默认 `public_only` 策略拒绝保留地址。
- 归因需要本次证据：从服务器所在环境解析实际目标，并核对 `outbound.json` 与错误日志。返回 `198.18.0.0/15` 等地址时，按策略拒绝不是远端站点已通过验收；不要为 smoke 放宽规则。
- 复验：使用正常 DNS 的环境，按 [手工 Smoke](./rssr-web-browser-smoke.md) 重跑同一远端地址。`bash scripts/run_server_web_regression.sh --suite feed-smoke` 只验证本地夹具流程，不替代公网可达性。
- 2026-09-26 的旧代理路径复现保留在 [发布准备交接](../handoffs/archive/2026-09/2026-09-26-push-and-release-readiness.md)，不能作为新版服务器的本次运行结果。

## 与 handoff 的关系

- 本页负责长期维护的环境限制索引
- `docs/handoffs/` 负责记录“某次工作在哪个环境里遇到了哪条限制”
- 新出现的环境现象先记录本次证据；只有可复用的诊断结论才更新本页，不因一次失败新增通用豁免规则
