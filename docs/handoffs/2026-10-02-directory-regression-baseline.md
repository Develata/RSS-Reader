# 目录行为回归与最小 GUI 性能基线

- 日期：2026-10-02（跨 UTC 日完成）
- 作者 / Agent：Codex delegated task
- 分支：`test/directory-regression-baseline`
- 当前 HEAD（实现提交）：`fa373ad606b5767247f7ac65500e1beffa116325`
- 生产基线：`e8c6ee165761f2e89baa6612b96324310def187a`
- 相关 commit：`fa373ad6`；后续交接记录提交仅补充此元数据，精确 PR head 以远端分支为准。
- 相关 tag / release：N/A；禁止 merge，未发布
- 状态：`draft`

## 工作摘要

为目录现有行为建立可由真实 CI 入口执行的回归，并分别测量 release Web / Windows WebView2 的三条路径。保持测试/基线范围；未修改生产行为、迁移 controller 或添加 IPC。

## 影响范围

- 模块：`scripts/browser/directory_*.mjs`、已有小视口 runner / SPA fixture server、`scripts/seed_directory_native.py`、四个固定 fixture、`controls.rs` 中仅 `#[cfg(test)]` 的八组合真值表；`sqlite_native.rs` 仅将测试专用 Unix import 加同样的 cfg，修复 Windows Clippy unused-import，未改 SQLite 行为。
- 平台：Windows Chrome Web 与 Windows 原生 WebView2；Android、macOS 未验。
- workflow / 文档：既有 CI `web-ui (default)` 经小视口入口执行新增契约；其余主题保留已有断言。没有新增 workflow 或盲目接入历史脚本。详见 `docs/testing/directory-regression-baseline.md`。

## 关键变更

- 字面量 fixture 预期，时间/来源分组的 96px 两侧边界；基础偏好与有效展开；Enter/Space 独立单次激活且同页滚走后验证；桌面/手机消费者和断点切换；局部折叠、移动横向滚动、DOM/VDOM 重绘、导航/hash、路由退出/返回、监听/rAF 清理。
- 新增断言不依赖生产选择函数、全局猴子补丁或任意 sleep；使用像素容差、条件等待和超时。复用既有 wheel 输入取消阅读位置恢复。
- 可选性能入口默认关闭，三批 × 每项 20 个原始样本，鼠标/Enter/Space 展开分列；DOM 完成 + 两帧只是代理指标。无 p95、掉帧、layout 耗时或迁移通过宣称。

## 验证与验收

### 自动化验证

- 原有 exact-main release Web 小视口 smoke：通过，修改前证据 `target/directory-baseline/existing-smoke/`。
- 最终完整 CI Web 入口 `bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build --port 8117 --log-dir target/directory-baseline/ci-entry-final`：203 条通过，4 条活动组键盘偏好失败，另有 1 条聚合失败，console error 0。
- Windows WebView2 `bun scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:18115 --native-target BBF8563CD5BE0B4109721AD2DFEE08BE --directory-only true --artifact-dir target/directory-baseline/native-contracts-final`：40 条通过，相同 4 条行为失败及 1 条聚合失败，console error 0。
- 两端原始截图/DOM/assertions 保留在上述目录；可审阅状态摘录已提交到 `docs/testing/baselines/2026-10-03-directory-known-failures.json`，附原报告 hash。
- Windows `cargo build --release --locked -p rssr-app -p rssr-cli -j 1`：通过；Rust 1.98.1 MSVC，17m49s。exe SHA-256 `e16c55bcb2c5bc1383828da8d907af064766f8eea7261744f952707a8c193265`。
- `cargo fmt --all --check`、JS `node --check`、修改 shell 的 `bash -n`、Python `py_compile`、`git diff --check`：通过。
- `cargo test --workspace --locked -j 1`：334 passed、0 failed、2 既有 ignored；三个 wasm harness 在 Windows native 测试下为 0 tests，不计作浏览器 contract 验收。
- `cargo clippy --workspace --all-targets --locked -j 1 -- -D warnings`：最终通过。首轮暴露的 Unix 测试 import 已按平台限定。
- `bash scripts/run_release_ui_regression.sh --release --skip-build --no-serve`：最终通过，退出码 0；含 wasm check、app tests、三个原生 contract harness、rssr-web tests。证据 `target/directory-baseline/aggregate-final/` 和 `aggregate-final-exit.json`。首轮会话失联、无最终退出码，只保留为诊断。
- 上述聚合命令未开启 browser contracts、rssr-web host smoke 或历史 fixed-smokes；本次完整小视口 UI 入口已独立运行，三个 wasm 浏览器 contracts 与其它四个主题等待远端 CI，Android 仍仅构建。
- native seeder：全新已迁移的隔离库成功写入 72 篇；再次对非空库执行会拒绝。未触碰日常数据库。

### 测试自审

- 独立预期来自固定 fixture ID 与真值表；没有调用生产选择函数生成 expected。
- Enter 与 Space 分开、每场景一次激活，随后同页滚走检查潜在偏好；非活动组配对用例验证输入确实激活，避免按键注入无效产生假通过。
- 局部折叠先把 rail 滚离 active，防止错误对齐恰好不移动而通过；导航先等待真实展开完成，适用于原生 IPC 时序。
- 每场景截图失败被单独保留，不覆盖断言；性能事件监听只在显式采样时安装并清理。无新增全局 API monkeypatch、永久 skip 或任意 sleep。

### Windows 与性能

- 真正的 Windows Dioxus WebView2，按 exe 目录隔离两个 SQLite 库，另设独立 WebView profile，显式 target；没有用 WSL GUI 或 Chrome emulation 代替。
- 第一次计时因 fixture 启动外部请求污染而废弃，保留为诊断。源码确认启动总会刷新；最终 fixture 接入 loopback 304 并等待刷新完成。不能把首轮写为正式基线。
- 正式两端各 3 批 × 5 个输入变体 × 20 样本，共 600 个；两份性能报告及其外围 assertions 均通过，console error 0。原始 CSV、环境、产物 SHA-256、批间中位数跨度与 pooled IQR 已提交到 `docs/testing/baselines/2026-10-03-directory-windows-{chrome,webview2}.{csv,json}`。
- Windows 11 10.0.26200 / i7-12700H / 约 16 GiB / 物理显示器 165Hz。Chrome 154.0.8037.93 headless 为 1280×800 DPR 1；可见原生 WebView2 154.0.4258.53 为 1280×900 DPR 1.5。
- 三批中位数：Web 滚动高亮 50.60–53.00ms、展开 62.80–65.00ms、导航 580.95–582.65ms；Native 分别 18.10ms、27.70–29.00ms、652.15–652.35ms。均为 DOM + 两帧代理，不能跨宿主排名；展开的三个输入变体和各自噪声预算见验收文档。
- 隐藏原生窗口、启动网络错误及 80px 准备定位偏差的早期尝试均为废弃诊断，不混入上述正式样本。

## 结果

基线 bug：活动组虽有 `aria-disabled=true` / `pointer-events:none`，Enter 或 Space 仍触发无条件 onclick，改变基础手动偏好；同页滚走后组折叠。时间/来源 × 两个键共四个独立失败。生产修复未授权，本 PR 保留 red regression；不得 merge。
建议最小修法：实际按钮激活资格跟随现有 viewport active 同步，不能只依赖可能落后于 JS 的 Rust 初始 active 值；不需 controller 迁移。

## 风险与后续事项

- 真实屏幕延迟、掉帧、layout cost、Android WebView 和迁移 candidate 未测。
- 重复三批只能提供噪声试测；后续先同条件 A/A，再交替 A/B，预算依实测毫秒范围与不确定性确定，不凭空设 5%/10%。
- CI 默认主题预期被四个生产回归阻断；必须与测试 harness / 基建失败区分。本记录截点为推送前本机验收，远端最终 head 的结果将在 draft PR 描述及 checks 中核对。

## 给下一位 Agent 的备注

- 原仓库 `E:/gitclone/RSS-Reader` 保留在原 main `1b5f8eb`，其 `.agents/`、`.specify/` 删除及 `.workbuddy/` 内容未改变；未 stash/reset/clean。
- 独立 worktree：`C:/Users/QQ/AppData/Local/Temp/rssr-directory-baseline-20261002`。只共享已有 Cargo 构建缓存 `E:/gitclone/RSS-Reader/target`；测试 exe、数据库、profile、Web artifact、日志均隔离。
- 最新 main 拉取最初 Git malloc 失败，减少并发后成功；WSL 启动 `CreateVm/E_ABORT`，备用路径未使用。未安装新工具或启动其它付费任务。
- 采样结束后已核对进程路径，关闭本任务隔离的原生窗口、Chrome 和两个 fixture 服务；产物与日志保留供复核。
- Web release 包来自成功 CI run `37071851764` 的 `ci-web-public`，head 恰为基线 SHA，wasm SHA-256 `4c4cf1e694d6ed62347b99b611eab490fbe114ad75366704591f1114c9e6b4d0`。
