# /feed-proxy 不可信文档隔离

- 日期：2026-10-07
- 作者 / Agent：Codex，Windows LAPTOP-H6JEOCF0
- 分支：`fix/feed-proxy-content-isolation`
- 基线 / 开始 HEAD：`88b6abb32b992c7b19bb81ff5718ebb4cd8a6be1`（fetch 后远端 main 一致）
- 产品修复 commit：`936bce15e5f01e7d5ec3834f005f061f790d8f4f`
- CI 清理修正 commit：本记录所在提交；提交前状态为 `commit: pending`
- Draft PR：[#25](https://github.com/Develata/RSS-Reader/pull/25)
- tag / release：N/A；只提交、推送、创建 Draft PR，不 merge/tag/release
- 状态：`validated`（本地确定性验收）；远端最终 head 的 CI 另见 PR checks

## 工作摘要

上游 Content-Type 与正文原样经认证代理送达应用同源时，顶层 HTML/SVG 文档可以执行脚本并访问 BrowserStore 所在的 localStorage。本次先通过无害确定性夹具复现，再在生产响应构造处强制文档 sandbox。未发现或声称线上利用事件。

## 影响范围

- 产品仅修改 `crates/rssr-web/src/proxy.rs`：提取原响应构造函数，并增加两个安全响应头。
- 新增 `cfg(test)` Rust 夹具和复用 `cdp_session.mjs` 的无依赖 Node 驱动；生产 binary 无夹具路由或开关。
- 更新 Web 部署/代理验收文档和 CI；不改 domain/application/infra/前端业务、鉴权语义、SSRF/DNS pinning、资源限制或依赖锁文件。
- 运行平台：Windows 原生 Rust 1.98.1、Node 24.21.0、Chrome 154.0.8037.93。Linux 验收交给本 PR 的实际 CI；本机未测 Firefox/WebKit、macOS 或 Android 运行时。

## 关键变更

- 所有已取得的上游响应（含错误状态与 304）强制设置：
  - `Content-Security-Policy: sandbox; default-src 'none'; base-uri 'none'; form-action 'none'`
  - `X-Content-Type-Options: nosniff`
- sandbox 没有 `allow-scripts` 或 `allow-same-origin`：文档禁止脚本，并获得 opaque origin。上游 CSP 不转发，不能削弱策略；不依赖反向代理额外设置 CSP。
- 保留 MIME/charset、正文原始字节、最终 URL、ETag、Last-Modified、状态码。选择理由：`feed_body` 读取 charset；`subscription_probe` 对非标准 HTML 开头仍需 MIME 分类；browser refresh 用 MIME 识别登录壳；HTML 相对 feed 链接依赖最终 URL。仅 XML 白名单会丢掉 HTML 发现，单独 nosniff 也不能隔离明确标记为 HTML/SVG 的内容。
- 解析/DNS/传输失败继续使用原有纯文本错误；上游抓取、逐跳验证与 8 MiB 有界读取未更改。
- CI 增加 `feed-proxy-isolation`，复用同次 `web-smoke` 产物及现有 Chrome 安装 action，并纳入已有 `lint-and-test` 汇总；未修改 GitHub 保护规则或权限。

## 验证与验收

全部新增构建、profile、认证状态、临时数据与日志放在 E 盘独立 worktree 的 `target/`。Cargo 使用已有依赖缓存并离线执行，无新增 C 盘依赖下载；`TEMP`/`TMP` 和 `CARGO_TARGET_DIR` 显式指向该目录，dev/test debug info 设为 0。

### 修复前后证据

- [修复前单测失败](../testing/evidence/2026-10-07-feed-proxy-isolation/before-unit.txt)：在仅提取原生产构造函数、未加安全头时，断言实际 CSP 为 `None`。
- [修复前浏览器](../testing/evidence/2026-10-07-feed-proxy-isolation/before.json)：`--expect-vulnerable` 成功。6 类顶层文档（HTML/SVG/XHTML/XML/缺 MIME/404 HTML）全部执行无害脚本，读出固定测试值并改写专用 localStorage 键。HTML/SVG/XHTML 的无隔离对照亦成功。
- [修复后浏览器](../testing/evidence/2026-10-07-feed-proxy-isolation/after.json)：同组文档均不执行脚本，读取/写入 localStorage 抛出 `SecurityError`，返回测试页检查存储未被改变。iframe 也隔离；无隔离对照仍能执行，排除夹具失效。
- 夹具用 `reqwest::ResponseBuilderExt::url` 注入固定上游响应，然后调用真实生产的 bounded reader/response builder。测试服务使用真实登录、`require_auth`、`session-probe`；没有为了 loopback 放宽生产网络策略。重定向最终 URL 是注入数据，未把它说成实际外部 HTTP 重定向复现。
- 同一真实 WASM 客户端完成 RSS、Atom、Latin-1（Café）、HTML 发现相对候选 URL 的添加、手动刷新、文章读取。最终驱动还确认每次手动刷新收到新的代理响应；外部请求被 CDP 拒绝，无法靠 direct fallback 掩盖失败。fetch 验证 404/304、元数据、charset，超限仍为 502，实际私网目标仍为 400。
- 本地复用审计基线的既有 CI Web artifact（run `37418907663`），随后复制到本任务 target；前端源码无变化。`index.html` SHA256 `06d16f771e7b5646ed7fca11a1675e41b079a3a4f76a79ec3ef5a0b995f9d20e`，WASM SHA256 `5b5932a9b7d26424ec4b63396e958c8789ca213bc9dc04f88fcc5f7d0656829c`。不是本机新鲜 dx 构建；本 PR 的 CI 将重新构建相同源码并复验。

### 命令与结果

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`：通过。
- `cargo test --workspace --locked --offline`：通过，337 passed、0 failed、3 ignored（两个既有手动性能探针和新 browser server；后者已由 Node 驱动单独实跑）。包括真实 loopback WebDAV/正文限制等现有测试。
- `cargo test -p rssr-web --locked --offline`：20 passed、0 failed、1 ignored；新单测覆盖隔离头、上游宽松 CSP 不透传、全部原始字节和元数据保持、8 MiB 超限。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked --offline`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked --offline`：通过；本机已有 NDK `28.2.13676358`、API 24 clang/llvm-ar。CI 仍使用项目原锁定 NDK，不将本机 check 当作 APK/实机验收。
- `node scripts/browser/feed_proxy_isolation.mjs target/evidence/before --expect-vulnerable`：修复前复现与兼容性通过。
- `node scripts/browser/feed_proxy_isolation.mjs target/evidence/after-final`：修复后隔离与兼容性通过；设置 `CHROME_BIN` 和 `RSSR_PROXY_TEST_PUBLIC_DIR`，详见代理验收文档。
- `actionlint .github/workflows/ci.yml`、`node --check scripts/browser/feed_proxy_isolation.mjs`、`git diff --check`：通过。
- `cargo build -p rssr-web --locked --offline` + 本地真实 binary HTTP smoke：登录 303，私网请求 400，测试域也走真实校验而非夹具；公开阮一峰 Atom 请求在目标地址校验阶段返回 400。该公开网络 smoke **未通过**，没有放宽网络策略来取得绿项。

### 开发中失败与平台限制

- Windows 执行沙箱中的 Chrome GPU 子进程以 `-1073741790` 退出，未到页面验收；正常宿主权限下、全新 E 盘 profile 复跑通过，未添加 `--no-sandbox` 或修改浏览器安全策略。
- 初版测试服务漏接 `/session-probe`，使真实前端返回登录页；补接现有 handler 后完整修复前/后通过。未修改产品认证代码。
- 首次 Linux CI [run 37593110460](https://github.com/Develata/RSS-Reader/actions/runs/37593110460) 中，6 类隔离断言及 UI 流程已经走到截图，随后驱动清理阶段向正在退出的 Chrome stdin 写入换行，触发未处理 `EPIPE`，未保存最终 JSON。因此该次 job 判失败，不能当作完整通过。修正只给 Rust fixture server 建立 stdin pipe，以 EOF 停止服务，Chrome stdin 使用 `ignore`；不改产品代码、断言、浏览器 sandbox 或安全策略。Windows 同一驱动补验通过；Linux 以修正提交的最终 CI 为准。
- 外部 feed smoke 被本机 DNS/网络返回的受限地址拦截；精确 HTTP/DNS 证据留在任务 target。确定性安全与兼容性验收不依赖这个外部站点。
- 现有全套发布 UI、三项 wasm contract、五主题矩阵、Android APK/实机验收未在本机重跑；没有相应产品变更。远端 CI 保留既有全量矩阵，新隔离测试单独报告，不把未跑项目算作本地通过。

## 结果

本地确定性安全修复与兼容性门禁通过，生产修改为局部响应边界加强。无业务迁移、无数据库变更。交付保持 Draft PR；远端 checks 的实际状态以最终 head 为准，不用本地通过替代远端结果。

## 风险与后续事项

- 需要支持 CSP sandbox 的现代浏览器；本机动态证据限上述 Chrome 版本，不推断所有浏览器已经实测。
- 反向代理应保留应用响应头；没有部署或改动线上配置。既有恶意内容若已在修复前执行所产生的影响不在本次事后取证范围内。
- 外部网络 smoke 需在 DNS 返回真实公网地址的环境补验；本次不改用户网络配置。

## 给下一位 Agent 的备注

- 独立 worktree：`E:/gitclone/RSS-Reader/target/feed-proxy-isolation-20261007/worktree`；本机完整日志在其 `target/evidence/`。
- 主目录仍在 `main@b745a846`：22 项既有删除、两份 handoff 修改及 `.workbuddy/` 未动，未 reset/stash/切分支或快进。
- PR24 worktree 仍为 `investigate/directory-scroll-20261007@d9e1577`、干净；仅只读核对和复用其基线 Web artifact，没有改动其源码、profile、数据或测试状态。
- 已核对 AGENTS.md、`.agents/skills` 的 spec-kit 条目及本机 memory_summary；本修复无 spec-kit tasks 计划，不新增 spec 工作流。
- 实现完成后只显式 stage 本批文件；远端 PR URL、最终 SHA、CI run/check 结果在最终回报与 PR 描述记录，任务 target 保存远端 receipts。
