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

### 1. `test_webdav_local_roundtrip` 的 loopback 端口权限限制

- 受影响入口：
  - [test_webdav_local_roundtrip.rs](../../crates/rssr-infra/tests/test_webdav_local_roundtrip.rs)
  - `cargo test --workspace`
  - `cargo test -p rssr-infra --test test_webdav_local_roundtrip`
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
  - 在允许 loopback 端口绑定的环境中单独复跑 `cargo test -p rssr-infra --test test_webdav_local_roundtrip`

### 2. 纯静态 Web 路径的 CORS 限制

- 受影响入口：
  - `dx serve --platform web --package rssr-app`
  - 纯静态 bundle 的浏览器直连路径
  - Web 端 feed 添加、刷新
- 触发环境：
  - 浏览器直接访问不开放 CORS 的 RSS / Atom 源
- 现象：
  - 某些 feed 在 Web 端无法添加或刷新
  - Console 可能出现跨域相关报错
- 为什么不算代码回归：
  - 这是浏览器安全模型边界，不是本仓库业务逻辑独有问题
  - 同样的 feed 在 `rssr-web` 同源代理模式下通常可成功导入或刷新
- 规避方式 / 复验方式：
  - Web smoke 优先使用已知开放 CORS 的 feed 源
  - 需要验证跨源抓取时，优先通过 `rssr-web` 的同源代理模式复验

普通跨源 `<img>` 显示不必然要求 CORS。正文图片加载失败应分别检查响应、HTTPS 混合内容、防盗链和网络策略；不能由跨源 URL 直接推断 CORS 失败。

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

2026-09-26 的同仓库 WSL/Linux 已通过三个真实 wasm browser harness，使用独立 driver 与 `CHROMEDRIVER_REMOTE=http://127.0.0.1:9518`；见 [集成复验](../handoffs/2026-09-26-integration-revalidation.md)。端口是该次记录，不要求其他环境照搬。

复验时检查 Chrome/driver 版本、实际监听地址、端口占用、沙箱权限与代理环境，并在隔离 profile 下重试 `scripts/run_wasm_contract_harness.sh`。若需要独立 driver，只启动并清理本次进程。仍无法启动时记录完整日志，浏览器契约标为未运行；`cargo test ... --no-run` 只证明可编译。也可使用相同版本的 Linux CI 环境验证。

### 5. Fake-IP DNS 与 feed 代理的 SSRF 防护

- 受影响入口：`scripts/run_rssr_web_proxy_feed_smoke.sh`、部署壳 `/feed-proxy`。
- 现象：本机网络把公共域名解析为 `198.18.0.0/15` 测试地址；代理返回 HTTP 400，提示禁止代理内网或本地地址。可用 `getent ahostsv4 <域名>` 核实实际解析结果。
- 该网段被代理的现有地址检查明确拒绝；不能把此失败记为真实远端 feed 验收通过，也不应为了 smoke 放宽 SSRF 防护。
- 复验：在不使用 Fake-IP DNS 的网络中运行相同脚本。登录、同源 fixture 添加/刷新可继续通过 `scripts/run_rssr_web_browser_feed_smoke.sh` 验证，但不替代真实远端代理链路。
- 2026-09-26 的 WSL/Linux 复现与证据见 [发布准备交接](../handoffs/2026-09-26-push-and-release-readiness.md)。

## 与 handoff 的关系

- 本页负责长期维护的环境限制索引
- `docs/handoffs/` 负责记录“某次工作在哪个环境里遇到了哪条限制”
- 新出现的环境现象先记录本次证据；只有可复用的诊断结论才更新本页，不因一次失败新增通用豁免规则
