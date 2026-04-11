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

## 2026-04-11 保留 class 边界审查

- 审查命令：
  - `rg --pcre2 -o '(^|[,\\s])\\.[A-Za-z][A-Za-z0-9_-]*(?=[\\s:{.#\\[,>+~)]|$)' assets/styles assets/themes -S`
  - `rg -o 'class: "[^"]+"' crates/rssr-app/src -S`
- 结论：
  - CSS 中剩余 class selector 已收敛到小集合。
  - 当前不应继续盲目把所有 class 改成 `data-*`，否则会把设计系统类和内部实现类都暴露成不必要的外部接口。
  - 只应继续处理“页面语义缺失”或“死样式”。

### 应保留为设计系统 / 全局状态 class

- `app-shell`
  - 根 shell class，承载全局壳层、密度和主题组合。
  - `data-density` 已承担密度状态；`app-shell` 本身可以继续作为设计系统根类。
- `theme-light` / `theme-dark` / `theme-system`
  - 由 `theme_class(settings().theme)` 动态注入，静态 class token 扫描不会命中。
  - 当前可保留为主题根状态类；若未来要继续统一，可单独迁成 `data-theme`，不要混在页面局部 CSS 收口里做。
- `button`、`text-input`、`text-area`、`select-input`、`field-label`
  - 表单与按钮设计系统类。
  - 状态已经通过 `data-variant` / `data-field` 表达，不需要为了“完全分离”强行改成 `data-layout`。
- `inline-actions` / `inline-actions__item`
  - 通用动作行布局类。
  - 可保留为设计系统布局 primitive；只有当某个页面需要表达业务动作槽时，再额外补 `data-layout`。
- `status-banner`
  - 通用反馈组件类。
  - 状态已走 `data-state`。
  - 布局定位已补 `data-layout="status-banner"`；主题需要定位页面内 banner 时优先用 `data-layout`。
- `icon-link-button`
  - 图标按钮 primitive；可保留。
- `sr-only` / `sr-only-file-input`
  - 可访问性 utility；必须保留为 utility class，不应语义化。

### 可保留为组件内部实现 class

- `reader-bottom-bar__button`
  - 当前选择器被限制在 `[data-layout="reader-bottom-bar"]` 下。
  - `data-state` 已承担状态语义；按钮本体 class 可作为 reader bottom bar 内部实现 class 保留。

### 低优先级候选

- `inline-actions__item`
  - 常与 `button`、`status-banner` 一起出现，更接近设计系统辅助 class。
  - 基础宽度规则已从 `entries.css` 回收到全局 `shell.css`；容器 `.inline-actions` 也已去掉通用 `margin-top`，间距改由各页面 `data-layout` 决定。
  - 若后续不需要跨主题重排，可继续保留，不必强行槽化。

### 已清理死样式

- `.entry-card__action`
  - Rust DOM 已统一使用 `data-slot="entry-card-action"`。
  - CSS 中的 `.entry-card__action` 已删除，保留 `[data-slot="entry-card-action"]`。

## 2026-04-11 深选择器复查

- 审查命令：
  - `rg -n "(^|[,{])[^{}]*(>|\\+|~)[^{}]*\\{" assets/styles assets/themes -S`
  - `rg -n "\\.[A-Za-z][A-Za-z0-9_-]+\\s+(h[1-6]|p|ul|ol|li|img|figure|button|span|div|input|textarea|select)\\b|\\]\\s+(h[1-6]|p|ul|ol|li|img|figure|button|span|div|input|textarea|select)\\b" assets/styles assets/themes -S`
- 结论：
  - `reader-html` 内容岛标签规则继续作为允许例外保留。
  - `atlas-sidebar` 中普通 page / reader page 的直接子布局规则继续保留，但入口必须优先使用 `data-layout` / `data-slot`。
  - 已将 `atlas-sidebar` 的 `.status-banner` 页面定位改为 `[data-layout="status-banner"]`。
  - 已将 `atlas-sidebar` 的 reader `.inline-actions` 页面定位改为 `[data-layout="reader-toolbar"]` / `[data-layout="reader-pagination"]`。

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
  - `.entry-card__action` -> 已删除死 selector
  - `.entry-card--reading + .entry-card--reading` -> `data-list-edge`
  - `.entry-card--reading:first-child/:last-child` -> `data-list-edge`
  - `.page-entries .reading-header` -> `.page-section-header--entries`

- 当前稳定接口：
  - `.page-title`
  - `.page-header__title`
  - `.page-header__actions`
  - `.page-section-header`
  - `.page-section-title`
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

### 主题作者 smoke review 结果

- fresh `debug/web/public` 构建已确认真实输出：
  - `data-layout`
  - `data-slot`
  - `data-page`
  - `data-nav`
  - `data-action`
  - `data-field`
  - `data-state`
  - `data-density`
- 通过浏览器注入最小 CSS 已确认：
  - `[data-layout="app-nav-shell"]`
  - `[data-layout="app-nav-links"]`
  - `[data-slot="app-nav-search-input"]`
  - `[data-page="settings"] [data-layout="settings-grid"]`
  这些公开接口已经足够驱动实际布局和主题覆写
- 静态审计原先暴露的内置主题旧 selector 依赖，现已完成第一轮收口：
  - `.app-nav*`
  - `.reader-page*`
  - `.entry-filters*`
  - `.button.secondary/.danger/.danger-outline`
  - `.theme-card.is-active`
  已从 `assets/themes/*.css` 清空

## P1：下一轮必须收掉

### 0. 内置主题资产继续收剩余旧口径

- 文件：
  - [atlas-sidebar.css](/home/develata/gitclone/RSS-Reader/assets/themes/atlas-sidebar.css)
  - [forest-desk.css](/home/develata/gitclone/RSS-Reader/assets/themes/forest-desk.css)
  - [midnight-ledger.css](/home/develata/gitclone/RSS-Reader/assets/themes/midnight-ledger.css)
  - [newsprint.css](/home/develata/gitclone/RSS-Reader/assets/themes/newsprint.css)
- 已完成：
  - 导航壳改成 `data-layout/data-nav`
  - 阅读壳改成 `data-layout/data-slot`
  - 按钮变体改成 `data-variant`
  - 主题卡激活态改成 `data-state`
- 下一步：
  - 继续判断是否还需要保留部分内部组件 class
  - 目前对应的公开 slot 已有：
    - `data-slot="feed-card-title"`
    - `data-slot="feed-card-meta"`
    - `data-slot="entry-card-title"`
    - `data-slot="entry-card-meta"`
    - `data-slot="page-intro"`
    - `data-slot="theme-card-title"`
    - `data-slot="theme-card-swatches"`
    - `data-slot="theme-card-swatch"`

### 1. 卡片头部剩余标签依赖

- 状态：已清理
- 处理结果：
  - `feed-workbench__note` 在页面层已经没有对应 DOM
  - 相关死 CSS 已从 [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css) 删除
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

### 3. 顶部标题区页面特化 class

- 文件：
  - [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
  - [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
- 状态：已收口
- 处理结果：
  - 页头统一使用 `data-layout="page-header"`
  - 页面标题统一使用 `data-slot="page-title"`
  - 页面页头差异统一使用 `data-slot="page-section-header"` + `data-section="entries|feeds|settings"`
  - CSS 不再依赖 `.page-title`、`.reading-header`、`.page-section-header--*`、`.page-header__title`

### 4. 布局容器仍主要靠 class 组合命名

- 文件：
  - [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)
  - [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
  - [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
  - [responsive.css](/home/develata/gitclone/RSS-Reader/assets/styles/responsive.css)
- 当前已迁移：
  - `page` -> `[data-page]:not([data-page="reader"])`
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
  - `entry-controls-*` -> `data-layout="entry-controls-*"` / `data-slot="entry-controls-toggle-chevron"`
  - `entry-overview*` -> `data-layout="entry-overview*"` / `data-slot="entry-overview-*"`
  - `entry-filters__source-chip` -> `data-layout="entry-filters-source-chip"`
- 下一步：
  - 继续减少只靠 class 命名表达布局角色的规则
  - 优先复查新增页面 wrapper 是否错误复用设计系统 class 做布局锚点，而不是继续迁移已定性的设计系统 class
  - 特别避免把页面间距、分栏、定位语义塞回 `.inline-actions` / `.button` 这类全局 class

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
  - `.entry-groups`
  - `.entries-layout`
  - `.settings-grid`
  - `.exchange-grid`
- 判断：
  - `entries-main` / `entries-page__*` 已迁到 `data-layout="entries-main"` / `entries-page-backlink` / `entries-page-state`，不再属于残留 page-local class 入口。
  - `entry-groups` / `entries-layout` / `settings-grid` / `exchange-grid` 已有 `data-layout`，CSS 不应再以 class 作为主入口。
  - 后续若要做到更极端的 CSS 重排，再补业务槽，不要预先暴露全部内部 wrapper。

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
  - `data-page`
  - `data-layout`
  - `data-slot`
  - `data-group-level`
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

1. 暂停对 `button` / `field-label` / `inline-actions__item` 这类设计系统 class 的机械迁移，除非有明确页面语义缺口。
2. 后续只处理新增死样式、深 DOM 层级 selector，或确实需要由主题作者重排的页面业务槽。
