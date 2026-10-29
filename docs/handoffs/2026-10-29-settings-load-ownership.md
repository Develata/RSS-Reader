# Settings 初次加载、下载与后续编辑的回填规则

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始时 HEAD：`b1b1d4a7c2c9ef75f2e5dd0038cc85f8af86982f`
- 相关 commit：与本记录同批本地提交；生成时 `commit: pending`。提交后以 `git log -1 -- docs/handoffs/2026-10-29-settings-load-ownership.md` 定位，最终 SHA 另记在本地 `final-receipt.json`。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`

## 工作摘要

修复初次加载或配置下载迟到时覆盖后续草稿/预设的问题，并防止同页保存与下载并发写入造成界面和实际存储不一致。先用旧构建复现，再补页面局部结果身份、编辑代次和共用写入门禁。正常设置、同步、主题文件操作继续走 facade/session → UiCommand → runtime → application/host。

## 影响范围

- 新增 `settings_page/request_lifecycle.rs`：纯 Rust 请求身份、两种编辑代次及单一写入准入状态，不保存设置值或平台句柄。
- `session.rs`：初次加载准入/结果归属，按编辑代次回填草稿和预设，分别发布已保存主题与未保存提示。
- `save/session.rs`、`sync/session.rs`：共用页面写入门禁，保留保存快照策略和下载的再次确认；`save/state.rs` 只保留渲染快照。
- `mod.rs`、`facade.rs`、`preferences.rs`、主题文件测试装配：接线更新；无法解析的数字输入也记录编辑意图，不改变最后有效设置值。
- 同步验收脚本增加 `lifecycle` 场景及 `settings_load_scenarios.cjs`；更新设计契约、用户指南和本记录。
- 共享页面行为影响 Web、Windows、Linux、macOS、Android；未改 application/domain/host/infra、renderer、依赖、配置格式、CSS 或组件树。

## 关键变更

### 回填与写入规则

- 初次加载只接纳当前身份一次，保留进入本页以来的编辑，包括加载 effect 尚未启动前的编辑。
- 下载在第二次确认并发出命令时捕获编辑代次：此前的草稿允许被替换，此后的草稿/预设分别保留。同值事件、改回原值也算新编辑；保留整份草稿，不逐字段合并。
- 成功加载/下载仍更新已保存的全局主题；若保留草稿与其不同，明确提示尚未保存。预设未被继续选择时根据有效草稿 CSS 识别。失败或 404 不回填。
- 保存自身沿用 `save_policy.rs` 的整值快照比较、零页大小归一化和后续预设保留规则；没有把保存回填改成编辑代次比较。
- 保存与实际下载共用一个写入 gate，必须在构造 Future 前同步准入。忙碌操作不构造第二个写入命令、不自动排队，草稿继续可编辑；被阻止的下载清除旧确认，重试需要再次确认。
- 写入准入永久使旧初次读取失效，包括迟到错误；写入失败也不恢复旧读取。当前下载的成功、缺失、失败和空结果都会释放 gate。
- 确认下载使此前尚未返回的主题文件读取失效；下载期间新导入的文件可以成为后续草稿，其自动保存受同一 gate 约束，之后需显式保存。
- 数字框继续使用现有最后有效值和错误提示；本批只记录编辑发生，不引入原始文本状态镜像或虚构默认数值。

### 选择单一写入门禁的原因

配置下载会实际导入本地设置和订阅。只按 UI 请求序号丢弃旧回复，无法撤销更晚发生的物理写入：旧基线可出现 SQLite/BrowserStore 已是下载值，界面却被迟到的保存回复恢复为旧值。此处只限制同一设置页面内的重叠写入，复用原有保存门禁的位置与提示方式；不增加全局队列、补偿写回或跨端业务分支。

## 验证与验收

证据根目录：`target/settings-load-20261029/`。最终产物仅为 `candidate-final/`；早期 `candidate/`、`candidate-web*` 和 `native-candidate/` 不是本批最终代码。`validation-identity.json` 记录已验证源码/脚本与最终产物哈希。

| 检查 | 实际结果 | 回执 / 日志 |
| --- | --- | --- |
| `cargo fmt --all --check` | 通过 | `final-fmt.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 | `final-clippy.log` |
| `cargo test -p rssr-app --locked settings_page` | 58 通过；新增 14 项 | `settings-final.log` |
| `cargo test --workspace --locked` | 571 通过、0 失败、2 忽略（既有性能探针） | `final-workspace-tests.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 | `final-check-wasm.log` |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | 通过，使用 NDK 28.2 clang/llvm-ar | `final-check-android.log` |
| `target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false` | 完整 client build 成功 | `final-build-web.log` |
| `cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b` | 通过 | `final-build-native.log` |
| 两个变更脚本的 `node --check` | 通过 | 命令退出 0 |
| 旧版同路径交错复现，1536×844 / 390×844 | 每视口 18 项旧行为按预期重现，不作为修复通过 | `baseline-final/acceptance.json` |
| 最终 Web 交错验收，相同两视口 | 36 项通过，每视口 72 篇文章标记及 6 个订阅保持 | `candidate-final-web/acceptance.json` |
| 原有 Web 设置保存回归 | 两视口正常流程、保存期间新草稿、失败重试通过 | `final-save/acceptance.json` |
| 原有 Web 配置交换回归 | 两视口各 1 PUT / 4 GET，确认、目标快照、导入、404、失败重试通过 | `final-sync/acceptance.json` |
| 原有主题文件及导入归属回归 | 文件操作两视口通过，导入交错 22 项通过 | `final-files/acceptance.json`、`final-import/acceptance.json` |
| Windows 正常同步路径，旧版 / 最终候选 | 两视口各 1 PUT / 4 GET；每视口核对 5000 篇文章标记与 6 个订阅 | `native-baseline/write/acceptance.json`、`native-candidate-final/write/acceptance.json` |
| `node scripts/browser/rssr_feeds_lifecycle_startup_test.cjs target/settings-load-20261029/startup settings-sync` | 缺目标页、缺数据库、拒绝 CDP 均正确失败并保留调用方数据 | `startup.log` 及 `startup/` |

浏览器运行方式：本机 `NODE_PATH=C:/Users/QQ/node_modules`；`PUBLIC_DIR` 分别指向旧版 `target/theme-import-20261028/candidate-final/public` 和最终 `target/settings-load-20261029/candidate-final/public`。执行 `node scripts/browser/rssr_settings_sync_acceptance.cjs --scenario lifecycle --artifact-dir <path>`，仅基线额外传 `--baseline true`。省略 scenario 为原配置交换回归；保存/文件回归分别使用现有 `rssr_settings_save_acceptance.cjs`、`rssr_theme_files_acceptance.cjs`。

- 18 个交错场景：初次加载期间编辑、改回原值、预设改回、无效数字、保存成功后的旧成功/失败、下载后的旧加载（7 项）；下载期间编辑、改回原值、预设改回、无效数字、保存、重复下载、较新文件导入、较早文件读取（8 项）；保存期间下载（1 项）；404 / 下载失败后明确保存（2 项）。
- Web 使用实际 WASM/BrowserStore。加载和保存回复通过真实 Web Lock 调用完成后延迟返回；下载由自管回环 HTTP 服务延迟。主题文件选择走 Playwright FileChooser，旧读取由 FileReader 故障注入控制。这些属于受控交错验收，不冒充自然磁盘/网络故障。
- 旧版和候选运行器 SHA-256 同为 `1d4939dde1e5a1a7df5f3375cbbc9c0300232c2e22576b3c4210ff15343a61f1`；场景脚本同为 `9166fd41d09a00638dc91cb1fa86c22ea5e01cd6407cf02e26107c14df6a030f`。候选没有开启 baseline 模式。
- Rust 测试覆盖请求一次性接纳、同值编辑、effect 前编辑、保存/下载互斥、确认与参数快照、旧成功/失败、404/空结果释放、文件身份以及页面销毁取消等待；未使用用户数据库或真实远端凭据。
- Windows 使用复制的已 checkpoint fixture、独立 EXE 目录与 WebView2 profile；隐藏启动，仅经 CDP 操作。实际原生 HTTP/SQLite 路径完成同步；脚本在修改前核对当前 Entries 页面与 5000 篇复制库身份。结束仅关闭自己的匹配进程，9276/9277 监听均已释放。运行器和复制库准备脚本保存在证据目录。
- Web、Windows 最终回执的页面异常和未预期 Console 错误均为 0。预期的 404 浏览器诊断单独记录。Web 回环请求由 Playwright 转发，未验真实远端站点的 CORS 或网络权限。
- 视觉：初始宽/窄屏与宽屏下载完成共 3 张截图与基线字节一致；窄屏下载完成图为 390×3742，仅顶栏 R 标识区域 `(44..56, 53..67)` 的 108 像素不同。查看两版未见布局、文案变化，不宣称四图逐像素一致；绘制差异根因未确定。见 `final-visual-comparison.json`、`final-visual-difference.json` 与两版 PNG。
- 一次早期脚本误要求无效数字输入在回填后仍为空；实际既有控件会回显最后有效值。最终测试改为对照编辑后的实际值及错误提示，旧版/候选重新同路径验收。最终截图使用相同设置数据，避免保护草稿后的有意持久化差异污染视觉比较。

### 最终产物身份

- 基线 Web WASM：`2be6cc54caaf55403167bebcb214da85b1c6b5b1bdb2ededadbc79060aef48d8`。
- 最终 Web WASM：`9bea9bf2e6af9f317ed290c8623d8931df770d396ee18afbaa776b8d0d161293`；HTML/JS 身份见 `candidate-final/web-identity.json`。
- 基线 Windows EXE：`3da5895d1de14928cf28c54b6d2c558b809450a6e0d1d8aca63fabe09591a70b`。
- 最终 Windows EXE：`7f1cd5e5b19b06f7827af65ce5d0efa03f813ac5314cb3d3ac338ad3b056980c`；原生测试前后使用该副本并核对身份。

## 结果

回填规则、同页写入互斥、主题文件衔接与对应验收完成，可作为本批本地交付。未发布；正常页面结构与操作入口保持，忙碌拒绝和保护后续草稿是本批有意修复的交错行为。

## 风险与后续事项

- 写入 gate 是页面局部状态，不提供跨页面、跨窗口、CLI 或多设备事务串行化，也不回滚已发生的设置/订阅导入。页面销毁后底层已开始写入的完成语义保持原样。
- Windows 实测为正常同步路径，受控乱序由 Web 和 Rust/Dioxus 测试覆盖；没有执行原生 OS 文件选择器 GUI 流程或隐藏窗口的视觉验收。Android 仅编译检查，macOS/Linux 与 `rssr-web` 托管模式未实机运行。
- 上传仍使用既有结果提示规则，不把远端上传、端点输入或 UI 状态扩成全局同步状态机。
- 下一步宜整理原生控件适配所需的输入、回填、取消和平台验收清单，优先补实际宿主边界；不继续为已有简单规则增加控制器或状态镜像。

## 给下一位 Agent 的备注

- 入口看 `request_lifecycle.rs`、`session.rs::load_with/apply_load_result` 和两个写入 session；不要用“最后请求胜出”的 UI 过滤替代有持久化副作用的写入准入。
- 保存回填仍按值相等判断，加载/下载按编辑代次判断；两者差异有意保留，避免改变既有保存归一化语义。纯确认不占写入 gate。
- 只使用 `candidate-final/`、`candidate-final-web/`、`native-candidate-final/` 与 `final-*` 日志作为最终候选证据。复验不能带 `--baseline true`。
- 原 checkout 的技能/规格文件删除、两份旧目录交接修改、附件及其他未跟踪交接全部保留在任务外。起始状态见 `initial-status.txt`，提交后核对见 `final-receipt.json`。
