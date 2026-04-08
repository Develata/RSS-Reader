# 2026-04-08 Settings Save Session Step 1

## Summary
- 将设置页“保存设置”主链路从主题区组件中抬出，收成独立的局部 save session。
- `AppearanceSettingsCard` 现在负责承载保存动作；主题区只负责编辑与局部主题工具。
- 保存设置的校验、状态提示、成功同步与失败回滚边界更明确。

## Modules
- `crates/rssr-app/src/pages/settings_page_appearance.rs`
- `crates/rssr-app/src/pages/settings_page_save_state.rs`
- `crates/rssr-app/src/pages/settings_page_save_effect.rs`
- `crates/rssr-app/src/pages/settings_page_save_runtime.rs`
- `crates/rssr-app/src/pages/settings_page_save_session.rs`
- `crates/rssr-app/src/pages/settings_page_themes/mod.rs`
- `crates/rssr-app/src/pages/settings_page_themes/theme_validation.rs`
- `crates/rssr-app/src/pages.rs`

## What Changed
- `ThemeSettingsSections` 不再直接持有“保存设置”按钮和保存逻辑。
- `AppearanceSettingsCard` 新增 `SettingsPageSaveSession`，统一驱动：
  - 自定义 CSS 校验
  - `save_settings`
  - 成功后 theme settings 同步
  - 失败时 draft / preset 回滚
  - 保存状态提示
- 新增 `SettingsPageSaveState`，当前用于承载 `pending_save`。
- “保存设置”现在明确属于阅读外观卡片级动作，不再埋在主题区内部。

## Acceptance
- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `git diff --check`
- Chrome MCP 最小回归：
  - 设置页打开正常
  - 修改“刷新间隔（分钟）”后点击“保存设置”成功
  - 状态提示显示“设置已保存。”
  - console 无新增 `error/warn`

## Notes
- 到这一步，设置页最重的两条行为链已经分别被收束：
  - WebDAV 同步：局部 sync session
  - 保存设置：局部 save session
- 如果继续推进 headless active interface，下一步更自然的是做一次跨页命令面/会话面盘点，而不是继续把设置页强行推进成全页状态机。
