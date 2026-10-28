# 主题文件交互契约与失败路径修复

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始时 HEAD：`f25dbdbb4f1fd9c07d0f786c13aa6b1645a06f4e`
- 相关 commit：与本记录同批本地提交；记录生成时 `commit: pending`。提交后以 `git log -1 -- docs/handoffs/2026-10-28-theme-file-contracts.md` 定位。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`

## 工作摘要

承接 Settings 边界盘点，验证主题文件的取消、编码、读取/保存失败和离页行为。先建立旧版本行为证据，再修复 4 类错误路径及 Web 离页时底层读取未取消的问题。保留现有模块与有效 UTF-8 文件的导入/导出行为，不增加控制器、状态镜像或新的 crate。

## 影响范围

- `settings_page/themes/theme_file_io.rs`：文件选择返回值、字节读取/解码、浏览器下载资源清理及真实临时文件测试。
- `themes/lab.rs`：Web 导入调用字节读取适配器。
- `themes/theme_io.rs`：保留页面任务和提示分支，增加私有 Future 注入点及 Dioxus 生命周期测试。
- `crates/rssr-app/Cargo.toml`：仅桌面明确启用已有 Tokio 的 `fs` feature；`gloo-file` 已是现有依赖，锁文件未变。
- 新增浏览器验收脚本，更新命令契约、原生前置边界和用户指南。
- Web/桌面文件行为受影响；Android 导入仍未实现，导出仍使用原系统保存器。未改变 application/domain/host/infra、配置格式、样式或组件树。

## 关键变更

### 复现与修复

| 场景 | 基线证据 | 候选行为 |
| --- | --- | --- |
| CSS 注释内含 `0xFF` | Web 将坏字节替换成 U+FFFD 后成功保存 | 先读取字节并严格检查 UTF-8，提示错误，草稿/持久化不变 |
| 浏览器 FileReader 触发 error | 原 Dioxus 读取接口只等待 load，没有错误反馈 | 复用已有 gloo reader，返回错误并可重试 |
| 下载节点插入失败 | object URL 保留 1 个 | 无论插入成功或失败均释放；节点准备失败前不创建 URL |
| 桌面所选文件已消失 | `rfd` 的 `Result::unwrap()` panic，专项测试退出 101 | `tokio::fs::read` 返回可显示的 I/O 错误 |
| 读取期间离页 | 新页面未被污染，但底层读取未 abort | 仍取消页面等待，同时 abort Web 读取 |

- 基线页面证据：`target/theme-files-20261028/baseline-web-final/acceptance.json`；其中 `baseline: true` 表示成功记录旧缺陷，不表示旧版本满足修复契约。
- 桌面 panic 复现：`baseline-native-read-error.log`。在原读取语句外增加可测试的选择结果函数后执行同一缺失文件测试，确认失败，再替换读取实现。
- 有效文件保留原语义：Web 去掉开头可选 UTF-8 BOM，桌面保留；文件读取层不改 CRLF，非 ASCII 内容按 UTF-8 保留。空 CSS 文件可以导入。
- 导出使用点击时草稿，不提交设置；全空白导出仍提前返回。原生选择取消仍显示取消提示，Web 空选择仍保持既有提示。已导入内容的持久化失败仍保留草稿，需要明确重试。

### 测试边界

- 新增 5 项原生文件适配测试：真实字节往返、取消、坏编码、文件消失、写入失败后重试。只使用唯一临时目录和自建文件，逐文件清理，不访问用户数据库。
- 新增 4 项 Dioxus 测试：导入取消/失败/无效 CSS 保留后来草稿和已应用主题；导出成功/取消/失败保留快照；空导出不启动选择器；销毁页面取消导入与导出等待。
- 这些 Dioxus 测试注入的是宿主结果，不能算系统选择器实机验收。已经发生的写入不承诺回滚，页面离开也不承诺强制关闭操作系统对话框。

## 验证与验收

证据根目录为 `target/theme-files-20261028/`，均为本地忽略产物。

| 命令 / 路径 | 结果 | 证据 |
| --- | --- | --- |
| `cargo fmt --all --check` | 通过 | `fmt.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 | `clippy.log` |
| `cargo test -p rssr-app pages::settings_page::themes --locked` | 14 通过 | `theme-tests.log` |
| `cargo test --workspace --locked` | 549 通过、0 失败、2 忽略（既有手工性能探针） | `workspace-tests.log` |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 | `check-wasm.log` |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | 通过，使用 NDK 28.2 的 clang/llvm-ar | `check-android.log` |
| `target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false` | 通过，完整日志有 client build 成功回执 | `build-web.log` |
| `cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b` | 通过 | `build-native.log` |
| Web 同路径验收，1536×844 与 390×844 | 两视口通过 | `candidate-web-final/acceptance.json` |
| 指定不存在的 Web 构建包 | 退出 1，旧成功回执被替换为 failed | `startup-negative/acceptance.json`、`startup-negative.log` |

浏览器运行方式：本机 `NODE_PATH=C:/Users/QQ/node_modules`；`PUBLIC_DIR` 分别指向 `target/settings-sync-20261028/candidate/public` 和 `target/theme-files-20261028/candidate/public`，执行 `node scripts/browser/rssr_theme_files_acceptance.cjs --artifact-dir <path>`。仅旧基线额外传 `--baseline true`，候选不传此参数。

- 使用真实 Chrome headless/WASM/BrowserStore 和已有离线 fixture。文件选择使用 Playwright FileChooser，导出读取真正的浏览器下载文件并核对 UTF-8 字节；不接触日常浏览器 profile 或用户文件。
- 两视口按相同顺序验证取消、无效 CSS、空文件、BOM/CRLF/Unicode、同名文件重选、当前未保存草稿的下载字节、导出中途失败及清理/重试、空白导出、坏编码、读取失败及重试、离页后的迟到读取、配置持久化失败及明确重试/重载。
- FileReader 失败/延迟、DOM 插入失败和 Storage 写入失败由脚本注入，走实际页面代码；它们是故障注入验收，不宣称实际磁盘故障或系统权限故障已在 GUI 中手工重现。
- 每个视口核对全部 72 篇文章标记保持不变；候选 `pageErrors`、`consoleErrors` 都为空。
- 初始页、有效文件导入后，各两视口共 4 张整页截图均逐字节一致；已查看桌面与窄屏截图。证据见 `screenshots-comparison.json` 和两次最终运行的 PNG。
- 首两次基线脚本使用了“空提示不存在”和“空 CSS 的 style 标签必存在”的错误假设，已修正；失败回执保留，以 `*-web-final` 为最终证据。
- 系统文件对话框的真实点击/取消、Windows GUI 文件选择流程、Android 实机、Linux/macOS 实机：未执行。本轮 Windows 覆盖真实临时文件 I/O、Dioxus 任务及原生编译，不等价于 GUI 全链路验收。

### 产物身份

| 产物 | SHA-256 |
| --- | --- |
| 基线 Web WASM | `62bcec002c5a55bffe5f994d9a47e901a252ff260d0ad9bcb24589667d21dfb3` |
| 候选 Web WASM | `bbef027a6e8d5928c5a7bf84f79d2047b357704d7237759f259f24efe5e549ac` |
| 候选 Windows EXE | `0AB53D942EB6ADCABB7ECFC49E17A60AFC9CE4E84E1DAB5D5FEAD930E9E12CD0` |

Web 完整引用文件身份见 `baseline-web-identity.json`、`candidate/web-identity.json`；源码、Cargo manifest 和脚本身份见 `validation-identity.json`；最终浏览器回执同时记录脚本哈希和构建目录。产物是本地验证候选，不是发布包。

## 结果

- 本批契约验收和已复现错误修复完成，可作为独立本地提交交付。
- 正常文件操作的页面布局和操作路径保持；错误路径的差异是本次明确修复的行为。

## 风险与后续事项

- 未定义同页多个文件导入之间的胜出规则，也未改变导入与用户新编辑交错时的草稿策略；下一批建议先复现这些交错场景，再确定最小结果归属保护。
- Android 导入仍提示手动粘贴。本批不增加平台能力，不替换系统文件选择器，不承诺取消已开始的物理写入。

## 给下一位 Agent 的备注

- 文件能力边界继续看 `theme_file_io.rs`；页面等待/反馈看 `theme_io.rs`；CSS 校验与配置保存继续复用既有 `theme_apply.rs` / save session。不要将平台文件类型传到 application/domain。
- 当前 checkout 的 `.agents` / `.specify` 删除、旧目录交接修改、附件和其他未跟踪交接均为任务外内容，保留且不纳入本批提交。
