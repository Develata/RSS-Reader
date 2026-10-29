# PR #40 最新源码的 Windows 原生验收重试

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：本地 `codex/pr40-main-integration`；推送目标 `codex/native-ui-preparation-20261029`
- 产品源码 HEAD：`50af7c99c9c47f045df8ccd7cf39c5dbba6524ed`
- 相关 commit：上述产品提交已包含 main `8932a5ac` / PR #42；本次仅提交验收文档和证据；文档 commit 为新增本记录的提交，可由本文件的 git log 查询
- 相关 tag / release：无
- 状态：`validated`（下列 Windows 原生自动化范围）

## 工作摘要

按用户“继续，再试一次”的要求，重新构建并启动 PR #40 最新产品源码，在独立的测试资料库和 WebView profile 中完成真实 Windows WebView2 / Rust / SQLite 验收。此前启动被自动审批拒绝的记录保留为历史；本次启动成功，补齐 PR #41 / #42 布局和主题预览接入后的原生传输覆盖。

## 影响范围

- 模块：原生启动与字符串传输、Settings 预览与显式保存、页面重挂载、SQLite 关闭持久化的验收记录。
- 平台：Windows 原生 debug EXE，WebView2 `156.0.4314.50`。
- 工作树：`C:/Users/QQ/.codex/worktrees/pr40-main-integration/RSS-Reader`。
- 本次未改产品源码、验收脚本、锁文件或 workflow；`E:/gitclone/RSS-Reader` 及其他工作树的改动未纳入提交。

## 关键变更

- 新增[紧凑验收回执](../testing/evidence/2026-10-29-pr40-native-retry/result.json)，固定产品 commit、脚本和 EXE SHA-256、13 项断言、数据库关闭校验以及清理结果。
- 更新[主线整合交接](./2026-10-29-pr40-main-integration.md)和交接索引，使当前原生结论指向本次运行。
- 测试使用当前 schema 的合成资料库：5000 篇文章、6 个回环订阅，不读取或修改用户资料库。测试 EXE 和所有运行产物均保留在忽略的 `target/` 下。

## 验证与验收

### 自动化验证

- `CARGO_BUILD_JOBS=2 cargo build -p rssr-app --locked`：通过（PowerShell 通过环境变量设置 jobs）。构建源码为 `50af7c99`。
- EXE SHA-256：`1c56d40f1957a0d3bc591b6fd1967e502efe6b00f1675cd71c0c74971caaba12`。
- `node scripts/browser/rssr_native_transport_acceptance.cjs --process-file target/native-pr40-retry-50af7c99/process.json --artifact-dir target/pr40-native-retry-50af7c99 --baseline false`：13/13 通过，页面和控制台错误均为 0。通过进程路径及 CDP 进程关系校验实际运行产物身份。
- 覆盖启动时 BOM、普通字段往返、真实 BOM/Unicode Rust 字符串批次、1280 / 390 视口无横向溢出、主题预览后显式保存、重挂载后继续保存。5000 篇文章的全部标志及 6 个订阅地址一致；当前页 50 篇标题经过显示核对。
- 正常关闭本次 app 进程后，数据库 `PRAGMA integrity_check` 为 `ok`，`PRAGMA foreign_key_check` 无结果，文章 5000、订阅 6。
- 仅复制已关闭的主 `library.sqlite3`，不复制 WAL/SHM，以只读 SQLite 独立打开：完整性为 `ok`，设置、全部文章标志和全部订阅地址与验收最终快照逐项相同。主库 SHA-256 为 `a4543d3ea86199abb5f6d25efc38c3d77831173f46e83a40eb2d34e2e0839070`。原目录 WAL 文件仍存在，大小为 0；不将其表述为 WAL 文件不存在。
- 原生进程正常退出，回环 fixture 已停止，9277 / 9278 无监听。专用 stdout/stderr 日志为空。
- 文档和回执：JSON 解析、断言数量/状态、源报告摘要及 SHA-256 比对、本地链接和 `git diff --check` 在提交前检查。

### 手工验收

- 未进行人工 UI 验收；本轮为 CLI / CDP 自动化，没有 Computer Use。
- 真实 OS 文件选择器、真实 IME、Android 真机、macOS/Linux GUI 和正式安装包未覆盖。390px 视口不代表 Android 验收。

### 证据位置与 CI

- 构建日志：`target/pr40-native-retry-build.log`。
- 测试日志：`target/pr40-native-retry-50af7c99.log`。
- 原始报告、关闭校验及独立主库：`target/pr40-native-retry-50af7c99/`。
- EXE、隔离库、WebView profile 和进程身份：`target/native-pr40-retry-50af7c99/`。
- 产品提交 `50af7c99` 的自动 [CI 40885265153](https://github.com/Develata/RSS-Reader/actions/runs/40885265153) 和 [Pages 40885265198](https://github.com/Develata/RSS-Reader/actions/runs/40885265198) 已通过，37 项成功、1 项预期跳过、0 失败。该结果不冒充本次文档提交的新 head 检查。
- 本次文档提交正常 push 后，另核对新 head 的自动检查；最终 head、run 链接和结果在 [PR #40](https://github.com/Develata/RSS-Reader/pull/40) 描述中记录，避免再次提交纯 CI 状态而产生新的待验 head。

## 结果

本次 Windows 原生自动化验收通过，先前“最新产品源码尚未原生复跑”的缺口已补齐。产品源码仍为 `50af7c99`，后续文档提交不改变已验收产物。按既有授权提交、push 并核对 CI；PR 保持草稿，不合并或发布。

## 风险与后续事项

- BOM 适配依赖当前锁定解释器版本，升级 Dioxus / sledgehammer 后仍须重新执行实际原生传输验收。
- 保留上述未覆盖的人工和平台范围，不用 Web 自动化或编译结果替代设备验收。
- 其他工作树传来的 Settings 建议按当前契约复核：无法解析的数字输入不产生补丁，等值回填不作废主题文件；PR #42 主题卡片已经即时预览，旧未应用下拉选择不再存在。本次不改这些行为，也不把其他工作树的测试计入本回执。

## 给下一位 Agent 的备注

- 从 `docs/design/native-control-adapter-contracts.md` 和 `docs/testing/native-control-acceptance.md` 理解边界。
- 优先读本回执及原始报告；旧原生成功运行与旧审批拒绝仅为历史记录。
- 保留当前工作树和忽略的证据目录；不清理其他工作树、分支或用户数据。
