# 2026-04-09 Command Surface Cleanup Step 2

## Summary

- 收掉了主题预设区中仍然用于“容器/展示位”的 `data-action`。
- 把阅读页“上一未读 / 下一未读”导航标记改成了与按钮语义一致的名称。
- `data-nav` 与 `data-action` 的边界更清楚了。

## Changes

### Removed container-style `data-action`

- 主题画廊容器不再使用 `data-action`
- 主题卡片壳不再使用 `data-action`

保留的稳定语义改为：

- `class="theme-gallery"`
- `class="theme-card"`
- `data-theme-preset="<preset-key>"`

### Tightened navigation naming

- `data-nav="previous-entry"` -> `data-nav="previous-unread-entry"`
- `data-nav="next-entry"` -> `data-nav="next-unread-entry"`

这样命名和页面按钮文案一致，不再把“上一篇/下一篇”的真实导航语义说模糊。

## Files

- `crates/rssr-app/src/pages/settings_page_themes/presets.rs`
- `crates/rssr-app/src/pages/reader_page.rs`
- `docs/design/frontend-command-reference.md`

## Verification

- 本轮只调整稳定接口命名，不改变主题应用或阅读页导航逻辑。
- 后续如继续清理，应优先检查是否还有：
  - 用 `data-action` 标容器
  - 用 `data-nav` 表达非导航副作用
