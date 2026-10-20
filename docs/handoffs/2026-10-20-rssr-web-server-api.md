# P1：rssr-web 服务端宿主与 `/api/v1`

- 日期：2026-10-20
- 作者 / Agent：Claude Code
- 分支：feat/rssr-web-server-api（基于 refactor/rssr-host）
- 当前 HEAD：见 `git log feat/rssr-web-server-api`
- 相关 commit：c64b899（cherry-pick 自 feat/reader-mathml 的 5070d71，抓取超时）、f7233c4（出站策略）、
  cc4ebda（服务门面与协议）、本记录同次提交（服务端 API）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按 `docs/design/rssr-web-server-mode.md` 第 5 节的 P1：`rssr-web` 启动时在 `RSS_READER_WEB_DATA_DIR`
打开与桌面相同的 `NativeHost`（SQLite 双库 + 宿主能力），启动服务端自动刷新，并经 `/api/v1`
向已登录会话提供界面运行时的全部能力。服务器替用户访问的地址默认只放行公网（D2）。
浏览器界面还没有接入这套接口（那是 P2），所以用户可见行为暂时不变。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/outbound.rs`（新增）及 feed 抓取、订阅探测、图片本地化、WebDAV、组装入口
  - `crates/rssr-host/src/{services,protocol,server}.rs`（新增）、`native.rs`（`NativeHostOptions`）
  - `crates/rssr-application`、`crates/rssr-domain`：`UiServices` 涉及的 DTO 加 serde derive
  - `crates/rssr-app/src/ui/runtime/services.rs`：端口改为委托 `HostServices`
  - `crates/rssr-web/src/{api,library}.rs`（新增）、`main.rs`、`auth.rs`（`request_has_valid_session`、`test_support`）
  - `Dockerfile`、`docker-compose.yml`、`scripts/run_rssr_web_proxy_feed_smoke.sh`、`scripts/release-ui/web_auth.rs`
  - `docs/deployment/web.md`、`CLAUDE.md`
- 平台：
  - Linux / Docker（rssr-web）为主；桌面、Android 只有内部委托变化，行为不变
- 额外影响：
  - Docker 镜像新增 `/app/data` 目录与 `RSS_READER_WEB_DATA_DIR=/app/data`，compose 新增 `rss-reader-data` 卷

## 关键变更

### 出站策略（f7233c4）

- `OutboundPolicy::{Unrestricted, PublicOnly}`。`PublicOnly` 做三件事：
  - 自定义 DNS 解析器：只要有一条解析结果是内网地址就整体拒绝。客户端直接连接解析器返回的地址，DNS rebinding 无从插手。
  - IP 字面量：首跳由调用方 `check_url` 检查，每次重定向由客户端的重定向策略检查。
  - 不使用环境代理。
- 内网地址的判定规则与 `/feed-proxy` 相同。
- 四条出站路径都已接入：feed 抓取、订阅探测、正文图片本地化、WebDAV。
- 桌面默认 `Unrestricted`，行为不变。`feed_http_client` 构建失败时不再退回 `reqwest::Client::new()`，
  否则会悄悄丢掉策略；此前的退回在同样的失败条件下也会 panic，所以没有实际行为变化。

### 服务门面与协议（cc4ebda）

- `HostServices`：`UiServices` 各端口方法的 UI 无关实现，桌面与服务端共用。
- `protocol`：25 个端点，每个端点在这里只定义一次路径与输入输出类型。另有协议版本头 `x-rssr-protocol: 1`、
  不需要登录的 `/api/v1/capabilities`，以及 `ApiError { messages }`：按错误链传递，客户端可原样还原
  `Display` 与 `{:#}`。
- `server::dispatch`：按路径反序列化入参、调用、序列化结果，区分未知端点、参数错误、用例失败三类结果。

### rssr-web（本次提交）

- `library.rs`：读取 `RSS_READER_WEB_DATA_DIR`。未设置时依次用 `$XDG_DATA_HOME/rss-reader-web`、
  `~/.local/share/rss-reader-web`，都没有时报错。读取 `RSS_READER_WEB_ALLOW_PRIVATE_FEEDS`。
  打开宿主并启动自动刷新。
- `api.rs`：`POST /api/v1/{*endpoint}` 依次检查：
  - 有效会话，否则 401 JSON（不重定向到登录页）；
  - 恰好一个 `Content-Type: application/json`，否则 415；
  - 恰好一个匹配的协议头，否则 409；
  - `Origin`（若带）与 Host 一致，否则 403。开启 `TRUST_PROXY_HEADERS` 时用 `X-Forwarded-Host` 比较。
- 通过检查后，调用放进 `tokio::spawn` 执行，客户端断开不会中断刷新。结果映射为
  200 / 404 / 400 / 422 / 500，均带 `Cache-Control: no-store`。请求体上限 16 MiB。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings`：通过
- `cargo test --workspace --locked -j 2`：exit 0，44 个测试二进制全部通过，0 失败。新增的测试：
  - 出站策略：`outbound` 模块 3 个，`feed_http` 1 个，`tests/test_outbound_policy.rs` 2 个（图片、WebDAV）
  - `rssr-host`：协议 2 个，分派 2 个（真实 `NativeHost` + 临时目录）
  - `rssr-web`：API 3 个（真实 axum 服务 + 临时文章库，覆盖鉴权、各类拒绝和类型化结果），配置 1 个
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check -p rssr-app --target aarch64-linux-android --locked`（NDK 环境变量见 rssr-host 交接）：通过
- `cargo check --manifest-path scripts/release-ui/Cargo.toml --locked`：通过

### 手工验收

- 桌面启动冒烟：日志正常，依次是 `rssr_host::native` 初始化、就绪、触发自动刷新。
- `rssr-web` 二进制端到端（临时目录，本机 127.0.0.1:8075）：
  - `/api/v1/capabilities` 返回 `{"protocol":1,"mode":"server"}`；登录返回 303；`settings/load` 返回默认设置。
  - 文章库按配置建在 `RSS_READER_WEB_DATA_DIR`。
  - 默认 `PublicOnly` 下添加 `https://develata.me/rss/daily-math-all.xml` 返回 422。错误链显示
    `develata.me 解析为 198.18.0.101`：本机代理软件的 fake-IP DNS 返回了基准测试保留段地址，按设计被拒绝
    （`docs/deployment/web.md` 已说明 fake-IP 环境需要调整 DNS）。
  - 设置 `RSS_READER_WEB_ALLOW_PRIVATE_FEEDS=true` 重启后，同一 feed 添加成功（`SavedAndRefreshed`，36.7 秒）。
    `feeds/snapshot` 显示 77 篇文章，正文库约 5.7 MB；`reader/load_entry` 返回 200，正文包含 MathML。
    今天 Web 端 localStorage 装不下的这个 feed，在服务端模式下可以正常使用。
- 未验证：Docker 镜像构建（本机没有构建）；在 fake-IP 之外的真实公网 DNS 环境下 `PublicOnly` 的放行路径
  （单元测试只覆盖了拒绝路径与公网 IP 字面量的放行）。

## 结果

- P1 完成，可合并。浏览器界面尚未接入，用户可见行为不变；Docker 部署多了一个数据卷。

## 风险与后续事项

- `/api/v1` 现在已经可用，任何持有有效会话的客户端都能直接调用，安全面相应扩大。开启
  `ALLOW_PRIVATE_FEEDS` 后，会话一旦被盗就能借服务器探测内网。
- 本机这类 fake-IP 环境下，`PublicOnly` 会拒绝所有 feed。部署方需要提供真实 DNS，或显式放开。
- 多个浏览器同时操作时 `app_state` 后写覆盖（D4），也没有推送：别的浏览器要等下次查询才看到变化。
- 5070d71 以 cherry-pick 形式进入本分支（c64b899），以后合并 `feat/reader-mathml` 时，同样的改动会以等价提交出现。
- 下一步 P2：wasm 端新增 `Backend::Remote`，按 `/api/v1/capabilities` 探测模式（要校验 JSON，不能只看状态码），
  收到 401 时跳转登录页；静态包仍使用 `BrowserStore`（D1）。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-host/src/protocol.rs`（端点表）、`crates/rssr-web/src/api.rs`、`crates/rssr-web/src/library.rs`。
