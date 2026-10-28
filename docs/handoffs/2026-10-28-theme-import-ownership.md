# 主题文件导入结果归属保护

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始时 HEAD：`49860dd44128d5e19dcda37e2b56be4fbab78f55`
- 相关 commit：与本记录同批本地提交；记录生成时 `commit: pending`。提交后以 `git log -1 -- docs/handoffs/2026-10-28-theme-import-ownership.md` 定位。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`

## 工作摘要

承接上一批主题文件契约，先在原构建上复现同页多个导入及后续编辑的交错问题，再由 Settings session 统一判定文件结果是否仍属于当前操作。迟到文件不再覆盖新草稿、恢复已清空的 CSS，或将旧错误显示在新操作之后。正常文件流程、已有设置保存规则和页面布局保持原有实现。

## 影响范围

- `settings_page/session.rs`：一个页面局部导入代次、草稿/预设修改入口，以及加载/保存回填时的失效判断。
- `settings_page/facade.rs`、`themes/theme_apply.rs`：明确的编辑、应用、清空和保存操作使旧导入失效。
- `themes/lab.rs`、`themes/theme_io.rs`：Web / 桌面的选择器身份捕获、统一结果准入和反馈。
- `scripts/browser/rssr_theme_files_acceptance.cjs`：新增 `ownership` 场景，具体用例在 `theme_import_scenarios.cjs`；默认文件回归路径保留。
- 更新原生 UI 前置边界、前端命令契约、用户指南；没有 workflow、依赖或数据库格式变更。
- 影响 Web 与桌面的导入等待；Android 导入仍提示手动粘贴，导出不变。application/domain/host/infra 和 renderer 未变。

## 关键变更

### 一次导入的结果只能接纳一次

- 在打开选择器之前同步分配 `ThemeImportRequest`，后来的导入立即取代旧请求；新选择器取消、新文件读取/校验失败均不会恢复旧身份。
- `complete_css_import` 在 CSS 校验、提示发布、草稿写入和设置保存之前检查并消耗身份。过期成功、失败和取消全部丢弃。
- 原生任务在首次轮询文件选择 Future 前检查身份；Web 隐藏 input 仅关联本次选择器事件，继续清空文件值以支持同名文件重选。
- 用户修改设置草稿、选择预设、应用/清空主题或显式保存都会使旧请求失效；改回原值、同值事件也不会令旧请求复活。
- 加载/保存实际改变草稿或预设时使旧导入失效；完全相同的回填、状态提示，以及既有保存保留后续草稿时，均保留新导入。复核中专门补上了相同初次加载不误取消导入的用例。
- 不增加操作队列、另一份设置或文件 busy 状态，不取消已开始的设置写入。原保存 gate 和后续编辑保留规则继续生效。

### 旧版本复现与候选验收

每种屏宽均按相同脚本执行以下 11 个用例；基线模式断言旧缺陷存在，不能作为修复通过证据。

| 场景 | 旧基线 | 最终候选 |
| --- | --- | --- |
| 新文件先完成，旧成功/读取失败/无效 CSS 后完成（3 项） | 旧内容或旧错误被接纳 | 新草稿、已应用主题和提示保持 |
| 读取中 CSS 改回原值、修改外观、选择预设、清空、同值显式保存（5 项） | 旧文件覆盖后续意图，甚至顺带保存后来修改的设置 | 后续草稿、预设及明确保存结果保持 |
| 打开新选择器，旧读取在新选择完成前返回，随后取消新选择 | 旧文件仍被应用 | 从打开新选择器起旧请求即失效 |
| 新文件 CSS 无效后旧文件返回 | 旧文件重新生效 | 保留新文件错误，不恢复旧请求 |
| 旧文件已进入保存，再导入新文件 | 原保存完成，保留后续草稿，需明确保存 | 同样行为，公共 pending gate 未改变 |

## 验证与验收

证据根目录：`target/theme-import-20261028/`。`candidate-final/` 为本批最终构建；早期 `candidate/` 和 `candidate-web/` 不作为最终交付身份。`validation-identity.json` 保存相关源码/脚本、锁文件和最终产物 SHA-256。

| 检查 | 实际结果 | 证据 |
| --- | --- | --- |
| `cargo fmt --all --check` | 通过 | `final-fmt.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 | `final-clippy.log` |
| `cargo test --workspace --locked` | 557 通过、0 失败、2 忽略（既有手工性能探针） | `final-workspace-tests.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 | `final-check-wasm.log` |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | 通过，NDK 28.2 clang/llvm-ar | `final-check-android.log` |
| `target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false` | 完整 client build 成功 | `final-build-web.log` |
| `cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b` | 通过 | `final-build-native.log` |
| 两个验收脚本的 `node --check` | 通过 | 命令退出 0 |
| 旧构建，1536×844 / 390×844，导入交错 | 两视口各 11 项旧行为按预期重现 | `baseline-web-final/acceptance.json` |
| 最终构建，相同两视口与脚本 | 22 项通过；每视口 72 篇文章标记保持，页面异常为 0 | `candidate-final-web/acceptance.json` |
| 最终构建，既有文件操作回归 | 两视口通过；有效/无效/空文件、UTF-8、下载字节、失败/离页与重试均通过 | `candidate-final-files/acceptance.json` |
| 针对截图差异单独复跑导入交错 | 两视口通过，窄屏截图差异仍存在 | `candidate-final-web-repeat/acceptance.json` |

浏览器命令：本机设置 `NODE_PATH=C:/Users/QQ/node_modules`；`PUBLIC_DIR` 分别指向旧版 `target/theme-files-20261028/candidate/public` 或最终 `target/theme-import-20261028/candidate-final/public`，运行 `node scripts/browser/rssr_theme_files_acceptance.cjs --scenario ownership --artifact-dir <path>`。只有基线额外传 `--baseline true`；候选不传。省略 `--scenario ownership` 运行既有文件回归。

- 验收使用真实 headless Chrome、Web/WASM、BrowserStore 和隔离 fixture。FileReader 延迟/错误由脚本注入，选择通过 Playwright FileChooser 完成，持久化延迟使用真实 Web Lock；不等同于手工制造磁盘故障或系统选择器操作。
- 基线与最终候选的运行器 SHA-256 均为 `bbbe2e7f27c36d285694a8b3de4103853ac0c093503997df224aa439ba5bdbc9`，场景脚本均为 `7a85fb4efe4ee1b927ab06525bd5ac45b7ae1ca2bb94392f27299de1c210317e`。
- Rust 新增 8 项测试，覆盖代次接纳/消耗、同值和改回原值、加载/保存回填、较新取消与旧失败交错、未开始的过期选择器不被轮询；扩展原反馈测试，区分当前和已被编辑取代的成功校验/取消/错误结果。页面释放与真实文件读写的既有测试也随完整测试执行。
- 视觉检查：初始宽/窄屏及宽屏导入完成共 3 张与基线字节一致。窄屏导入完成图为 390×3718，其中 110 像素不同，分布于顶栏图标和主题文件区底边；查看两版截图未见布局或文案变化，但不宣称四图逐像素一致，未确定绘制差异的原因。见 `visual-comparison.json`、`visual-difference.json` 和各目录 PNG。
- Windows 原生系统选择器 GUI、macOS/Linux 和 Android 实机未运行；Windows 有 release 构建及 Rust 文件/任务测试，Android 仅编译检查。本批不宣称这些平台的手工文件流程已验收。

### 最终产物身份

- 旧基线 WASM：`bbef027a6e8d5928c5a7bf84f79d2047b357704d7237759f259f24efe5e549ac`。
- 最终 Web WASM：`2be6cc54caaf55403167bebcb214da85b1c6b5b1bdb2ededadbc79060aef48d8`，HTML/JS 清单见 `candidate-final/web-identity.json`。
- 最终 Windows EXE：`3da5895d1de14928cf28c54b6d2c558b809450a6e0d1d8aca63fabe09591a70b`，路径 `candidate-final/rssr-app.exe`。

## 结果

同页主题导入的身份保护和配套回归完成，可作为这一批本地交付。正常行为沿用原入口；新语义只丢弃已被后续操作取代的未接纳文件结果。未推送或发布，实机未测范围与截图差异保留在上述记录中。

## 风险与后续事项

- 当前 Load / WebDAV 下载仍可整份回填设置，与保存保留后续编辑的策略不同。本批只让这种实际回填使旧文件导入失效，没有修改这些命令自身的胜出规则。下一步建议先复现初次加载、下载与新编辑/保存的交错，再确定最小保护；不要把主题文件代次扩成全局设置状态机。
- 已开始的保存不回滚；Web 新草稿遇到公共 pending gate 后仍要求明确保存。离页不承诺关闭系统对话框或取消物理写入。
- Android 文件导入能力未新增；文件句柄与浏览器事件继续留在当前前端适配层，不传入 application/domain。

## 给下一位 Agent 的备注

- 从 `session.rs` 的 begin/complete/update 入口和 `theme_io.rs::complete_css_import` 开始；Web 必须在打开选择器时取得身份，不能退回到 onchange 才分配。
- 重新运行最终候选时不要带 `--baseline true`；也不要以早期 `candidate/` 的产物或历史上一批回执替代本批最终身份。
- checkout 原有 `.agents` / `.specify` 删除、两份旧目录交接修改、附件及其他未跟踪交接均为任务外内容，保留且不纳入本批提交。开始状态见 `initial-status.txt`；最终本地提交及范围核对另记在忽略目录的 `final-receipt.json`。
