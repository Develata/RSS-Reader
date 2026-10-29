# 原生控件契约与验收清单 Review 修正

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 审查提交 / 开始时 HEAD：`1d27eae408775f3f56b23cc5f059c5e67ce57f23`
- 相关 commit：本文与修正同批本地提交，生成时 `commit: pending`；提交后用 `git log -1 -- docs/handoffs/2026-10-29-native-control-review.md` 定位，最终身份写入本地 `final-receipt.json`。
- 相关 tag / release：无；不 push、不打 tag、不发布
- 状态：`validated`（源码对照与定向测试；不代表系统 GUI 验收）

## 工作摘要

审查上一批六个文档的改动，发现并修正两项 P2 问题：请求身份过滤被写成了所有操作的通则；Windows 文件验收遗漏了旧 fixture 的合法化准备。此次只改契约、验收清单与本交接，不改变运行行为或扩大原生迁移范围。

## 影响范围

- [原生控件适配契约](../design/native-control-adapter-contracts.md)：收窄请求身份检查的适用范围。
- [原生控件验收清单](../testing/native-control-acceptance.md)：补上传/导出提示场景及 Windows fixture 准备顺序。
- 平台：共享页面结果语义、Windows 测试准备；Android 导出原有单在途限制仍保留。
- 未修改生产 Rust、JavaScript、Kotlin、依赖、CSS 或构建配置；历史交接及原审计回执保持原样。

## 关键变更

### P2：不能把上传和导出套入请求代次过滤

- 原契约笼统要求所有完成提示必须通过身份检查，但 `SettingsPageSyncSession::push` 只投影状态，`export_css_file_with` 直接在页面任务完成时发布结果，没有页面结果代次过滤。
- 同页继续编辑后，较早上传/导出的完成提示仍可能覆盖当前提示；导出使用点击时 CSS 快照且不保存设置。将它们改成“只接纳最新请求”会引入本轮未授权的行为变化。
- 修正页面装配与取消表述，增加上传/导出结果行及 `STATUS-01`，与导入的身份过滤、保存/下载 gate 明确区分。Android 系统导出仍有自身的单在途限制，页面卸载仍取消等待。
- 依据：[sync/session.rs](../../crates/rssr-app/src/pages/settings_page/sync/session.rs)、[theme_io.rs](../../crates/rssr-app/src/pages/settings_page/themes/theme_io.rs)。导出后编辑仍发布完成提示由现有 Dioxus 测试覆盖；上传完成交错本次为源码核对，未增加宿主故障注入。

### P2：旧 fixture 会令有效 CSS 导入因设置校验失败

- 只读检查本机历史 fixture：5000 篇固定 ID/标题、6 个订阅，`archive_after_months = 0`；检查前后数据库 SHA-256 相同。
- CSS 导入经 `apply_custom_css_from_raw` 保存整份草稿；`SettingsService::save` 调用 domain 校验，归档阈值 0 被拒绝。因此原清单直接执行 W-FILE-01，不能得到它要求的成功持久化结果。
- 补入与既有 Settings 运行器一致的顺序：先确认 EXE 同目录的复制数据库和独立 profile，启动前确认 fixture 身份及本次独占的回环来源，再核对实际 Entries 页面；仅对旧样本中的 0 在隔离 UI 设置为 120 并保存、只读确认，最后记录文件场景的基线。准备差异单列，不能放宽生产校验或改源样本。
- 明确 Windows 数据目录取自 EXE 位置；只改工作目录或 profile 不会隔离数据库。依据：[sqlite_native.rs](../../crates/rssr-infra/src/db/sqlite_native.rs)、[Settings 验收准备](../../scripts/browser/rssr_settings_save_acceptance.cjs)、[theme_apply.rs](../../crates/rssr-app/src/pages/settings_page/themes/theme_apply.rs)、[validation.rs](../../crates/rssr-domain/src/validation.rs)。

## 验证与验收

本地证据：`target/native-control-review-20261029/`（ignored）。

| 命令 / 检查 | 结果与范围 | 证据 |
| --- | --- | --- |
| `cargo test -p rssr-app --locked pages::settings_page::themes::theme_io::tests:: -- --nocapture` | 6 通过；导入身份、导出快照/完成提示、空导出、页面销毁取消等待 | `theme-io-tests.log` |
| `cargo test -p rssr-domain --locked validation::tests::rejects_out_of_range_settings -- --exact` | 1 通过，包含归档阈值 0 的拒绝 | `settings-validation-test.log` |
| Node `DatabaseSync` 以 `readOnly: true` 检查历史 fixture | 身份符合、归档阈值为 0；源数据库字节未变；未启动应用、未写用户库 | `fixture-inspection.json` |
| `node target/native-control-review-20261029/audit.cjs` | 通过：6 项检查、51 处有效本地链接、23 个引用源码文件的身份、18 个唯一场景 ID；确认生产源码无变更及测试/fixture 回执一致 | `audit.json` |
| `git diff --check` 与提交路径检查 | 仅三个本批文档；任务外状态对照最终回执 | `final-receipt.json` |

- 未重跑全工作区测试、构建、Clippy、Web、Windows OS 选择器或 Android 设备：变更只在文档，定向测试用于核实两项修正的既有行为。
- 上一批记录的 571 个工作区测试和 36 个 Web 场景未重新执行，不计入本次测试数。W-FILE 仍是待执行清单。

## 结果

- 两项审查问题均已修正文档；当前共 18 组适配场景，其中 Windows 文件场景仍为 7 组。
- 生产行为未变，本批可按文档修正范围本地提交；不构成原生 GUI 通过或发布声明。

## 风险与后续事项

- 下一批继续执行 W-FILE 系统 GUI 场景；遇到旧 fixture 先按修正后的准备顺序建立有效基线。
- 上传/导出没有页面结果代次过滤是当前事实，不等于未来必须保持这种体验；若要改变，应另列行为变更与测试，不能混入适配等价工作。
- 原生输入法、系统窗口取消与离页、Android 导出回调仍须实际宿主证据。

## 给下一位 Agent 的备注

- 不运行旧的本地 audit 脚本去覆盖上一批回执；本批另用 `target/native-control-review-20261029/`。
- 仅暂存两个修正文档与本交接；既有 `.agents/skills` / `.specify` 删除、两份 10/03 handoff 修改及其他未跟踪内容保留。
