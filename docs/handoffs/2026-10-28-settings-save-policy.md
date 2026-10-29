# Settings 保存与草稿回填规则提取

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前 HEAD（实施基线）：`ed90aa531c454a3b7cef9f6982748baaf22c12fb`
- 相关 commit：与本记录同批本地提交；记录生成时 `commit: pending`。提交后可用 `git log -1 -- docs/handoffs/2026-10-28-settings-save-policy.md` 定位。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`

后续复查：`dda9aefa` 的 Rust 保存规则未发现行为回归；Windows 验收的目标校验时机已在 [Settings review 交接](./2026-10-28-settings-save-review.md) 中修正并补负向测试，后续运行以修正后的验收脚本为准。

## 工作摘要

继续 Entries 请求/写入与 Reader 结果归属之后的原生界面前置拆分，把 Settings 保存副本准备、完成结果归并、草稿与预设回填判定移入页面内普通 Rust 模块。保留既有行为、命令链、页面任务归属和应用服务的持久化语义；仍使用当前 Dioxus / WebView 界面。

## 影响范围

- 模块：`settings_page/save_policy.rs`、保存 session、页面 session 与模块装配；移除 `ui/mod.rs` 中失去外部调用方的单个 helper 再导出，helper 本体仍供内部使用。
- domain 仅更新 CSS 校验调用位置注释，无业务实现改动。
- 平台：共用页面影响 Web、Windows、Android、Linux、macOS；本批实际运行覆盖 Web 和隔离 Windows WebView2。
- 文档与测试：原生前置边界、页面职责、命令参考、新增 Settings Playwright / CDP 验收脚本。
- 无新依赖、crate、schema、迁移或 workflow 变更。

## 关键变更

### 纯规则与页面适配

- `prepare_save` 使用原 CSS 校验，页大小 0 仅在提交副本中回退为 100；其他数值仍由 application 校验，不提前改变用户草稿。
- `SaveCompletion` 按原顺序消耗投影结果；最后设置和最后消息独立生效。有设置结果即成功，不以消息 tone 推断成败；无设置结果（含空集合）保留草稿并显示重试提示。
- `saved_draft_update` 比较整份草稿值：相等时回填保存值，否则保留整份新草稿，不逐字段合并。改动后改回原值可回填，不新增编辑代次。
- 预设选择单独比较；仅在仍等于提交选择时，根据有效草稿 CSS 回填。仅新选择而未应用不单独算未保存编辑；有效草稿与保存结果不同时才追加未保存提示。
- session 仍先同步防重复，再捕获快照，通过准备后置 pending，在 `spawn` 前构造命令；完成后先解锁，再归并和发布。草稿、预设、全局主题、提示的发布次序保持不变；页面销毁仍取消自己的任务，不承诺回滚已提交写入。

### 测试与验收约束

- 新增 9 个纯策略测试、4 个 Dioxus 会话测试，保留原有 3 个保存集成测试。覆盖合法值/仅零值回退、CSS 错误、整份草稿保留、独立预设、A→B→A 值比较、最后结果顺序、空结果、提示、同步命令构造、跨页面投影过滤与页面释放。
- 验收脚本用完整设置对象比对持久化，并对照全体 fixture 文章标记，避免只检查按钮或少数样本。
- Web 在真实 WASM / Chrome 中用 Web Locks 暂停实际写入，验证保存期间改主题/草稿和所有入口共享 gate；另注入 Storage 写失败后显式重试、重载。
- 原生采用复制 EXE、独立 WebView profile 与复制 SQLite fixture。通过 DOM 操作和只读 SQLite 校对，旧进程结束后启动新进程核对重开。隐藏窗口截图不作原生视觉通过证据。

## 验证与验收

证据目录：`target/settings-save-20261028/`（忽略产物，未提交）。脚本与本记录入库。

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过，`clippy.log`。
- `cargo test --workspace --locked`：最终源码 513 passed / 0 failed / 2 ignored（既有性能测试），`workspace-tests-final.log`。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过，`wasm-check.log`。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过，`android-check.log`。使用本机 NDK 28.2.13676358 的 API 24 clang / llvm-ar；这是编译覆盖。
- `node --check scripts/browser/rssr_settings_save_acceptance.cjs`、任务范围 `git diff --check`：通过。

### 构建与同路径验收

- 基线复用上一批已验证候选包，对应当前基线 HEAD 的生产源码；已重新核对源文件和实际 index → JS → WASM 哈希，见 `baseline-identity.json`。不是拿当前 candidate 冒充 baseline。
- Web 构建命令：`target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false`。必须使用匹配 Dioxus 的 0.7.9 工具和该参数，检查 wasm-opt 与最终完成日志。
- Windows 构建命令：`cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`。
- Web：设置 `NODE_PATH=C:/Users/QQ/node_modules`、`PUBLIC_DIR=<该版本 public>`，运行 `node scripts/browser/rssr_settings_save_acceptance.cjs --artifact-dir <证据目录>`。同 `/settings`、同 1536×844 / 390×844、同数据与步骤；基线 `baseline-web-final/acceptance.json`、候选 `candidate-web-final/acceptance.json` 均 4/4 通过，pageErrors / consoleErrors 均为空。
- Windows：本地辅助脚本 `run-native.ps1` 只启动/结束此次复制的进程，显式核对 EXE 哈希及端口占用；调用 `rssr_settings_save_acceptance.cjs --mode write|reopen --cdp <endpoint> --database <fixture-db> --artifact-dir <path> --expect <write/acceptance.json>`。基线 `native-baseline-receipt`、候选 `native-candidate-receipt` 各通过两个视口流程和独立进程重开，比较完整设置与全部 5000 篇文章标记；无页面或 console 错误。
- Web 两张截图与基线 SHA-256 完全一致：1536 宽 `B1736BC7A047118FF1D39F2D5F2D95335AE6870C4426FE63B1ABC166B2CA23D8`；390 宽 `840392C125BBDCC5F1EE588A802B5AB9564103102EE6A62E44277F0F9C87512D`。检查了实际图片，见 `visual-equivalence.json`。
- 基线 WASM SHA-256 `6c391d1d08116bfebf1a4561af932eba1a0b2466c12eb50870caf63e89de973f`；候选 `5bf3880a1f2a24aeb07ad2b1aa74ffbbb5df9e27483a282dcfa263f5cb336171`。候选引用链 `candidate/web-identity.json`；源码清单 `candidate/source.json`，最终验收脚本身份另见 `validation-identity.json`。
- 基线 EXE SHA-256 `99B45FA069AD91B387E6E4796E76BA49CC7F914E2F4562BE1DF51F54464CC889`；候选 `957F53A0C4774C0A0161567B0440716328EABF6E4328CCF7397EADFBD2AFE83C`。实际运行副本与哈希记录在各原生运行目录的 `identity.json`，构建日志 `web-build-final.log` / `native-build-final.log` 均成功。

### 验收中发现并修正的测试前提

- 既有目录/阅读 fixture 使用历史归档阈值 0，当前设置保存会因其失败。Web fixture 在锁内设置为合法的 120 后重载；Windows 仅在复制数据中通过设置 UI 准备该值。数值失败用例进一步检查具体“文章页每页数量”错误，防止其他非法字段掩盖目标验证。
- 空 CSS 不挂载 `#user-custom-css`；读取验收按空字符串处理。主题 textarea 会把 CRLF 表示为 LF，而预设保存原始嵌入文本；输入显示按 LF 对照，持久化仍与原始主题文件精确比较。
- 首轮构建发现已无外部调用方的 helper 再导出产生 unused import；已删除该再导出并重新构建，最终 Clippy 零警告。上述均有初次日志保留，不把失败尝试计入通过项。
- 最终报告的设置/标记快照在所有场景结束后读取，避免 Web 后续失败重试场景完成后仍报告较早的中间值；基线和候选均用最终脚本重新执行。

### 手工与未执行覆盖

- Web 截图由真实无头 Chrome 生成，并检查桌面/窄屏布局；不声称人工逐项点击。
- 未运行 Android 设备交互、Linux/macOS 原生 UI、Windows 前台输入法/键盘、托管 `rssr-web` 服务端模式。
- 本批未改动的 WebDAV 远端上传/下载、主题文件导入/导出和 GitHub 外部入口未执行外部 I/O 验收。

## 结果

- 代码、测试、同路径运行验收和交接已完成，可按本批范围本地提交；提交身份见本记录开头的定位命令。
- 没有改变保存期间编辑、失败重试、主题选择或提示的既有契约；不宣称性能收益或已替换原生渲染器。

## 风险与后续事项

- 保留现有整份草稿值比较与全局主题/页面草稿双快照语义；本次没有增加字段级合并、自动重试或保存队列。
- 加载设置及 WebDAV 同步的异步竞争不属于本批，不以保存规则提取证明这些路径已验收。
- 下一批建议检查 `FeedsPageSession` 的快照查询归属及单来源刷新提示生命周期，继续保留 reducer、命令串接和 Shell 手动刷新职责，先建立相同路径基线。

## 给下一位 Agent 的备注

- 从 `settings_page/save_policy.rs` 及 `save/session.rs` 测试进入；实现依据见 `docs/design/native-ui-preparation.md` 的 Settings 约束。
- 初始工作区包含任务外 `.agents/skills/speckit-*`、`.specify/*` 删除、两份 10/03 handoff 改动和其他未跟踪记录；本批按精确路径提交，保留这些状态。初始清单 `initial-status.txt`。
- 只有本地 commit 授权；这次交付不包含 push、合并远端、tag 或发布。
