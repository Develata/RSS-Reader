# CI 三个请求收尾超时的调查与驱动修复

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 当前 HEAD：`2b09f16`
- 相关 commit：pending
- 相关 tag / release：无
- 状态：`validated`（本地通过；远端复验待执行）

## 工作摘要

调查 CI `39816994053` 首次默认主题 job `119959031736` 的三个超时。已独立复现并修复 `ApiTraffic` 跨文档等待的缺陷：Chrome 把旧文档放进 BFCache 后，可以延迟发送旧请求的结束事件；驱动却把整个标签页历史中的未结束请求都算进当前页面，导致 HTTP 200 之后仍等待 20 秒。

原始 CI 没有记录请求 ID、loader ID 或结束状态，无法逐条证明三次失败都是这一原因。它们的相同报错与复现一致；此项属于有复现支持的归因，不能把后来重跑通过当作原始三次故障的完整时序证据。

## 影响范围

- 浏览器验收驱动、驱动自身测试、CI `test-tools` 和验收文档。
- 平台：Windows Node 测试；Ubuntu 24.04 Docker + Chrome for Testing 156.0.8078.47。
- 没有修改应用、HTTP 协议、服务器或持久化逻辑，没有发布、打 tag 或合并分支。

## 根因证据与关键变更

### 原始失败

- 源码 `129996af85798c07c623255be88e18a5706426d1`，Chrome `156.0.8078.47`。
- `reader-recovers-without-localizing`、`settings-read-fails-at-start`、`http-backend-only` 都在 `settled()` 报 `GET /api/v1/settings`，每项约 22 秒；24/27 场景通过，意外控制台错误为零。
- 原 artifact SHA-256：`4570f41b6b570e79ef350c66566dec68c1ec98e716bdc8a3f1873cc32e0cb573`。同一源码的失败 job 重跑 27/27，只说明该问题间歇出现。

### 独立真实浏览器复现

本地 HTTP 服务器给 `/api/v1/settings` 回 200，先发送 JSON 的一部分，1 秒后发完。在页面 A 收到响应头后跳转 B，再调用原 `settled()`：开启和关闭 Fetch 拦截都会等待 20 秒并报相同错误。此时等待的是 A 的 loader，B 已是另一个 loader；HTTP 服务器已发完。

随后返回 A，CDP 明确报告 `Page.frameNavigated.type = BackForwardCacheRestore`，原请求的 `Network.loadingFinished` 才到达，原 `settled()` 随即通过。原始事件摘录、完整日志哈希、修复前后及对照结果保存在 [复现证据](../testing/evidence/2026-10-22-ci-timeouts/bfcache-reproduction.json)。

### 修复与保留的约束

- 跟踪主文档 loader（完整导航，不含 Dioxus 页内路由）；只从等待集合中排除已确认离开的主文档，恢复旧文档时重新等待它。未知 loader 和子 frame 仍保守等待。
- 不清空请求历史，不伪造 `done`，历史写入计数不变。
- 当前文档仍等真实结束事件；200 响应头不算完成；20 秒时限、600 ms 连续静默及请求开始/结束时重置静默的规则不变。
- 错误文字和失败 JSON 记录当前 loader、在途 request ID；CI 加入驱动回归测试。

## 验证与验收

- 修复前，Linux Chrome 156 上三个原场景连续 5 轮通过；60 次设置读取最长 69 ms。完整原驱动 `--long` 也通过 27/27。这些结果说明单纯重跑不足以确认根因。
- 独立复现：原驱动开启/关闭 Fetch 各一次均超时；修复后相同导航各一次均通过，旧请求仍保留 `done: false`。
- 真实浏览器反例：刻意无限扣住旧响应体，切到 B 可以静默；恢复 A 后必须超时，只有释放响应体后才能通过。两种 Fetch 配置均通过该检查。
- `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs`：10 passed / 0 failed。
- 三项隔离 mutation 均被测试拒绝：恢复跨全部文档等待；把 200 响应头当作完成；取消请求事件对静默窗口的重置。
- `actionlint .github/workflows/ci.yml`、`git diff --check`：通过。
- 修复后 Linux release 页面 + release 服务器的完整 `--long`：27/27 场景、108 检查、0 意外控制台错误。报告 `target/handoff/evidence-ci-timeouts/linux156-fixed-full-1/assertions.json`，SHA-256 `c00b9ce6c567c316553885490921b2ec790bb6f8aeeb2438af8c6f7aaccc0534`。
- 修复后远端 CI：pending。

## 结果与风险

- 已定位可复现的驱动缺陷；原始三个超时缺少身份信息，逐条归因仍有证据缺口。
- 不声称覆盖嵌套 frame 的 BFCache 生命周期；本应用的 API 来自主文档，其它 loader 采用保守等待。
- 本轮只验证驱动修改，没有重做应用原生/wasm 测试或独立外部评审。
- 自动审批拒绝将原 CI 构建 artifact 下载到本机，理由为需要批准但当前审批策略为 Never；没有换通道重试该下载。本地复现用当前未改产品源码构建的产物，显式构建标识 `c6-package-a13a1e5` 两端一致，不冒充原 CI 二进制。原失败日志及先前已留存的 artifact 可读。

## 给下一位 Agent 的备注

- 工作树：`target/audit-20261009/worktree`；主工作树的任务外变更未触碰。
- 本地完整证据：`target/handoff/evidence-ci-timeouts/`；诊断入口：`target/handoff/scripts/probe-navigation-events.mjs`、`trace-ci-timeouts.mjs`、`run-ci-timeouts.py`。
- Docker 诊断镜像：`rssr-ci-timeouts:chrome156`，镜像摘要见复现 JSON；隔离测试容器运行结束自动移除，镜像保留。
- 后续再出现超时，先查看失败 JSON 的 `mainLoaderId` 和 `inFlightRequestIds`，核对真实结束事件，再判断是否是另一类问题。
