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
  - 出站策略：`outbound` 模块 4 个（含解析器成功路径），`feed_http` 1 个，`tests/test_outbound_policy.rs` 2 个（图片、WebDAV）
  - `rssr-host`：协议 2 个，分派 2 个（真实 `NativeHost` + 临时目录）
  - `rssr-web`：API 4 个（真实 axum 服务 + 临时文章库，覆盖鉴权、各类拒绝和类型化结果；另有 Origin/Host 三分支单测），配置 1 个
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
- `PublicOnly` 解析器的放行路径由单元测试 `resolver_success_path_connects_to_the_requested_port` 验证：测试注入放行回环地址的判定，
  通过解析器把 `localhost` 解析到本地监听端口，请求成功。这证明解析器返回的端口占位 0 会被换成请求端口；
  真实环境下只是判定函数不同。
- 未验证：Docker 镜像构建（本机没有构建）；真实公网 DNS 环境下 `PublicOnly` 的端到端放行（本机 DNS 是 fake-IP）。

## 结果

- P1 完成，可合并。浏览器界面尚未接入，用户可见行为不变；Docker 部署多了一个数据卷。

## 风险与后续事项

- `/api/v1` 现在已经可用，任何持有有效会话的客户端都能直接调用，安全面相应扩大。开启
  `ALLOW_PRIVATE_FEEDS` 后，会话一旦被盗就能借服务器探测内网。
- 本机这类 fake-IP 环境下，`PublicOnly` 会拒绝所有 feed。部署方需要提供真实 DNS，或显式放开。
- 多个浏览器同时操作时 `app_state` 后写覆盖（D4），也没有推送：别的浏览器要等下次查询才看到变化。
- 5070d71 以 cherry-pick 形式进入本分支（c64b899），以后合并 `feat/reader-mathml` 时，同样的改动会以等价提交出现。
- P2 接入界面后，release-ui runner 与代理 feed 冒烟用的 `127.0.0.1` 夹具 feed 会被 `PublicOnly` 按设计拒绝，
  这些运行需要设置 `RSS_READER_WEB_ALLOW_PRIVATE_FEEDS=true`。
- 反向代理必须原样转发 `Host`，或设置 `X-Forwarded-Host` 并开启信任，否则 `/api/v1` 返回 403（已写入 `web.md`，
  `origin_check_covers_default_ports_and_trusted_forwarded_host` 覆盖三个分支）。
- `AGENTS.md:57`（"Web … 不使用 SQLite"）对 rssr-web 已不再准确，按设计在 P3 修订。
- 下一步 P2：wasm 端新增 `Backend::Remote`，按 `/api/v1/capabilities` 探测模式（要校验 JSON，不能只看状态码），
  收到 401 时跳转登录页；静态包仍使用 `BrowserStore`（D1）。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-host/src/protocol.rs`（端点表）、`crates/rssr-web/src/api.rs`、`crates/rssr-web/src/library.rs`。

## 交叉模型审查（codex-reviewer，2026-10-20，范围 c64b899..c4d7dd2）

结论：1 个阻断项、6 个应修项、1 个待验证项。逐条核实后全部属实，处理如下：

1. **阻断：Docker 构建器是 Rust 1.88。** P1 之后服务端会编译 `rssr-infra` 的原生代码，其中
   `File::try_lock` 从 1.89 才稳定。已改为 `rust:1.99-bookworm`，与 CI 的 stable 和本机工具链一致。
2. **会话检查发生在读完请求体之后**，匿名客户端可以让服务端缓冲 16 MiB。已把全部调用条件移到
   `guard` 中间件，在读取请求体之前执行；声明的 `Content-Length` 超限时直接 413。请求体改为
   `to_bytes` 限量读取，期限 30 秒，读取失败与超时都返回 JSON。`no-store` 改由外层中间件统一设置，
   框架生成的 405 / 413 也带上。测试 `rejections_happen_before_the_body_is_read` 用原始 TCP 只发请求头、
   不发请求体，断言 401 与 413 都会立即返回。
3. **脱离请求的任务没有全局并发上限。** 已加进程级准入：信号量 32，许可移入后台任务并持有到完成，
   饱和时返回 503。测试 `saturated_admission_returns_503_json`。
4. **IPv6 分类不完整。** 已补充：NAT64 知名前缀（按嵌入的 IPv4 判断）、NAT64 本地前缀 `64:ff9b:1::/48`、
   Teredo `2001::/32`、6to4 `2002::/16`（按嵌入的 IPv4 判断）、`2001:2::/48`、`3fff::/20`、`100::/64`、
   `fec0::/10`，以及 IPv4 的 `192.0.0.0/24`。`/feed-proxy` 改为共用 `rssr_infra::outbound::is_disallowed_ip`，
   同时修掉了参考实现原有的这些缺口。测试中包含公网 NAT64 / 6to4 应放行，以及 `3fff::/20` 边界外应放行的用例。
5. **Origin 只比较主机与端口。** 已改为严格同源：恰好一个 `Origin`，必须是不含路径的规范源，scheme 也要一致。
   开启信任时以 `X-Forwarded-Proto` 为准；否则开启 `Secure` cookie 时要求 https，都没有时接受 http 与 https。
6. **数据目录没有私有权限。** 改为复用 `sqlite_native::create_private_dir`（由原 `create_local_data_dir`
   改名并公开）：在 Unix 上新建目录为 0700。Docker 中的 `/app/data` 也设为 `chmod 700`。
   `XDG_DATA_HOME` / `HOME` 只接受绝对路径，与桌面版一致。
7. **设计文档写的是"服务端读取时消毒"，但实现不是这样。** 已按实现修正：消毒与 MathML 预算在渲染端执行，
   协议上传的是原始正文，任何渲染端都必须先消毒。这一点是 P2 客户端必须遵守的约束。
8. **待验证：端点覆盖。** 新增 `every_endpoint_accepts_a_valid_input`：逐个枚举 `ENDPOINT_PATHS`，
   缺少入参就失败；另有 `write_endpoints_change_what_read_endpoints_return`，覆盖设置保存、OPML 导入、
   配置包导出后删除再导入、工作区保存的副作用。客户端断开后后台刷新能否继续完成，仍然没有自动化测试。

修复后的验证：fmt、clippy（`--workspace --all-targets`）、`cargo test --workspace` 全部通过
（exit 0，44 个测试二进制）。wasm32 与 aarch64-android 检查通过。
Docker 镜像构建的结果另行记录（见下）。
