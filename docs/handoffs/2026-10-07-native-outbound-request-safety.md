# 原生图片 Referer 与 WebDAV 出站边界

- 日期：2026-10-07
- 作者 / Agent：Codex，Windows LAPTOP-H6JEOCF0
- 分支：`fix/native-outbound-request-safety`
- 基线：`88b6abb32b992c7b19bb81ff5718ebb4cd8a6be1`；远端 main 与 CI success 已核对
- 当前代码 HEAD：`0bad743876b1e150680b4aa43f1d34c49623c378`
- 相关 commit：代码 / 测试 `0bad743876b1e150680b4aa43f1d34c49623c378`；本证据文档见包含本文件的后续提交
- 相关 tag / release：N/A；仅授权独立分支、push、Draft PR，不 merge、不发布、不打 tag
- 状态：`validated`（本地预推送检查；远端回执见 Draft PR）

## 工作摘要

仅修复原生正文图片请求泄露 Referer 和 WebDAV 目标变化后仍附带认证 / 配置正文的问题。
先在基线生产代码上运行 loopback 失败回归，再修改两个 infra 文件。
没有叠加 PR #24 / #25，没有前后端迁移或 application/domain 职责变化。

## 影响范围

- 模块：`crates/rssr-infra/src/fetch/client/body_asset_localizer.rs`、`src/config_sync/webdav.rs`。
- 测试：新增原生出站请求回归；从既有 `test_webdav_local_roundtrip` 提取 loopback request-capture，保留其完整配置恢复、旧订阅删除、设置和请求路径断言。
- 平台：原生 desktop / Android 网络适配；WASM 仅编译兼容检查。这两处生产模块本来就受 `not(wasm32)` 门控。
- Web / Docker：未修改 Browser WebDAV、Fetch/CORS 重定向、Browser 下载预算或 rssr-web 代理。没有宣称修复 Web 端。
- workflow / 依赖 / 锁文件：无变更。

## 关键变更

### 图片 Referer 和跳转

- 首次请求仅发送 HTTP(S) 文章的序列化 origin 加 `/`，不含 userinfo、path、query、fragment。HTTPS 文章到 HTTP 初始图片不发送 Referer。
- 客户端显式 `referer(false)`，后续跳转仍沿用安全的文章 origin 或不发送。锁定 reqwest **0.12.28** 的 `redirect.rs::make_referer/on_request` 默认会从上一跳 URL 重建 Referer，保留 path/query；只清洗首跳不足以阻止签名图片 query 泄露。
- 保留 reqwest `Policy::limited(10)`；允许 HTTP(S) 同源及跨源图片跳转、HTTP→HTTPS，拒绝 HTTPS→HTTP、非 HTTP(S) 及带 userinfo 的目标。
- 原有单图 / 总量 / HTML / 图片数 / 并发 / 8–10 秒整体 timeout / MIME 预算未变。客户端构建失败不回落到丢失策略的默认客户端；请求和读取错误去掉 reqwest URL。
- generation 与正文 hash 写回保护未变，仍由既有刷新和持久化测试覆盖。

### WebDAV 认证前的目标与跳转校验

- Remote Path 拒绝绝对 URL、新 authority、反斜线、控制字符、首尾空白；保留合法相对路径和单前导 `/` 相对于 endpoint collection 的既有语义。
- 使用解析后的 URL origin 比较 scheme / host / effective port；不用字符串前缀。路径不预先解码，编码分隔符保留，点路径、query、fragment 继续由 URL parser 处理。
- `take_url_credentials` 原样复用，Basic 凭据只解码一次。HTTP / loopback / 私网 endpoint 仍可用。
- 重定向限制为初始已校验请求的同 origin、无 userinfo、最多 10 跳，跨 origin 307/308 不发送配置正文。关闭自动 Referer，避免 endpoint/path/query 进入后续头部。
- 锁定的 reqwest 0.12.28 / tower-http 0.6.11 对 PUT 的 301/302/307/308 保留方法和正文；303 会变 GET。因此这个共享 GET/PUT 客户端保守拒绝全部 303。
- 30 秒整体请求 timeout、10 秒 connect timeout、4 MiB 下载预算、404→None 均保留。未实现逐跳手工请求循环，整体 timeout 不因跳转重新计时。网络 / 读取错误调用 `without_url()`，策略错误只用固定文案。

## 验证与验收

全部新增网络回归只访问本机临时 loopback 端口，使用 `fixture` / `fake` 凭据和合成配置。未访问真实 WebDAV 服务或使用真实密钥。

| 项目 | 状态 | 结果 / 证据 |
| --- | --- | --- |
| 基线 `cargo test --locked -p rssr-infra --test test_native_outbound_requests` | failed（预期） | 3 passed、9 failed；证实首跳完整 URL、后续签名 query、降级首跳 Referer、userinfo、跨站 WebDAV、303 假成功、错误 query 泄露 |
| 修复后 infra lib + 出站回归 + WebDAV roundtrip | passed | 48 单元 passed / 1 原有 ignored；14 出站回归和 1 原有 roundtrip passed |
| `cargo fmt --all --check` | passed | 无格式差异 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | passed | 首次新增测试触发 `chunks_exact_to_as_chunks`，修正为 `as_chunks` 后通过 |
| `cargo test --workspace --locked` | passed | 354 passed、0 failed、2 原有性能 probe ignored |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | passed | 原生模块隔离与 Web 编译兼容 |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | passed | 本机 NDK 28.2.13676358，API 34；非 APK / 实机验收 |
| 仓库 release-ui `--no-serve` 聚合自动门禁 | passed | wasm-check / app-tests / host-contracts / web-tests 全部 passed；其他未选择阶段 skipped |
| 精确远端 head CI / Draft PR | not-run（本文件冻结时） | 本文件是本地预推送检查点；push 后的精确 head 与最终 CI 回执记录在 Draft PR 和最终交付中 |
| Desktop/Android 手工或实机、实际 HTTPS fixture | not-run | 本轮为 loopback HTTP 捕获 + HTTPS/default-port/IPv6 等纯 URL 策略测试；不冒充 TLS 或实机验收 |
| 本地完整发布 UI / Browser 网络安全验收 | not-run | 本轮无 UI / Browser 行为修改，不作为本批修复声明 |

原始日志和命令元数据在 `target/outbound-security/`；[可追踪证据](../testing/evidence/2026-10-07-native-outbound-request-safety/README.md) 包含基线失败、修复回归、Clippy、workspace 汇总、WASM/Android 编译和聚合 summary。聚合命令为 `cargo run --manifest-path scripts/release-ui/Cargo.toml --locked --target-dir target/release-ui-runner -- --no-serve --bash C:/Users/QQ/scoop/apps/git/current/bin/bash.exe --log-dir target/outbound-security/aggregate`。

## 结果

- 本地安全回归、全 workspace、WASM/Android 编译与聚合自动门禁均通过；仅交付 Draft PR，不执行合并或发布。
- 性能仅核对复杂度：增加有限 URL 校验和 origin 字符串，复用原客户端与有界重定向；没有新增无界循环、逐跳整体超时重置或配置正文复制。没有性能基准或性能提升声明。

## 风险与后续事项

- 少数要求完整文章路径 Referer 的防盗链图片可能不再本地化；现有失败回退仍保留远端图片地址。origin-only 不等于完全隐藏文章站点。
- WebDAV 303（包含下载 GET）统一拒绝；依赖跨站跳转、绝对 Remote Path、多个前导斜线或未编码反斜线的设置需要改为最终 endpoint 与合法相对路径。301/302/307/308 同源 PUT/GET 有实际方法、正文、认证捕获断言。
- 点路径允许离开 collection，但始终留在 endpoint origin；未新增目录沙箱。编码路径保持 URL parser 语义，不代表远端服务器如何解释路径。
- 原有图片日志仍可能记录 asset/entry URL；本轮出站 Referer 与 WebDAV 错误去 URL 不代表一次完整日志隐私审计。
- Browser WebDAV 认证 / Fetch/CORS 重定向 / 下载预算留到下一批；OPML、搜索、登录限流和其他审计项不在本批。

## 给下一位 Agent 的备注

- 全部实现与验证在 `E:/gitclone/RSS-Reader/`；没有转移 C 盘工作副本、reset、stash 或修改系统网络/权限。
- 原有 25 项工作区状态（技能与 `.specify` 删除、两份 10-03 handoff 修改、`.workbuddy/`）已记录 SHA-256 并复核；不纳入本批提交。初始清单位于 `target/outbound-security/initial-worktree.json`。
- 本机相关 `.agents/skills` 文件在任务开始时已删除，未恢复或改写。根 AGENTS 和 handoff 模板已读取；未需要读取 Codex memory。没有启动子 agent 或改动模型 / fast 设置。
- 后续升级 reqwest/tower-http 时，必须保留跨跳 Referer 和 PUT 方法 / 正文回归。


## 同日复核补充：循环跳转与认证断言

- 基线 HEAD：`3b7fda50f204dbb16e62382edf94825bdc9be225`；PR #26，原分支继续。
- 本次测试提交：commit: pending（此处为预提交验证快照，提交后以包含本节的提交为准）。生产代码仍为 `0bad743876b1e150680b4aa43f1d34c49623c378`。
- 影响范围仅 `test_native_outbound_requests.rs` 和验收记录；未修改生产功能、依赖、共享测试夹具或 Browser WebDAV。
- 已读取实际锁定 reqwest 0.12.28 的 `redirect.rs`：每次响应先将当前 URL 加入 previous，仅在 `previous.len() > max` 时返回 `TooManyRedirects`。上限 10 对应首次请求加 10 次已跟随跳转，故两个循环测试都精确断言 **11 次请求**。
- WebDAV 同时 downcast 到 `reqwest::Error` 并检查 `is_redirect()`，根因必须是锁定版本的 `too many redirects`；保留错误无 URL 秘密断言。图片公开 API 吸收抓取错误，因此检查精确请求数及原始远端 src 回退，不为测试暴露或改写生产错误接口。
- 同源 301/302/307/308 的 GET/PUT 每一跳均直接断言 `Some(Basic(base64("fixture%40user:fake%3Apassword")))`，不再与可能为空的首跳头比较。跨站目标零请求断言原样保留。

### 本次实际验证

- 三个仅改测试夹具的负向对照：图片首跳 503、WebDAV 首跳 303、重定向场景移除全部 Basic 认证，均以 exit 101 在预期断言处失败（前两项实际 1 请求 / 预期 11，后一项实际 None / 预期 Basic）。这些场景此前的宽松断言可能通过。对照结束后测试文件逐字节恢复；未临时修改生产代码。
- `cargo test --locked -p rssr-infra --test test_native_outbound_requests --test test_webdav_local_roundtrip`：passed，14 + 1；负向对照恢复后再次通过。
- `cargo fmt --all --check`：passed。
- `cargo clippy --locked -p rssr-infra --all-targets -- -D warnings`：passed。
- 本地全 workspace / WASM / Android / UI 聚合：本轮 not-run；只变更原生集成测试，前批本地结果保留为历史，不冒充本轮重跑。新 head 的完整 CI 会跟踪至终态并将精确 SHA / 回执写入 PR。
- [实际断言失败与最终检查证据](../testing/evidence/2026-10-07-native-outbound-request-safety/test-strengthening.txt)；完整本地日志在 `target/outbound-security/test-strengthening/`。
- 状态：本地 validated；未发现生产问题，不需扩大范围。既有兼容边界保持不变，不 merge、不发布、不打 tag。
- 工作区：开始时重新快照 25 项任务外改动；本地 `.agents/` 已不存在，未恢复其已删除内容。继续在 Windows `LAPTOP-H6JEOCF0` 的原 E 盘目录操作。
