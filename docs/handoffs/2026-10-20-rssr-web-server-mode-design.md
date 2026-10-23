> 历史分支记录（main，来源 `d9ebe35`）。保留当时的实现和验证上下文；当前 v0.2 的适配与状态见[整合记录](./2026-10-22-branch-integration.md)。下文的旧路径、架构及待办不构成当前契约。

# rssr-web 服务端模式设计草案

- 日期：2026-10-20
- 作者 / Agent：Claude Code
- 分支：design/rssr-web-server-mode（基于 main e4c4a82）
- 当前 HEAD：见 `git log design/rssr-web-server-mode`
- 相关 commit：本记录同次提交
- 相关 tag / release：N/A
- 状态：`draft`

## 工作摘要

Develata 要求 `rssr-web` 单独发布 Linux 二进制，并在设计上与桌面 GUI 等价（数据在服务端本地
SQLite）。这改变了"Web = 浏览器 localStorage、rssr-web 只是薄壳"的成文约束，属于架构级变更，
本次只交付设计草案，等待确认。

## 影响范围

- 模块：
  - `docs/design/rssr-web-server-mode.md`（新增）
  - `docs/design/README.md`（索引）
- 平台：
  - 无代码改动
- 额外影响：
  - 设计落地后将修订 `specs/001`、`AGENTS.md`、`CLAUDE.md`、`docs/deployment/web.md`、`README.md`

## 关键变更

- 远程边界定在 `rssr-app` 的 `UiServices`；抽出 UI 无关的原生宿主 crate `rssr-host`，
  桌面与 `rssr-web` 共用。
- 列出出站 SSRF 策略、刷新脱离请求生命周期、模式探测、数据目录与发布物、四个待决策项 D1–D4。

## 验证与验收

### 自动化验证

- 无代码改动，未运行构建。

### 手工验收

- 设计中的事实性断言已对照代码核实：`UiServices` 方法清单；`bootstrap/native.rs` 不依赖 Dioxus；
  DTO 的 serde 现状；`rssr-app` 无错误 downcast；会话 Cookie 属性；`RefreshFlight` 的取消语义；
  `/feed-proxy` 的内网拦截规则；`RSS_READER_WEB_AUTH_STATE_FILE` 配置。

## 结果

- 草案待 Develata 确认方向与 D1–D4，确认前不写实现代码。

## 风险与后续事项

- 设计依赖 5070d71（抓取超时、`feed_http_client`），该提交目前在 `feat/reader-mathml`，
  P1 开工前需要先合入 main。

## 给下一位 Agent 的备注

- 入口：`docs/design/rssr-web-server-mode.md` 第 5 节分阶段计划。
