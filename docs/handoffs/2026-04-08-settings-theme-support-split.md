# 2026-04-08 Settings Theme Support Split

## Summary
- 将设置页主题实验室原本聚合在单个 `support.rs` 中的职责拆成四块：
  - 预设
  - 校验
  - 应用
  - 导入导出
- 保持现有页面行为不变，只收束内部边界。

## Modules
- `crates/rssr-app/src/pages/settings_page_themes/mod.rs`
- `crates/rssr-app/src/pages/settings_page_themes/lab.rs`
- `crates/rssr-app/src/pages/settings_page_themes/presets.rs`
- `crates/rssr-app/src/pages/settings_page_themes/theme_preset.rs`
- `crates/rssr-app/src/pages/settings_page_themes/theme_validation.rs`
- `crates/rssr-app/src/pages/settings_page_themes/theme_apply.rs`
- `crates/rssr-app/src/pages/settings_page_themes/theme_io.rs`

## What Changed
- 删除旧的 `settings_page_themes/support.rs`。
- `theme_preset.rs` 负责：
  - 主题 CSS 常量
  - `preset_css`
  - `preset_display_name`
  - `detect_preset_key`
  - 内置主题预设列表
- `theme_validation.rs` 负责：
  - `validate_custom_css`
- `theme_apply.rs` 负责：
  - `apply_builtin_theme`
  - `apply_settings_immediately`
  - `apply_custom_css_from_raw`
- `theme_io.rs` 负责：
  - CSS 文件导入
  - CSS 文件导出
  - wasm / native 文件交互分支
- `lab.rs` 与 `presets.rs` 现在只依赖自己需要的子模块，不再依赖一个大而杂的 support 文件。

## Acceptance
- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `git diff --check`

## Notes
- 这轮仍然没有把主题实验室做成完整 session；它目前更适合作为“按职责拆开的工具集合”。
- 如果后续继续推进设置页结构化，下一步更自然的是评估“保存设置”这条总提交流是否要独立成 theme save/apply session，而不是继续把大量逻辑堆回页面组件。
