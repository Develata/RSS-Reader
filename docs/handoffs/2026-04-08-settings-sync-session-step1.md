# 2026-04-08 Settings Sync Session Step 1

## Summary
- 将设置页的 WebDAV 配置交换卡片收成局部 sync session。
- 把 `endpoint` / `remote_path` 从父级 `SettingsPage` 移回卡片内部状态。
- 为同步卡片补齐独立 `state`、`effect`、`runtime`、`session`，避免父页面继续承担这段副作用流。

## Modules
- `crates/rssr-app/src/pages/settings_page.rs`
- `crates/rssr-app/src/pages/settings_page_sync.rs`
- `crates/rssr-app/src/pages/settings_page_sync_state.rs`
- `crates/rssr-app/src/pages/settings_page_sync_effect.rs`
- `crates/rssr-app/src/pages/settings_page_sync_runtime.rs`
- `crates/rssr-app/src/pages/settings_page_sync_session.rs`
- `crates/rssr-app/src/pages.rs`

## What Changed
- `SettingsPage` 删除仅服务于 WebDAV 卡片的 `endpoint` 与 `remote_path` signals。
- `WebDavSettingsCard` 现在通过 `SettingsPageSyncSession` 读取快照、更新输入框、触发 push/pull。
- 首次下载仍保留“确认覆盖”保护，但确认态现在属于卡片局部 state，不再散在组件闭包里。
- WebDAV push/pull 的共享 service 调用收进 `execute_settings_page_sync_effect(...)`。
- pull 成功后的 draft/theme/preset 同步逻辑也集中在 session 的 outcome 应用阶段。

## Acceptance
- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `git diff --check`
- Chrome MCP 最小回归：
  - 设置页正常打开
  - WebDAV Endpoint / Remote Path 输入绑定正常
  - 首次点击“下载配置”后进入确认态
  - 状态提示正常显示
  - console 无新增 `error/warn`

## Notes
- 这轮只收了 WebDAV 配置交换卡片，不涉及整个设置页的总 session。
- 继续往下做时，更自然的下一步是评估主题实验室 support 是否要再拆成更清晰的 theme I/O / apply/save 边界，而不是给整个设置页强行套一个大状态机。
