# Feeds 生命周期拆分复查与验收启动失败修复

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前 HEAD（复查基线）：`2b551537791be9824b878b83e2d64ee343576982`
- 相关 commit：与本记录同批本地提交；记录生成时 `commit: pending`。提交后用 `git log -1 -- docs/handoffs/2026-10-28-feeds-lifecycle-review.md` 定位。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`

## 工作摘要

复查上一批 Feeds 请求归属与刷新提示规则提取，发现并修复一项 P2 验收基础设施问题。Rust 生产代码未发现本批引入的行为回归，保持不变。

## 影响范围

- 模块：`scripts/browser/rssr_feeds_lifecycle_acceptance.cjs`；新增 `rssr_feeds_lifecycle_startup_test.cjs`。
- 平台：验收运行器的 Web / CDP 模式；未改应用本体、数据库格式或平台适配。
- 文档：原生前置验证入口、原批次交接追加复查链接、本记录。

## 关键变更

### P2：启动失败绕过报告与资源回收

- 原脚本在 `try/finally` 之前打开 SQLite、连接浏览器并检查目标页；这些步骤抛错时不会写失败报告，也不会关闭已取得的连接。
- 已用自建无头 Chrome 的空白页复现：脚本输出 `explicit target page` 断言后，5 秒仍未退出，由复现工具终止自己的子进程；复用目录中的旧 `{"status":"passed","oldRun":true}` 原样保留。证据 `target/feeds-review-20261028/before/reproduction.json`。
- 将初始化和目标检查纳入同一错误处理范围；取得资源前先写本次 `running` 报告，失败写 `failed`，不会继续保留旧成功结果。
- 回环服务、只读数据库和浏览器连接分别尝试清理；清理失败也计入失败。最终报告在清理后写出，报告写入异常不再阻止释放资源。
- 新负向脚本运行真实子进程，验证缺少目标页、数据库不存在、CDP 返回 503。每个用例预置旧成功报告，断言及时 exit 1、新失败原因、空场景结果，以及测试数据库哈希和调用方浏览器保持不变。

### Rust 行为复核

- 对照父提交检查了命令分类、同步分配查询身份、任务作用域、整批过期结果过滤、结果发布顺序、写入不被查询失效、Shell 分流、提示捕获与清除条件。
- 没有发现提取导致的语义变化；query signal、reducer、真实等待适配与应用命令链均未修改。

## 验证与验收

### 自动化验证

- `node --check scripts/browser/rssr_feeds_lifecycle_acceptance.cjs`：通过。
- `node --check scripts/browser/rssr_feeds_lifecycle_startup_test.cjs`：通过。
- `NODE_PATH=C:/Users/QQ/node_modules` 下执行 `node scripts/browser/rssr_feeds_lifecycle_startup_test.cjs target/feeds-review-20261028/startup`：3/3 通过，子进程均自行 exit 1，无超时；各 `acceptance.json` 为本次 `failed`。总记录 `startup/results.json`。
- `cargo test -p rssr-app --locked feeds_page`：31 passed / 0 failed，`target/feeds-review-20261028/feeds-tests.log`。
- 本批范围 `git diff --check`：通过。

### 正常流程复测与产物身份

- Rust 生产文件哈希与上一批 `validation-identity.json` 一致，复用 `2b551537` 的已验证候选产物，不重建没有变化的应用。
- 已重新核对 Web index → JS → WASM 引用链；WASM SHA-256 `9905a36017e8fd0c64c762368a4cb3330d3096d7aa3bbd3a3cc28b32abf16b50`。Windows EXE SHA-256 `8AD0653A863ED11FF9625D23962D6CDE255EB84E045C85B6FF4B88A72ED92A54`。
- Web 使用 `PUBLIC_DIR=target/feeds-lifecycle-20261028/candidate/public`，运行修正后的 `rssr_feeds_lifecycle_acceptance.cjs --artifact-dir target/feeds-review-20261028/web`：1536/390 两种视口 2/2 通过。
- Windows 使用上一批独立副本启动器 `run-native.ps1 -Variant review-fix`，校验同一 EXE 哈希；新的复制库、profile 和进程运行修正后脚本，`target/feeds-lifecycle-20261028/native-review-fix/write/acceptance.json`：2/2 通过。结束后 9276/9277 端口均释放。
- 正常流程的结果与修复前完全一致；Web 的 72 篇和 Windows 的 5000 篇文章标记保持不变。两端 pageErrors / consoleErrors 均为空，没有清理错误。
- Web 两张截图与上一批候选逐字节一致：1536 宽 `752FACB58EDD72C6C273375A2AC5FA5BAB6DF237675220B7BD42B2888C2969F7`；390 宽 `AFAA3557AFDE179E1A79A9767D6897A0CB8FC32D2689C31EFDEE78CFF3FF5CF6`。
- 修正后验收脚本 SHA-256 `E6E80BA54B6FBF1AE97A1F612A85E2C92C80785F4A5BFDD4C31D19EC614127D7`；负向脚本 SHA-256 `3154E5BDDC1FEE6C90FC6774CF8B6161B6883BD579CE0C493703A20148755CA1`。证据清单 `target/feeds-review-20261028/validation-identity.json`。

### 未执行覆盖

- 没有 Rust 改动，本次不重复全工作区测试、Clippy 或跨平台构建；上一批结果属于上一批，不计为本次新执行。
- 未运行 Android 设备、Linux/macOS、托管 `rssr-web`，未做 Windows 前台视觉/输入法或人工逐项点击。
- 本次负向测试覆盖启动失败；未注入文件系统写入失败或资源关闭失败。相关分支已代码复核，不称为实测通过。

## 结果

- 一项 P2 已修复并有原始复现、三项负向回归及 Web/Windows 正常流程复测。
- Rust 拆分无需改动；本批可按精确路径本地提交。

## 风险与后续事项

- 报告只有在本次运行结束且 `status: passed` 时才可作为成功依据；强制终止留下的 `running` 不能计入通过。
- 原生内容检查仍不能替代独立进程、复制数据库和独立 profile。保持原调用方隔离要求。
- 本次限定为 Feeds 新脚本，未扩改其他历史验收脚本。

## 给下一位 Agent 的备注

- 优先读验收脚本顶层 `try/catch/finally` 和新的启动失败脚本；正常页面动作与断言未改。
- `target/feeds-review-20261028/initial-status.txt` 保存本轮起始状态。任务外删除、两份 10/03 handoff 改动和其他未跟踪文件均保留。
- 证据产物在忽略目录中，不提交数据库、浏览器 profile 或构建包；仅提交脚本和文档。无远端操作授权。
