# 2026-04-09 Settings Field / Action Boundary

## Summary

- 设置页把字段型接口从 `data-action` 迁到了 `data-field`。
- 真正触发副作用的按钮仍保留 `data-action`。
- 命令面与输入面之间的边界更清楚了。

## Why

- `theme-mode`、`refresh-interval`、`webdav-endpoint` 这类接口表达的是“字段值”，不是一次性命令。
- 把它们放在 `data-action` 下，会把“输入控件”和“动作语义”混在一起。
- `headless active interface` 需要：
  - 动作接口先于视图
  - 字段接口不伪装成命令

## Changes

- 下列控件改为 `data-field`：
  - 主题
  - 列表密度
  - 启动视图
  - 刷新间隔
  - 自动归档阈值
  - 阅读字号缩放
  - 自定义 CSS
  - 预设主题选择
  - WebDAV endpoint
  - WebDAV remote path

- 下列控件继续保持 `data-action`：
  - `save-settings`
  - `push-webdav`
  - `pull-webdav`
  - `apply-custom-css`
  - `export-custom-css-file`
  - `import-custom-css-file`
  - `apply-selected-theme`
  - `apply-theme-preset`
  - `remove-theme-preset`
  - `clear-custom-css`

## Files

- `crates/rssr-app/src/pages/settings_page_preferences.rs`
- `crates/rssr-app/src/pages/settings_page_sync.rs`
- `crates/rssr-app/src/pages/settings_page_themes/lab.rs`
- `crates/rssr-app/src/pages/settings_page_themes/presets.rs`
- `docs/design/frontend-command-reference.md`

## Verification

- 本轮只调整稳定接口命名，不改变设置页保存、同步和主题操作逻辑。
- 后续跨页整理时，可以继续用同样标准检查：
  - 搜索框
  - 导入导出文本框
  - 其它“持续输入值”控件
