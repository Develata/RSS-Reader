# rssr-web 服务端模式：与桌面 GUI 等价的自托管部署

- 状态：**已确认**（2026-10-20 起草；同日 Develata 确认方向与 D1–D4，按建议执行）
- 范围：`rssr-web`、`rssr-app` 的宿主层与 Web 启动、`rssr-infra` 原生出站策略、发布流水线
- 不改：domain / application 的用例语义、桌面与 Android 的存储

## 1. 目标

`rssr-web` 单独发布 Linux 二进制（Docker 镜像与它同源），设计上与桌面 GUI 等价：

- 数据保存在服务器本地的 SQLite 索引库与正文库，迁移、查询、正文消毒与桌面完全相同。
- 刷新、自动刷新、正文图片本地化、WebDAV 配置同步都在服务端执行，宿主语义与桌面相同。
  自动刷新不依赖浏览器在线。
- 浏览器只负责界面，不持久化文章库。

非目标：

- 桌面 GUI 连接远程 `rssr-web`。"等价"指服务端部署对齐 GUI，不是给 GUI 加远程模式。
- 多用户、实例之间的文章库同步或冲突合并。
- Android 改动。

## 2. 与现有成文约束的关系

服务端模式是**一个实例一个文章库**，多个浏览器访问同一个库，不是同步。功能哲学中
"文章库同步 / 阅读历史同步 / 跨设备冲突合并"的排除项保持不变。需要修订的是下面几处
把 Web 等同于 `localStorage` 的条文：

| 位置 | 现文 | 修订方向 |
| --- | --- | --- |
| `specs/001-minimal-rss-reader/plan.md:42` | 无服务端依赖；部署包装层不得膨胀为核心产品功能 | 桌面、Android、CLI、静态 Web 包无服务端依赖；`rssr-web` 服务端模式是与桌面并列的另一种宿主 |
| `AGENTS.md:57` | Web 使用 `BrowserStore`，不使用 SQLite | 静态 Web 包使用 `BrowserStore`；服务端模式下浏览器不持久化文章，数据在服务端 SQLite |
| `CLAUDE.md`「架构」 | `rssr-web` 是仅用于 Web 部署的薄 axum 服务 | `rssr-web` 是服务端宿主：原生用例 + 宿主能力 + 认证 + 静态托管 |
| `docs/deployment/web.md:3` | 容器卷只保存登录凭据，不是文章库备份 | 数据卷就是文章库，按桌面数据目录同样的方式备份 |
| `README.md` 平台与数据说明 | Web 状态在浏览器 `localStorage` | 分开写静态 Web 包与服务端模式 |

"本地优先"的含义收窄为：数据在用户自己控制的机器上（桌面、手机，或自托管服务器），
不依赖第三方服务。

## 3. 方案选择

远程边界放在 `rssr-app/src/ui/runtime/services.rs` 的 `UiServices`。它把界面运行时能调用的
全部能力收成 5 组端口（entries / shell / settings / reader / feeds），约 30 个方法，已经区分
"用例服务"和"宿主能力"，输入输出都是普通值。界面代码只经过这一层，因此只需替换它的实现，
页面、`ui/runtime`、application 都不用改。

已否决：

- **仓储 / 端口级 RPC**（在浏览器里跑 application，远程实现 `FeedRepository` 等）：往返次数多，
  跨调用的写入失去事务边界，而且自动刷新仍要浏览器在线，达不到"与 GUI 等价"。
- **Dioxus fullstack / server functions**：线协议和构建链绑定框架，改动面大于收益。
- **容器内运行桌面 GUI，经 noVNC / Xpra 远程桌面访问**：零代码，但体验是远程桌面，手机上不可用。
- **浏览器内 SQLite（wasm + OPFS）**：能解决配额，但数据仍在各自的浏览器里，不在服务器上。

## 4. 结构

```text
桌面        rssr-app (Dioxus desktop) ─ UiServices::Local ─ rssr-host(native) ─ SQLite 双库
服务端模式  浏览器 rssr-app (wasm) ─ UiServices::Remote ─ HTTP /api/v1 ─ rssr-web ─ rssr-host(native) ─ SQLite 双库
静态 Web 包 浏览器 rssr-app (wasm) ─ UiServices::Local ─ Web 宿主 ─ BrowserStore (localStorage)
```

### 4.1 抽出原生宿主 crate `rssr-host`

- 移入：`rssr-app/src/bootstrap/native.rs` 的 `AppServices`、`refresh_flight.rs`、图片本地化
  worker；`bootstrap.rs` 中的宿主能力 trait（`AutoRefreshPort`、`RefreshPort`、`ReaderAssetPort`、
  `RemoteConfigPort`）、`HostCapabilities` 与各 `*Outcome` 类型。
- 依赖只有 `rssr-application`、`rssr-infra`、`tokio`，不依赖 Dioxus。trait 与 Outcome 两个目标
  都编译；原生实现在 `cfg(not(target_arch = "wasm32"))` 下。Web 宿主（`bootstrap/web.rs`）
  留在 `rssr-app`。
- 数据目录：保留 `NativeSqliteBackend::from_default_location()` 供桌面使用，另加显式目录入口供服务端。
- 这一步不改行为，桌面改为依赖 `rssr-host`。`rssr-cli` 的 `CliServices` 也重复了这段组装，
  可以后续收敛，不在本轮。

### 4.2 线协议

- `POST /api/v1/<端口>/<方法>`：请求体是方法入参的 JSON，响应体是 Outcome 的 JSON。
- 错误统一为 `{ "messages": ["…", …] }`（`rssr_host::protocol::ApiError`），按错误链排列，最外层上下文在前。
  经检查，`rssr-app` 没有任何 `downcast` / `is::<>`，界面不按错误类型分支，所以只传文本即可；客户端用
  `ApiError::into_anyhow` 还原成 `anyhow::Error`，`Display` 与 `{:#}` 都和服务端一致。
- 序列化：给 `UiServices` 触及的 application / domain 类型加 `Serialize` / `Deserialize`。
  目前只有 `EntriesWorkspaceState`、`UserSettings` 有 serde；需要补的包括 `EntriesBootstrapOutcome`、
  `EntriesListOutcome`、`FeedsSnapshotOutcome`、`ReaderEntrySnapshot`、`MarkReadPreview`、
  `MarkReadOutcome`、`EntryQuery`、`FeedDiscoveryCandidate`、各 Toggle 输入输出、导入导出 Outcome
  与 `StartupTarget`。代价是 application 类型即协议，因此加协议版本：请求带
  `x-rssr-protocol: 1`，不匹配返回 409，界面提示刷新页面。`rssr-web` 托管的就是同版本的包，
  正常情况下不会不匹配。
- `GET /api/v1/capabilities` 返回 `{"protocol":1,"mode":"server"}`，用于模式探测。
- 不做推送：别的浏览器做的修改，本端下次查询时看到。

### 4.3 服务端行为

- 启动：打开数据目录（必要时迁移），立即启动自动刷新。
- 刷新必须脱离 HTTP 请求的生命周期。`RefreshFlight` 在发起方的 future 被丢弃时会取消刷新
  （`refresh_flight.rs` 的注释写明了这一点）；桌面上这没有问题，但在服务端，浏览器断开就会中断
  整轮刷新。服务端改为 `tokio::spawn` 刷新任务，请求处理只等待结果。
- 多客户端：`RefreshFlight` 是进程级单飞，已覆盖不同客户端同时刷新的情况。`app_state`
  （工作区、启动目标）与设置在各客户端之间共享，后写覆盖；单用户在手机和电脑之间切换时，
  这正是期望的行为。界面偏好（shell prefs）和阅读位置仍按浏览器保存。
- 正文消毒与 MathML 预算在**渲染端**执行，与桌面相同：`reader/load_entry` 返回仓储中的原始正文，
  阅读页在渲染前调用 `rssr_infra::html::sanitize_reader_html`。线协议上的 HTML 一律视为不可信；
  任何渲染它的客户端都必须先走同一消毒与预算，不能直接插入 DOM。（P1 审查时修正：此前写的是“服务端读取时执行”，与实现不符。）

### 4.4 安全

- `/api/*` 在读取请求体之前由守卫检查会话（不经过会重定向的 `require_auth`）。Cookie 已是 `HttpOnly; SameSite=Lax`，生产环境
  强制 `Secure`。另加：API 只接受 `Content-Type: application/json` 的 POST（跨源需要预检，
  浏览器会拦截），并校验 `Origin` 与服务地址一致。未登录或会话过期时返回 401，界面跳转 `/login`。
- 资源边界：同时在途的调用最多 32 个，准入许可随后台任务持有到完成，饱和时返回 503；
  请求体上限 16 MiB，读取期限 30 秒，声明的 `Content-Length` 超限时在守卫阶段直接 413。
  `Origin` 必须恰好一个、是规范的源（不含路径），scheme 也要一致：可信代理的 `X-Forwarded-Proto` 优先，
  否则开启 `Secure` cookie 时要求 https。
- **出站请求是新的服务端风险。** 订阅抓取、订阅探测、图片本地化、WebDAV 都是服务器替用户请求
  用户提供的地址。原生抓取目前不拦截内网地址，因为桌面需要访问局域网 feed；`/feed-proxy`
  有完整规则（解析后把 IP 钉死，任一解析结果落在内网就拒绝，防 DNS rebinding）。做法：在原生
  HTTP client 的构造处加可注入的出站地址策略。这个构造处就是 `feed_http_client`，在 5070d71
  修抓取超时时引入，目前还在 `feat/reader-mathml` 分支。服务端默认采用 `/feed-proxy` 的规则，
  用 `RSS_READER_WEB_ALLOW_PRIVATE_FEEDS=true` 显式放开；桌面行为不变。只有一个已认证用户能
  降低风险，但不能消除：会话一旦被盗，就等于可以探测内网。

### 4.5 浏览器端

- `UiServices` 内部改为 `enum Backend { Local { … }, Remote(ApiClient) }`；桌面只编译 `Local`。
- 模式探测（实现见 `rssr-app/src/ui/runtime/services.rs` 的 `server_mode`）：只在页面由 rssr-web 托管、存在门禁 cookie 时才请求 `/api/v1/capabilities`；静态托管不发探测。此时只有合法的服务端能力响应才算成功；网络错误、超时、5xx、HTML、格式不对都是可重试的错误，不缓存，也不退回本地存储。rssr-web 总是携带同版本的前端包发布，不存在“有门禁但没有接口”的合法组合。判定成功后对整个页面固定：之后门禁 cookie 到期或被登出清除，只会让接口返回 401 并跳回登录页，不会改用另一套存储（P2 审查时修正）。响应必须是 `application/json` 且字段匹配才进入
  服务端模式。静态托管的 SPA 回退会对任意路径返回 200 HTML，不能只看状态码（`parse_feed` 的
  `looks_like_html_response_body` 是同一类教训）。
- 服务端模式下不初始化 `BrowserStore`，不读写 localStorage 中的文章数据。feed 抓取在服务端
  直连，不经过浏览器，也不受 CORS 限制。

### 4.6 数据目录、凭据与发布

- 文章库：`RSS_READER_WEB_DATA_DIR`。未设置时为 `$XDG_DATA_HOME/rss-reader-web/`
  （否则是 `~/.local/share/rss-reader-web/`），**有意不和桌面版的 `rss-reader/` 共用**，
  避免两个进程写同一个库。Docker 中为 `/app/data` 卷。
- 登录凭据：保持现有 `RSS_READER_WEB_AUTH_STATE_FILE`，Docker 仍是 `/app/auth` 卷，不迁移。
  凭据含密码哈希和 session secret，备份与权限要求不同于文章库，所以分开存放。
- 发布物新增 `rssr-web-linux-x86_64.tar.gz`（`rssr-web` + `public/`），文档附 systemd unit 示例；
  Docker 镜像用同一份构建。
- 现有 Docker 用户的文章在各自浏览器的 localStorage 里，不会自动迁移。迁移方式：在旧网页导出
  配置 JSON 或 OPML，导入新实例后由服务端重新抓取文章。已读和收藏状态不迁移，与"仅配置交换"
  的边界一致。

## 5. 分阶段

进度：P0 已完成（`refactor/rssr-host`）；P1 已完成（`feat/rssr-web-server-api`）；P2 已完成（`feat/rssr-web-remote-ui`），见对应的 handoff。

每阶段单独 PR，各自验证：

1. **P0 抽出 `rssr-host`**：不改行为。验证：全量测试，桌面手工冒烟。
2. **P1 服务端**：serde 与协议版本、`/api/v1`、鉴权与 Origin 校验、服务端宿主与数据目录、
   出站策略、刷新脱离请求生命周期。验证：进程内 axum + 临时 SQLite 的契约测试，覆盖每个端点的
   往返；未认证、跨源、协议不匹配的拒绝路径；出站策略拒绝内网。
3. **P2 浏览器远程后端**：`Backend::Remote`、模式探测、401 跳转。验证：wasm check；在 `rssr-web`
   部署态下用浏览器做验收，并用今天出问题的 5.6 MB 真实 feed 复测；静态包模式回归。
4. **P3 发布与文档**：发布流水线新增 Linux 二进制，Docker 增加数据卷，修订第 2 节列出的条文，
   编写迁移说明。

## 6. 已确认的决策（2026-10-20）

- **D1** 保留静态 Web 包与 GitHub Pages 演示的 `BrowserStore` 模式。
- **D2** 服务端默认拦截内网 feed，用 `RSS_READER_WEB_ALLOW_PRIVATE_FEEDS=true` 显式放开。
- **D3** 文章库（`RSS_READER_WEB_DATA_DIR`）与登录凭据（`RSS_READER_WEB_AUTH_STATE_FILE`）分开存放。
- **D4** 多端共享 `app_state` 与设置，后写覆盖。

## 7. 与 2026-10-20 问题的关系

- 抓取超时修复（5070d71）在原生抓取路径上，服务端模式直接继承。
- 大 feed 超出 localStorage 配额的问题，在服务端模式下不再存在；静态 Web 包（若按 D1 保留）
  仍受这个配额限制，需要在文档里写明。
