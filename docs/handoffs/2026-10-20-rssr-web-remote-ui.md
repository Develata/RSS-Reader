# P2：浏览器界面接入 rssr-web 服务端

- 日期：2026-10-20
- 作者 / Agent：Claude Code
- 分支：feat/rssr-web-remote-ui（基于 feat/rssr-web-server-api）
- 当前 HEAD：见 `git log feat/rssr-web-remote-ui`
- 相关 commit：本记录同次提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按 `docs/design/rssr-web-server-mode.md` 的 P2：由 `rssr-web` 托管时，浏览器界面经 `/api/v1`
调用服务端的 `NativeHost`（SQLite），不再使用 `BrowserStore`/`localStorage`。单独托管的静态包与
GitHub Pages 演示仍是本地模式（D1）。

## 影响范围

- 模块：
  - `crates/rssr-host/src/client.rs`（新增）：`ApiClient`、`RemoteServices`、`Backend { Local, Remote }`
  - `crates/rssr-app/src/ui/runtime/services.rs`：端口改为持有 `Backend`；wasm 上的 `server_mode` 判定
  - `crates/rssr-app/src/web_auth.rs`：公开 `server_gate_present`、`browser_origin`
  - `crates/rssr-web/src/api.rs`：客户端与服务端之间的契约测试
  - `scripts/release-ui/web_auth.rs`：冒烟服务端设置 `RSS_READER_WEB_ALLOW_PRIVATE_FEEDS=true`
  - 文档：`docs/deployment/web.md`、`README.md`、`AGENTS.md`、`CLAUDE.md`、`crates/rssr-app/src/bootstrap/web/AGENTS.md`、设计文档进度
- 平台：
  - Web（rssr-web 托管）：存储从 localStorage 换成服务端 SQLite
  - 静态 Web 包、Pages 演示：不变
  - 桌面 / Android：只有内部的 `Backend::Local` 包装，行为不变

## 关键变更

- **模式判定**（`server_mode::client`）：
  - 演示包，或没有门禁 cookie：本地模式。
  - 有门禁 cookie：探测 `/api/v1/capabilities`。只有返回 200 + `application/json` + `mode: "server"`
    才进入服务端模式，其它一切（网络、超时、5xx、HTML、格式不对）都是可重试的错误，不缓存、不退回本地存储。
    （审查后修订：原先 404/HTML 会退回本地模式，现已去掉。）
  - 整个判定（包括门禁检查）放在 `static tokio::sync::OnceCell<bool>`：成功后对整个页面固定，并发的首次调用
    合并成一次探测。之后门禁 cookie 消失，也不会改变存储。
- **远程调用**：`ApiClient::call::<Endpoint>` 带上协议头与 JSON 类型，期限 600 秒（能力探测为 15 秒）。
  响应体按预算流式读取：成功响应 64 MiB、错误正文 1 MiB、能力信息 64 KiB。收到 401 时，只看状态行就调用
  `web_auth::recover_from_expired_server_session` 整页跳回登录页，不读正文。失败时用 `ApiError::into_anyhow`
  还原错误链，界面看到的错误文案与本地调用一致。
- **服务端模式下的自动刷新**由服务端常驻执行，`ensure_auto_refresh_started` 在远程模式下不做任何事。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings`：通过
- `cargo test --workspace --locked -j 2`：exit 0，44 个测试二进制全部通过。新增 3 个契约测试（真实 axum 服务 + 临时文章库）：
  - `remote_backend_round_trips_through_the_api`：设置往返、OPML 导入后快照与列表，以及 PublicOnly 下错误链原样还原
  - `remote_backend_calls_the_unauthorized_handler_on_401`
  - `capabilities_probe_ignores_an_spa_fallback_page`
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过
- `cargo check --manifest-path scripts/release-ui/Cargo.toml --locked`：通过

### 手工验收（内置浏览器 + `dx build --platform web` 的 debug 包 + 本机 rssr-web）

- 服务端模式（`127.0.0.1:8076`，`ALLOW_PRIVATE_FEEDS=true`，因为本机 DNS 是 fake-IP）：
  - 用仓库冒烟凭据登录后进入 `/entries`，`localStorage` 为空。启动阶段的请求依次是：
    `capabilities`、`settings/load`、`shell/resolve_startup_target`、`entries/bootstrap`、`entries/list`、
    `entries/save_workspace_if_changed`，全部 200。
  - 在订阅页通过界面添加 `https://develata.me/rss/daily-math-all.xml`：页面显示"订阅已保存并完成首次刷新"、
    77 篇文章；`localStorage` 仍为空。
  - 阅读页 `/entries/1` 能正常打开，请求为 `reader/load_entry` 与 `reader/localize_entry_assets`。
  - 重启 rssr-web 后重新打开 `/entries`：文章库仍在（显示 72 篇未归档），每次页面加载只有 1 次能力探测。
  - 控制台除 debug 包热重载客户端连接 `/_dioxus` WebSocket 的失败外，没有其它错误。
- 静态模式回归（`python -m http.server` 托管同一个包，地址用 `localhost:8077`，避免与 127.0.0.1 共享 cookie）：
  完成本地门禁初始化后进入 `/entries`，BrowserStore 的分片与提交头都已写入 `localStorage`，控制台无错误。
  测试数据已清除。
- 会话在页面打开期间失效（换 `RSS_READER_WEB_SESSION_SECRET` 重启服务端，页面不刷新）：在页内跳到订阅页时，`feeds/snapshot` 返回 401，页面随即跳到 `/login?next=%2Ffeeds`，回跳路径保留。这验证的是 P2 新增的 `on_unauthorized` 路径，不是既有的 `/session-probe` 路径。
- 阅读页里没有 `<math>`：MathML 功能在 `feat/reader-mathml` 上，不在本分支栈（基于 main）里，这是预期行为。

## 结果

- P2 完成，可合并（需按分支栈顺序：design → refactor/rssr-host → feat/rssr-web-server-api → 本分支）。

## 风险与后续事项

- 覆盖范围说明：`server_mode` 中"有门禁 cookie，但服务端没有接口（404 / HTML）→ 退回本地"这个分支，只由 `capabilities_probe_ignores_an_spa_fallback_page` 单测覆盖；浏览器里的静态模式回归走的是"没有门禁 cookie"分支。
- 在同一主机的不同端口上，同时运行 rssr-web 和静态包时，两者共享门禁 cookie：静态包会被既有的
  `web_auth` 流程判为会话过期并跳转登录页。这是既有行为，本次未改；真实部署不会这样混用。
- 多个浏览器同时操作时没有推送（D4），各端在下次查询时才看到变化。
- 会启动 rssr-web 并抓取 `127.0.0.1` 夹具 feed 的脚本，都已设置 `RSS_READER_WEB_ALLOW_PRIVATE_FEEDS=true`，并给出独立的 `RSS_READER_WEB_DATA_DIR`：release-ui runner、`run_rssr_web_browser_smoke.sh`、`run_rssr_web_browser_feed_smoke.sh`、`run_windows_chrome_visible_regression.sh`。这些脚本本次没有实际运行，只做了 `bash -n` 语法检查。`run_rssr_web_proxy_feed_smoke.sh` 只验证 `/feed-proxy`，不受影响。
- `.dockerignore` 补上了 `.claude`、`.workbuddy`：本机 `.claude/worktrees` 里有其它会话的工作树（4.4 GB），会进入构建上下文，并在传输时被其它会话改动，导致 `context canceled`。
- 下一步 P3：发布流水线新增 `rssr-web` Linux 二进制；`specs/001` 中"无服务端依赖"等条文的修订；
  Docker 镜像构建验证（P1 修复的 1.99 工具链，见 P1 交接）。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-host/src/client.rs`、`crates/rssr-app/src/ui/runtime/services.rs` 末尾的 `server_mode`。

## 交叉模型审查（codex-reviewer，2026-10-20，范围 dbc8e73..a067b54）

结论：2 个阻断项、3 个应修项、1 个待验证项。逐条核实后全部属实，处理如下：

1. **阻断：门禁 cookie 消失后，页面会改用本地存储。** 原实现先检查门禁再读缓存，cookie 到期或在别的标签页登出后，
   下一次命令就会装配 BrowserStore。已改为把整个判定（含门禁检查）放进 `OnceCell` 并对整个页面固定。
   浏览器复验：登录后用 JS 删除 `rssr_web_gate`，再在页内跳到订阅页，仍然调用 `feeds/snapshot`，显示服务端的 77 篇，
   `localStorage` 为空。
2. **阻断：首次探测的 5xx / HTML / 损坏的 JSON 被缓存为本地模式。** `ApiClient::capabilities` 现在返回
   `Result<Capabilities>`，除合法的服务端能力响应外一律报错，不缓存。"旧版服务端退回本地"的兼容已去掉，
   理由见设计 §4.5。测试：`capabilities_probe_reports_a_server_error_instead_of_local_mode`（503）、
   `capabilities_probe_ignores_an_spa_fallback_page`（HTML，现在断言为报错）。
3. **没有超时。** 能力探测 15 秒，端点调用 600 秒。超时提示"操作可能仍在服务端执行"，不自动重试。
4. **响应体先整个缓冲再解析。** 新增 `read_limited`：先按 `Content-Length` 提前拒绝，再流式累计；预算为能力信息 64 KiB、
   错误正文 1 MiB、成功响应 64 MiB。测试 `oversized_responses_are_refused_before_buffering`。
5. **401 的处理依赖读完正文。** 现在只看状态行就调用 `on_unauthorized`，并直接返回，不读正文。
   测试 `unauthorized_is_handled_from_the_status_line_even_if_the_body_stalls`（服务端回 401 后正文停住，调用 5 秒内返回）。
6. **待验证：覆盖不足。**
   - 往返测试现在逐字比较远程与本地调用的错误链（`Display` 与 `{:#}`）。
   - `server_mode` 的 wasm 逻辑仍然只有浏览器手工验收覆盖：本轮验了门禁消失；首次探测 503 之后恢复的场景，
     只由客户端单测与 `OnceCell::get_or_try_init` 出错不缓存的语义支撑，没有在浏览器里实测。

修复后的验证：fmt、clippy、`cargo test --workspace`（exit 0，44 个测试二进制）、wasm32、aarch64-android 全部通过；
`rssr-web` API 测试共 12 个。

## CI 修复（PR #30，`feed-proxy-isolation`）

- 失败原因：这个 harness 的夹具服务端有登录和门禁 cookie，但没有 `/api/v1`。审查修复后，这种页面按设计会报
  "确认服务端模式失败"，不再退回 BrowserStore，所以原流程等不到订阅卡片而超时。`lint-and-test` 只是汇总任务，
  随之失败。
- 修复（783f0e0）：harness 先断言"有门禁、没有接口 → 报错，并且不写 BrowserStore 提交头"，再删除门禁 cookie，
  继续回归本地模式（BrowserStore + 同源 `/feed-proxy`）。浏览器经 `rssr-fixture.test` 映射到 127.0.0.1，
  避开仅限 localhost 的本地登录；只把该源视为安全上下文，满足 Web Locks；不使用系统代理。
- 本机验证：用 debug 包与 Windows Chrome 运行 `node scripts/browser/feed_proxy_isolation.mjs target/fpi-local`，
  `pass: true`：10 个隔离用例全部通过，门禁断言为真，4 个真实 feed 夹具都完成了添加、经代理刷新与阅读。
