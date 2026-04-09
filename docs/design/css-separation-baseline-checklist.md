# CSS 完全分离基线检查

## 目标

这份清单用于推进：

- `headless active interface`
- `CSS 完全分离`
- `infra` 承担真实行为

它只关注一个问题：

- 当前样式是否仍然依赖页面内部 DOM 结构，而不是稳定语义接口

结论先说：

- 当前主页面已经比早期干净很多。
- 但仍有几类选择器会妨碍“页面只是默认语义壳、CSS 可以自由重排”的终局。
- 第一轮状态接口迁移已经落地：
  - `status-banner` 已切到 `data-state`
  - `button` 已切到 `data-variant`
  - `app-shell` 密度已切到 `data-density`
  - 主题卡、来源筛选 chip、阅读底栏按钮已切到 `data-state`
- 因此本清单里与这些点对应的条目，现在应视为“已完成第一轮收口，后续只剩一致性复查”。

---

## 总体判断

### 已经做对的部分

- 页面级作用域已经比较稳定：
  - `data-page`
  - `data-action`
  - `data-field`
  - `data-nav`
  - `data-state`
- 主页面的大块布局主要仍靠稳定 class，而不是匿名 DOM 层级。
- `reader-html` 已经被限制在单独内容岛里，没有把远端 HTML 样式扩散到整个页面。

### 仍需收口的部分

- 一些布局样式仍依赖：
  - 标签后代选择器
  - `> *`
  - `:first-child` / `:last-child`
  - 相邻兄弟 `+`
- 一些状态样式仍依赖：
  - `.is-*`
  - `.info`
  - `.error`
  - `.secondary`
  - `.danger`
  - `.danger-outline`
  - `.density-compact`

这些都不是运行时 bug，但会削弱 CSS 对页面结构的独立控制能力。

---

## 第一组：必须收掉的层级依赖

### 1. 页面标题和页头仍依赖内部标签结构

文件：

- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)
- [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css)

问题点：

- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css#L196) `.page h2`
- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css#L210) `.page-header h2`
- [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css#L47) `.page h2`
- [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css#L56) `.page-header .icon-link-button`

为什么是问题：

- 这些规则要求页面标题继续是 `h2`，而且要求按钮继续挂在 `page-header` 下面。
- 一旦页面为了 CSS 重排把标题换成别的语义元素，或者把动作按钮搬离这个容器，样式就会失效。

下一轮建议：

- 补稳定标题 hook，例如：
  - `.page-title`
  - `.page-header__title`
  - `.page-header__actions`
- 让样式改为针对语义槽，而不是标签名或后代位置。

### 2. 布局规则仍假设子节点是匿名直系子元素

文件：

- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
- [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css)

问题点：

- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css#L156) `.settings-form-grid > div`
- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css#L425) `.entry-card__actions > *`
- [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css#L174) `.inline-actions > *`
- [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css#L183) `.entry-card__actions > *`

为什么是问题：

- 这些规则默认子元素是什么都无所谓，但必须是“直接子节点”。
- 如果页面为了语义结构多包一层容器，布局就立刻变掉。

下一轮建议：

- 给动作槽和表单项补语义子类：
  - `.settings-form-grid__item`
  - `.inline-actions__item`
  - `.entry-card__action`
- 避免继续使用 `> *` 作为长期接口。

### 3. 阅读列表仍依赖顺序与相邻关系

文件：

- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)

问题点：

- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css#L236) `.entry-list--grouped .entry-card--reading + .entry-card--reading`
- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css#L255) `.entry-card--reading:first-child`
- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css#L259) `.entry-card--reading:last-child`

为什么是问题：

- 这些规则直接把“在列表里的位置”当成视觉状态来源。
- 一旦未来页面加分隔容器、广告位、注释位或可折叠组，这些样式会变脆。

下一轮建议：

- 用显式状态替代位置判断，例如：
  - `data-list-edge="start|middle|end|single"`
  - 或在 facade/session 里为阅读列表项补稳定边界状态

### 4. 页面特化仍靠页面类和内部块类组合

文件：

- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
- [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css)

问题点：

- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css#L35) `.page-entries .reading-header`
- [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css#L86) `.page-entries .reading-header`

为什么是问题：

- 这还在依赖“某块元素在某页面里”的结构组合，而不是纯页面语义接口。

下一轮建议：

- 统一成页面数据接口，例如：
  - `[data-page="entries"] [data-slot="reading-header"]`
  - 或稳定 class `.page-header--entries`

---

## 第二组：必须收掉的 class 状态依赖

### 1. 状态 banner 仍靠 `.info` / `.error`

文件：

- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)

问题点：

- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css#L293) `.status-banner.info`
- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css#L297) `.status-banner.error`

当前代码已经有：

- [status_banner.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/components/status_banner.rs) `data-state="{tone}"`

当前状态：

- 已完成，样式已迁到：
  - `.status-banner[data-state="info"]`
  - `.status-banner[data-state="error"]`

后续建议：

- 继续避免在新组件里回流 `.info/.error` modifier class。

### 2. 主题卡、筛选项、阅读底栏仍靠 `.is-*`

文件：

- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
- [reader.css](/home/develata/gitclone/RSS-Reader/assets/styles/reader.css)

问题点：

- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css#L193) `.theme-card.is-active`
- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css#L223) `.entry-filters__source-chip.is-selected`
- [reader.css](/home/develata/gitclone/RSS-Reader/assets/styles/reader.css#L209) `.reader-bottom-bar__button.is-active`
- [reader.css](/home/develata/gitclone/RSS-Reader/assets/styles/reader.css#L215) `.reader-bottom-bar__button.is-disabled`

当前状态：

- 已完成第一轮迁移：
  - `.theme-card[data-state="active"]`
  - `.entry-filters__source-chip[data-state="selected"]`
  - `.reader-bottom-bar__button[data-state="starred|read|available|unavailable"]`

后续建议：

- 新增交互状态时，优先补 `data-state`，不要再引入 `.is-*`。

### 3. 按钮视觉变体仍靠 class modifier

文件：

- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)

问题点：

- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css#L356) `.button.secondary`
- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css#L361) `.button.danger`
- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css#L365) `.button.danger-outline`

为什么是问题：

- 这些 class 本质上是在表达视觉语义，而不是结构语义。
- 它们还在被页面 facade 和组件硬编码返回，例如：
  - [settings_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/facade.rs#L118)
  - [feeds_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/facade.rs#L45)
  - [reader_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/facade.rs#L95)

当前状态：

- 已完成，当前按钮视觉变体统一走：
  - `data-variant="primary|secondary|danger|danger-outline"`

后续建议：

- facade 和页面 session 不再返回视觉 class 字符串，只返回语义变体名。

### 4. density 仍靠根 class

文件：

- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)
- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)

问题点：

- `.app-shell.density-compact ...`

为什么是问题：

- 这不是深 DOM 问题，但仍然是状态靠 class modifier，而不是公开状态接口。

当前状态：

- 已完成，根壳已改为：
  - `.app-shell[data-density="compact"]`
  - `.app-shell[data-density="comfortable"]`

后续建议：

- 继续把所有密度相关规则集中到 `data-density`，不要再引入 `density-*` class modifier。

---

## 第三组：允许保留的例外

### `reader-html` 内容岛

文件：

- [reader.css](/home/develata/gitclone/RSS-Reader/assets/styles/reader.css)

问题点：

- [reader.css](/home/develata/gitclone/RSS-Reader/assets/styles/reader.css#L73) `.reader-html p:first-child`
- [reader.css](/home/develata/gitclone/RSS-Reader/assets/styles/reader.css#L116) `.reader-html p > img:only-child ...`
- 以及整组：
  - `h1/h2/h3/h4`
  - `blockquote`
  - `ul/ol`
  - `img/video/canvas/svg/picture`
  - `figure`
  - `table`

判断：

- 这组样式可以作为**允许保留的内容 HTML 例外**。
- 原因不是页面结构耦合，而是它本来就在给远端 HTML 内容岛做排版修正。

要求：

- 继续把它限制在 `.reader-html` / `[data-state="html"]` 语义范围内
- 不要把这类标签规则扩散到 `.page`、`.reader-page` 或全局

---

## 文档漂移

文件：

- [theme-author-selector-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/theme-author-selector-reference.md)

问题：

- 这份文档仍然列着旧接口，例如：
  - `data-action="feed-url-input"`
  - `data-action="search-title"`
  - `data-action="filter-unread"`
  - `data-action="filter-starred"`
- 但当前代码已经收成：
  - `data-field="entry-search"`
  - `data-field="read-filter-unread"`
  - `data-field="starred-filter-starred"`
  - 以及更多 `data-state`

下一轮建议：

- 让主题作者文档与：
  - [frontend-command-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/frontend-command-reference.md)
  完全对齐。

---

## 下一轮收口清单

### P1

- 把 `.status-banner.info/.error` 迁到 `data-state`
- 把 `.theme-card.is-active`、`.entry-filters__source-chip.is-selected`、`.reader-bottom-bar__button.is-*` 迁到 `data-state`
- 把 `.button.secondary/.danger/.danger-outline` 迁到 `data-variant`
- 把 `.app-shell.density-compact` 迁到 `data-density`

### P2

- 给页头和标题补稳定语义槽：
  - `.page-title`
  - `.page-header__title`
  - `.page-header__actions`
- 去掉 `.page h2`、`.page-header h2`、`.page-header .icon-link-button`

### P3

- 去掉 `.settings-form-grid > div`、`.inline-actions > *`、`.entry-card__actions > *`
- 给子项补稳定语义 class / `data-slot`

### P4

- 去掉 `.entry-card--reading:first-child/last-child` 和相邻兄弟规则
- 用显式列表边界状态替代位置依赖

### P5

- 同步更新 `theme-author-selector-reference.md`
- 明确把 `reader-html` 标记成“允许保留的内容岛例外”
