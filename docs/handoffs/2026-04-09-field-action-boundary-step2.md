# 2026-04-09 Field / Action Boundary Step 2

## Summary

- 把订阅页和文章页里“持续输入值”的接口也从 `data-action` 迁到了 `data-field`。
- 这样当前前端的字段/动作边界不再只在设置页成立，而是开始跨页统一。

## Changes

- 订阅页：
  - `feed-url-input` -> `data-field`
  - `config-text` -> `data-field`
  - `opml-text` -> `data-field`
- 文章页：
  - `search-title` -> `data-field`

## Why

- 这些控件都表达持续输入值，不是一次性动作。
- 如果它们继续挂在 `data-action` 下，会让自动化、用户 CSS 和后续命令面抽象都混淆“字段”和“命令”的边界。

## Files

- `crates/rssr-app/src/components/entry_filters.rs`
- `crates/rssr-app/src/pages/feeds_page_sections/compose.rs`
- `crates/rssr-app/src/pages/feeds_page_sections/config_exchange.rs`
- `docs/design/frontend-command-reference.md`

## Verification

- 本轮只调整稳定接口，不改变输入、导入导出、搜索或订阅添加逻辑。
- 后续如果继续整理，应优先检查是否还存在把容器/字段误标成 `data-action` 的地方。
