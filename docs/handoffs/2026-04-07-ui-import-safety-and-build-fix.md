# 配置导入防误触与 wasm 构建脚本修复

- 日期：2026-04-07
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：b49b6ad
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

在第二轮仓库级 bug 排查中，修复了两类真实问题：一是 GUI 中覆盖式配置导入缺少显式确认，容易误删本地订阅与文章；二是 `rssr-app` 的 `build.rs` 按宿主机而不是按目标平台判断 Windows 资源嵌入，导致 `wasm32-unknown-unknown` 检查失败。

## 影响范围

- 模块：
  - `crates/rssr-app/build.rs`
  - `crates/rssr-app/src/pages/feeds_page.rs`
  - `crates/rssr-app/src/pages/settings_page_sync.rs`
  - `crates/rssr-app/src/pages/feeds_page_sections/`
- 平台：
  - Windows
  - Web
  - macOS
  - Linux
  - Android
- 额外影响：
  - docs
  - handoff

## 关键变更

### 覆盖式配置导入确认

- 订阅页“导入配置”现在改为二次确认流，第一次点击只提示风险，第二次才真正执行覆盖式导入。
- 设置页“从 WebDAV 下载配置”同样增加二次确认，避免误把远端旧配置覆盖到当前本地库。
- 风险提示文案明确指出：缺失订阅会被移除，对应本地文章也会被清理。

### 页面模块拆分

- 将原 `feeds_page_sections.rs`（366 行）拆为 `feeds_page_sections/` 子模块目录。
- 拆分后分别承载新增订阅、配置交换、已保存订阅和辅助格式化逻辑，降低页面层继续堆积复杂闭包的风险。

### wasm 构建脚本修复

- `rssr-app/build.rs` 改为读取 `CARGO_CFG_TARGET_OS` / `CARGO_CFG_TARGET_ENV` 判断目标平台。
- 现在只有目标平台确实是 `windows-gnu` 或 `windows-msvc` 时才尝试嵌入图标资源，不再误伤 wasm target。

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo test --workspace`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过

### 手工验收

- 代码级复核“导入配置 / WebDAV 下载”入口确认逻辑：通过
- 代码级复核 `build.rs` 目标平台判定：通过
- 浏览器 / GUI 实机点击验证：未执行

## 结果

- 本次交付可继续合并。
- `rssr-app` 的 Web target 检查链路已恢复，覆盖式配置导入的误操作风险显著下降。

## 风险与后续事项

- CLI `refresh` 子命令当前在不传 `--all` 与 `--feed-id` 时也会刷新全部，但帮助文本会让人误以为需要显式选择；这是行为与认知之间的残余歧义，后续可再统一。
- GUI 这次只补了显式确认，没有新增“预览将移除哪些订阅”的细粒度确认界面；若后续继续降低误操作成本，可再加导入 diff 预览。

## 给下一位 Agent 的备注

- 如果继续看配置交换风险，先读 `crates/rssr-app/src/pages/feeds_page_sections/config_exchange.rs` 和 `crates/rssr-app/src/pages/settings_page_sync.rs`。
- 如果继续看多目标构建链，先读 `crates/rssr-app/build.rs`，再跑 `cargo check -p rssr-app --target wasm32-unknown-unknown` 验证没有回归。
