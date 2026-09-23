# Home 刷新反馈、窄侧栏搜索与原生右键菜单

- 日期：2026-09-23
- 作者 / Agent：Codex
- 分支：`main`
- 当前 HEAD（任务基线）：`1bcc1f260614c18f392fcdceeb7adbdd9be17b40`
- 相关 commit：本记录与实现一同提交，提交号见本文件 Git 历史
- 相关 tag / release：基线 `v0.1.15`；本次修改尚未发布
- 状态：`validated`（Windows 右键菜单的实际弹出仍待复验）

## 工作摘要

修复 R 手动刷新成功提示停留过久、切页后可能再次出现旧提示、窄侧栏主题下搜索框被挤小，以及 release 桌面端无法通过右键调用复制 / 全选菜单。

## 影响范围

- 模块：`rssr-app` 的共享 shell 状态、shell 计时适配、原生启动配置；默认 shell CSS；现有浏览器断言与 fixture server；README、前端命令文档。
- 平台：Web、Windows / Linux / macOS 桌面、Android 的共享 shell；原生端的 WebView 菜单配置。
- 额外影响：没有变更 application/domain、RSS 刷新用例、HTML sanitizer 或剪贴板接口；没有打 tag、push 或发布。

## 关键变更

### 刷新反馈

- 原成功结果只靠 CSS 在 6 秒后隐藏，`ManualRefreshState::Finished` 会一直保留；导航重新挂载后旧结果可能再次出现。
- 现在 App 级状态在成功约 1 秒、错误约 6 秒后回到 `Idle`。计时器使用完成 revision 校验：若期间已开始或完成下一轮刷新，旧计时器不会清除新状态。跨平台差异仅限 shell 计时适配；刷新任务和列表失效 revision 原语义不变。
- 浏览器断言检查成功提示实际清空，并检查切到 Reader 再回 Home 不会重现。

### 搜索宽度

- 1280×800 浏览器基线中，冻结的旧 Atlas Sidebar CSS 把导航栏限制到 220px；展开后的文章页输入框只有 74px，确实难以识别。
- 当导航容器宽度不超过 280px 时，按钮行允许换行，搜索输入框占满下一行。现行 Atlas 侧栏与旧版持久化 CSS 都得到可用宽度；无新增 JS 搜索状态或平台分叉。
- 现有固定视口断言新增可见宽度、焦点、点击命中与横向溢出检查，并在 1280×800 下覆盖现行及旧版 Atlas 的文章页、Reader。

### 原生右键菜单

- Dioxus 0.7.9 的 release 默认向 WebView 注入 `contextmenu` 的 `preventDefault`，导致文章正文右键没有系统菜单。原生启动配置现显式调用 `with_disable_context_menu(false)`，让 WebView 使用原生菜单；没有自建复制 / 全选或 Clipboard service。
- 该 Dioxus 设置同时启用 WebView devtools，是现有公开配置接口的副作用。浏览器断言增加原生 Reader `contextmenu` 未被取消的检查，但本轮没有取得 Windows 实际菜单弹出的自动化证据。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`、`node --check scripts/browser/rssr_small_viewport_assertions.mjs`、`bash -n scripts/run_web_spa_regression_server.sh`、`git diff --check`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `bash scripts/run_static_web_small_viewport_smoke.sh --release --viewport 360,800 ...`：通过；本机忽略目录 `target/search-timer-final-20260923/assertions.json` 记录 128 项成功。命令内部执行锁定依赖的 Web release build。搜索框在现行 Atlas 下为 262×44 CSS px，旧版 Atlas 下为 174×44 CSS px；无水平溢出。刷新成功约 1 秒后状态变为 `idle`，切页不重现。
- 隔离 Windows 源码/fixture 中 `cargo build --locked --release -p rssr-app`：通过；`cargo test --locked --release -p rssr-app`：103 个单测通过、1 个 ignored，另 4 个集成测试通过；`cargo clippy --locked --release -p rssr-app --all-targets -- -D warnings`：通过。
- `cargo test --workspace --exclude rssr-app --locked` 与对应 `cargo clippy --workspace --exclude rssr-app --all-targets --locked -- -D warnings`：通过。
- `cargo test --workspace --locked`：未通过，Linux 主机缺少 `cairo.pc` / `libsoup-3.0.pc` 等 GTK/WebKit 开发库，尚未进入 app 测试；已在 Windows 上单独运行 app 测试。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：未通过，当前 Linux Rust toolchain 未安装 `aarch64-linux-android` target；不是源码诊断。

### 手工验收

- 真实 Chromium 渲染截图复核旧 Atlas 侧栏搜索在文章页与 Reader 均完整可见，正文没有水平溢出；基线 74px，修改后 174px。截图保存在 `target/search-timer-final-20260923/`。
- 隔离 Windows release 可执行文件已构建、启动并关闭。尝试连接其隔离 WebView 调试端口来验证右键菜单时，工具自动审批以“需要批准，但当前审批模式为 Never”拒绝了本地端口访问；没有据此宣称 Windows 系统菜单已实际弹出。
- Android 真机与 macOS 实机：未执行；本轮可用主机为 Windows，Android target 在本机缺失。

## 结果

- 三处修改均在现有 shell / host 配置边界内，Web 可见交互与 Windows release 编译、单测已验证。
- 本工作树尚未发布；`v0.1.15` 不包含本修复。

## 风险与后续事项

- Windows 原生菜单实际项目（复制、全选）以及 Android 长按选择，需要在可访问的设备/窗口上复验；Dioxus 的配置行为、构建通过和合成 `contextmenu` 断言不能替代 OS 菜单点选。
- 任意自定义 CSS 仍可能覆盖默认 shell；本轮精确覆盖仓库中冻结的旧 Atlas CSS 与现行 Atlas CSS。
- 若希望右键菜单开放同时彻底隐藏开发者工具，需要 Dioxus/Wry 更细粒度的宿主配置；本轮未为此引入私有 fork。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-app/src/ui/shell.rs`、`ui/shell_state.rs`、`ui/shell_browser.rs`、`assets/styles/shell.css`、`crates/rssr-app/src/main.rs`。
- 复验 Windows 菜单时使用隔离可执行文件和 SQLite fixture，避免触碰用户正在使用的便签或阅读数据；不要把 `v0.1.15` 的 CI 结果当作本次尚未发布修改的验收。
