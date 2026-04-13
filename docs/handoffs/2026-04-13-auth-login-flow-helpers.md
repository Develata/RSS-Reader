# Auth Login Flow Helpers

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：7ec485a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

在登录页 renderer 拆分之后，继续收敛 `rssr-web/src/auth.rs` 的 handler 级重复逻辑，把登录 redirect URL 构造与成功登录响应构造抽成小 helper，不改变鉴权安全语义。

## 影响范围

- 模块：
  - `crates/rssr-web/src/auth.rs`
  - `crates/rssr-web/src/auth/login_flow.rs`
- 平台：
  - Linux
  - Docker / `rssr-web`
  - Web
- 额外影响：
  - auth redirect flow
  - auth unit tests

## 关键变更

### Auth Redirect Flow

- 新增 `login_redirect(next, error)`，统一这些 redirect 位置：
  - 登录限速：`/login?error=rate_limited&next=...`
  - 账号密码错误：`/login?error=invalid_credentials&next=...`
  - session 过期：`/login?error=session_expired&next=...`
  - 未登录访问受保护路径：`/login?next=...`
- 保留既有 `sanitize_next(...)` 与 `urlencoding::encode(...)` 使用方式。
- 保留既有 error code 文案映射，由 `auth/login_page.rs` 继续负责用户可见文案。
- `login_redirect(...)` 已放入新增的 `auth/login_flow.rs`，handler 侧只保留调用点。

### Successful Login Response

- 新增 `successful_login_response(config, next, token)`。
- 将成功登录后的 `Redirect::to(next)` 与 session / gate cookie 写入集中到 helper。
- `successful_login_response(...)` 已放入新增的 `auth/login_flow.rs`，方便后续继续拆 handler flow。
- 未改动 cookie 名称、cookie header 构造、session token 生成或登录限速分支。

### Tests

- 新增 `login_redirect_encodes_next_and_optional_error`，锁定带 query 的 `next` 在有无 `error` 两种登录 redirect 中都正确编码。
- 该测试随 helper 迁移到 `auth::login_flow::tests`。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-web`：通过，16 passed
- `cargo check -p rssr-web`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- 静态代码复核：通过
- 对照 `crates/rssr-web/src/auth/AGENTS.md` 检查：通过，未改 session/cookie/rate-limit/security 语义

## 结果

- `handle_login(...)` 与 `require_auth(...)` 中的 redirect 字符串拼接重复已收口。
- 成功登录响应构造已从 handler 主流程中抽出，后续继续拆 `auth.rs` 时可以更聚焦认证流程分支。

## 风险与后续事项

- `auth.rs` 仍包含多个职责：handler、middleware、smoke auth helper 与综合测试。
- 下一步建议继续拆 `auth.rs` 时先提取更纯的 flow helpers 或测试模块，再考虑文件级拆分；不要同时改登录限速、session token 或 cookie 语义。

## 给下一位 Agent 的备注

- 入口文件：
  - `crates/rssr-web/src/auth.rs`
  - `crates/rssr-web/src/auth/login_flow.rs`
  - `crates/rssr-web/src/auth/login_page.rs`
  - `crates/rssr-web/src/auth/AGENTS.md`
- 本次工作基于同日未提交的登录页 renderer 拆分；两个 handoff 都是 `commit: pending`。
