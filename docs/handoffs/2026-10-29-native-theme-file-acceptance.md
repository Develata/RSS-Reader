# Windows 文件部分验收与原生 BOM 传输修复

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始时 HEAD：`075569b390f41a1eb1981025dc5a794ac8570fc1`
- 相关 commit：与本记录同批本地提交；生成时 `commit: pending`。提交后用 `git log -1 -- docs/handoffs/2026-10-29-native-theme-file-acceptance.md` 定位；最终 SHA 另记本地 `final-receipt.json`。
- 相关 tag / release：无；不 push、不打 tag、不发布
- 状态：`validated`（BOM 修复与直接回归）；Windows 系统文件流程仍为**部分验收**

## 工作摘要

执行原生控件清单的 Windows CSS 文件切口时发现：导入含 BOM 的有效文件，SQLite 中原文完整，但文本框、枚举、数字、WebDAV 路径及保存按钮状态同时错位。随后按用户要求停止 Computer Use，改用命令行和原生 WebView/CDP 直接复现、修复及验收。无需文件选择器，直接输入带 BOM 的 CSS 并点击“应用当前 CSS”也可稳定触发同一缺陷。

## 影响范围

- 生产代码：`crates/rssr-app/src/bootstrap/native_document.rs`、`bootstrap.rs`、`main.rs`。
- 验收工具：`scripts/browser/rssr_native_transport_acceptance.cjs`。
- 文档：[适配契约](../design/native-control-adapter-contracts.md)、[验收清单](../testing/native-control-acceptance.md)、[原生界面前置边界](../design/native-ui-preparation.md)。
- 平台：Windows 实际 EXE / WebView2 / SQLite；Android 共用启动适配，仅编译检查。静态 Web 不加载该模块，完成 WASM 编译检查；Linux/macOS 未运行。
- 未新增依赖、crate、配置格式、迁移、业务状态或渲染器。未改文件读写、导入身份、保存 gate、Web 去 BOM 规则和 Android 文件能力。

## 关键变更

### 原生字符串表保留 BOM

锁定版本 Dioxus 0.7.9 / `sledgehammer_bindgen_macro` 0.6.5 用默认 `TextDecoder` 解码整批字符串，再按 Rust 提供的 UTF-16 长度切分。默认解码器会丢弃表首 BOM，长度因此相差一个 UTF-16 单元，后续字段发生位移。实际抓到的旧批次同时包含 CSS、`status-banner`、`idle`、三枚举、四数字和配置路径；原始字符串与解码结果恰好相差开头的 BOM。

将已有原生标题/favicon 组装移到 `bootstrap/native_document.rs`，在 module 创建解释器之后、Dioxus `load` 开始接收变更之前的 `DOMContentLoaded` 中，仅将该实例的 `c` 解码器设为 `ignoreBOM: true`。名称中的 ignore 表示不解释 BOM、将其保留为文本。浏览器全局 `TextDecoder` 行为不变，CSS 原文不被改写。Rust 模块中的少量 WebView 初始化脚本只承担当前宿主传输兼容，不承载业务或页面状态。

这是对已锁定依赖内部字段和启动顺序的窄范围兼容措施。升级 Dioxus / sledgehammer 时必须复核并重跑传输验收；若上游已修正，可移除此措施，不能把它当稳定公开接口。

### 可重复的直接回归

新增运行器先核对真实进程路径、CDP 进程链、独立 profile 路径、EXE 同目录复制库、5000 个固定文章 ID/标题、当前 Entries 页和 6 个回环订阅，随后才操作设置。SQL 连接只读，设置写入走实际页面 → Rust → SQLite 路径。

运行器覆盖普通 CSS、表首 BOM、连续 BOM、中文/emoji、中间 BOM、后续字段和已应用样式对齐、页面重开与继续保存；观察真实解码调用但不改其返回值。记录 EXE/脚本 SHA-256、版本、原始/解码字符串表、设置、全部文章标记和订阅。启动失败替换旧成功回执，退出非零；连接释放失败也记为失败。它不驱动 OS 文件窗口或真实输入法。

## 验证与验收

证据目录：`target/native-theme-files-20261029/`（本机 ignored，缺失时不可凭本文补写通过）。

### 产物和隔离身份

- 旧 EXE：复用 `target/settings-load-20261029/candidate-final/rssr-app.exe`；源码 `ea70ff19f7713220e8c1dec93aa53fa6fd679bc9`，至开始 HEAD 的生产代码无变化。SHA-256：`7f1cd5e5b19b06f7827af65ce5d0efa03f813ac5314cb3d3ac338ad3b056980c`。这是复用产物，不是本批重构建。
- 新 EXE：本批 `cargo build -p rssr-app --release --locked` 构建，复制到 `candidate-run/rssr-app.exe`；SHA-256：`43cc29086c9f9097aa6426c7eefc055e82778792a62ae398e52bffafbc1ef8ee`。源码/脚本最终哈希见 `validation-identity.json`。
- 系统：Windows 11 家庭版中文版，10.0.26200；WebView2 `156.0.4314.50`。旧、新 EXE 各使用独立运行目录、`RSS-Reader/` 数据库与 `webview/` profile。
- 来源为 `target/reading-baseline-20261025/seed-matched/` 的已知 fixture，先校验 5000 个固定 ID/标题与 6 个来源，再将**复制库** feed URL 改为本批独占的 `127.0.0.1:9277` 服务；服务只返回受控响应。
- 旧 fixture 归档阈值 0，完成身份检查后在隔离 Settings 页改为 120 并明确保存；准备差异见 `prepared-baseline.json`。源 fixture 未改，未放宽生产校验。

### 自动化验证

| 命令 / 路径 | 实际结果 | 回执 |
| --- | --- | --- |
| `cargo fmt --all --check` | 通过 | `fmt.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 | `clippy.log` |
| `cargo test -p rssr-app --locked settings_page` | 58 通过，0 失败；包括 5 项真实文件 I/O 和 6 项 Dioxus 文件任务测试 | `test-settings.log` |
| `cargo build -p rssr-app --release --locked` | 通过 | `build-candidate.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 | `check-wasm.log` |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | 通过，NDK 28.2.13676358 的 API 24 clang / llvm-ar | `check-android.log` |
| `node --check scripts/browser/rssr_native_transport_acceptance.cjs` | 通过 | 退出 0 |
| 旧 EXE 同路径传输探针，`--baseline true` | 8 项断言完成，状态 `baseline-reproduced`；不是修复通过 | `transport-baseline-final/acceptance.json` |
| 新 EXE 同一探针，`--baseline false` | 13 项通过，page/console 错误均为 0 | `transport-candidate-final/acceptance.json` |
| `rssr_settings_save_acceptance.cjs --mode reopen` | 新 EXE 退出再启动后，通过设置/已应用 CSS/全体文章标记核对 | `reopen/acceptance.json` |
| 启动隔离检查负向验收 | 缺少进程回执、CDP 属于其他进程两种实际失败均退出 1、覆盖旧成功报告，未改变测试库 | `probe-startup.json` |
| 启动兼容范围 | 标题/favicon 保持；仅解释器保留 BOM，全局解码器仍按默认行为去 BOM | `startup-scope.json` |
| 1280×900 / 390×900 普通 CSS 同路径渲染 | 两张旧/新 PNG 各自逐字节一致，无横向溢出；仅 CDP 渲染证据 | `data-audit.json`、两个 `transport-*-final/ordinary-*.png` |
| 数据与清理 | 两个运行副本共 4 个数据库完整性 `ok`；每份索引库 5000 个标记和 6 个来源保持，2 个源库与 7 个源样本哈希保持；退出本批应用、关闭服务，9276/9277/9278 监听释放 | `data-audit.json`、`closed-processes.json`、`cleanup.json` |

执行命令示例（本批 shell 的 `NODE_PATH=C:/Users/QQ/node_modules` 用于本机已有 Playwright；不代表项目新增依赖）：

```powershell
node scripts/browser/rssr_native_transport_acceptance.cjs --process-file target/native-theme-files-20261029/run/process.json --artifact-dir target/native-theme-files-20261029/transport-baseline-final --baseline true
node scripts/browser/rssr_native_transport_acceptance.cjs --process-file target/native-theme-files-20261029/candidate-run/process.json --artifact-dir target/native-theme-files-20261029/transport-candidate-final --baseline false
node scripts/browser/rssr_settings_save_acceptance.cjs --cdp http://127.0.0.1:9276 --database target/native-theme-files-20261029/candidate-run/RSS-Reader/rss-reader.db --mode reopen --expect target/native-theme-files-20261029/transport-candidate/acceptance.json --artifact-dir target/native-theme-files-20261029/reopen
node target/native-theme-files-20261029/probe-startup-check.cjs
```

`process.json` 中的 PID 只是当次执行身份，已退出，不可直接复用。重启时须重新生成进程回执。最终脚本在旧、新候选上均重跑；早期无 `-final` 的传输报告保留作过程证据，重启验收使用的早期候选报告与最终 EXE 相同。

### 已执行的系统窗口动作和边界

以下全部发生在旧 EXE 上，使用真实 Windows 打开窗口；不是修复候选的 GUI 验收。

- W-FILE-01：选择中文路径有效 CSS、自动保存、重开页面通过。
- W-FILE-02：取消、同名重选、空文件导入共 3 项通过；取消未改草稿/设置，焦点返回导入按钮，滚动位置保持。
- W-FILE-03：无效 CSS、无效 UTF-8 两项通过，草稿与已保存值保持。BOM/CRLF 文件原文入库，但显示错位，证据在 `w-file-03-bom-crlf-discrepancy.json` 及对应截图。该缺陷由直接 CDP 同路径复现与修复回归闭环。
- 上述 6 个通过子场景保留于根目录 `acceptance.json`，整体状态为 `partial-with-reproduced-defect`。最初一次工具点击“打开”没有提交，重新观察后用 Alt+O 完成，记录为自动化操作重试，不算产品失败。
- W-FILE-03 选定后的真实读失败、04/05 系统导出/覆盖/写失败重试、06 系统窗口等待期间离页、07 Tab/Shift+Tab 与真实 IME **未覆盖**。中文路径是路径设置，不是输入法组词证据。前台 OS 截图只保留此前旧候选现场。

## 结果

BOM 传输缺陷已修复，真实新 EXE 的直接输入、Rust 传输、SQLite 保存、重开/重启和相称编译/测试通过，可按此范围本地提交。系统文件流程仍是部分验收；不声明原生适配全部完成或发布就绪。

用户要求后续测试不用 Computer Use，本批随后只通过命令行/CDP/现有测试完成验证。未新跑静态 Web UI、工作区全量测试、Android 设备、Linux/macOS；既有 571 项工作区结果属于上一批，不能算作本批执行数。

## 风险与后续事项

- 原生兼容措施依赖锁定版本的内部解码器字段与启动顺序。升级依赖时首先运行传输探针；探针中 `startup preserves BOM before page edits` 及真实 BOM 表检查应继续通过。
- 下一步优先用同样的直接测试方式验证 Settings 三个枚举、四个数字输入与程序回填的宿主通知边界，复用现有 Rust 规则；不要为此新建一套设置状态或更换渲染器。系统窗口/真实 IME 缺口继续单列，不以直接测试补写通过。
- Android 使用同一原生文档适配但本批只有编译证据；Web 文件读取去 BOM、桌面保留 BOM 的既有差异保持。
- `target/` 证据不随提交发布。异机重跑需先重建隔离 fixture 与空闲回环服务，不能使用用户数据库或历史 PID。

## 给下一位 Agent 的备注

- 从 `native_document.rs` 的兼容说明、运行器和本记录进入；问题不在 `theme_file_io.rs` 的 UTF-8 解码，也不是保存/导入生命周期需要新增 gate。
- 原生字符串表捕获只在测试里观察，最终释放恢复原方法；生产代码不包含捕获或测试入口。
- 仅暂存本批 3 个 Rust 文件、1 个运行器和 4 个文档。原有 `.agents/skills` / `.specify` 删除、两份 10/03 handoff 修改和既有未跟踪项全部保留；初始状态在 `initial-status.txt` / `initial-staged.txt`。
- 本批独立进程与监听已退出；样本、截图、数据库副本和日志保留，没有进行工作区清理或访问用户库。
