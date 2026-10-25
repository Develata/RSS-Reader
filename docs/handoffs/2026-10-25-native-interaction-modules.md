# 原生界面前置工作：阅读位置与下拉刷新模块

- 日期：2026-10-25
- 作者 / Agent：Codex；Claude CLI 只读评审
- 分支：`feat/reading-workspace-20261024`
- 实施基线 HEAD：`730d29e3`
- 相关 commit：pending（第一项阅读位置控制器已验证，待本地提交）
- 相关 tag / release：N/A
- 状态：`validated`（阅读位置）；下拉刷新迁移待继续

## 工作摘要

用户恢复 Claude 登录后，Codex 用英文与实际 Claude CLI 讨论原生 Rust 界面之前最值得推进的模块化工作。双方确认先独立阅读位置控制器，再迁移下拉刷新的剩余手势规则，暂缓目录文件拆分。保持现有七个 crate、facade/session → UiCommand → runtime → application/host 边界与界面样式。

## 影响范围

- 模块：`ui/reading_position.rs`、新增 `ui/reading_position/controller.rs`；后续项为 `pages/entries_page/pull_refresh`。
- 平台：共享前端影响 Web、Windows、Linux、macOS、Android；本轮验证 Web 浏览器和 Windows/Android 编译路径。
- 额外影响：前端命令契约与本交接文档；无依赖、数据库、配置格式、workflow 或发布变化。

## 关键变更

### Claude 共识与评审边界

- 真实会话：`52eee468-9625-4a4e-a4c2-60dd07a78b7f`。使用 `claude -p --tools Read,Glob,Grep --permission-mode plan --strict-mcp-config --output-format json`，后续复用该会话；没有编辑或执行 Shell 权限。三次响应均成功且无权限拒绝。
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
- 仅在操作边界捕获，不增加连续滚动正文扫描。12 项纯控制器测试覆盖释放/清理、访问隔离、页码回退、超时、未知事件、宿主捕获与命令字段。

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

## 结果

阅读位置内部策略已可脱离 Dioxus 测试；这次是模块化而非新增 Rust 迁移量。下一项按双方共识，在阅读位置独立提交后先补嵌套滚动、控件、选区与反向移动的浏览器基线，再实施下拉刷新迁移。

## 风险与后续事项

- 没有选择或接入原生渲染器，不改变业务职责；尚无性能或内存收益结论。
- 下拉刷新摘要测量的额外祖先遍历成本保持待测；适配器继续负责测量、输入与实际滚动。
- 任务外 `.agents/`、`.specify/` 删除、旧目录交接记录修改和未跟踪设计产物均保持原样。
- 仅本地提交，不 push、不打 tag、不发布。

## 给下一位 Agent 的备注

- 从 `controller.rs` 的输入/输出与状态序列测试进入；不要把恢复状态移到共享 TLS，否则会破坏适配器重挂载后的访问代次。
- 迁移下拉刷新时保留上一轮修复的真实释放语义：最终坐标、零剩余触点、取消后不可复活。先建立正反对照，再移动规则。
