# 原生化准备复核：下拉刷新释放与阅读加载状态

- 日期：2026-10-24
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 审查基线 / 开始时 HEAD：`bc417992`
- 相关 commit：与本记录同一笔本地提交；用 `git log -1 -- docs/handoffs/2026-10-24-pull-refresh-release-review.md` 定位
- 相关 tag / release：N/A；没有 push、tag 或发布
- 状态：`validated`（本地自动化；实机覆盖限制见下文）

## 工作摘要

用户要求 review and fix。本轮复核原生化前置方案所涉及的现有 Rust 手势状态和浏览器适配，修复释放判断缺陷和 Reader 加载状态误报，再完善后续模块化应保留的失败路径。较大的 JS 迁移和模块拆分尚未实施；Claude CLI 仍未登录，没有取得其评审或共识。

## 影响范围

- 模块：`rssr-app` 全局文章页的 `PullState`，Reader state / facade / 根组件，现有小视口浏览器回归及新增 `pull_refresh_contract.mjs`。
- 平台：共享前端逻辑影响 Web / Windows / Linux / macOS / Android；运行验收使用隔离 Edge，Android 只做编译检查。
- 额外影响：前端命令契约、前置方案和本交接记录。没有新增依赖、crate、数据库迁移或业务接口；视觉和 80 CSS px 阈值保持不变。

## 审查发现与关键变更

### P2：未完整释放与最终坐标变化可能误触发刷新

- 原 `PullState::update` 只排除 `touches > 1`，因此仍有一个触点留在屏幕上的 `End` 会被视为合法释放。
- 原 `End` 直接读取上一次 `Move` 保存的距离；即使最终松手坐标已回退到阈值以下或横向偏离，仍会沿用 armed 状态刷新。
- 新增两条 Rust 回归后先在旧实现执行，均失败。证据：`%TEMP%/rssr-nativeprep-review-before.log`。
- 新浏览器回归反测旧 Web 构建，向真实 DOM adapter 注入“仍有一指”的边界 `TouchEvent`，触发一次刷新轮次（fixture 的两个订阅请求，before=0 / actual=2）。证据：`target/nativeprep-baseline/pull-release-before.json`。这是边界事件注入，不是物理触屏或可信 CDP 手势复现。
- 修复将触点数按事件区分：Start / Move 须为 1，End 须为 0；任何不匹配都取消当前手势。End 与 Move 共用最终位移和方向判断，释放完成后立即回到 Idle。
- 既有单元测试的 End 样本错误地使用 `touches=1`，现已改为符合松手语义的 0。新增覆盖部分释放、最终回退、最终横向偏移、最终恰好达到阈值、重复 End 与 Cancel。

### 浏览器回归补齐输入来源区别

- `pull_refresh_contract.mjs` 通过完整 DOM → JS adapter → Rust → shell 路径检查释放边界，不直接调用 Rust reducer 或伪造 UI 状态。
- 最终坐标和部分释放采用明确标记的 DOM 边界样本；普通双指介入与取消另外用可信 `Input.dispatchTouchEvent` 验证。
- 已接入现有 `rssr_small_viewport_assertions.mjs`；原有短拉、非顶部、正常刷新和共享 in-flight 检查继续运行。
- 更新前置方案，明确模块化应继承产品契约和正确的事件语义，不能只搬动旧实现及错误测试。

### P2：Reader 在加载中误报 loaded，验收可错误地检查占位内容

- 首轮修复后浏览器套件 272 项断言通过，但实际查看 `target/nativeprep-release-review/reader.png` 发现它仍显示“正在加载…”。该结果没有被当作最终视觉验收通过。
- 根因是 Reader 根组件只根据 error 是否存在选择 `error / loaded`，未检查 `content_loaded`。回归脚本虽然等待 `data-state="loaded"`，仍可能在正文读取完成前测量并截图。
- 在纯 Rust `ReaderPageState` 中增加当前路由文章的加载状态判断；根节点和原有位置就绪判断复用它。切换文章、开始加载、失败重试、完成分别得到正确的 `pending / error / loaded`，加载期间提供 `aria-busy="true"`。
- 已补状态切换回归，并让浏览器测量确认 fixture 的真实长标题、位置已就绪和 `aria-busy="false"`。默认阅读布局和样式没有变化。
- 首轮产物保留，最终复跑另写 `target/nativeprep-release-review-final/`，避免覆盖这份反映验收漏洞的截图。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-app --locked -j2 pull_refresh::tests::release_`：旧实现上 2 条新增回归失败，符合复现预期。
- `cargo test -p rssr-app --locked -j2`：最终代码 115 passed / 1 ignored / 0 failed（111 个主程序测试和 4 个样式契约测试；忽略项为既有性能探针）。日志：`%TEMP%/rssr-nativeprep-review-tests-final.log`。
- `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings`：最终代码通过。日志：`%TEMP%/rssr-nativeprep-review-clippy-final.log`。
- `cargo fmt --all --check`：通过。
- `node --check scripts/browser/pull_refresh_contract.mjs`、`node --check scripts/browser/rssr_small_viewport_assertions.mjs`：通过。
- `dx build --platform web --package rssr-app --locked`：最终代码通过，退出码 0；现有 dx 0.7.10 / Dioxus 0.7.9 的版本提示仍存在，没有更改工具或依赖版本。日志：`%TEMP%/rssr-nativeprep-review-web-final.log`。
- `cargo check -p rssr-app --target aarch64-linux-android --locked -j2`：最终代码通过。使用 NDK 28.2.13676358 / API 24；日志：`%TEMP%/rssr-nativeprep-review-android-final.log`。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:9229 --static-base http://127.0.0.1:8098 --artifact-dir target/nativeprep-release-review-final`：最终 273 项断言通过，0 个应用 Console 错误。25 条静态服务器缺少调试热重载端点的 WebSocket 日志单独列为已知忽略项。
- `node scripts/browser/rssr_reading_position_acceptance.cjs`：最终代码在 360 / 1280 × 800 两个视口通过返回恢复、键盘切换、输入法、捕获事件桥接和顶部/底部检查；连续滚动及编辑期间正文块布局读取为 0。输出保存在最终产物目录的 `reading-position.log`。
- 本轮未重新运行无改动业务层的完整 workspace 测试；运行了完整 app 测试和 workspace 全目标 Clippy。

### 浏览器检查与证据边界

- 使用独立浏览器和测试数据，没有操作用户订阅数据库。
- 上一轮同一基线的 262 项浏览器断言和阅读位置专项仍保留在 `target/nativeprep-baseline/`；本轮最终结果写入 `target/nativeprep-release-review-final/`，不覆盖原始基线和本轮首轮截图。
- 浏览器边界样本与可信 CDP 输入分别记录；没有把注入的部分松手事件称为真实硬件多指行为。
- 未运行 WebView2、Android 设备或 macOS / Linux 宿主验收，也未测量性能与内存收益。
- 最终桌面文章页、桌面 Reader 与窄窗口 Reader 的 PNG 哈希均与原始基线一致。已检查最终窄窗口目录展开效果；不把目录滚动时刻不同的截图宣称为逐像素一致。
- 最终窄窗口 Reader HTML 确认当前文章为 `reader:2`、`data-position-ready="true"`、`data-state="loaded"`、`aria-busy="false"`，包含完整测试长标题。
- 本轮创建的隔离浏览器与测试服务器已关闭。

## 结果

两项修复已完成最终测试、静态检查、Web/Android 编译及浏览器验收，并按本地提交授权交付。没有推送或发布；较大的原生化前置迁移仍待共同评审。

## 风险与后续事项

- 后续模块化继续以修正后的释放语义为基线。手势历史迁回 Rust、阅读位置控制器与目录状态解耦仍是待共同评审方案，本轮没有宣称已经完成。
- 当前 JS 中的嵌套滚动测量和方向摘要如何分工，仍需在迁移前权衡实际测量成本。

## 给下一位 Agent 的备注

- 修复入口：`crates/rssr-app/src/pages/entries_page/pull_refresh.rs` 的 `PullState::update`。
- 任务外 `.agents/skills/speckit-*` / `.specify` 删除、两份 2026-10-03 交接修改、Penpot 交接和附件保持原样，不纳入本次提交。
- 原生化前置盘点文档是本任务此前创建的文件，本轮补充复核结论后随本修复记录保留。
