# Entries 写入规则提取复核与验收修正

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前 HEAD / 审查目标：`f76b15c490d4aec0be21d90db90279ddfe514819`
- 比较基线：`6109380b1563ddb1a8812a654980729a9eb9ac3b`
- 相关 commit：修复与本文同批本地提交，以 `git log -1 -- docs/handoffs/2026-10-28-entries-write-review.md` 定位；记录生成时 `commit: pending`
- 相关 tag / release：N/A
- 状态：`validated`（以下明确列出的范围）

## 工作摘要

复核第二批 Entries 写入及偏好规则提取，检查生产调用链、状态所有权、结果顺序、任务取消、测试与验收声明。未发现此次 Rust 提取引入行为差异；发现两处新增验收脚本问题，分别用延迟查询和隔离数据库错误注入复现后修复。

## 影响范围

- 审查：`f76b15c4` 的全部 10 个文件，以及相关 reducer、查询 effect、UI runtime 和 application 偏好串行保存边界。
- 修改：`scripts/browser/rssr_workspace_acceptance.cjs`、`scripts/browser/rssr_entries_writes_native.cjs`、本记录及原交接的复核链接。
- 平台：静态 Web 的真实 WASM 页面、Windows WebView2/SQLite 自动化验收。
- 产品 Rust、依赖、布局、命令、存储格式和平台行为均未修改。

## 关键变更

### P2：批量重试验收提前检查尚未返回的查询

- 原 `bulkFailureRetry` 仅等待勾选清空，就立即断言 2 篇已读。`SelectionApplied` 先清空勾选并增加 reload revision，列表经独立异步查询更新，两者不是同一完成信号。
- 在隔离浏览器中，让批量写入提交后的下一次存储读取延迟 1200 ms，旧脚本稳定报 `0 !== 2`，当时勾选已经清空，页面无未捕获异常。该失败是验收竞态，不是产品写入失败。
- 修复后等待目标文章的已读状态到达，再比对确切 ID；不靠固定 sleep 等待完成。同时比对整个 72 篇 fixture 的已读、收藏、归档标记：失败不改变持久化数据，重试只改变指定文章的指定字段，重开后保持一致。

### P2：Windows 抽查不能证明只修改所选文章

- 原脚本只检查两个目标 ID 和一个未选中 ID，重开时也未比对第二个目标的收藏状态。其他文章被误改仍可报通过。
- 在独立复制的测试库中，write 验收结束并关闭进程后，仅将未选中的文章 ID 1 从未读改为已读；旧脚本重新启动后仍返回 9 项通过，复现了漏报。
- 修复后，从操作前建立的完整快照出发，每个单条/批量操作只在预期快照上应用目标字段变化，再通过只读 SQL 按 ID 比对全部 5000 篇的三个标记。完整预期快照写入 `expected.json`，进程重启后再次完整比对，能同时识别误改、漏写、增删文章。
- 新脚本会拒绝上述错误注入，报错位置为 `SQLite retains exact flags for every entry`；正常流程仍为 11 项写入 + 9 项重开检查通过。旧格式的 `expected.json` 没有完整快照，需重新运行 write 阶段生成，不应沿用旧回执执行新 reopen。

## 验证与验收

证据目录为 `target/entries-write-review-20261028/`（ignored）。故障注入只作用于新建的隔离浏览器或复制测试库；生产文件、个人数据和上一批验收产物未改动。

### 自动化验证

- `cargo test -p rssr-app --locked`：191 个单元测试 + 4 个集成测试通过，0 失败，1 个原有性能测试 ignored；日志 `app-test.log`。
- `node --check scripts/browser/rssr_workspace_acceptance.cjs` 与 `node --check scripts/browser/rssr_entries_writes_native.cjs`：通过。
- 延迟查询复现：`node target/entries-write-review-20261028/slow-query-repro.cjs`，使用 `NODE_PATH=C:/Users/QQ/node_modules`、`PUBLIC_DIR=target/entries-writes-20261028/candidate/public`。`REVIEW_WORKSPACE` 分别选择保存的 `workspace-before.cjs` 与当前工作区脚本，`REVIEW_REPORT` 分别输出 `slow-query-before.json` / `slow-query-after.json`。旧版失败、新版通过，两者均无 page error。
- 完整 Web 场景：同上 `NODE_PATH` / `PUBLIC_DIR`，设置 `ARTIFACT_DIR=target/entries-write-review-20261028/browser-candidate`，运行 `node scripts/browser/rssr_entries_lifecycle_acceptance.cjs`：12 组通过，page/console errors 均为空；日志 `browser-candidate.log`。
- Windows 正常流程：`target/entries-write-review-20261028/run-native-review.ps1 -Variant after-normal -Script scripts/browser/rssr_entries_writes_native.cjs`：11 + 9 项通过，无未捕获页面错误。
- Windows 漏报复现：相同 helper，以 `-Variant before-corrupt -Script target/entries-write-review-20261028/native-before.cjs -CorruptOtherRow` 运行，注入后的错误数据仍被旧脚本接受；原始注入记录 `native-before-corrupt/injected-corruption.log`，旧 reopen 回执为 pass。
- Windows 修复负向对照：`-Variant after-corrupt -Script scripts/browser/rssr_entries_writes_native.cjs -CorruptOtherRow`；write 阶段通过，reopen 按预期拒绝错误数据。外层检查确认其失败原因确实是全量标记不一致，负向对照通过；这不是产品正常流程失败。
- helper 为每次运行复制 fixture、独立 profile 和 EXE；隐藏启动并只关闭可执行路径匹配的本轮进程，结束后确认 CDP 端口 9276 释放。重跑需选择新的 variant，不能覆盖已有证据。
- 本批精确路径的 `git diff --check`：通过。

### 产物身份与未覆盖范围

- 四份生产源码 SHA256 与 `target/entries-writes-20261028/candidate/source.json` 一致，当前 `crates/` 无未提交差异；未为脚本修复重编产品。
- 已复核沿用 Web 包的 index、实际引用的 JS/WASM 哈希与原回执一致；WASM SHA256 为 `2b8e5416b386c8b1b78db888e29d57fe945d541c69da1726c40483e026010474`。
- 所有 Windows 复核使用相同候选 EXE，SHA256 为 `ced555f575e192c0e7eb3d7211ead1351e55f8b1cb5741047f2414e4376d1b8c`。
- 本轮未重跑全工作区、Clippy、各平台编译和目录专项；此前结果属于[原实现验收](./2026-10-28-entries-write-lifecycle.md)，不能写成本轮新执行结果。生产代码未变化，本轮验证聚焦审查和脚本修正。
- 未做人工桌面操作、Android 设备、Linux/macOS、真实 IME、服务端 UI 或性能验收。完整标记比对只覆盖 ID、已读、收藏、归档，不代表对正文、订阅等其他数据表做了全量比对。

## 结果

- 两项已复现的验收缺陷已修复；未发现本次 Rust 规则提取的新增生产问题。
- 正常流程与针对性负向对照均完成。仅本地提交，不 push、打 tag 或发布。
- 原有 Speckit 删除、旧交接修改及 Penpot/Figma 等未跟踪内容保持原样，排除在本批提交之外。

## 风险与后续事项

- 后续重构继续使用最终查询结果和已提交数据作为验收条件；勾选清空、按钮恢复或临时无锁不等同于所有后续读取已完成。
- Windows 全量标记快照为测试数据，额外读取成本仅存在于验收脚本，不进入产品执行路径。

## 给下一位 Agent 的备注

- 先读本记录的两项复现，再看 `bulkFailureRetry` 与原生脚本的 `checkFlags` / reopen 比对。
- 本轮改进的是验收可信度，不是发现并修复了产品误写数据。错误数据由负向测试主动注入，勿将其转述为真实用户数据损坏。
