# 2026-04-09 Command Surface Naming Alignment Step 1

## Summary

- 统一了主题预设区的 `data-action` 命名。
- 移除了把具体预设名和 DOM 位置写进 `data-action` 的旧写法。
- 改为“统一动作 + `data-theme-preset` 载荷”的形式。

## Why

- 旧命名里混有：
  - `apply-theme-atlas-sidebar`
  - `apply-theme-newsprint`
  - `apply-theme-card`
  - `remove-theme-card`
- 这些名称把具体预设名或视图位置编码进了动作语义，不符合 `headless active interface` 里“动作先于视图”的要求。

## Changes

- 主题快捷预设按钮统一为：
  - `data-action="apply-theme-preset"`
  - `data-theme-preset="<preset-key>"`
- 主题画廊卡片统一为：
  - `data-action="theme-preset-card"`
  - `data-theme-preset="<preset-key>"`
- 主题画廊容器统一为：
  - `data-action="theme-preset-gallery"`
- 主题移除动作统一为：
  - `data-action="remove-theme-preset"`
  - `data-theme-preset="<preset-key>"`

## Files

- `crates/rssr-app/src/pages/settings_page_themes/presets.rs`
- `docs/design/frontend-command-reference.md`

## Verification

- 本轮为命名与稳定接口整理，没有改变动作实现。
- 后续应在下一轮跨页命令面整理中，继续检查：
  - 设置页字段型 `data-action`
  - 导航型 `data-nav`
  - 页面局部 session 命名是否需要进一步统一
