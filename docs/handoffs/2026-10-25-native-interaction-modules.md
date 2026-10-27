# 原生界面前置工作：阅读位置与下拉刷新模块

- 日期：2026-10-25
- 作者 / Agent：Codex；Claude CLI 只读评审
- 分支：`feat/reading-workspace-20261024`
- 实施基线 HEAD：`730d29e3`
- 相关 commit：阅读位置 `238cfec5`；下拉刷新与本文更新同一提交（提交前 commit: pending；用 `git log -1 -- docs/handoffs/2026-10-25-native-interaction-modules.md` 定位）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

用户恢复 Claude 登录后，Codex 用英文与实际 Claude CLI 讨论原生 Rust 界面之前最值得推进的模块化工作。双方确认先独立阅读位置控制器，再迁移下拉刷新的剩余手势规则，暂缓目录文件拆分。保持现有七个 crate、facade/session → UiCommand → runtime → application/host 边界与界面样式。

## 影响范围

- 模块：`ui/reading_position.rs`、新增 `ui/reading_position/controller.rs`；`pages/entries_page/pull_refresh.rs` / `.js`、新增 `pull_refresh/gesture.rs`；浏览器手势回归脚本。
- 平台：共享前端影响 Web、Windows、Linux、macOS、Android；本轮验证 Web 浏览器和 Windows/Android 编译路径。
- 额外影响：前端命令契约与本交接文档；无依赖、数据库、配置格式、workflow 或发布变化。

## 关键变更

### Claude 共识与评审边界

- 真实会话：`52eee468-9625-4a4e-a4c2-60dd07a78b7f`。使用 `claude -p --tools Read,Glob,Grep --permission-mode plan --strict-mcp-config --output-format json`，后续复用该会话；没有编辑或执行 Shell 权限。候选评审、共识确认与两项实现复核的四次响应均成功且无权限拒绝。
- 范围确认原文：“No blocking issue remains. I confirm the bounded scope: B first, then A, C deferred, with your refinements.” B 为阅读位置，A 为下拉刷新，C 为目录拆分。
- 阅读位置采用独立的保存集合和每次桥接专属的恢复器；保留当前协议，未知事件不能中断桥接或误发归零滚动。
- 下拉刷新使用可空的触点与双方向嵌套滚动摘要。Rust 管理起点、上一触点、方向、排除条件、阈值和释放；适配器测量 DOM。无触点的结束/取消必须传入并终止手势。
- Codex 明确质疑双方向测量的性能成本；双方认同它可能比旧的单方向短路遍历更多祖先，尚未测量，不能宣称“开销可忽略”或性能提升。
- 目录状态转移核心已经是纯 Rust；本轮拆文件收益有限，且可能引入状态与 signal 同步风险，因此不动目录协议或时序。
- Claude 对阅读位置实现的静态复核未发现正确性问题，指出未知事件测试断言可加强；已用不同页码/坐标验证未知事件确实不覆盖已保存位置。Claude 没有代跑编译或浏览器测试。

### 已完成：阅读位置控制器

- 纯 Rust `SavedPositions` 管理运行期位置；`Restorer` 管理访问代次、两秒校正窗口、输入/捕获取消、锚点与像素回退及输出命令。
- Dioxus 外层仅管理桥接和生命周期，Android 返回前捕获仍走已有边界。共享集合的借用不跨越 await；恢复器随桥接重建，避免重挂载后的低代次事件被旧状态拒绝。
- 用类型明确事件；字段完整的未知事件被接受但不保存或滚动。已知事件协议和 `reading_position.js` 完全不变。
- 仅在操作边界捕获，不增加连续滚动正文扫描。12 项纯控制器测试覆盖保存/清理、访问隔离、页码回退、超时、未知事件、宿主捕获与命令字段。

### 已实现：下拉刷新规则收口

- 新增不依赖 Dioxus 的 `gesture.rs`，统一保存起点和上一触点，判断 80px 阈值、总方向、每次移动方向、控件/选区/多指/页面顶部排除、取消与最终释放。
- `pull_refresh.js` 不再保存手势历史或计算 eligible；一次祖先遍历汇总上/下两个方向的滚动余量，仅在两者都已确定为真时提前结束，不传祖先数组。
- Dioxus 外层保存每次桥接专属的控制器，只在显示阶段变化时写 signal；沿用原来的 data-state、提示和 `shell.manual_refresh()` 调用边界。
- 修复了旧适配器丢弃无触点结束/取消事件的问题：现在发送 `point: null`，由 Rust 取消。已在迁移前的 `238cfec5` Web 构建中复现 end/cancel 后都仍为 armed；证据位于 `target/nativeprep-a-baseline/contactless-before.json`。
- 13 项纯手势测试保留原释放修复，增加方向反转、两种距离、0.5px 边界、嵌套容量、各排除条件、新开始重置与无坐标事件。
- Claude 静态复核无实现阻塞项，指出多层祖先测试和 idle 断言固定等待两个覆盖局限。已补两层滚动的正反对照；旧适配器在隔离真实 DOM 中的对照显示外层有上方内容时 eligible=false、两层均到顶部时 eligible=true，见 `scroll-chain-before.json`。该额外旧版探针只验证旧 JS 适配器，未连接 Rust，不能等同整条应用链。
- idle 的浏览器断言仍包含 180ms 等待，并以随后释放无请求、取消后不能复活和相同目标的正向示例补充观察；这不是桥接处理完成的正式确认协议。没有为测试引入生产代码 hook。

## 验证与验收

### 自动化验证：阅读位置

- `cargo fmt --all --check`：通过。
- `cargo test -p rssr-app --locked -j2`：通过，主测试 119 + 集成测试 4，共 123 通过、1 忽略；其中阅读位置专项 12 通过。
- `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked -j2`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked -j2`：通过（NDK 28.2.13676358）。
- `dx build --platform web --package rssr-app --locked`：退出 0。现有 dx 0.7.10 与 Dioxus 0.7.9 版本提示仍存在；本轮未升级依赖。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:9234 --static-base http://127.0.0.1:8103 --artifact-dir target/nativeprep-b`：273 项断言通过，应用 Console 错误 0；静态服务器没有调试热重载端点的 24 条日志独立记录。
- `node scripts/browser/rssr_reading_position_acceptance.cjs`（`STATIC_BASE=http://127.0.0.1:8103`）：360 / 1280 两视口通过，涵盖返回恢复、键盘、输入法、宿主捕获事件桥接、顶部/底部；连续滚动与编辑期间正文块布局读取均为 0。
- 对照同一源码基线 `730d29e3` 的 `target/nativeprep-release-review-final/`；本轮 `target/nativeprep-b/entries-desktop.png` 与基线 SHA256 相同，已查看窄窗口正文截图，未发现布局差异。
- `git diff --exit-code -- crates/rssr-app/src/ui/reading_position.js`：通过，适配器未改。

### 手工验收与证据范围

- 已直接查看真实浏览器生成的桌面文章页与窄窗口阅读页截图。使用隔离测试数据，未操作用户数据库。
- 未做 WebView2 宿主交互、Android 真机触摸或 Linux/macOS 运行验收；Android 捕获事件的浏览器模拟不能替代系统返回键验收。
- 机器上的详细日志位于临时目录 `rssr-nativeprep-b-*.log`，浏览器产物位于 `target/nativeprep-b/`，不随源码提交。

### 自动化验证：下拉刷新

- 迁移前基线：`238cfec5` 构建运行扩展后的回归，293 项断言通过、0 应用 Console 错误；正反对照覆盖嵌套滚动顶端/中段、控件、选区与反向拖动，产物 `target/nativeprep-a-baseline/`。此轮尚无无触点修复和两层祖先新增测试；不能把它表述为最终 302 项全部在旧版通过。
- 旧版无触点复现：实际 `238cfec5` Web 构建中先拖至 armed，再发无触点 cancel/end，均仍为 armed。新版对应四项断言通过：立即取消且不刷新，后续 end 不能复活。
- `cargo test -p rssr-app --locked -j2 pull_refresh`：13 项纯手势测试通过。
- `cargo test -p rssr-app --locked -j2`：主测试 127 + 集成测试 4，共 131 通过、1 忽略。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked -j2`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked -j2`：通过（同一 NDK 配置）。
- `dx build --platform web --package rssr-app --locked`：退出 0；同上保留已有 CLI/依赖版本提示。
- 首轮迁移后浏览器回归：297 项通过，0 应用 Console 错误，产物 `target/nativeprep-a/`。随后根据 Claude 反馈补两层滚动用例，生产代码没有再改。
- 最终命令：`node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:9234 --static-base http://127.0.0.1:8103 --artifact-dir target/nativeprep-a-final`：302 项通过，0 应用 Console 错误；36 条已知调试热重载端点日志独立记录。内层顶部/外层中段时取消，两层均到顶部时可 armed，正反对照通过。
- DOM 边界样本用于稳定保持嵌套滚动位置、文字选区及释放坐标；普通下拉、多指与取消另有真实 CDP 输入。这些证据仍不等于 Android 设备触摸或 WebView2 实机验收。
- 最终桌面文章页与迁移前基线截图 SHA256 相同；已查看最终窄窗口正文截图，未发现布局差异。正文截图存在像素差异，未声称逐像素相同。
- A 未修改阅读位置模块或 JS，保留 B 的位置专项验收证据；没有重复把相同源码重新测一遍计作额外覆盖。最终检查日志已复制到 `target/nativeprep-a-final/`。

## 结果

阅读位置内部策略已可脱离 Dioxus 测试；这次是模块化而非新增 Rust 迁移量。下拉刷新先在阅读位置独立提交上补齐浏览器基线，再把剩余手势历史和判断从 JavaScript 收入纯 Rust。两项均已完成相称验证，按职责分别本地提交，目录拆分继续暂缓。

## 风险与后续事项

- 没有选择或接入原生渲染器，不改变业务职责；尚无性能或内存收益结论。
- 下拉刷新摘要测量的额外祖先遍历成本保持待测；适配器继续负责测量、输入与实际滚动。
- 任务外 `.agents/`、`.specify/` 删除、旧目录交接记录修改和未跟踪设计产物均保持原样。
- 仅本地提交，不 push、不打 tag、不发布。
- 本轮创建的隔离浏览器和临时 HTTP 服务器已关闭，未关闭用户浏览器或应用。

## 给下一位 Agent 的备注

- 从 `controller.rs` 的输入/输出与状态序列测试进入；不要把恢复状态移到共享 TLS，否则会破坏适配器重挂载后的访问代次。
- 迁移下拉刷新时保留上一轮修复的真实释放语义：最终坐标、零剩余触点、取消后不可复活。先建立正反对照，再移动规则。
