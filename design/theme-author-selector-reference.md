# 主题作者 Selector 参考

这份文档面向自定义主题作者，汇总当前 UI 中可依赖的稳定样式接口。

目标：

- 让主题作者优先依赖稳定 hook，而不是内部 DOM 层级
- 降低页面重构时对主题兼容性的破坏
- 提供一份“可直接开写”的 selector 速查表

## 使用约定

- 优先使用 `data-page`、`data-action`、`data-nav`
- 其次使用稳定组件 class
- 尽量避免依赖深层后代选择器，如 `.page > div > div > button`
- 如果只想改某个页面，先用 `data-page` 限定作用域
- 如果想从现成主题开始，优先参考：
  - `assets/themes/newsprint.css`
  - `assets/themes/forest-desk.css`
  - `assets/themes/midnight-ledger.css`

推荐写法：

```css
[data-page="feeds"] [data-action="remove-feed"] {
  order: 10;
}
```

不推荐写法：

```css
.page > div:nth-child(4) > ul > li > div > button:last-child {
  order: 10;
}
```

## 页面级 hook

- `[data-page="home"]`
- `[data-page="feeds"]`
- `[data-page="entries"]`
- `[data-page="reader"]`
- `[data-page="settings"]`

建议：

- 所有页面定制都先从页面级 hook 开始
- 如果主题需要完全不同的布局，这一层是最安全的作用域入口

## 导航 hook

- `[data-nav="home"]`
- `[data-nav="feeds"]`
- `[data-nav="entries"]`
- `[data-nav="settings"]`
- `[data-nav="feed-entries"]`

用途：

- 改导航按钮顺序
- 隐藏某个导航入口
- 给不同导航入口设置不同视觉层级

示例：

```css
.app-nav [data-nav="settings"] {
  margin-left: auto;
}
```

## 命令 hook

### 订阅页

- `[data-action="feed-url-input"]`
- `[data-action="add-feed"]`
- `[data-action="refresh-all"]`
- `[data-action="config-text"]`
- `[data-action="export-config"]`
- `[data-action="import-config"]`
- `[data-action="opml-text"]`
- `[data-action="export-opml"]`
- `[data-action="import-opml"]`
- `[data-action="refresh-feed"]`
- `[data-action="remove-feed"]`

### 文章页

- `[data-action="search-title"]`
- `[data-action="filter-unread"]`
- `[data-action="filter-starred"]`
- `[data-action="mark-read"]`
- `[data-action="toggle-starred"]`

### 阅读页

- `[data-action="mark-read"]`
- `[data-action="toggle-starred"]`

### 设置页

- `[data-action="theme-mode"]`
- `[data-action="list-density"]`
- `[data-action="startup-view"]`
- `[data-action="refresh-interval"]`
- `[data-action="reader-font-scale"]`
- `[data-action="custom-css"]`
- `[data-action="current-custom-css-source"]`
- `[data-action="import-custom-css-file"]`
- `[data-action="export-custom-css-file"]`
- `[data-action="preset-theme-select"]`
- `[data-action="apply-selected-theme"]`
- `[data-action="theme-gallery"]`
- `[data-action="theme-card"]`
- `[data-action="apply-theme-card"]`
- `[data-action="apply-theme-newsprint"]`
- `[data-action="apply-theme-forest-desk"]`
- `[data-action="apply-theme-midnight-ledger"]`
- `[data-action="clear-custom-css"]`
- `[data-action="save-settings"]`
- `[data-action="webdav-endpoint"]`
- `[data-action="webdav-remote-path"]`
- `[data-action="push-webdav"]`
- `[data-action="pull-webdav"]`

## 稳定组件 class

这些 class 当前已作为公开样式接口的一部分使用：

- `.app-shell`
- `.app-header`
- `.app-eyebrow`
- `.app-subtitle`
- `.app-nav`
- `.app-nav__link`
- `.page`
- `.page-home`
- `.page-feeds`
- `.page-entries`
- `.page-settings`
- `.reader-page`
- `.stats-grid`
- `.stat-card`
- `.stat-card__label`
- `.stat-card__value`
- `.feed-form`
- `.feed-list`
- `.feed-card`
- `.feed-card__title`
- `.feed-card__meta`
- `.exchange-grid`
- `.exchange-card`
- `.settings-grid`
- `.settings-card`
- `.entry-list`
- `.entry-card`
- `.entry-card__title`
- `.entry-card__meta`
- `.entry-card__actions`
- `.entry-filters`
- `.entry-filters__toggle`
- `.reader-meta`
- `.reader-body`
- `.reader-html`
- `.status-banner`
- `.text-input`
- `.text-area`
- `.select-input`
- `.field-label`
- `.button`
- `.button.secondary`
- `.button.danger`
- `.button.danger-outline`
- `.inline-actions`
- `.theme-gallery`
- `.theme-card`
- `.theme-card.is-active`
- `.theme-card__title`
- `.theme-card__description`
- `.theme-card__notes`
- `.theme-card__swatches`
- `.theme-card__swatch`

## 可用 CSS 变量

当前默认主题暴露的变量：

- `--bg`
- `--panel`
- `--panel-strong`
- `--ink`
- `--muted`
- `--line`
- `--accent`
- `--accent-strong`
- `--shadow`
- `--font-display`
- `--font-ui`

主题作者可以优先覆写这些变量，而不是整份 CSS 全改。

示例：

```css
:root {
  --accent: #0c7a5c;
  --accent-strong: #095c45;
  --panel: rgba(247, 252, 250, 0.9);
}
```

## 常见定制示例

### 1. 把设置导航挪到最右边

```css
.app-nav [data-nav="settings"] {
  margin-left: auto;
}
```

### 2. 在订阅页隐藏 OPML 区块

```css
[data-page="feeds"] [data-action="opml-text"] {
  display: none;
}

[data-page="feeds"] [data-action="export-opml"],
[data-page="feeds"] [data-action="import-opml"] {
  display: none;
}
```

### 3. 强调删除按钮

```css
[data-page="feeds"] [data-action="remove-feed"] {
  min-width: 7rem;
  font-weight: 700;
}
```

### 4. 优化阅读页排版

```css
[data-page="reader"] .reader-body {
  max-width: 72ch;
  margin: 0 auto;
  font-size: 1.12rem;
  line-height: 1.9;
}
```

## 将示例主题应用到应用中

### 在设置页中使用

1. 打开对应的主题文件
2. 复制 CSS 内容
3. 粘贴到“设置”页的“自定义 CSS”
4. 点击“保存设置”

### 用 CLI 应用

```bash
cargo run -p rssr-cli -- save-settings --custom-css-file assets/themes/newsprint.css
```

也可以换成：

```bash
cargo run -p rssr-cli -- save-settings --custom-css-file assets/themes/forest-desk.css
cargo run -p rssr-cli -- save-settings --custom-css-file assets/themes/midnight-ledger.css
```

## 兼容性说明

- `data-page`、`data-action`、`data-nav` 视为优先保持稳定的公开接口
- 组件 class 在当前版本也可稳定依赖
- DOM 层级、容器嵌套关系、某些卡片内部结构不承诺永久不变

如果后续需要新增公开 hook，应优先新增语义化 `data-*` 标记，而不是要求主题作者依赖新的内部层级。
