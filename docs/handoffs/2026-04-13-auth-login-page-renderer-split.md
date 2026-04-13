# Auth Login Page Renderer Split

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：7ec485a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

继续按 P1-P2 拆高复杂文件，先从 `rssr-web/src/auth.rs` 中分离登录页 HTML 输出，把登录页渲染与鉴权/session/rate-limit handler 流程解耦。

## 影响范围

- 模块：
  - `crates/rssr-web/src/auth.rs`
  - `crates/rssr-web/src/auth/login_page.rs`
- 平台：
  - Linux
  - Docker / `rssr-web`
  - Web
- 额外影响：
  - `rssr-web` login page rendering
  - auth unit tests

## 关键变更

### Login Page Rendering

- 新增 `crates/rssr-web/src/auth/login_page.rs`。
- 将这些登录页呈现职责从 `auth.rs` 移到 `login_page.rs`：
  - `APP_NAME`
  - `WEB_LOGIN_MARKUP`
  - 登录页 HTML 模板
  - error code -> 用户可见文案映射
  - Secure cookie 提示文案
  - HTML escape helper
- `show_login(...)` 现在只负责：
  - sanitize `next`
  - 调用 `render_login_page(...)`
  - 返回 `Html(...)`
- 登录 token 构造失败时继续返回 `500`，但错误文本改为复用 `render_login_failure(...)`。

### Auth Flow

- 未改动：
  - credential verification
  - rate limit key / failure recording / unblock behavior
  - session token format
  - session / gate cookie 写入
  - logout cookie 清理
  - `require_auth(...)` redirect 逻辑

### Tests

- 新增 `auth::login_page::tests::html_escape_escapes_attribute_sensitive_characters`，锁定登录页渲染层的 HTML escape 行为。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-web`：通过
- `cargo check -p rssr-web`：通过
- `cargo fmt --check`：通过
- `git diff --check`：通过

### 手工验收

- 静态代码复核：通过
- 对照 `crates/rssr-web/src/auth/AGENTS.md` 检查：
  - 未改登录限速规则
  - 未改 cookie/session 规则
  - 未改认证状态文件落盘或配置校验
  - 未把 feed proxy / RSS 领域逻辑引入 auth 模块

## 结果

- `auth.rs` 不再直接承载整段登录页 HTML 模板。
- 鉴权 flow 与页面呈现的边界更清楚，后续继续拆 `auth.rs` 时可以聚焦 handler / redirect / session flow。

## 风险与后续事项

- 登录页 CSS 仍是内联 HTML 模板的一部分，只是已经移到 renderer 模块；如果后续要进一步收口，可考虑拆成静态模板或更小的 rendering helpers。
- `auth.rs` 仍承载：
  - login handler
  - logout handler
  - auth middleware
  - smoke auth helper
  - auth 相关综合测试
- 下一步建议继续拆 `auth.rs`：
  - 先把 redirect URL 构造 helper 收口
  - 再把 handler-level login flow 提成小函数，便于独立测试 rate-limit / credential / token 写 cookie 的分支

## 给下一位 Agent 的备注

- 优先看：
  - `crates/rssr-web/src/auth.rs`
  - `crates/rssr-web/src/auth/login_page.rs`
  - `crates/rssr-web/src/auth/AGENTS.md`
- 如果继续拆 auth：
  - 不要改 session/cookie/rate-limit 行为
  - 先拆纯 helper，再考虑 handler flow
