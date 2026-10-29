# Settings 表单验收与数字提示回填归属

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始时 HEAD：`906035eff7c83348350b8eceecf67cfa80d4830e`
- 相关 commit：与本记录同批本地提交；生成时 `commit: pending`。提交后用 `git log -1 -- docs/handoffs/2026-10-29-settings-form-feedback.md` 定位，最终 SHA 另记本地 `final-receipt.json`。
- 相关 tag / release：无；不 push、不打 tag、不发布
- 状态：`validated`（现有 Web / Windows WebView 表单）；不代表新原生控件或真实 IME 已验收

## 工作摘要

继续原生界面前置工作，核对 Settings 阅读偏好的三个枚举和四个数字字段。旧版将数字解析提示绑定到当时的数值：保存或下载相同值后提示残留；配置 A→B→A 会让旧提示重新出现。问题在真实 Web/WASM 与独立 Windows EXE 中均复现。

本批将提示归属接到既有 Rust 请求生命周期中的草稿回填代次。接纳整份草稿替换时永久作废旧提示；因后续编辑拒绝回填时保留提示。输入解析、范围校验、保存/下载准入和草稿保留规则不变。按用户要求全程使用 CLI / Playwright / 原生 CDP，没有 Computer Use。

## 影响范围

- 生产模块：`settings_page/{request_lifecycle,session,facade,preferences}.rs`。
- 验收：扩展 `scripts/browser/rssr_settings_sync_acceptance.cjs`，新增 `settings_form_scenarios.cjs`。
- 文档：原生控件适配契约、原生界面前置边界、原生控件验收清单及本记录。
- 平台：静态 Web 与 Windows EXE / WebView2 / SQLite 实际运行；Android 仅交叉编译检查。托管 `rssr-web`、Linux/macOS 未运行。
- 未新增依赖、crate、renderer、配置字段、业务设置副本或存储迁移；仍通过 facade/session → `UiCommand` → runtime → application/host。

## 关键变更

### 提示属于一次草稿投影

- `SettingsRequestLifecycle` 持有 `draft_replacement_revision`，仍不包含设置值、Signal 或 I/O。
- `apply_loaded_settings` / `apply_saved_settings` 仅在既有规则允许替换整份草稿时推进该代次，即使新旧值完全相同。草稿没有获准替换时不推进。
- 四个字段各记录发生解析失败时的代次，渲染只比较代次。合法输入继续清除该字段提示；新错误不能让其他字段的旧错误复活。没有 effect 清理、渲染期写入或控件重建。
- 用户编辑代次独立：初次加载/下载仍保留后续同值事件、A→B→A 和无法解析的输入；保存仍按整份值相等回填。回填同值仍不会额外取消主题文件导入。
- 新增三个 Dioxus session 测试，覆盖同值/往返回填、拒绝回填保留提示代次，以及保存值相等与用户编辑代次的区别。

### 复用现有双平台运行器

`--scenario form` 两视口各 45 场景：初次投影无编辑通知/自动保存、三枚举全部选项、四数字上下界、清空/整数小数/整数溢出、范围失败重试、零页大小保存回退、相同配置和往返配置清除提示、新错误仅属于当前字段、七字段各两次同值事件、四数字在下载期间清空。

实际下载响应由自管 HTTP 服务延迟，经过真实 Rust handler 和持久化路径。重复同值通知通过 `dispatchEvent` 合成并明确记在回执中，不把它当真实 IME。观察回填通知前先结束用户 blur，避免把正常失焦 `change` 错算成程序回填。

运行器保留原 sync/lifecycle 默认路径，增加浏览器版本/场景脚本哈希。`BROWSER_EXECUTABLE_PATH` 可选择已安装 Chrome，不传时仍用原 channel；原生可通过 `--feed-origin` 指定调用方持有的独立回环订阅服务，默认仍为 9277。任何编辑前继续核对复制库的 5000 个固定文章 ID/标题、当前 Entries 页及订阅 URL。

## 验证与验收

证据目录：`target/settings-form-20261029/`（本机 ignored；其他 checkout 缺失时不可凭本文补写通过）。

### 源码、产物和隔离

| 产物 | SHA-256 / 来源 |
| --- | --- |
| 旧 Windows EXE | `43cc29086c9f9097aa6426c7eefc055e82778792a62ae398e52bffafbc1ef8ee`；复用上一批 BOM 修复产物，对应开始 HEAD |
| 新 Windows EXE | `ed0e282fca8a8f931e0ae31bd2b6e96588c89ed8726291f67ab2ed7d3a331cc0`；本批 release 构建并复制到 `native-candidate/` |
| 旧 Web WASM | `9bea9bf2e6af9f317ed290c8623d8931df770d396ee18afbaa776b8d0d161293`；复用 `target/settings-load-20261029/candidate-final/public/`，该版本至开始 HEAD 的 Settings 表单代码未变 |
| 新 Web WASM | `f831cdb00761bcd68a65bc44ef9a63c78189bee642070ac8d0a66f5decae9b10`；本批 dx release 构建，保存于 `web-candidate-public/` |
| 最终同步运行器 | `74b35df199a96ea20b786005a1aae602910fe5b9ed4a58d13c29257a66c2e809` |
| 表单场景模块 | `fb7634d0c2932f5fe0aedcd992bdf78e82ea3875194c0edcf6c69ea65d367690` |

Web 构建目录可能含旧资产，身份沿 `index.html` → 实际入口 JS → `module_or_path` 指定的 WASM 核对，不取目录中的首个 WASM。完整路径/哈希见 `web-identity.json`；原生来源/副本见 `native-identity.json`；最终源码/工具哈希见 `validation-identity.json`。

Chrome `155.0.8059.40`；WebView2 `156.0.4314.50`。旧、新原生 EXE 各自使用独立目录、复制的 SQLite 索引/正文库和 `webview/` profile。样本来自 `target/reading-baseline-20261025/seed-matched/`；复制库订阅改为本批 `127.0.0.1:9281` 服务，配置服务由运行器持有 `127.0.0.1:9277`。旧样本归档阈值 0 仅在身份检查后通过实际 UI 保存为合法 120；场景结束恢复该合法基线。

### 自动化验证

| 命令 / 路径 | 实际结果 | 回执 |
| --- | --- | --- |
| `cargo test -p rssr-app --locked settings_page` | 61 通过 / 0 失败，新增 3 项；随后清除测试中的 unused-mut 警告，最终全工作区重验 | `settings-tests.log`、`workspace-tests.log` |
| `cargo test --workspace --locked` | 574 通过 / 0 失败 / 2 个既有性能探针忽略 | `workspace-tests.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 | `clippy.log` |
| `cargo fmt --all --check` | 通过 | `fmt.log` |
| `cargo build -p rssr-app --release --locked` | 通过 | `build-native.log` |
| `target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false` | 通过 | `build-web.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 | `check-wasm.log` |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | 通过；NDK 28.2.13676358，API 24 clang / llvm-ar | `check-android.log` |
| `node --check` 两个本批 JS 文件；`git diff --check` | 通过 | 提交前检查 |
| Web 表单基线与候选 | 1536×844 / 390×844 各 45 场景；基线 `baseline-reproduced`，候选通过 | `web-{baseline,candidate}-final/acceptance.json` |
| Windows 表单基线与候选 | 同样各 45 场景；基线复现，候选通过；每视口完整比对 5000 个文章标记、6 个订阅 | `native-{baseline,candidate}-verified-final/acceptance.json` |
| 候选原生进程重启 | 完整设置与 5000 个标记一致 | `native-reopen-final/acceptance.json` |
| Web `--scenario lifecycle` | 两视口各 18 场景通过；含真实 Web Lock / HTTP 延迟 | `web-lifecycle-final/acceptance.json` |
| Web Settings 保存运行器 | 两视口正常流程、pending 草稿/共享 gate、存储失败重试共 4 场景通过 | `web-save-final/acceptance.json` |

实际表单命令如下，环境 `NODE_PATH=C:/Users/QQ/node_modules`；Web 指定 `BROWSER_EXECUTABLE_PATH=C:/Program Files/Google/Chrome/Application/chrome.exe` 和准确的 `PUBLIC_DIR`。配置运行器共用 9277，须顺序执行：

```powershell
node scripts/browser/rssr_settings_sync_acceptance.cjs --scenario form --baseline true --artifact-dir target/settings-form-20261029/web-baseline-final
node scripts/browser/rssr_settings_sync_acceptance.cjs --scenario form --baseline false --artifact-dir target/settings-form-20261029/web-candidate-final
node scripts/browser/rssr_settings_sync_acceptance.cjs --scenario form --baseline true --cdp http://127.0.0.1:9280 --database target/settings-form-20261029/native-baseline/RSS-Reader/rss-reader.db --feed-origin http://127.0.0.1:9281 --artifact-dir target/settings-form-20261029/native-baseline-verified-final
node scripts/browser/rssr_settings_sync_acceptance.cjs --scenario form --baseline false --cdp http://127.0.0.1:9280 --database target/settings-form-20261029/native-candidate/RSS-Reader/rss-reader.db --feed-origin http://127.0.0.1:9281 --artifact-dir target/settings-form-20261029/native-candidate-verified-final
node scripts/browser/rssr_settings_sync_acceptance.cjs --scenario lifecycle --artifact-dir target/settings-form-20261029/web-lifecycle-final
node scripts/browser/rssr_settings_save_acceptance.cjs --artifact-dir target/settings-form-20261029/web-save-final
node scripts/browser/rssr_settings_save_acceptance.cjs --mode reopen --cdp http://127.0.0.1:9280 --database target/settings-form-20261029/native-candidate/RSS-Reader/rss-reader.db --expect target/settings-form-20261029/native-candidate/expected-reopen.json --artifact-dir target/settings-form-20261029/native-reopen-final
```

原生调用方负责启动/停止独立 EXE，设置独立 `WEBVIEW2_USER_DATA_FOLDER` 和 CDP 参数；每次先确认实际进程路径，候选 CDP 进程父链另保存于 `native-candidate/process-chain.json`。重启前真正结束旧进程；重启对照快照通过只读 SQL 获取。

### 视觉、数据保护与未执行项目

- Web 两视口的初始/恢复图共四对逐字节一致，见 `visual-comparison.json`。反馈图中旧版四处残留提示变为候选当前字段的一处提示，这是有意修复；已检查窄屏整页及无横向溢出。
- 最终表单、加载、保存和重启回执均无页面/控制台错误。宽、窄原生控件由真实 CDP 操作；它们仍是 WebView 控件。
- 原生旧版第一次运行在窄视口完整截图超时，第二次视口截图也超时；保留 `native-baseline-fullpage-attempt/` 与 `native-baseline-final/`。最终恢复原生不截图的既有路径，功能验收完整通过。连接早于 CDP 就绪的失败尝试也单独保留，不充作通过证据。
- 两份复制库的索引/正文 `PRAGMA integrity_check` 均为 `ok`。完整 `entries` 和 `entry_contents` 表各 5000 行，相对原始 fixture 双向 `EXCEPT` 均为 0；6 个订阅 ID/URL 保持准备后的值，源 fixture 两文件哈希不变。见 `data-audit-before-restart.json`、`data-audit-final.json`。
- 已结束本次原生进程、CDP 和自管回环服务；日志和 profile 留作证据，未清理用户工作区或数据。
- 手工 OS 控件、前台原生视觉、真实输入法、Android 设备与 Linux/macOS 未执行；不会由合成事件、小视口或编译结果推定通过。上一批系统文件验收仍为部分验收。

## 结果

已修复两平台可复现的数字提示归属问题；现有解析和持久化语义保持，验收与交接齐全，可作本批本地提交。该结果不等于发布验收，不包含远端操作。

## 风险与后续事项

- 回填代次和用户编辑代次用途不同。后续适配器不得合并它们，也不得把程序回填回送为用户输入；保存继续保留值相等语义。
- 新原生控件仍需独立确定宿主、渲染和 CSS 兼容方案；本批没有改变这一架构边界。
- 下一批可检查 Reader 快捷键与输入框焦点、页面切换的接线边界，复用现有纯 Rust 策略及直接 CDP 验收；真实 IME 继续单独记缺口，不新增渲染器。

## 给下一位 Agent 的备注

- 先读 `request_lifecycle.rs` 的回填代次、session 两个应用结果入口、`preferences.rs` 的四个局部提示，再看表单运行器。不要把提示代次扩大为另一份设置状态。
- 初始 staged 为空，开始 HEAD 和完整 dirty/untracked 快照在本批证据目录。任务外 `.agents/skills/`、`.specify/` 删除，两个 10/03 handoff 修改及已有 untracked 目录/设计交接均保留；本批只按明确路径暂存提交。
- 复验前核对实际入口 WASM/EXE 和独立数据库身份。失败尝试、基线复现、候选成功是不同证据，不得用历史成功替代当前产物。
