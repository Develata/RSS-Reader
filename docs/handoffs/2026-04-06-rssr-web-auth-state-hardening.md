# rssr-web 认证状态路径加固

- 日期：2026-04-06
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：194bc17
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

围绕仓库级梳理后的 `rssr-web` 鉴权包装层，修复认证状态文件默认路径在 Windows 上可能落入当前工作目录的隐性问题，并把认证状态持久化逻辑从超长 `config.rs` 中拆出独立模块，补足跨平台回归测试。

## 影响范围

- 模块：
  - `crates/rssr-web/src/auth.rs`
  - `crates/rssr-web/src/auth/config.rs`
  - `crates/rssr-web/src/auth/persisted_state.rs`
- 平台：
  - Windows
  - Linux
  - macOS
  - Web
- 额外影响：
  - docs
  - handoff

## 关键变更

### 认证状态路径解析

- 新增 `auth/persisted_state.rs`，集中处理认证状态文件路径解析、读写与权限收紧。
- 默认认证状态路径解析顺序调整为：
  - `RSS_READER_WEB_AUTH_STATE_FILE`
  - `HOME`
  - `USERPROFILE`
  - `HOMEDRIVE + HOMEPATH`
  - 当前工作目录
- 修复 Windows 环境下 `HOME` 缺失时，认证状态文件意外写到启动目录的潜在问题。

### 模块收敛与代码清理

- `config.rs` 仅保留鉴权配置与策略判断，把持久化细节移出，降低单文件复杂度。
- 清理 `rssr-web` 中此前 `cargo clippy` 暴露的无效 import / 无效参数警告。

### 回归覆盖

- 为 `persisted_state` 增加回归测试，验证 `HOME` 缺失时会正确回退到 `USERPROFILE`。
- 保留原有密码哈希、session secret 持久化与权限相关测试路径。

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo test --workspace`：通过
- `cargo clippy --workspace --all-targets`：通过

### 手工验收

- Chrome MCP 访问 `http://127.0.0.1:18080/entries`：通过，未登录时重定向到 `/login`
- Chrome MCP 提交本地 smoke 凭证后返回目标页：通过
- `http://127.0.0.1:18080/healthz`：通过

## 结果

- 本次交付可合并，`rssr-web` 的鉴权状态落盘行为在 Windows 上更可预测。
- 本次修改不改变阅读器核心产品边界，只加固部署包装层实现。

## 风险与后续事项

- `crates/rssr-web/src/auth/config.rs` 虽已瘦身，但仍偏长；后续可继续把环境变量解析和登录限流配置再拆一层。
- 这次浏览器验收使用的是最小静态壳，不等价于完整 Dioxus 前端联调；若后续改登录包装层与前端资源协作，再做一次完整 web bundle 验证更稳妥。

## 给下一位 Agent 的备注

- 如果要继续看 `rssr-web` 鉴权链路，先读 `crates/rssr-web/src/auth/config.rs` 与 `crates/rssr-web/src/auth/persisted_state.rs`。
- 如果要继续做 repo 级梳理，可参考同日未提交的 `docs/handoffs/2026-04-06-repo-doc-analysis.md`，但不要默认覆盖它。
