# 阅读位置改为操作时采集

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：6ecf189
- 相关 commit：本记录所在的功能提交（父提交 6ecf189）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

落实用户提出的优化：连续上下滚动不扫描位置，在点击操作、切换文章或返回时采集旧页面位置。保留本次运行内的锚点恢复和短暂图片布局校正。

## 影响范围

- UI bridge、阅读位置 runtime、Android 返回 host hook、Web 启动时的历史事件装配。
- Web、Linux/Windows/macOS 桌面、Android 共用的位置语义；本轮实际运行 Web 和 Linux。
- 无新增依赖、数据库迁移、CLI 或配置交换变化。

## 关键变更

| 文件 | 变更 |
| --- | --- |
| `crates/rssr-app/src/ui/reading_position.js` | 移除 scroll 监听和逐滚轮扫描；操作时采集锚点；每次进入页面只发送一次轻量输入取消事实；响应序号过滤过期命令。 |
| `crates/rssr-app/src/ui/reading_position.rs` | Rust 只保存 capture，输入及布局探测不覆盖锚点；提供 Android 返回前有限等待的采集能力；增加非 capture 不覆盖和顶部归零测试。 |
| `crates/rssr-app/src/hooks/use_mobile_back_navigation.rs` | Android 返回先采集，合并等待期间的重复返回，路由已变化则不执行旧返回；取消任务也释放等待标记。 |
| `crates/rssr-app/src/bootstrap/web.rs` | 先于 Dioxus history 注册浏览器 popstate 通知，避免旧正文卸载后才采集。 |
| `crates/rssr-app/src/bootstrap.rs`、`crates/rssr-app/src/main.rs` | 导出并在 Web launch 前调用上述 host 初始化。 |
| `scripts/browser/rssr_reading_position_acceptance.cjs` | 可复用 Playwright 回归：2000 段长文连续滚动零布局读取、浏览器返回、键盘切文、顶部/末尾和 native capture 事件。 |
| `README.md`、`docs/user-guide.md`、`docs/design/frontend-command-reference.md` | 说明操作时采集与内部桥接边界。 |
| 本文件 | 记录验证与风险。 |

### 实际接口变更

- 内部 `Fact` 和 `Command` 新增 `sequence: u64`，只用于 UI bridge 响应时效。
- 内部 `#[cfg(target_os = "android")] pub(crate) async fn capture_current_position()`。
- 内部 Web host `pub(crate) fn install_history_capture()`。
- 内部 DOM 事件 `rssr-history-leave`、`rssr-capture-position`；后者通过 detail.fact 返回一次测量。
- `navigate_back(Navigator, Option<AppRoute>) -> bool` 签名不变，Android 返回前最多等待 500ms；失败仍返回，不因采集失败阻断导航。
- application/domain trait、ReaderEntrySnapshot、repository、CLI、数据库和 data-* 主题接口全部不变。

## 验证与验收

日志位于忽略目录 `target/reader-operation-capture/`。

### 自动化验证

| 命令 | 退出码 / 结果 |
| --- | --- |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0；`clippy-verified.log` |
| `cargo test --workspace` | 0；304 passed、2 ignored；`tests-verified.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown` | 0 |
| `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings` | 0 |
| `cargo check -p rssr-app --target aarch64-linux-android` | 0；沿用已安装 SDK/NDK 环境 |
| `cargo build -p rssr-app` | 0；用于原生位置回归 |
| `dx build --platform web --package rssr-app --locked` | 0；输出仍提示 dx 0.7.10 与 Dioxus 0.7.9 版本差异，未升级依赖 |
| `git diff --check` | 0 |

### 浏览器与原生验收

- Playwright / Chromium 151，360×800 和 1280×800：`bun scripts/browser/rssr_reading_position_acceptance.cjs`，退出 0；两个视口下连续滚动的正文块布局读取均为 0，浏览器返回、键盘切文、顶部/末尾和 native capture 事件均通过。使用既有安装的 Playwright、Chromium，路径由 NODE_PATH / CHROME_BIN 注入。
- 同一 Web bundle 的原 Task 2 位置回归：`bun target/task2-validation/positions.cjs target/reader-operation-capture/positions-verified.json`，退出 0。覆盖列表分页、筛除文章后回退、点击切文、输入中止校正、刷新清除记忆和高亮到期。
- 延迟图片回归：`bun target/task2-validation/delayed-image.cjs`，退出 0，两个视口均恢复正确。关闭浏览器自身 scroll anchoring，图片延迟 650ms。
- Linux WebKitGTK 2.52.6 / WSLg，1280×900：原生 Inspector DOM click/scrollTo 回归退出 0；列表 2500→2500px、正文 3200→3200px。二进制包含本轮全部原生逻辑；其后只修改 Web 启动捕获顺序。强制软件渲染与禁用合成仅用于兼容性验收，不能当帧率验收。

### 失败与修正记录

- 初轮旧位置脚本在重新打包期间 reload 超时；构建完成后复验通过。
- 新增测试最初发现浏览器返回后位置归零：页面挂载后安装的 popstate 监听晚于旧正文卸载；改为 Web launch 前注册通知后该断言通过。
- 新测试初版假设 seed 中文章 1 总有下一篇，实际没有；改为选择存在的相邻方向，不改变产品导航语义。
- 一次临时脚本生成的 shell 引号错误（退出 2）和 `bun` stdin 调用方式错误（退出 1），均修正后重跑，未改动产品数据。

## 结果与风险

- 已移除连续滚动的正文扫描成本；恢复时仍最多校正 2 秒，操作边界仍做一次锚点扫描。
- 本轮不宣称固定 FPS 或所有卡顿都已消失；上轮诊断表明 WSLg 呈现也可能影响流畅度。
- 未运行：Android 实机/模拟器、Windows/macOS 界面和物理滚轮帧率验收；Android 编译通过不代表实机行为通过。
- 未运行 wasm 数据适配器契约 harness：本次未变更浏览器持久化或 application/infra 适配器，仅改变 Web host 事件安装顺序；相关能力由浏览器导航回归覆盖。
- 新行为仍只在本次运行内保存位置，关闭进程和 Web reload 不恢复。自定义主题未新增接口。
- 未 push、tag、release；保留任务外 worktree 和忽略目录数据。

## 给下一位 Agent 的备注

先看 `reading_position.js`、Rust runtime 和本轮新增浏览器脚本。不要恢复每帧/每滚轮的锚点扫描来补漏导航；新增非 DOM 导航入口需在旧正文仍存在时采集。Android 返回采集必须保持超时与重复操作保护。可从上一份 `2026-09-26-reader-scroll-performance.md` 查看优化前诊断。
