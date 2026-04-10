# 主题作者 Selector 参考

这份文档面向自定义主题作者，记录当前 UI 中可长期依赖的稳定样式接口。

目标：

- 优先依赖语义接口，而不是内部 DOM 层级
- 让页面重构不轻易打碎主题
- 给手写主题和 AI 生成主题提供同一份约束

## 使用原则

- 优先使用：
  - `data-page`
  - `data-layout`
  - `data-slot`
  - `data-nav`
  - `data-action`
  - `data-field`
  - `data-state`
  - `data-variant`
  - `data-density`
- 其次使用明确公开的通用壳类：
  - `.app-shell`
  - `.app-header`
  - `.page`
  - `.button`
  - `.status-banner`
  - `.text-input`
  - `.text-area`
  - `.select-input`
  - `.field-label`
  - `.feed-card`
  - `.entry-card`
  - `.settings-card`
  - `.exchange-card`
  - `.theme-card`
- 避免依赖深层后代和匿名位置：
  - `.page > div:nth-child(3) > button`
  - `.entry-card > :first-child > :last-child`
- 如果只想改某个页面，先用 `data-page` 限定作用域

推荐：

```css
[data-page="feeds"] [data-action="remove-feed"] {
  order: 10;
}
```

不推荐：

```css
.page > div:nth-child(4) > ul > li > div > button:last-child {
  order: 10;
}
```

## AI 生成主题的最小约定

如果把这份文档直接喂给 AI 生成主题 CSS，建议约束成：

- 输出一份完整、可直接保存为 `.css` 的样式文件
- 不改行为，只改视觉和布局
- 优先覆写变量，再做少量结构性覆盖
- 不依赖未知 DOM 层级
- 不默认隐藏关键功能入口
- 只依赖本文档列出的稳定接口

推荐输出结构：

```css
:root {
  /* 变量层 */
}

/* 全局壳层 */
.app-shell { ... }

/* 语义布局与槽位 */
[data-layout="app-nav-shell"] { ... }
[data-slot="page-header-actions"] { ... }

/* 页面级布局 */
[data-page="feeds"] { ... }
[data-page="entries"] { ... }
[data-page="reader"] { ... }
[data-page="settings"] { ... }

/* 动作与字段 */
[data-action="add-feed"] { ... }
[data-field="feed-url-input"] { ... }
```

## 可直接复制给 AI 的提示模板

```text
请为 RSS-Reader 生成一份完整的自定义 CSS 主题。

约束：
- 只修改样式和布局，不修改任何行为
- 只依赖这份文档中列出的稳定接口：
  - :root 变量
  - data-page / data-layout / data-slot
  - data-nav / data-action / data-field / data-state
  - 明确公开的通用壳类
- 不依赖未知 DOM 层级
- 不默认隐藏关键功能入口
- 优先覆写变量，再少量重排布局
- 输出必须是一份可直接保存为 .css 文件的完整样式

目标风格：
- {在这里写用户想要的风格}

额外要求：
- {在这里写希望特别改动的点}
```

## 页面结构心智模型

把当前应用理解成 4 个稳定页面和 3 层公共结构会更容易产出主题：

- 公共结构
  - `.app-shell`
  - `.app-header`
  - `[data-layout="app-nav-shell"]`
- 页面
  - 订阅页：管理订阅、导入导出、统计概览
  - 文章页：文章列表、筛选、目录导航
  - 阅读页：正文阅读、元信息、分页导航
  - 设置页：主题与偏好、WebDAV 同步

如果用户要求“整体改版”，推荐优先修改：

- `.app-shell`
- `.app-header`
- `.page`
- `[data-layout="app-nav-shell"]`
- `[data-layout="settings-grid"]`
- `[data-layout="exchange-grid"]`
- `[data-layout="entries-layout"]`
- `[data-layout="reader-body"]`

如果用户要求“只换皮不改布局”，推荐优先修改：

- `:root`
- `.button`
- `.status-banner`
- `.text-input`
- `.select-input`
- `.settings-card`
- `.feed-card`
- `.entry-card`
- `.theme-card`

## 页面级接口

- `data-page="feeds"`
- `data-page="entries"`
- `data-page="reader"`
- `data-page="settings"`

这是最安全的页面级作用域入口。

## 导航接口

- `data-nav="feeds"`
- `data-nav="entries"`
- `data-nav="settings"`
- `data-nav="back"`
- `data-nav="feed-entries"`
- `data-nav="previous-feed-entry"`
- `data-nav="next-feed-entry"`
- `data-nav="previous-unread-entry"`
- `data-nav="next-unread-entry"`
- `data-nav="entry-directory-month"`
- `data-nav="entry-directory-date"`

示例：

```css
[data-layout="app-nav-links"] [data-nav="settings"] {
  margin-left: auto;
}
```

## 动作接口

### 订阅页

- `data-action="add-feed"`
- `data-action="refresh-all"`
- `data-action="refresh-feed"`
- `data-action="remove-feed"`
- `data-action="export-config"`
- `data-action="import-config"`
- `data-action="export-opml"`
- `data-action="import-opml"`

### 文章与阅读

- `data-action="mark-read"`
- `data-action="toggle-starred"`
- `data-action="group-by-source"`
- `data-action="group-by-time"`
- `data-action="toggle-archived"`

### 设置与同步

- `data-action="save-settings"`
- `data-action="apply-custom-css"`
- `data-action="clear-custom-css"`
- `data-action="import-custom-css-file"`
- `data-action="export-custom-css-file"`
- `data-action="apply-selected-theme"`
- `data-action="apply-theme-preset"`
- `data-action="remove-theme-preset"`
- `data-action="push-webdav"`
- `data-action="pull-webdav"`
- `data-action="open-github-repo"`
- `data-action="show-top-nav"`
- `data-action="hide-top-nav"`

说明：

- 输入字段不再伪装成 action
- 结构容器不应使用 `data-action`

## 字段接口

- `data-field="entry-search"`
- `data-field="search-title"`
- `data-field="read-filter-unread"`
- `data-field="read-filter-read"`
- `data-field="starred-filter-starred"`
- `data-field="starred-filter-unstarred"`
- `data-field="entry-source-filter"`
- `data-field="entry-grouping-mode"`
- `data-field="show-archived"`
- `data-field="feed-url-input"`
- `data-field="config-text"`
- `data-field="opml-text"`
- `data-field="theme-mode"`
- `data-field="list-density"`
- `data-field="startup-view"`
- `data-field="refresh-interval"`
- `data-field="archive-after-months"`
- `data-field="reader-font-scale"`
- `data-field="preset-theme-select"`
- `data-field="custom-css"`
- `data-field="webdav-endpoint"`
- `data-field="webdav-remote-path"`

## 布局与槽位接口

这些接口是现在主题和 CSS 完全分离最值得依赖的部分：

### 应用壳

- `data-layout="app-nav-shell"`
- `data-layout="app-nav-topline"`
- `data-layout="app-nav-links"`
- `data-layout="app-nav-search"`
- `data-layout="web-auth-shell"`
- `data-layout="web-auth-card"`
- `data-layout="web-auth-brand"`
- `data-layout="web-auth-form"`

### 页面通用

- `data-layout="page-header"`
- `data-layout="page-section-header"`
- `data-slot="page-header-actions"`
- `data-slot="page-section-row"`
- `data-slot="page-title"`
- `data-slot="page-section-title"`

### 订阅页与设置页

- `data-layout="stats-grid"`
- `data-layout="feed-workbench-single"`
- `data-layout="exchange-grid"`
- `data-layout="settings-grid"`

### 文章页

- `data-layout="entries-layout"`
- `data-layout="entry-groups"`
- `data-layout="entry-directory-rail"`
- `data-layout="entry-directory-nav"`
- `data-layout="entry-directory-section"`
- `data-layout="entry-directory-children"`
- `data-layout="entry-directory-grandchildren"`
- `data-layout="entry-directory-link"`
- `data-layout="entry-directory-toggle"`
- `data-layout="entry-top-directory"`
- `data-layout="entry-top-directory-chip"`
- `data-layout="entry-filters"`
- `data-layout="entry-filters-toggle"`
- `data-layout="entry-filters-sources"`
- `data-layout="entry-filters-source-grid"`

### 阅读页

- `data-layout="reader-page"`
- `data-layout="reader-header"`
- `data-layout="reader-toolbar"`
- `data-layout="reader-meta-block"`
- `data-layout="reader-body"`
- `data-layout="reader-pagination"`
- `data-layout="reader-bottom-bar"`

### 常用槽位

- `data-slot="app-nav-brand"`
- `data-slot="app-nav-brand-mark"`
- `data-slot="app-nav-brand-name"`
- `data-slot="app-nav-search-input"`
- `data-slot="entry-directory-heading"`
- `data-slot="entry-directory-title"`
- `data-slot="entry-directory-meta"`
- `data-slot="entry-filters-sources-label"`
- `data-slot="feed-card-title"`
- `data-slot="feed-card-meta"`
- `data-slot="entry-card-title"`
- `data-slot="entry-card-meta"`
- `data-slot="page-intro"`
- `data-slot="reader-title"`
- `data-slot="reader-meta"`
- `data-slot="reader-bottom-bar-icon"`
- `data-slot="reader-bottom-bar-label"`
- `data-slot="theme-card-title"`
- `data-slot="theme-card-swatches"`
- `data-slot="theme-card-swatch"`
- `data-slot="web-auth-title"`
- `data-slot="web-auth-intro"`
- `data-slot="web-auth-note"`

## 状态与变体接口

### `data-state`

- `expanded | collapsed`
- `info | error | success`
- `pending | idle`
- `confirm | idle`
- `active | inactive`
- `available | unavailable`
- `read | unread`
- `starred | unstarred`
- `html | text`
- `empty | populated`

### `data-variant`

- `secondary`
- `danger`
- `danger-outline`

### `data-density`

- `compact`
- `comfortable`

## 稳定通用类

以下 class 仍然是公开可依赖的通用样式接口：

- `.app-shell`
- `.app-header`
- `.app-eyebrow`
- `.app-subtitle`
- `.page`
- `.feed-card`
- `.entry-card`
- `.settings-card`
- `.exchange-card`
- `.theme-card`
- `.status-banner`
- `.button`
- `.text-input`
- `.text-area`
- `.select-input`
- `.field-label`
- `.reader-html`

说明：

- `.reader-html` 是内容岛例外，可以继续依赖
- `.page` / `.app-header` 是通用壳类，不是下一轮优先清理对象

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

建议优先覆写这些变量，再做局部布局调整。

## 常见定制示例

### 1. 把设置导航挪到最右边

```css
[data-layout="app-nav-links"] [data-nav="settings"] {
  margin-left: auto;
}
```

### 2. 在订阅页隐藏 OPML 区块

```css
[data-page="feeds"] [data-field="opml-text"] {
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
[data-page="reader"] [data-layout="reader-body"] {
  max-width: 72ch;
  margin: 0 auto;
  font-size: 1.12rem;
  line-height: 1.9;
}
```

### 5. 自定义阅读底栏动作样式

```css
[data-page="reader"] [data-action="mark-read"],
[data-page="reader"] [data-action="toggle-starred"] {
  border-radius: 999px;
  padding: 10px 14px;
  min-width: 5.5rem;
  font-weight: 700;
}
```

### 6. 重排筛选区

```css
[data-page="entries"] [data-layout="entry-filters"] {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
```

## 生成后自检清单

- 是否只改了样式，没有假设 Rust 行为会变化
- 是否优先使用了 `data-page / data-layout / data-slot / data-action / data-field / data-nav`
- 是否避免依赖深层 DOM 层级
- 是否没有默认隐藏关键功能入口
- 是否在桌面宽度和窄屏宽度下都没有明显溢出
- 是否没有依赖仓库里不存在的外部资源

## 将示例主题应用到应用中

- 通过设置页的主题卡片或“载入所选主题”按钮应用内置主题时，主题会立即生效并自动保存
- `移除这套主题` 只会移除当前卡片对应的内置主题；如果当前并未启用该主题，只会提示，不会清空其它自定义 CSS

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
