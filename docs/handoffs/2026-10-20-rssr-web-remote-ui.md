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
    才进入服务端模式；404 或 HTML（还没有接口的旧版服务端、SPA 回退页）沿用本地模式。
  - 网络错误直接报错，不悄悄退回 localStorage。
  - 判定结果放在 `static tokio::sync::OnceCell<bool>`：成功后缓存，并发的首次调用合并成一次探测。
- **远程调用**：`ApiClient::call::<Endpoint>` 带上协议头与 JSON 类型。收到 401 时调用
  `web_auth::recover_from_expired_server_session`，整页跳回登录页。失败时用 `ApiError::into_anyhow`
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
