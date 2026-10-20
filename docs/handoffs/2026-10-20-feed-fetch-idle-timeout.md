# 订阅抓取改用空闲读超时 + 整体上限

- 日期：2026-10-20
- 作者 / Agent：Claude Code
- 分支：feat/reader-mathml
- 当前 HEAD：见 `git log feat/reader-mathml`
- 相关 commit：`5070d71`（修复）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

桌面端添加 `https://develata.me/rss/daily-math-all.xml`（全部历史全文，5.6 MB，gzip 后约 893 KB）
失败：原生抓取用 30 秒总超时，且计入响应体读取；慢链路上这个 feed 下载要 27–40 秒，
于是报 `读取 feed 响应正文失败 … operation timed out`。改为空闲读超时判死停住的连接，
整体上限放宽到 120 秒，只用来截断涓流。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/fetch/client/feed_http.rs`
  - `crates/rssr-infra/src/subscription_probe.rs`
  - `crates/rssr-infra/src/composition.rs`
- 平台：
  - Windows / macOS / Linux / Android（原生抓取）
  - Web 不变：浏览器端抓取与 `rssr-web` 的 `/feed-proxy` 仍是 30 秒总超时
- 额外影响：
  - N/A

## 关键变更

### 超时语义

- 新增 `FeedTimeouts { connect: 10s, read_idle: 30s, total: 120s }`（`FEED_TIMEOUTS`）。
  `read_idle` 用 reqwest `ClientBuilder::read_timeout`：两次成功读之间（含等响应头）的最长间隔，
  每次成功读后重置。
- `feed_http_client(timeouts)` 构造原生 feed client；`FetchClient` 与原生 `HttpSubscriptionProbe`
  共用它。此前订阅探测用 `reqwest::Client::new()` + 单请求 30 秒总超时，没有连接超时。
- 去掉 `FetchClient::fetch` 和探测里重复的单请求 `.timeout(30s)`，以 client 级配置为准。

### 测试

- `feed_http.rs` 单元测试：持续传输超过空闲超时仍成功；发完响应头后停住按空闲超时快速失败；
  无尽涓流被整体上限截断。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test -p rssr-infra --locked --lib feed_http`：通过（3 passed）
- `cargo test --workspace --locked`：通过（0 failed）
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过

### 手工验收

- 修复前，经本机代理 `rssr-cli add-feed https://develata.me/rss/daily-math-all.xml`
  （每次用新的数据库）：4 次中 3 次失败（`operation timed out`），1 次成功，耗时 30.4 秒。
- 修复后同一命令（新数据库）：3 次全部成功，耗时 12 / 18 / 40 秒；40 秒那次在旧的 30 秒总超时下会失败。
- 排除项：5.6 MB 低于 8 MiB 响应上限；用 `sanitize_reader_html` 处理全部 77 篇正文，
  14268 个 `<math>` 全部保留，无一降级，MathML 预算不是失败原因。

## 结果

- 已验证：CLI 可添加并首刷该 feed；可合并。
- 推断（桌面未实测）：桌面端与 CLI 共用 `compose_native_sqlite_use_cases` 的同一原生抓取路径，`rssr-app` 里没有包裹订阅抓取的外层超时（只有图片本地化和界面提示计时）。
- 未在 Android 真机上复测；Android 走同一原生抓取代码。

## 风险与后续事项

- 整体上限 120 秒：数据库在 WAL 模式下刷新并发，一个涓流源最多占一个并发槽 2 分钟；非 WAL 时刷新退回串行，它会让整轮刷新最多多等 2 分钟。停住的连接仍在 30 秒空闲后失败。
- Web 端未改：同一 feed 的正文 JSON 约 543 万 UTF-16 码元，超过 Chromium localStorage
  每源约 524 万码元的配额，加上写 `-next` 槽的写时复制，Web 端几乎必然写入失败。
  这一条是推断，未实测；要解决需改存储后端（如 IndexedDB），属于骨架级变更，需单独决策。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-infra/src/fetch/client/feed_http.rs` 的 `FEED_TIMEOUTS`。
