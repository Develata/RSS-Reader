# 2026-04-08 交接汇总

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：c6f7271（汇总覆盖到当日记录对应状态）
- 相关 commit：dc6d9b6, 5069bdd, 7f648bc, 61ad9cf, 821b5e0, 52db606, b836619, 6078c3c, 0a7ff54, 05a1632, 8972474, 9a6e5a9, e301f63, 4796c85, 8d677c3, c6f7271, 4171a34
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

本日是当前主线最密集的一轮重构日，主目标包括三条线：吸收 `audit/refactor` 分支的共享 use case 方向、完成 Web 行为层向 `rssr-application` / `rssr-infra` 的收束、把文章页/阅读页/设置页逐步推进到局部 workspace 或 session 结构。

## 当日提交时间线

- `dc6d9b6` `refactor: adopt architecture review and centralize app use cases`
- `5069bdd` `refactor: route web subscription refresh through shared use cases`
- `7f648bc` `refactor: route web config exchange through shared use cases`
- `61ad9cf` `fix: stop web entries page reload loop`
- `821b5e0` `refactor: move web browser backend into infra adapters`
- `52db606` `fix: restore web export flow after use case routing`
- `b836619` `refactor: thin web command surface`
- `6078c3c` `refactor: route web queries through shared services`
- `0a7ff54` `refactor: thin entries page css surface`
- `05a1632` `refactor: add entries page command dispatch`
- `8972474` `refactor: route entries preferences through command dispatch`
- `9a6e5a9` `refactor: turn entries page into a local workspace session`
- `e301f63` `refactor: turn reader page into a local workspace session`
- `4796c85` `refactor: add an explicit reader page session`
- `8d677c3` `refactor: isolate settings sync into a local session`
- `4171a34` `refactor: isolate settings save into a local session`
- `c6f7271` `refactor: split theme support concerns`

## 影响范围

- 模块：
  - `crates/rssr-application/`
  - `crates/rssr-infra/src/application_adapters/browser/`
  - `crates/rssr-app/src/bootstrap/web.rs`
  - `crates/rssr-app/src/pages/entries_*`
  - `crates/rssr-app/src/pages/reader_*`
  - `crates/rssr-app/src/pages/settings_*`
  - `crates/rssr-app/src/pages/settings_page_themes/`
  - `assets/styles/entries.css`
  - `assets/styles/responsive.css`
  - `docs/architecture-review-2026-04.md`
- 平台：
  - Web
  - Windows
  - macOS
  - Linux
  - Android
- 额外影响：
  - docs
  - architecture
  - headless

## 关键变更

### 来源记录

- `2026-04-08-application-usecases-main-step1.md`
- `2026-04-08-wasm-web-usecases-step1.md`
- `2026-04-08-wasm-web-usecases-step2.md`
- `2026-04-08-web-regression-entries-loop-fix.md`
- `2026-04-08-wasm-web-browser-backend-externalization.md`
- `2026-04-08-web-regression-browser-backend-followup.md`
- `2026-04-08-web-command-surface-thinning.md`
- `2026-04-08-web-query-surface-thinning.md`
- `2026-04-08-entries-css-surface-thinning.md`
- `2026-04-08-entries-headless-command-surface-step1.md`
- `2026-04-08-entries-headless-command-surface-step2.md`
- `2026-04-08-entries-workspace-session-step1.md`
- `2026-04-08-reader-workspace-session-step1.md`
- `2026-04-08-reader-workspace-session-step2.md`
- `2026-04-08-settings-sync-session-step1.md`
- `2026-04-08-settings-save-session-step1.md`
- `2026-04-08-settings-theme-support-split.md`
- `2026-04-08-headless-phase1-inventory.md`
- `2026-04-08-entries-session-explicitization.md`

### 共享 use case 与架构主线

- 吸收 `audit` 的架构审查方向，并在 `rssr-application` 中正式引入共享 `RefreshService`、`SubscriptionWorkflow` 等用例层。
- `native`、`CLI`、`Web` 的刷新、订阅、配置交换逐步统一到共享服务，而不再长期留在入口层自带业务编排。
- `FeedService`、`ImportExportService` 也在这一轮成为更真实的共享支点，不再只是薄壳。
- `rssr-infra` 同期补上了 application adapter，标志着“共享 use case + adapter”开始替代“入口层自写业务流”。

### Web 行为收束

- Web 的 add/remove subscription、refresh、config exchange、query surface 都接到了共享 `FeedService`、`EntryService`、`RefreshService`、`ImportExportService`。
- browser backend 正式从 `rssr-app` 外移到 `rssr-infra/src/application_adapters/browser/`。
- `web.rs` 收敛为装配层和平台胶水层。
- 收束顺序是：
  - 先接入 shared use case 的 subscription / refresh
  - 再接 config exchange
  - 然后修复由收束带出的回归
  - 最后继续把 command/query surface 从 `web.rs` 中抽薄
- 对 browser backend 的判断也在当天固定下来：
  - browser 特化允许存在
  - 但应该属于 `infra adapter`
  - 不应继续回流到 `rssr-app` 入口层

### Web 回归修复

- 修复文章页 `use_resource` 自触发循环导致的 Web 主线程拖死问题。
- 修复 Web 在走共享 `ImportExportService` 后，导出配置 / OPML 时 wasm 时间 API panic 的问题。
- 完成 browser backend 外移后的回归，确认登录、订阅、文章列表与导出流程无行为回退。
- 这两处修复都来自真实浏览器回归，而不是纯代码推演：
  - `EntriesPage` 的 resource 循环会让 Chrome MCP 截图、快照、脚本执行全面超时
  - `OffsetDateTime::now_utc()` 在 wasm 下会直接 panic，导致导出配置 / 导出 OPML 失败

### Entries / Reader / Settings 的局部 session 化

- 文章页：
  - CSS surface 继续收薄。
  - 卡片动作与偏好保存走命令/绑定链路。
  - 逐步演化为局部 workspace/session 结构。
- 阅读页：
  - 建立局部 workspace/session 骨架。
  - 显式引入 `ReaderPageSession`，接管正文加载、已读/收藏动作与快捷键入口。
- 设置页：
  - `settings_page_sync` 收成局部 sync session。
  - `save-settings` 主链路收成独立 save session。
  - 主题实验室 support 按 apply / io / preset / validation 拆分。

### 文章页推进细节

- 文章页在当天不是“一次大重写”，而是按这条路线逐步推进：
  - 先收 CSS surface
  - 再收 card actions 的 command / dispatch
  - 再收偏好保存
  - 再引入 `state / intent / reducer / presenter / queries / effect / runtime`
  - 最后补显式 `EntriesPageSession`
- 这意味着到 04-08 结束时，文章页已经是仓库里最接近完整 headless workspace 的页面。

### 阅读页推进细节

- 阅读页先补齐 `state / intent / reducer / bindings / effect / runtime`
- 再补 `ReaderPageSession`
- 快捷键 `use_reader_shortcuts` 也被接到统一动作链，而不再直接碰局部 signal

### 设置页推进细节

- 当天的策略不是把设置页整页推成状态机，而是承认它更像“工具卡片集合”
- 因此只对最重的三块下刀：
  - WebDAV 同步
  - 保存设置
  - 主题实验室 support

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-infra`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-infra --test test_application_refresh_store_adapter`：通过

### 手工验收

- Chrome MCP：`rssr-web` 登录、进入订阅页、添加并首次刷新 feed、回到文章页：通过
- Chrome MCP：文章页“标已读/标未读”状态切换：通过
- Chrome MCP：阅读页正文加载、底部栏已读/收藏切换、快捷键：通过
- Chrome MCP：设置页 WebDAV 卡片输入绑定与确认态：通过
- 设置页保存“刷新间隔（分钟）”并显示“设置已保存。”：通过
- Chrome MCP：配置导出与 OPML 导出恢复正常：通过
- Android 目标编译：通过

## 结果

- Web 行为层与 browser backend 已基本完成共享 use case 收束。
- Entries / Reader / Settings 的 headless 化从分散闭包迁移到更稳定的局部 session 架构。
- 当日形成了当前主线最关键的一批结构性重构成果。
- `audit` / `refactor` 分支中真正高价值的结构方向，到这一天基本已经开始被当前 `main` 重新实现，而不是停留在旧分支里。

## 风险与后续事项

- 文章页和阅读页的 session 结构已经起势，但仍需继续做跨页命令面一致性整理。
- 设置页整体不值得硬推成全页状态机，应继续按卡片/能力收束。
- Web 这条线虽然已经薄化，但仍需后续契约测试与 CI runner 来闭环跨实现一致性。
- 此时 browser adapter 虽已进入 `rssr-infra`，但 browser / sqlite 之间的行为一致性还主要靠人工回归，尚未进入 contract harness 阶段。

## 给下一位 Agent 的备注

- 继续推进时，优先看 `docs/architecture-review-2026-04.md`、`docs/design/headless-active-interface.md` 以及 `crates/rssr-app/src/pages/entries_*` / `reader_*` / `settings_*`。
- 如果要理解 Web 收束边界，先看 `crates/rssr-infra/src/application_adapters/browser/` 与 `crates/rssr-app/src/bootstrap/web.rs`。
- 如果要理解当天为什么会有这么多小步提交，关键是它们分别对应：
  - 共享 use case 接线
  - 实际回归触发的修复
  - 页面 session 化
  - CSS / 命令面收口
