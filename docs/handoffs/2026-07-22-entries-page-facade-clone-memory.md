# 文章列表页 facade 逐卡片深拷贝导致的内存放大修复

- 日期：2026-07-22
- 作者 / Agent：Claude Code (math-architect)
- 分支：main
- 当前 HEAD：9faacc1
- 相关 commit：a5f8753（fix，内存修复）、9faacc1（chore，clippy 1.97 清理）
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

修复 Windows 桌面端「文章」页内存超过 100 MB 的根因：每张文章卡片的 onclick 闭包各持有一份
`EntriesPageFacade` 深拷贝（含全量 `Vec<EntrySummary>` 与全部 presenter 分组数据），默认每页
100 张卡片 × 2 个闭包 ≈ 200 份全量副本常驻。进入阅读页组件卸载后内存回落至 ~20 MB，与用户
观察一致。同时清理 clippy 1.97 新增 lint 导致的全仓 `-D warnings` 失败。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/entries_page/`（cards / facade / session / mod / groups / controls）
  - `crates/rssr-app/src/pages/reader_page/support.rs`（仅 lint 修复）
  - `crates/rssr-app/src/main.rs`（仅 lint 修复）
  - `crates/rssr-application/src/{settings_service.rs, import_export_service/rules.rs}`（仅测试代码 lint 修复）
  - `crates/rssr-infra/src/fetch/client/image_html.rs`（仅 lint 修复）
- 平台：
  - Windows / macOS / Linux / Android / Web（页面层共用路径，三端同受益）
- 额外影响：
  - N/A（无行为语义变化，无迁移）

## 关键变更

### 内存修复（entries_page）

- `cards.rs`：`render_entry_card` 改为接收 `EntriesPageSession`（`Copy` 的 signal 句柄）
  而非 `EntriesPageFacade`，两个 onclick 闭包直接调用 `session.toggle_read` /
  `session.toggle_starred`。每卡片捕获成本从 ~2 份全量深拷贝降到 ~16 字节。
- `facade.rs`：`snapshot`、`presenter` 字段改为 `Arc<_>` 持有，facade 克隆保持 O(1)，
  防止未来调用点（如 `render_entry_pagination_controls` 里的 `facade.clone()`）再次放大；
  `EntriesPageFacade::new` 直接用传入 snapshot 构建 presenter，消除原先经
  `session.presenter()` 二次读取 signal 造成的一次额外全量深拷贝。
- `session.rs`：删除因此不再被引用的 `presenter()` 方法。
- `mod.rs`：workspace 装配改为 `Arc::new(session.snapshot())` 后共享，卡片渲染处传
  `session`；facade 上已无逐卡片 `clone()`。
- `facade.rs` 删除不再有调用方的 `toggle_read` / `toggle_starred` 转发方法（dead_code）。

### clippy 1.97 漂移清理（与内存修复无关，为恢复 `-D warnings` 基线）

- `collapsible_if`（let-chain 折叠）：`image_html.rs` ×3、`main.rs` ×1。
- `question_mark`：`image_html.rs`、`reader_page/support.rs` 的 `decode_numeric_html_entity`。
- `field_reassign_with_default`：application 层两个测试文件改为结构体更新语法。
- `redundant_closure` / `type_complexity`（新增 `MonthKeyedEntries` 别名）/
  `items_after_test_module`：`entries_page/controls.rs`、`groups.rs`。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过（注意：本机 PostToolUse rustfmt 钩子的 style edition 与仓库
  不一致，编辑后需以 `cargo fmt --all` 收尾）
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- `cargo test --workspace`：通过（0 失败；首轮曾因 link.exe LNK1102 内存不足留下损坏产物，
  `cargo clean -p` 目标 crate 后以 `-j 4` 重跑通过）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：未执行（无移动端交互语义变化）

### 手工验收

- Windows 桌面端 ~2000 条目下「文章」页前后 working set 对比：未执行（需用户实测确认
  100 MB → 数十 MB 量级回落；测的是 `rssr-app.exe` 进程本身，WebView2 子进程另计）

## 结果

- 已按 fix + chore 两个 commit 提交至 main（a5f8753、9faacc1），未推送。
- 用户可见影响：文章列表页常驻内存显著下降；点击卡片按钮行为不变。

## 风险与后续事项

- presenter 每次渲染仍对全部可见条目做 `Arc::new(entry.clone())`（O(N) 瞬时分配）。
  按评审结论保留为二阶段优化：需先实测，再考虑让页面状态直接持有 `Arc<EntrySummary>`
  或把全局目录投影与当前页实体投影解耦（目录语义依赖全量集合，不能只裁剪到当前页）。
- `decode_numeric_html_entity` 在 `rssr-infra/image_html.rs` 与
  `rssr-app/reader_page/support.rs` 存在重复实现，可考虑收敛到一处。
- 本机 rustfmt 钩子 style edition 与仓库 `rustfmt.toml`（edition 2024 排序）不一致，
  每次编辑 .rs 后钩子会产生反向 import 排序，需 `cargo fmt --all` 兜底。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-app/src/pages/entries_page/mod.rs`（装配）、`facade.rs`（Arc 持有约束）、
  `cards.rs`（卡片只依赖 session）。
- 改 `entries_page/` 前先读 `crates/rssr-app/src/pages/AGENTS.md`；涉及排序/筛选还要跑
  `cargo test -p rssr-infra --test test_entry_state_and_search`。
