# Settings 保存规则 review 与验收目标保护修复

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前 HEAD / review 对象：`dda9aefa1533589a4caf7c2f968829f3ad1b21b6`
- 相关 commit：与本记录同批本地提交；记录生成时 `commit: pending`，提交后以 `git log -1 -- docs/handoffs/2026-10-28-settings-save-review.md` 定位。
- 相关 tag / release：N/A；本批无远端操作。
- 状态：`validated`

## 工作摘要

复查 Settings 提取提交，逐段比较重构前后的提交准备、pending gate、结果投影与归并、整份草稿比较、预设回填和页面任务取消。未发现本批 Rust 生产逻辑的行为回归；确认并修复 1 项 P2 验收脚本问题。

## 影响范围

- 模块：`scripts/browser/rssr_settings_save_acceptance.cjs`，原生前置验收文档及上批交接补充链接。
- 平台：Windows 验收前置检查；Web 共用报告增加总体状态。
- 生产 Rust、CSS、数据库 schema、依赖、workflow 均未改动。

## 关键变更

### [P2] 错误原生目标在被拒绝前已经写入设置

- 原实现的 `seed()` 会先把原生库中的旧归档阈值 0 通过 UI 保存为 120，之后主场景才检查文章数量是否为 5000。传入错误目标时，“验收失败”不能保证此前未改设置；仅靠数量也无法区分同数量的普通数据。
- 在明确隔离的 fixture 副本中删除 ID 1，剩 4999 篇。未修脚本以 `4999 !== 5000` 失败，但只读前后核对证明 `archive_after_months` 已由 0 变为 120。复现目录 `native-original-short`；没有使用个人数据或个人浏览器 profile。
- 新 `checkNativeFixture` 在任何 UI 操作和 `seed()` 之前运行：先核对 SQLite 中全部 5000 个连续 ID 及 `scripts/seed_reading_native.py` 定义的固定标题，再等待新进程的 Entries 页就绪，将已挂载文章 ID/标题逐一与所提供数据库对照。write 与 reopen 共用此检查。
- 验收报告增加总体 `running/passed/failed` 状态与失败原因，前置检查失败时也保留明确失败记录；成功报告记录数据库文章数及页面比对行数。调用方仍须自行保证独立进程、复制数据库与独立 WebView profile。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-app --locked settings_page`：24 passed / 0 failed，`settings-tests.log`。核对原有保存集成测试及本批提取的纯策略、同步 gate、投影过滤、命令构造和页面取消测试。
- `node --check scripts/browser/rssr_settings_save_acceptance.cjs`：通过。
- `git diff --check`（本批精确路径）：通过。
- `node target/settings-save-review-20261028/verify-review.cjs`：通过；汇总 `review-receipt.json` 验证负向拒绝原因、设置前后相等、正常场景与上批结果相等、截图相等、生产源码和 EXE 哈希不变。

### 真实 Web / Windows 自动化验收

证据位于 `target/settings-save-review-20261028/`，运行产生的 profile、复制数据库和产物未入库。

| 场景 | 执行方式与结果 |
| --- | --- |
| 旧脚本的 4999 篇目标 | `run-native-review.ps1 -Variant original-short ... -Corrupt shorten -ExpectRejection`：错误拒绝晚于设置写入，0 → 120，复现成立 |
| 修复后的 4999 篇目标 | `-Variant fixed-short ... -Corrupt shorten -ExpectRejection`：文章数量前置检查拒绝，完整设置与运行前相同，归档阈值仍为 0 |
| 5000 篇但数据库标题错误 | `-Variant fixed-wrong-db ... -Corrupt rename -ExpectRejection`：数据库身份检查拒绝，完整设置不变 |
| 正确的只读核对库，实际 UI 文章标题错误 | `-Variant fixed-wrong-ui ... -Corrupt rename-ui -ExpectRejection`：页面与数据库比对拒绝，实际进程数据库的完整设置不变 |
| 合法 Windows fixture | `-Variant valid ...`：两个视口保存流程及进程结束后重开全部通过；完整设置和全部 5000 篇文章标记与上批结果一致，无 page/console 错误 |
| Web 四项流程 | `node scripts/browser/rssr_settings_save_acceptance.cjs --artifact-dir target/settings-save-review-20261028/web`：4/4 通过，包含两种视口、保存期间编辑、写失败重试，无 page/console 错误 |

- 辅助启动脚本 `run-native-review.ps1` 由上一批隐藏进程启动器派生，校验复制 EXE 哈希、专用端口和进程路径，只关闭自己的进程。负向改动通过 `fixture-audit.cjs` 施加于本次证据目录内的新副本；该辅助脚本验证绝对路径范围后才修改副本。`before.json` / `after.json` 保存完整设置与行数，三个修复后负向场景均无设置差异，报告 `results` 为空、`status` 为 `failed`，未进入保存场景。
- 合法写入与重开的命令仍是 `rssr_settings_save_acceptance.cjs --mode write|reopen --cdp <endpoint> --database <isolated-db> --artifact-dir <path> --expect <write/acceptance.json>`。Web 使用 `NODE_PATH=C:/Users/QQ/node_modules` 和 `PUBLIC_DIR=target/settings-save-20261028/candidate/public`。
- 本次复用 `dda9aefa` 已验证的生产包，重新核对源码及产物身份：EXE SHA-256 `957F53A0C4774C0A0161567B0440716328EABF6E4328CCF7397EADFBD2AFE83C`；Web WASM SHA-256 `5bf3880a1f2a24aeb07ad2b1aa74ffbbb5df9e27483a282dcfa263f5cb336171`。本批只改测试脚本/文档，没有重建生产包。
- Web 1536×844 与 390×844 截图均与上批完全相同，SHA-256 分别为 `b1736bc7a047118ff1d39f2d5f2d95335ae6870c4426fe63b1abc166b2ca23d8`、`840392c125bbdcc5f1ee588a802b5ab9564103102ee6a62e44277f0f9c87512d`。

### 手工与未执行覆盖

- 本轮是自动化真实浏览器/WebView2 操作，不称人工点击验收；隐藏 Windows 截图不作为视觉证据。
- Rust 生产文件完全未改，本轮未重复全工作区测试、Clippy、WASM/Android 编译；上批对应结果保留在原交接，不当成本轮重跑结果。
- 未验证 Android 实机、Linux/macOS 原生 UI、托管服务端模式、真实 WebDAV 和系统文件对话框；这些边界与上批相同。

## 结果

- 确认的 P2 问题已修复，负向复现与正常回归均完成；本批可本地提交。
- Rust 提取本身未发现新的可复现问题，本轮不改变生产行为或扩大下一批范围。

## 风险与后续事项

- 新检查是针对固定阅读 fixture 的误接防护，不是通用数据身份或安全认证。仍应由调用方创建、管理独立原生进程及测试数据库，不能拿正常用户会话运行此写入验收。
- 后续更换 fixture 时，需同步调整明确的 ID/标题契约及前置负向检查，不能删除前置检查或退回仅校验文章数量。

## 给下一位 Agent 的备注

- 入口是 `checkNativeFixture` 及最外层验收状态；核对前置调用必须一直早于 `seed()`。
- 初始 dirty / untracked 内容与上一批相同，保存在 `initial-status.txt`；仅提交本批脚本和三份文档改动，保留任务外文件。
