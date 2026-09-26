# 主线推送与发布前浏览器验收修复

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：105b7474579276a910e535dac03cf810c42caf2a（本轮开始）
- 相关 commit：105b747 已推送；验收脚本与本记录归入本记录所在提交
- 相关 tag / release：远端最新为 v0.1.19；候选 v0.1.20，本轮尚未创建或推送 tag
- 状态：`validated`（本地修复验证通过；新提交的完整远端 CI 尚待执行）

## 工作摘要

按用户授权将 main 的 15 个本地提交推送至 origin，并核对下一版本的发布条件。首次远端 CI 暴露 Amethyst 浏览器场景缺少真实用户滚轮输入；本地发布预检另暴露固定虚拟时间截图无法可靠等待异步初始化。两处均在验收脚本内修复，未改变产品行为或降低断言要求。

## 影响范围

- 模块：`scripts/browser/`、两个浏览器 smoke shell 入口。
- 平台：Web、`rssr-web` 部署壳；Android、原生程序无源码变化。
- workflow / 文档：工作流本身未改；发布预检说明、环境限制索引和本交接。
- 公开契约：application/domain trait、端口、数据库 schema、产品 `data-*` 接口均未改。新增脚本接口为 `capture_smoke_page.mjs URL READY_SELECTOR OUTPUT_PREFIX [CHROME_BIN]`，支持 `SMOKE_PAGE_TIMEOUT_MS`（默认 45000）。

## 关键变更

- `scripts/browser/capture_smoke_page.mjs`：复用现有 CDP 工具，等待真实页面就绪，从同一页面生成 DOM 与截图；总等待有上限，退出时终止本次 Chrome 并清理独立 profile，无新依赖。
- `scripts/run_static_web_reader_theme_matrix.sh`：等待 Reader 正文与 `data-position-ready="true"` 后采集，替代两个固定虚拟时间 Chrome 进程。
- `scripts/run_rssr_web_browser_feed_smoke.sh`：等待 helper 的实际成功状态；支持既有 `NODE_BIN`、`CHROME_BIN` 选择方式。
- `scripts/browser/rssr_small_viewport_assertions.mjs`：刷新失败场景复用成功场景已有的 `beginManualScroll`。位置恢复会在短期内校正布局，程序调用 `scrollTo` 不等于用户输入；真实滚轮才会取消校正。保留原来的正文身份及 1px 误差断言，增加前后位置诊断。
- `docs/testing/release-ui-regression-checklist.md`：说明真实就绪等待和脚本运行依赖。
- `docs/testing/environment-limitations.md`：记录 Fake-IP DNS 与现有 SSRF 规则的冲突，不放宽地址检查。

## 验证与验收

### 推送与首轮远端验证

- `git fetch origin`：退出 0。
- `git -c push.followTags=false push origin HEAD:refs/heads/main`：退出 0；`5e0d8e5..105b747`，仅 main。
- `git ls-remote origin refs/heads/main`：退出 0，远端为完整 `105b7474579276a910e535dac03cf810c42caf2a`。
- [CI 36248375463](https://github.com/Develata/RSS-Reader/actions/runs/36248375463)：失败。21 个 job 中 `web-ui (amethyst-glass)` 与依赖它的汇总 `lint-and-test` 失败，其余通过，包括 Android smoke、原生、wasm 与其他主题。
- [Docker 36248375497](https://github.com/Develata/RSS-Reader/actions/runs/36248375497)：成功。main 执行镜像构建和健康检查，不推送版本镜像。

### 本地自动化验证

本轮复用 105b747 的已有 debug Web 构建；Amethyst 对照使用上述 CI 下载的同一份 release Web 产物，放在隔离的 `target/tag-readiness/repro/target/`，未覆盖已有 release 构建。运行环境为 WSL/Linux、Bun、Playwright 自带 Chromium 1234；Chrome 路径通过 `CHROME_BIN` 指定。

| 命令 / 场景 | 结果与退出码 |
| --- | --- |
| `bash scripts/run_release_ui_regression.sh --debug --skip-automated --skip-build --with-rssr-web --with-fixed-smokes --no-serve --log-dir target/tag-readiness/preflight` | 退出 1；旧虚拟时间采集停在初始化 helper，无法读到 Reader |
| 同一预检，`--log-dir target/tag-readiness/preflight-final` | 退出 1；初版就绪条件只判断 loaded，可能仍显示“正在加载”；已收紧为正文存在且位置就绪 |
| 同一预检，`--log-dir target/tag-readiness/preflight-settled` | 退出 1；部署壳基础 smoke、5 个阅读主题、默认小视口 128 条断言通过，随后真实远端 feed 代理因 Fake-IP DNS 返回 HTTP 400；整套不能记为通过 |
| `NODE_BIN=bun bash scripts/run_rssr_web_browser_feed_smoke.sh --skip-build --port 18114 --log-dir target/tag-readiness/deploy-browser` | 退出 0；真实浏览器登录、添加同源 fixture、刷新、进入文章列表通过 |
| 隔离目录中 `RSSR_REPO_ROOT=/home/deve/gitclone/RSS-Reader NODE_BIN=bun bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build --preset amethyst-glass --port 8125 --log-dir /home/deve/gitclone/RSS-Reader/target/tag-readiness/amethyst-diagnostic` | 修复前退出 1；正文节点相同，但 scrollY 为 68 → 0，复现远端失败 |
| 同一命令，修复后产物目录 `amethyst-fixed` | 退出 0；128 条断言通过，scrollY 为 68 → 68，正文节点保持；无新增控制台异常 |
| `node --check scripts/browser/capture_smoke_page.mjs` | 退出 0 |
| `node --check scripts/browser/rssr_small_viewport_assertions.mjs` | 退出 0 |
| `shellcheck scripts/run_static_web_reader_theme_matrix.sh scripts/run_rssr_web_browser_feed_smoke.sh` | 退出 0 |
| `node scripts/browser/capture_smoke_page.mjs about:blank body target/tag-readiness/node-capture "$CHROME_BIN"` | Node v24.18.0，退出 0，生成 DOM 与截图 |
| `SMOKE_PAGE_TIMEOUT_MS=1500 bun scripts/browser/capture_smoke_page.mjs about:blank '#never-ready' target/tag-readiness/timeout-check "$CHROME_BIN"` | 预期退出 1，明确报告 1500ms 超时；进程检查未见本次临时 profile 的残留 Chrome |
| `git diff --check` | 退出 0 |

真实代理失败证据：`getent ahostsv4 www.ruanyifeng.com` 返回 `198.18.0.172`，其他公共域名也落入 `198.18.0.0/15`；`feed-proxy.headers` 为 HTTP 400，响应提示禁止内网或本地地址。这是现有防护的预期拒绝，未修改网络设置或安全规则。

本次没有在本地重复整套 cargo / wasm harness：Rust 生产代码未改，105b747 的完整本地记录见 [刷新边界与写入成本](2026-09-26-refresh-bounds-and-write-cost.md)，首轮远端相应 job 也已通过。脚本修复提交推送后仍应以新提交的完整远端 CI 为最终主线结论。

### 浏览器与图像验收

- Chromium 自动化：360×800 @ DPR 3 与 1280×800，Amethyst 的列表、订阅、设置、阅读、刷新失败、位置保持等 128 条断言通过。
- 1440×1200 阅读主题矩阵：默认、Atlas Sidebar、Newsprint、Amethyst Glass、Midnight Ledger 均通过原有结构和主题样式断言。
- 人工查看生成截图：默认 / Amethyst 桌面阅读页与 Amethyst 360px 阅读页，长标题、来源和元信息可换行，未见横向越界；不据截图宣称帧率改善。
- 未运行本轮 Windows / macOS / Android 真机或模拟器交互：本轮为浏览器验收脚本修复，现有 Android 编译 / 打包检查不等于设备验收。未重复原生系统浏览器打开测试。

## 结果

- 已验证：105b747 成功推送；两个验收脚本问题均有复现和修复后的真实浏览器证据。
- 推断：首轮 Amethyst 失败由测试输入未取消位置校正引起，同一 release 产物的前后对照支持此判断，未发现需修改产品的证据。
- 未验证：修复提交的远端完整 CI（推送后再看）；本机无法覆盖正常 DNS 下的真实远端 feed 代理；设备交互和下一版本发布安装包。
- 本轮未创建 tag、未触发 Release、未发布 Docker 版本镜像。推送 `v*` tag 会自动发布安装包与 Docker，不能把主线 CI 成功表述为 Release 已成功。

## 风险与后续事项

- 在不使用 Fake-IP DNS 的网络补跑真实远端代理 smoke，不能用同源 fixture 代替。
- 核对修复提交的 CI 全部成功后再判断候选 v0.1.20；发布说明保留设备验收边界。
- 产物保留在忽略目录 `target/tag-readiness/`。原有 `.handoff/` 排除规则与任务外 worktree 未动。

## 给下一位 Agent 的备注

- 首轮失败日志：`target/tag-readiness/ci-failed.log`；精确对照：`amethyst-diagnostic.log`、`amethyst-fixed/assertions.json`。
- 发布预检完整日志：`preflight-settled.log`；其中已经成功的子流程不要与整套退出 1 混为一谈。
- 用户已授权本地 commit 与 push。本轮只判断 tag 就绪程度，未把问题自动转成发布操作。
