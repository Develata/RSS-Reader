# 主线整合后的设置会话内聚与边界整理

> 历史分支记录，2026-10-30 从 `2312dd0e` 补收归档。主线后来独立实现了 `draft_edits` / `request_lifecycle`；下文旧 `edits` / `draft_lifecycle` 和三项交错行为建议不作为现行契约，见[分支逐项复核](../../2026-10-30-branch-consolidation.md)。原始验收 JSON 保持字节不变，只证明原记录指定的版本。

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/native-main-integration`
- 初始 HEAD：`03b4dbd9ac7ebe205820ba9ca5657a6404d5a05a`
- 相关 commit：见本记录所在的 `refactor(ui): encapsulate settings edit and request ownership` 提交
- 相关 tag / release：N/A
- 状态：`validated`（本地分支与所列验收）

## 工作摘要

用户要求整合时进一步注重高内聚、低耦合。沿用[上一轮整合](./2026-10-29-native-main-integration.md)的七 crate / Backend、单库、逐字段保存与应用任务边界，只整理设置页内部所有权。PR #40 在本轮开始时为另一对话负责的 `72e0516b`，无合并冲突、未合并；本轮不改写该工作树或远端分支。

## 影响范围与关键变更

- `settings_page/edits.rs` 封装 pending / submitted 字段层，唯一负责提交、确认、失败恢复、后来编辑优先和无效数字输入的保留。字段保持私有；不新增业务状态副本。
- `settings_page/request_lifecycle.rs` 统一读取身份、保存/下载互斥、暂存读取与写入完成顺序。请求使用具名类型，绑定读取代次、页面已写入代次、应用已存设置计数，避免独立整数传错或状态更新遗漏。
- `draft_lifecycle.rs` 只接收四个数字字段的保留标志，不再访问 `SettingsPatch` 或设置值。预设与文件请求的代次职责继续独立。
- `session.rs` 保留 Dioxus 任务执行、ThemeController 与 shell 草稿发布，加载、下载和保存统一使用 `project_stored_settings`，不重复实现回填步骤。
- 新增模块维护入口 `settings_page/README.md`，更新[控件契约](../../../design/native-control-adapter-contracts.md)。数据库、HTTP、application / domain、公开接口、CSS 和渲染器均未变更。归档注：该 README 只存在于来源 `9a6700fc` 的 `crates/rssr-app/src/pages/settings_page/`，可从[恢复包](../../2026-10-30-branch-consolidation.md#恢复方法)读取，不指向当前主线的不存在文件。

## 验证与验收

证据目录：`target/settings-cohesion-20261029/`；可提交回执见 [result.json](../../../testing/evidence/2026-10-29-settings-cohesion/result.json)。

- `cargo test -p rssr-app --locked -j 2 settings_page::`：64 通过、0 失败，包含 5 项新增纯规则测试和原有 Dioxus 可控 Future 测试。
- 新增测试覆盖写失败恢复、同值后编辑、保存期间无效数字输入、迟到读取、暂存读取被新请求取代、写入成功淘汰旧读取、重复写入拒绝及原始应用版本保留。
- `cargo test --workspace --locked -j 2`：1065 通过、0 失败、4 项既有忽略项，共 63 个套件。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings`、文档相对链接和 `git diff --check`：通过。
- `cargo build --locked -j 2 -p rssr-web -p rssr-app`：通过。`dx build --platform web --package rssr-app --locked --debug-symbols false`：通过；本机 CLI 0.7.10 与锁定 Dioxus 0.7.9 的版本提示仍存在，没有改变锁文件。
- 新 Web 产物运行 `rssr_native_preparation_acceptance.cjs`：1536 / 390 两种宽度共 10 组通过，页面异常 0；预期校验 HTTP 400 与显式允许的 debug 开发工具 WebSocket 404 在 JSON 中分别记录，不能表述为全控制台零错误。
- 新 Cargo 原生产物运行 `rssr_native_transport_acceptance.cjs`：13 项通过、页面与控制台异常均为 0，覆盖当前 72 篇 / 6 订阅的单库夹具、BOM/中文、字段保存及页面重挂载。独立进程与回环服务已关闭。
- 使用现有隔离构建目录、2 个构建任务和关闭 dev/test 调试符号；本轮没有重跑已知不兼容的本机 dx desktop 路径。

## 当前状态、风险与后续

本轮为当前工作树内的行为保持整理；不能将这里的测试结果直接视为 PR #40 的 CI。代码已提交为 `9a6700fc99b1bce9fd58fbdb6ea7200120709478`。同日后续用户已确认分工并授权发送交接，提交与差异已发给“规划原生 Rust 化前的模块优化”，由其负责 PR 接入、推送与 CI；发送回执与分支复核见[整合交接](./2026-10-29-native-main-integration.md)。本对话保留本地来源，尚不代表对方已接入这些差异。

本提交依赖 `03b4dbd9` 的整合版本；交接时 PR #40 已更新为 `50af7c99`，包含 main `8932a5ac` 的主题预览和明确保存行为，以及独立的 `draft_edits` / `request_lifecycle` 封装。接入时要沿模块 README 核对字段叠加和读写时序，保留新主线行为，再复核此前修正的无效数字输入、预设选择代次与文件导入身份；不能机械选择整棵 ours/theirs 或保留两套状态所有者。

Android 设备、OS 文件选择器、真实 IME/触摸、远端 WebDAV 和 release 包均不包含在本轮验收中。继续优化应从模块 README 选择唯一所有者，避免把这些页面生命周期规则上移到 application / domain。
