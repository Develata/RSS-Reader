# CSS 完全分离执行清单

## 目标

- 推进：
  - `headless active interface`
  - `CSS 完全分离`
  - `infra` 承担真实行为
- 判断标准：
  - CSS 优先依赖稳定语义接口
  - 页面只提供默认语义壳
  - 避免继续依赖深 DOM 层级、匿名子节点、位置选择器、modifier class

## 当前结论

- 第一轮状态接口迁移：已完成
  - `status-banner` -> `data-state`
  - `button` -> `data-variant`
  - `app-shell` 密度 -> `data-density`
  - 主题卡 / 来源筛选 / 阅读底栏按钮 -> `data-state`
- 第二轮结构槽迁移：进行中
  - 标题槽、页头槽、表单项槽、动作项槽、阅读列表边界槽已补
  - 卡片头部标题已开始统一迁到 `.card-title`
  - entries 分组头部已开始统一迁到 `.group-header` / `.group-header__title` / `.group-header__meta`
  - 关键布局容器已开始统一迁到 `data-layout`
- 当前剩余问题：
  - 高密度 class 驱动区域已经基本清空。
  - 当前剩余问题主要是：
    - 少量通用布局 class 仍在承担组织作用
    - `reader-html` 内容岛作为允许保留的例外继续存在

## 已完成项

### 状态接口

- 已迁移：
  - `.status-banner.info/.error`
  - `.button.secondary/.danger/.danger-outline`
  - `.app-shell.density-compact`
  - `.theme-card.is-active`
  - `.entry-filters__source-chip.is-selected`
  - `.reader-bottom-bar__button.is-active/.is-disabled`

- 当前稳定接口：
  - `data-state`
  - `data-variant`
  - `data-density`

### 结构槽

- 已迁移：
  - `.page h2` -> `.page-title`
  - `.page-header h2` -> `.page-header__title`
  - `.page-header .icon-link-button` -> `.page-header__actions .icon-link-button`
  - `settings-form-grid > div` -> `.settings-form-grid__item`
  - `inline-actions > *` -> `.inline-actions__item`
  - `entry-card__actions > *` -> `[data-slot="entry-card-action"]`
  - `.entry-card--reading + .entry-card--reading` -> `data-list-edge`
  - `.entry-card--reading:first-child/:last-child` -> `data-list-edge`
  - `.page-entries .reading-header` -> `.page-section-header--entries`

- 当前稳定接口：
  - `.page-title`
  - `.page-header__title`
  - `.page-header__actions`
  - `.page-section-header`
  - `.page-section-title`
  - `.settings-form-grid__item`
  - `.inline-actions__item`
  - `[data-slot="entry-card-action"]`
  - `data-list-edge="start|middle|end|single"`

### 导航壳

- 已迁移：
  - `app-nav-shell` -> `data-layout="app-nav-shell"`
  - `app-nav-reveal` -> `data-layout="app-nav-reveal"`
  - `app-nav__topline` -> `data-layout="app-nav-topline"`
  - `app-nav` -> `data-layout="app-nav-links"`
  - `app-nav__search` -> `data-layout="app-nav-search"`
  - `app-nav__brand* / reveal* / search* / collapse` -> `data-slot="app-nav-*"`
  - `app-nav__link` -> `data-nav`

- 当前稳定接口：
  - `data-layout="app-nav-shell|app-nav-reveal|app-nav-topline|app-nav-links|app-nav-search"`
  - `data-slot="app-nav-*"`
  - `data-nav`

### 目录栏与顶部目录

- 已迁移：
  - `entry-directory-rail` -> `data-layout="entry-directory-rail"`
  - `entry-directory-rail__nav` -> `data-layout="entry-directory-nav"`
  - `entry-directory-rail__section/subsection` -> `data-layout="entry-directory-section"`
  - `entry-directory-rail__children` -> `data-layout="entry-directory-children"`
  - `entry-directory-rail__grandchildren` -> `data-layout="entry-directory-grandchildren"`
  - `entry-directory-rail__link` -> `data-layout="entry-directory-link"`
  - `entry-directory-rail__toggle` -> `data-layout="entry-directory-toggle"`
  - `entry-directory-rail__title` -> `data-slot="entry-directory-heading"`
  - `entry-top-directory` -> `data-layout="entry-top-directory"`
  - `entry-top-directory__chip` -> `data-layout="entry-top-directory-chip"`
  - 目录文案统一到：
    - `data-slot="entry-directory-title"`
    - `data-slot="entry-directory-meta"`
- 当前稳定接口：
  - `data-layout="entry-directory-*|entry-top-directory*"`
  - `data-slot="entry-directory-heading|entry-directory-title|entry-directory-meta"`
  - `data-nav="entry-directory-*"`

### 阅读页外壳

- 已迁移：
  - `reader-page` -> `data-layout="reader-page"`
  - `reader-header` -> `data-layout="reader-header"`
  - `reader-toolbar` -> `data-layout="reader-toolbar"`
  - `reader-meta-block` -> `data-layout="reader-meta-block"`
  - `reader-body` -> `data-layout="reader-body"`
  - `reader-pagination` -> `data-layout="reader-pagination" + data-context`
  - `reader-bottom-bar` -> `data-layout="reader-bottom-bar"`
  - `reader-title / reader-meta / reader-bottom-bar__icon / reader-bottom-bar__label` -> `data-slot`
- 当前稳定接口：
  - `data-layout="reader-*"`
  - `data-slot="reader-title|reader-meta|reader-bottom-bar-icon|reader-bottom-bar-label"`
  - `data-context="feed"`

### Web 本地门禁壳

- 已迁移：
  - `web-auth-shell` -> `data-layout="web-auth-shell"`
  - `web-auth-card` -> `data-layout="web-auth-card"`
  - `web-auth-brand` -> `data-layout="web-auth-brand"`
  - `web-auth-form` -> `data-layout="web-auth-form"`
  - `web-auth-brand__mark / name / title / intro / note` -> `data-slot`
- 当前稳定接口：
  - `data-layout="web-auth-shell|web-auth-card|web-auth-brand|web-auth-form"`
  - `data-slot="web-auth-brand-mark|web-auth-brand-name|web-auth-title|web-auth-intro|web-auth-note"`

### 筛选布局

- 已迁移：
  - `entry-filters` -> `data-layout="entry-filters"`
  - `entry-filters__toggle` -> `data-layout="entry-filters-toggle"`
  - `entry-filters__sources` -> `data-layout="entry-filters-sources"`
  - `entry-filters__source-grid` -> `data-layout="entry-filters-source-grid"`
  - `entry-filters__sources-label` -> `data-slot="entry-filters-sources-label"`
- 当前稳定接口：
  - `data-layout="entry-filters|entry-filters-toggle|entry-filters-sources|entry-filters-source-grid"`
  - `data-slot="entry-filters-sources-label"`

## P1：下一轮必须收掉

### 1. 卡片头部仍依赖标签名

- 文件：
  - [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
- 目标规则：
  - `.feed-workbench__note h3`
- 下一步：
  - 将剩余 note / 说明块标题也统一到稳定标题槽
  - 不再新增针对 `h3` 的卡片头部规则

### 2. 分组头部仍依赖内部标题/元信息结构

- 文件：
  - [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
- 目标区域：
  - 历史 selector 已迁移为：
    - `.group-header`
    - `.group-header__title`
    - `.group-header__meta`
    - `data-group-level="primary|date|source"`
- 下一步：
  - 清理遗留 selector 文档和辅助样式
  - 保持后续新增分组头部只走统一槽

### 3. 顶部标题区仍带页面特化 class

- 文件：
  - [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
  - [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
- 现状：
  - 已有 `.page-section-header--entries`
  - `feeds` 已迁到 `.page-section-header--feeds`
  - `settings` 已迁到 `.page-section-header--settings`
- 下一步：
  - 统一成：
    - `.page-section-header--entries`
    - `.page-section-header--feeds`
    - `.page-section-header--settings`
  - 保持后续新增页头只走 `page-section-header--*`

### 4. 布局容器仍主要靠 class 组合命名

- 文件：
  - [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)
  - [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
  - [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
  - [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css)
- 当前已迁移：
  - `page-header` -> `data-layout="page-header"`
  - `stats-grid` -> `data-layout="stats-grid"`
  - `feed-workbench--single` -> `data-layout="feed-workbench-single"`
  - `exchange-grid` -> `data-layout="exchange-grid"`
  - `settings-grid` -> `data-layout="settings-grid"`
  - `entries-layout` -> `data-layout="entries-layout"`
  - `entry-groups` -> `data-layout="entry-groups"`
  - `entry-filters` -> `data-layout="entry-filters"`
  - `page-header__actions` -> `data-slot="page-header-actions"`
  - `reading-header__row` -> `data-slot="page-section-row"`
- 下一步：
  - 继续减少只靠 class 命名表达布局角色的规则
  - 但 `.page` 和 `.page-header` 当前可视为通用壳类，不作为高优先级槽化目标

### 5. 已清掉的高密度 class 驱动区域

- 文件：
  - [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)
  - [reader.css](/home/develata/gitclone/RSS-Reader/assets/styles/reader.css)
- 关注点：
  - `app-nav*`
  - `entry-directory-rail*`
  - `reader-page*`
  - `web-auth-*`
- 判断：
  - 这些专项区域已经迁到稳定语义接口
  - 下一轮不再需要按“高密度 class 驱动区域”继续拆，而应转去做局部一致性复查

## P2：可以继续收，但不阻塞

### 1. 页面级布局仍依赖部分结构 class

- 文件：
  - [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)
  - [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
  - [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
- 关注点：
  - `.page`
  - `.page-header`
  - `.entry-groups`
  - `.entries-layout`
  - `.settings-grid`
  - `.exchange-grid`
- 判断：
  - 这些目前还算合理，不属于第一优先级问题
  - 但后续若要做到更极端的 CSS 重排，需要继续槽化

### 2. 响应式规则仍偏 class 驱动

- 文件：
  - [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css)
- 重点：
  - 移动端规则大多已跟随第一轮、第二轮迁移
  - 仍需要在后续同步检查新增槽位是否都覆盖了 mobile path

## P3：只做一致性复查

### 状态接口

- 确保新增状态只走：
  - `data-state`
  - `data-variant`
  - `data-density`
- 不再引入：
  - `.is-*`
  - `.info/.error`
  - `.secondary/.danger/.danger-outline`
  - `density-*`

### 结构槽

- 确保新增布局规则优先依赖：
  - `.page-title`
  - `.page-header__title`
  - `.page-header__actions`
  - `.page-section-header`
  - `.page-section-title`
  - `.card-title`
  - `.group-header`
  - `.group-header__title`
  - `.group-header__meta`
  - `data-group-level`
  - `data-layout`
  - `.settings-form-grid__item`
  - `.inline-actions__item`
  - `[data-slot="entry-card-action"]`
  - `data-list-edge`

## 允许保留的例外

### `reader-html` 内容岛

- 文件：
  - [reader.css](/home/develata/gitclone/RSS-Reader/assets/styles/reader.css)
- 原因：
  - 这是正文内容岛，不是页面壳结构
  - 可以继续保留内容标签样式，例如：
    - `.reader-html p`
    - `.reader-html h1/h2/h3/h4`
    - `.reader-html ul/ol`
    - `.reader-html img`
    - `.reader-html figure`

## 验收方式

- grep 不应再命中以下模式：
  - `.status-banner.info`
  - `.status-banner.error`
  - `.button.secondary`
  - `.button.danger`
  - `.button.danger-outline`
  - `.theme-card.is-active`
  - `.entry-filters__source-chip.is-selected`
  - `.reader-bottom-bar__button.is-active`
  - `.reader-bottom-bar__button.is-disabled`
  - `.app-shell.density-compact`
  - `.page h2`
  - `.page-header h2`
  - `settings-form-grid > div`
  - `inline-actions > *`
  - `entry-card__actions > *`
  - `.entry-card--reading:first-child`
  - `.entry-card--reading:last-child`
  - `.entry-card--reading + .entry-card--reading`
  - `.page-entries .reading-header`

- 编译验证：
  - `cargo fmt --all`
  - `cargo check -p rssr-app`
  - `cargo check -p rssr-app --target wasm32-unknown-unknown`
  - `git diff --check`

## 当前最值得继续做的两刀

1. 收 `feed-workbench__note` 这条剩余 `h3` 依赖。
2. 复查剩余通用布局 class 是否还应该继续槽化，优先看 `.page`、`.page-header`、`.entry-filters`。
