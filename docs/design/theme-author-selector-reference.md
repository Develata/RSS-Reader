# 主题作者 Selector 参考

这份文档面向自定义主题作者，汇总当前 UI 中可依赖的稳定样式接口。

目标：

- 让主题作者优先依赖稳定 hook，而不是内部 DOM 层级
- 降低页面重构时对主题兼容性的破坏
- 提供一份“可直接开写”的 selector 速查表
- 让 AI 在只拿到这份文档时，也能生成一份可直接使用的主题 CSS

## 使用约定

- 优先使用 `data-page`、`data-action`、`data-nav`
- 其次使用稳定组件 class
- 尽量避免依赖深层后代选择器，如 `.page > div > div > button`
- 如果只想改某个页面，先用 `data-page` 限定作用域
- 如果想从现成主题开始，优先参考：
  - `assets/themes/atlas-sidebar.css`
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

## AI 生成主题的最小约定

如果这份文档被直接喂给 AI，用来生成一份新的主题 CSS，建议把输出约束成下面这样：

- 输出一份完整、可直接保存为 `.css` 的样式文件
- 默认先覆写 `:root` 里的变量，再做少量结构性覆盖
- 不修改行为，只改视觉和布局
- 不依赖未知 DOM 层级
- 优先作用在：
  - `:root`
  - `[data-page="..."]`
  - `[data-action="..."]`
  - `[data-nav="..."]`
  - 本文列出的稳定组件 class
- 避免默认隐藏关键功能入口
  - 例如 `add-feed`、`refresh-all`、`save-settings`、`mark-read`
- 如果需要重排页面，应当通过 `flex`、`grid`、`order`、`gap`、`max-width`、`margin` 完成
- 不要使用远端 `@import` 字体或第三方资源，除非用户明确要求

推荐让 AI 输出时遵循这个结构：

```css
:root {
  /* 变量层 */
}

/* 全局壳层与导航 */
.app-shell { ... }
.app-nav { ... }

/* 页面级布局 */
[data-page="feeds"] { ... }
[data-page="entries"] { ... }
[data-page="reader"] { ... }
[data-page="settings"] { ... }

/* 关键操作按钮 */
[data-action="add-feed"] { ... }
[data-action="mark-read"] { ... }
[data-action="toggle-starred"] { ... }
[data-action="save-settings"] { ... }
```

## 可直接复制给 AI 的提示模板

如果你希望另一个 AI 在只阅读这份文档的前提下生成一份新主题，可以直接把下面这段作为提示词起点：

```text
请为 RSS-Reader 生成一份完整的自定义 CSS 主题。

约束：
- 只修改样式和布局，不修改任何行为
- 只依赖这份文档里列出的稳定接口：
  - :root 变量
  - [data-page]
  - [data-action]
  - [data-nav]
  - 稳定组件 class
- 不依赖未知 DOM 层级
- 不默认隐藏关键功能入口
- 优先通过变量层建立主题基调，再做局部布局调整
- 输出必须是一份可直接保存为 .css 文件的完整样式

目标风格：
- {在这里写用户想要的风格，例如“像报纸编辑台”“像轻量知识库”“像冷淡风终端工具”}

额外要求：
- {在这里写用户希望特别改动的点，例如“把导航改成纵向侧栏”“让阅读页更窄更专注”“把设置页做成两栏”}
```

如果用户只给一句比较抽象的风格描述，建议 AI 默认按这个优先级输出：

1. 先改 `:root` 变量
2. 再改 `.app-shell` / `.app-nav` / `.page`
3. 最后只微调关键按钮和阅读页排版

## 页面结构心智模型

把当前应用理解成 5 个稳定页面和 3 层公共结构会更容易产出主题：

- 公共结构
  - `.app-shell`
  - `.app-header`
  - `.app-nav`
- 页面
  - 首页：统计与概览
  - 订阅页：feed 管理 + 导入导出
  - 文章页：文章列表 + 筛选 + 列表动作
  - 阅读页：正文阅读 + 元信息 + 上下篇导航
  - 设置页：主题/阅读选项 + WebDAV

如果用户要求“整体改版”，推荐优先修改：

- `.app-shell`
- `.app-header`
- `.page`
- `.settings-grid`
- `.exchange-grid`
- `.entry-list`
- `.reader-body`

如果用户要求“只换皮不改布局”，推荐优先修改：

- `:root`
- `.button`
- `.status-banner`
- `.text-input`
- `.select-input`
- `.settings-card`
- `.feed-card`
- `.entry-card`

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

说明：

- 这两个 hook 同时会出现在文章列表和阅读页，用于“标已读/未读”和“收藏/取消收藏”
- 如果只想改阅读页里的按钮，建议加上 `[data-page="reader"]` 作为作用域
- 如果只想改文章列表里的按钮，建议加上 `[data-page="entries"]` 作为作用域

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
- `[data-action="remove-theme-card"]`
- `[data-action="apply-theme-atlas-sidebar"]`
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

建议理解方式：

- `--bg`
  - 整个应用的外层背景
- `--panel`
  - 普通卡片、输入框、列表项背景
- `--panel-strong`
  - 更强调的卡片或操作面板
- `--ink`
  - 主要正文和标题颜色
- `--muted`
  - 次级说明文字
- `--line`
  - 细边框、分隔线
- `--accent`
  - 主操作按钮、高亮链接、活动态
- `--accent-strong`
  - 主操作 hover / active / 更强调状态
- `--shadow`
  - 卡片和面板阴影
- `--font-display`
  - 标题字体
- `--font-ui`
  - 普通 UI 字体

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

### 5. 自定义“已读 / 收藏”按钮样式

```css
[data-page="reader"] [data-action="mark-read"],
[data-page="reader"] [data-action="toggle-starred"] {
  border-radius: 999px;
  padding: 10px 14px;
  min-width: 5.5rem;
  font-weight: 700;
}

[data-page="reader"] [data-action="mark-read"] {
  background: #0f766e;
  color: #f7fffd;
}

[data-page="reader"] [data-action="toggle-starred"] {
  background: #f59e0b;
  color: #241d16;
}
```

如果你想改布局而不只是改颜色，也可以直接改它们所在的操作区：

```css
[data-page="reader"] .entry-card__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
```

## 生成后自检清单

无论是人工写主题还是让 AI 生成主题，交付前都建议快速自检：

- 是否只改了样式，没有假设 Rust 行为会变化
- 是否优先使用了 `data-page` / `data-action` / `data-nav`
- 是否避免依赖深层 DOM 层级
- 是否没有默认隐藏关键功能入口
- 是否至少覆盖了导航、列表、阅读页、设置页这 4 类界面
- 是否在桌面宽度和窄屏宽度下都没有明显溢出

### 6. 生成一套“完全不同布局”的主题

下面这类写法适合把应用改成更偏工具型或侧栏型布局：

```css
.app-shell {
  max-width: 1400px;
}

.app-header {
  display: grid;
  grid-template-columns: 220px 1fr;
  align-items: start;
}

.app-nav {
  flex-direction: column;
  align-items: stretch;
}

[data-page="settings"] .settings-grid,
[data-page="feeds"] .exchange-grid {
  grid-template-columns: 320px minmax(0, 1fr);
}
```

这种改法仍然是安全的，因为它依赖的是公共壳层和稳定 class，而不是临时 DOM 层级。

## 主题生成检查表

无论是人工写主题还是让 AI 生成主题，最后都建议快速检查：

- 是否只依赖了稳定 hook/class
- 是否保留了导航入口
- 是否保留了关键命令按钮
- 是否没有把正文区压得过窄
- 是否没有让按钮文字与背景对比不足
- 是否没有让输入框和状态提示不可见
- 是否在移动端仍然能滚动与阅读
- 是否没有依赖仓库里不存在的字体或图片资源

## 给 AI 的直接提示模板

如果你想让另一个 AI 只根据本文档生成主题，可以直接给它这样的指令：

> 为 RSS-Reader 生成一份完整的自定义主题 CSS。
> 只允许使用本文档中列出的 `data-page`、`data-nav`、`data-action`、稳定组件 class 和 CSS 变量。
> 不要修改行为，不要假设额外 DOM。
> 优先覆写变量，再少量重排布局。
> 保留导航、订阅、保存设置、标已读、收藏等关键按钮的可见性。
> 输出一个可直接保存为 `.css` 文件的结果。

## 将示例主题应用到应用中

- 通过设置页的主题卡片或“载入所选主题”按钮应用内置主题时，主题会立即生效并自动保存。
- `移除这套主题` 只会移除当前卡片对应的内置主题；如果当前并未启用该主题，只会提示，不会清空其它自定义 CSS。

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
