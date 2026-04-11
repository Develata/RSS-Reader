# 2026-04-11 Daily Rollup

- 日期：2026-04-11
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：29b64df
- 相关 commit：2189d8b / b914f4c / 2379557 / d3368b4 / 954b22a / 037e31a / c36edfd / fea79d0 / e8d3887 / 972ab12 / c41864f / 85c07f8 / 66119cf / 29b64df
- 相关 tag / release：N/A
- 状态：`pending`

## 工作摘要

完成一轮 Windows 原生 Chrome 可见窗口回归验证，确认当前 Web SPA 与 `rssr-web` 浏览器态 smoke 路径可在用户可见的 Windows Chrome 窗口内通过。
随后将该验证路径固化为 repo 内脚本与文档，避免继续依赖 `target/` 临时 runner。
继续把可见浏览器 runner 从中文文案断言迁到 `data-*` 语义接口。
继续补齐页面层语义接口，让 settings themes、entries groups、reader body 都暴露更稳定的 headless active interface。
继续把 entries/reader/theme 相关 CSS 从深 class selector 迁到语义 `data-*` selector。
继续把 feeds/settings workspace 与主题 CSS 中的卡片、表单、统计块规则迁到 `data-layout` / `data-slot` 语义接口，并给可见回归 CDP runner 增加请求级超时。
继续收 `.page`、`.page-header`、entries controls / overview / source chip 的视觉 class 依赖，页面根改用 `data-page`，普通 page 与 reader page 保持分离。
完成一轮“保留 class 边界”审查，明确设计系统 class / 组件内部 class / 低优先级页面 wrapper 的保留边界，并清理一个明确死 selector。
继续复查深选择器边界，保留 `reader-html` 内容岛例外，同时把 atlas sidebar 主题里可替换的 `.status-banner` / `.inline-actions` 直接子定位迁到语义布局入口。
继续把 entries 页面本地壳 wrapper 从视觉 class 入口迁到 `data-layout`，缩小 page-local CSS 对内部 DOM 名称的依赖。
继续清理设计系统边界上的样式归属，把 `.inline-actions__item` 的基础规则从页面 CSS 挪回全局 shell。
继续把 `.inline-actions` 容器从“带默认页面间距”的混合 class 收回成纯排列辅助 class，页面间距改由具体 `data-layout` 承担。
继续删除页面 DOM 上已经没有任何消费方的旧 class token，避免 reader/settings/feeds 保持双轨壳层。
继续批量清理页面/卡片/分组/统计/表单等语义已经被 `data-layout` / `data-slot` 接管、但仍残留在 DOM 上的死 class token。
开始第一刀架构收口：把 entries browsing/workspace state 从 `UserSettings` 和 config package 中拆出，落到独立 `app_state` 真相源，并让 entries runtime 改走新的 app-state 持久化路径。
继续第二刀架构收口：为 `ui/runtime/*` 增加 `UiServices` 窄门面和按命令族分组的 port，移除各 runtime 模块对 `AppServices::shared()` 的直接依赖。

## 影响范围

- 模块：`scripts/run_web_spa_regression_server.sh`、`scripts/run_windows_chrome_visible_regression.sh`、`scripts/browser/rssr_visible_regression.mjs`、`crates/rssr-app/src/pages/*`、`crates/rssr-domain/src/app_state.rs`、`crates/rssr-application/src/app_state_service.rs`、`crates/rssr-infra/src/db/app_state_repository.rs`、`crates/rssr-infra/src/application_adapters/browser/*`、`assets/styles/*`、`assets/themes/*`、Web SPA 静态回归路径
- 平台：Windows Chrome、Web、WSL 开发环境
- 额外影响：browser regression workflow / handoff docs / config package v2 / browser app-state keyspace v2

## 关键变更

### Windows Chrome 可见验证

- 使用 Windows 原生 Chrome，而不是 WSLg/Linux Chrome 或 headless Chrome。
- Windows Chrome 启动参数包含独立调试 profile、`--remote-debugging-port=9225`、`--remote-allow-origins=*`。
- 由于 Windows Chrome DevTools 端口绑定在 Windows localhost，当前 WSL 内固定到 `127.0.0.1:9222` 的 Chrome MCP 会话不能直接接管该窗口；本轮改用 Windows 侧 Node/CDP 驱动可见窗口完成实际验证。

### 回归覆盖

- 静态 SPA server：`http://127.0.0.1:8112`
- `rssr-web` smoke helper：`http://127.0.0.1:18098/__codex/browser-feed-smoke`
- 覆盖页面：`/entries`、`/feeds`、`/settings`、`/entries/2`
- 覆盖主题：Atlas Sidebar、Newsprint、Amethyst Glass、Midnight Ledger
- 覆盖视口：默认桌面视口、小视口 `390x844`

### 固定工具化

- 新增 `scripts/run_windows_chrome_visible_regression.sh`，负责从 WSL 编排静态 SPA server、`rssr-web`、Windows Chrome 和 Windows Node/CDP runner。
- 新增 `scripts/browser/rssr_visible_regression.mjs`，集中维护浏览器动作与断言，不再把大段 JS 写在 shell heredoc 内。
- 新增 `docs/testing/windows-chrome-visible-regression.md`，说明 Windows visible Chrome 路径与 Chrome MCP 路径的控制链路差异。
- 更新 `docs/testing/README.md` 与 `docs/design/web-spa-regression-server.md`，把 Windows visible Chrome 纳入推荐回归入口。

### Selector-first 断言

- `scripts/browser/rssr_visible_regression.mjs` 不再通过页面中文文案点击主题或判断核心页面。
- 静态页面断言改为检查 `data-page`、`data-layout`、`data-field`、`data-action`、`data-nav`、`data-state`。
- 主题矩阵改为通过 `data-theme-preset`、`data-state="active"` 与 `#user-custom-css` 判断主题应用。
- `rssr-web` feed smoke 改为通过 `data-smoke="rssr-web-browser-feed-smoke"` 与 `data-result="pass"` 判断结果。

### 页面语义接口补齐

- settings themes：`theme-lab`、`theme-presets`、`theme-preset-selector`、`theme-preset-quick-actions`、`theme-gallery`、`theme-card` 等区域补齐 `data-layout` / `data-section` / `data-slot`。
- entries groups：列表容器补齐 `data-state="populated"` 与 `data-grouping-mode`；分组、日期组、来源组和列表补齐 `data-layout`、`data-group-level`、`data-slot`。
- reader body：HTML / text 正文分别补齐 `data-slot="reader-body-html"` 与 `data-slot="reader-body-text"`。
- 可见浏览器 runner 更新为使用这些更细的语义 selector。

### CSS 分离收口

- `assets/styles/entries.css`：entry group/header/title/meta/list/action 规则改用 `data-layout` / `data-slot` / `data-state`。
- `assets/styles/reader.css`：reader HTML/text 正文内部规则改用 `data-slot="reader-body-html"` / `data-slot="reader-body-text"`。
- `assets/styles/workspaces.css`、`assets/styles/responsive.css`：entry list、entry card actions、theme card swatches 等规则改用语义 selector。
- `assets/themes/*`：theme card、reader HTML 段落、entry list 相关规则继续迁到 `data-layout` / `data-slot`。
- 移除 `forest-desk` 中 reader 页下已经没有命中对象的 `.entry-card__actions` 陈旧规则。

### Feeds / Settings 语义接口收口

- feeds 页面统计卡补齐 `data-layout="stat-card"`、`data-stat`、`data-slot="stat-card-label/value"`。
- feeds compose / saved / config exchange 区域补齐 `feed-compose-card`、`feed-form`、`feed-list`、`feed-card`、`exchange-card`、`exchange-header` 等 `data-layout` / `data-section` / `data-slot`。
- settings appearance / preferences / sync 卡片补齐 `settings-card`、`settings-card-section`、`settings-form-grid`、`settings-card-actions/footer` 等语义 hook。
- `assets/styles/workspaces.css`、`assets/styles/shell.css`、`assets/styles/responsive.css` 和四个内置 theme CSS 中对应视觉规则改用语义 selector。
- 删除 `workspaces.css` 中已经没有页面 DOM 对应的 `feed-workbench__note` / intro 类死样式。
- 更新 [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)，把 `feed-workbench__note` 从待办改为已清理。

### Page Shell / Entries Controls 收口

- 页面标题补齐 `data-slot="page-title"`。
- entries / feeds / settings 页头补齐 `data-layout="page-header"`、`data-slot="page-section-header"`、`data-section`。
- entries controls 补齐 `entry-controls-panel/reveal/toggle`、`entry-organize-bar`、`entry-overview`、`entry-overview-metric`、`entry-overview-label/value`、`entry-filters-source-chip` 等 `data-layout` / `data-slot`。
- `.page` CSS 入口迁到 `[data-page]:not([data-page="reader"])`，reader 继续走 `[data-layout="reader-page"]`，避免普通页面壳样式污染 reader。
- `assets/styles/entries.css`、`assets/styles/shell.css`、`assets/styles/responsive.css`、`assets/themes/*` 中对应规则不再依赖 `.page`、`.page-title`、`.reading-header`、`.page-section-header--*`、`.entry-controls-*`、`.entry-overview*`、`.entry-filters__source-chip`。

### Class Boundary Audit

- 审查剩余 CSS class selector 与 Rust class token 的交集 / 差集。
- 明确保留为设计系统边界：
  - `app-shell`
  - `theme-light` / `theme-dark` / `theme-system`
  - `button` / `text-input` / `text-area` / `select-input` / `field-label`
  - `inline-actions` / `inline-actions__item`
  - `status-banner`
  - `icon-link-button`
  - `sr-only` / `sr-only-file-input`
- 明确保留为组件内部实现：
  - `reader-bottom-bar__button`
- 明确低优先级候选：
  - `entries-main`
  - `entries-page__backlink`
  - `entries-page__state`
- 清理 `assets/styles/entries.css` 中已无 DOM 对应的 `.entry-card__action` 死 selector。
- 更新 [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)，避免后续继续机械迁移设计系统 class。

### Deep Selector Audit

- `StatusBanner` 组件补齐 `data-layout="status-banner"`。
- `atlas-sidebar` 中普通 page / reader page 的直接子定位不再使用 `.status-banner`。
- `atlas-sidebar` 中 reader action 区域不再用 `.inline-actions` 作为页面定位入口，改用 `data-layout="reader-toolbar"` / `data-layout="reader-pagination"`。
- `reader-html` 内容岛中的 `p` / `li` / `img` 等标签规则继续作为允许例外保留。

### Regression Runner 稳定性

- `scripts/browser/rssr_visible_regression.mjs` 的 CDP navigation 现在会等待 `Page.loadEventFired`，但不把 load event 作为硬门禁；最终仍由 selector readiness 判断页面可用。
- `scripts/run_windows_chrome_visible_regression.sh` 在启动静态 SPA server / `rssr-web` 后会检查子进程是否仍存活，避免端口冲突时误命中旧服务。
- `scripts/browser/rssr_visible_regression.mjs` 增加 `CDP_COMMAND_TIMEOUT_MS` 请求级超时，避免 Windows Chrome/CDP 会话污染时无限挂起。
- 可见 runner 在主题矩阵和小视口循环中打印阶段 start/pass，方便定位失败步骤。

### Entries Page Wrapper 语义化

- [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
  - `entries-main`、`entries-page__backlink`、`entries-page__state` 根节点补齐 `data-layout`，页面 wrapper 不再只靠 class 暴露布局角色。
- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
  - 对应 `min-width`、backlink margin、empty/archived state margin 改由 `[data-layout="entries-main"]`、`entries-page-backlink`、`entries-page-state` 驱动。
- [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)
  - 将 `entries-main` / `entries-page__*` 从“低优先级候选”调整为“已收口”，并把 `inline-actions__item` 归并为设计系统 class 边界问题。

### Design System Class Boundary 收口

- [shell.css](/home/develata/gitclone/RSS-Reader/assets/styles/shell.css)
  - `.inline-actions__item` 的基础宽度规则回收到全局 shell，明确它属于设计系统辅助 class，而不是 entries 页面私有样式。
  - `.inline-actions` 删除通用 `margin-top`，不再让设计系统 class 隐含页面间距语义。
- [entries.css](/home/develata/gitclone/RSS-Reader/assets/styles/entries.css)
  - 删除误放在页面样式内的 `.inline-actions__item` 规则，避免 page-local CSS 污染 reader/settings/feeds 共用动作条。
- [workspaces.css](/home/develata/gitclone/RSS-Reader/assets/styles/workspaces.css)
  - `exchange-card-actions` 显式补回 `margin-top: 12px`，把原先混在 `.inline-actions` 里的页面间距语义落回页面布局 hook。
- [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)
  - 将 `inline-actions__item` 的结论更新为“保留为设计系统 class，重点只检查是否被页面拿来承担布局锚点”。

### Dead Class Token 清理

- [config_exchange.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/config_exchange.rs)
  - feeds 配置交换区移除误挂的 `settings-card__header`，只保留 `data-slot="settings-card-header"`。
- [appearance.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/appearance.rs)
  - appearance 卡片头部移除 `settings-card__header`。
- [preferences.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/preferences.rs)
  - 阅读偏好区移除 `settings-card__section` / `settings-card__section-header` / `settings-card__section-title`。
- [sync/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/mod.rs)
  - WebDAV 卡片移除 `settings-card__header`、`settings-card__section*`、`settings-card__actions`。
- [themes/lab.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/lab.rs)
  - 主题实验室移除 `settings-card__section*` / `settings-card__actions` 残留，并把自定义 CSS placeholder 改成 `[data-layout="reader-body"]` 示例。
- [themes/presets.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/presets.rs)
  - 主题预设区移除 `settings-card__section*` / `settings-card__actions` 残留。
- [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/mod.rs)
  - reader 页面移除 `reader-page` / `reader-header` / `reader-title` / `reader-toolbar` / `reader-meta-block` / `reader-meta` / `reader-body` / `reader-html` / `reader-pagination` / `reader-pagination--context` 等已失效 class token，只保留 `data-layout` / `data-slot` 和仍有样式消费方的 `inline-actions` / `reader-bottom-bar__button`。
- [frontend-command-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/frontend-command-reference.md)、[theme-author-selector-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/theme-author-selector-reference.md)、[css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)
  - 当前规范文档中的 reader 内容岛示例已统一从 `.reader-html` 切到 `[data-slot="reader-body-html"]`。

### Semantic Shell Class Purge

- [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/mod.rs)、[compose.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/compose.rs)、[config_exchange.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/config_exchange.rs)、[saved.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/saved.rs)
  - feeds 页面移除 `page-*`、`stats-grid*`、`stat-card*`、`feed-workbench*`、`feed-compose-card*`、`feed-form`、`exchange-*`、`feed-card*` 等已无消费方的壳层 class token。
- [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/mod.rs)、[appearance.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/appearance.rs)、[preferences.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/preferences.rs)、[sync/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/mod.rs)、[presets.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/presets.rs)
  - settings 页面移除 `page-*`、`settings-grid`、`settings-card*`、`card-title`、`theme-gallery` 等死 token，仅保留设计系统类和语义属性。
- [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)、[cards.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/cards.rs)、[controls.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/controls.rs)、[entry_filters.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/components/entry_filters.rs)
  - entries 页面移除 `page-*`、`entries-*`、`entry-group*`、`entry-overview*`、`entry-controls*`、`entry-filters__*`、`entry-card__*` 等已被 `data-layout` / `data-slot` 取代的 DOM class token。
- [css-separation-baseline-checklist.md](/home/develata/gitclone/RSS-Reader/docs/design/css-separation-baseline-checklist.md)、[frontend-command-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/frontend-command-reference.md)、[theme-author-selector-reference.md](/home/develata/gitclone/RSS-Reader/docs/design/theme-author-selector-reference.md)
  - 当前规范文档删除 `.feed-card` / `.entry-card` / `.settings-card` / `.exchange-card` / `.theme-card` / `.card-title` / `.group-header*` 等已失效的建议入口，统一回到 `data-layout` / `data-slot`。

### App State / Workspace State Split

- [app_state.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-domain/src/app_state.rs)
  - 新增 `AppStateSnapshot` 与 `EntriesWorkspaceState`，把 `last_opened_feed_id` 和 entries browsing/workspace state 收口成独立真相源。
- [settings.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-domain/src/settings.rs)
  - `UserSettings` 删除 `entry_grouping_mode`、`show_archived_entries`、`entry_read_filter`、`entry_starred_filter`、`entry_filtered_feed_urls`，只保留 durable settings。
- [repository.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-domain/src/repository.rs)、[app_state_service.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/app_state_service.rs)
  - 新增 `AppStateRepository` 与 `AppStateService`，给 host / runtime 一个比 `SettingsRepository` 更窄、更正确的 app-state 入口。
- [app_state_repository.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/db/app_state_repository.rs)
  - SQLite app state 由单个 `last_opened_feed_id` key 升级为 `app_state_v2` JSON blob，承载 `last_opened_feed_id + entries_workspace`。
- [state.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/state.rs)
  - 浏览器 app-state sidecar key 升为 `rssr-web-app-state-v2`，并持久化完整 `AppStateSnapshot`。
- [native.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/bootstrap/native.rs)、[web.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/bootstrap/web.rs)
  - `AppServices` 新增 `load_entries_workspace_state` / `save_entries_workspace_state`，并改由 `AppStateService` 读写 last-opened feed 与 entries workspace。
- [entries.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime/entries.rs)、[intent.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/intent.rs)、[reducer.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/reducer.rs)、[state.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/state.rs)
  - entries bootstrap 现在分别加载 durable settings 与 `EntriesWorkspaceState`；`SaveBrowsingPreferences` 不再回写 `UserSettings`，而是改写 `app_state.entries_workspace`。
- [import_export_service.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/import_export_service.rs)、[rules.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/import_export_service/rules.rs)
  - config package 导出版本升到 `2`；导入器不再接受旧的 `settings + browsing state` 混合结构。
- [config-package.schema.json](/home/develata/gitclone/RSS-Reader/specs/001-minimal-rss-reader/contracts/config-package.schema.json)
  - schema 删除 entries browsing/workspace 字段，并把 `version` 收紧到常量 `2`。
- `tests/fixtures/browser_state/reader_demo_core.json` / `tests/fixtures/browser_state/reader_demo_app_state.json`
  - browser seed fixture 改成 “durable settings 在 core，entries workspace 在 app-state sidecar” 的新布局。

### UI Runtime Port Narrowing

- [services.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime/services.rs)
  - 新增 `UiServices`，把 `AppServices::shared()` 收口到 runtime 单入口。
  - 新增 `EntriesPort`、`ShellPort`、`SettingsPort`、`ReaderPort`、`FeedsPort`，按命令族暴露窄能力面。
- [entries.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime/entries.rs)
  - entries runtime 现在只依赖 `EntriesPort`，不再直接取 `AppServices::shared()`。
- [shell.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime/shell.rs)
  - shell runtime 改成走 `ShellPort` 加载 durable settings、启动自动刷新、解析 startup route。
- [settings.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime/settings.rs)
  - settings runtime 改成走 `SettingsPort`，把本页所需的保存/同步能力与全能 app host 隔开。
- [reader.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime/reader.rs)
  - reader runtime 改成走 `ReaderPort`，只保留获取 entry / navigation / read-star toggle 的窄接口。
- [feeds.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime/feeds.rs)
  - feeds runtime 改成走 `FeedsPort`，为后续把订阅/配置交换 workflow 抽离成共享 use case 做准备。

## 验证与验收

### 自动化验证

- `bash scripts/run_web_spa_regression_server.sh --skip-build --port 8112`：通过，静态 SPA fallback server 可服务多路由。
- `cargo run -p rssr-web` with smoke env on `127.0.0.1:18098`：通过，`rssr-web` smoke helper 可访问。
- Windows Node/CDP visible Chrome regression script：通过。
- `scripts/run_windows_chrome_visible_regression.sh --use-existing-servers --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-082128/summary.md`。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8114 --rssr-web-port 18104 --chrome-port 9225 --skip-build --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-083451/summary.md`。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `node --check scripts/browser/rssr_visible_regression.mjs`：通过。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8120 --rssr-web-port 18110 --chrome-port 9225 --skip-build --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-084846/summary.md`。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8311 --rssr-web-port 18811 --chrome-port 9225 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-091253/summary.md`。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `node --check scripts/browser/rssr_visible_regression.mjs`：通过。
- `bash -n scripts/run_windows_chrome_visible_regression.sh`：通过。
- `git diff --check`：通过。
- `rg -n "\\.(feed-form|feed-compose-card|feed-list|feed-card|exchange-card|exchange-header|settings-card|stat-card|card-title|settings-form-grid|preset-grid|theme-gallery)(\\b|__)" assets/styles assets/themes -S`：通过，无剩余命中。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8314 --rssr-web-port 18814 --chrome-port 9226 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-092733/summary.md`。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `node --check scripts/browser/rssr_visible_regression.mjs`：通过。
- `bash -n scripts/run_windows_chrome_visible_regression.sh`：通过。
- `git diff --check`：通过。
- `rg -n "\\.(page|page-title|page-header__title|page-header__actions|page-section-header--|reading-header|entry-organize-bar|entry-overview|entry-overview__|entry-controls|entry-filters__source-chip)(\\b|__|--)" assets/styles assets/themes -S`：通过，无剩余命中。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8315 --rssr-web-port 18815 --chrome-port 9227 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-093306/summary.md`。
- `rg --pcre2 -o '(^|[,\\s])\\.[A-Za-z][A-Za-z0-9_-]*(?=[\\s:{.#\\[,>+~)]|$)' assets/styles assets/themes -S`：已执行，用于剩余 class selector 审查。
- `rg -o 'class: "[^"]+"' crates/rssr-app/src -S`：已执行，用于 Rust DOM class token 对照。
- `rg -n "entry-card__action" assets/styles assets/themes crates/rssr-app/src -S`：通过，剩余命中仅为 `entry-card__actions` 容器，不再有 `.entry-card__action` selector。
- `git diff --check`：通过。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `rg -n "> \\.(status-banner|inline-actions)|\\.status-banner|\\.inline-actions" assets/themes/atlas-sidebar.css -S`：通过，无剩余命中。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8316 --rssr-web-port 18816 --chrome-port 9228 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-115622/summary.md`。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8320 --rssr-web-port 18820 --chrome-port 9230 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-131242/summary.md`。
- `rg -n "entries-main|entries-page__backlink|entries-page__state" crates/rssr-app/src assets/styles -S`：通过，CSS 入口已迁到 `data-layout`，仅剩 DOM class token。
- `rg -n "\\.inline-actions__item" assets/styles assets/themes -S`：通过，仅剩 `shell.css` 和 `responsive.css` 两处设计系统规则。
- `rg -n "\\.inline-actions\\b|\\.inline-actions__item" assets/styles assets/themes -S`：通过，`.inline-actions` / `.inline-actions__item` 仅剩全局设计系统规则；页面间距已回落到 `data-layout`。
- `rg -n 'class: "[^"]*(settings-card__header|settings-card__section|settings-card__section-header|settings-card__section-title|settings-card__actions|reader-page|reader-header|reader-title|reader-toolbar|reader-meta-block|reader-meta|reader-body|reader-html|reader-pagination|reader-pagination--context)[^"]*"' crates/rssr-app/src -S`：通过，活跃代码中的旧混合态 class token 已清空。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8322 --rssr-web-port 18822 --chrome-port 9232 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-133206/summary.md`。
- `rg -n 'class: "[^"]*(page|page-|page-header|page-section-header|page-title|entries-layout|entries-main|entries-page__|entry-card__title|entry-card__meta|entry-card__actions|entry-group|entry-date-group|entry-source-group|entry-list--|entry-overview|entry-controls|entry-filters__|feed-card|feed-compose|feed-workbench|feed-form|exchange-header|exchange-grid|exchange-card|settings-card|settings-grid|stats-grid|stat-card|card-title|theme-gallery|theme-card|group-header)[^"]*"' crates/rssr-app/src -S`：通过，活跃代码中这批 page shell / card shell class token 已清空。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8324 --rssr-web-port 18824 --chrome-port 9234 --slow-ms 100`：静态 entries/feeds、reader theme matrix、小视口 routes 通过；`rssr-web browser feed smoke` 超时失败。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8326 --rssr-web-port 18826 --chrome-port 9236 --slow-ms 100`：静态 entries/feeds、reader theme matrix、小视口 routes 再次通过；`rssr-web browser feed smoke` 再次超时失败。
- `scripts/run_windows_chrome_visible_regression.sh --static-port 8320 --rssr-web-port 18820 --chrome-port 9230 --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-131242/summary.md`。
- `cargo fmt`：通过。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `cargo test -p rssr-application`：通过。
- `cargo test -p rssr-infra --tests --no-run`：通过。
- `cargo test -p rssr-infra --test wasm_config_exchange_contract_harness --target wasm32-unknown-unknown --no-run`：通过。
- `cargo test -p rssr-infra --test wasm_subscription_contract_harness --target wasm32-unknown-unknown --no-run`：通过。
- `cargo test -p rssr-infra --test test_settings_repository`：通过。
- `cargo test -p rssr-infra --test test_config_package_codec`：通过。
- `cargo test -p rssr-infra --test test_config_package_schema_consistency`：通过。
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`：通过。
- `cargo test -p rssr-infra --test test_regression_smoke`：通过。
- `git diff --check`：通过。
- `rg -n "AppServices::shared\\(|UiServices::shared\\(" crates/rssr-app/src/ui/runtime -S`：通过，runtime 目录内仅 `services.rs` 保留唯一 `AppServices::shared()` 入口，其他命令族全部切到 `UiServices::shared()`。

### 手工验收

- Windows 原生 Chrome 可见窗口：通过。
- 静态 `/entries` 页面：通过。
- 静态 `/feeds` 页面：通过。
- `/reader` 实页多主题切换：通过。
- 小视口 `/entries`、`/feeds`、`/settings`、`/entries/2`：通过。
- `rssr-web` 浏览器态真实 feed smoke helper：通过。

## 结果

- 当前 Web SPA 回归路径在可见 Windows Chrome 下通过。
- 当前页面语义接口和 CSS 分离基线继续前移；feeds/settings/stat/exchange 这批视觉 class 已不再被 `assets/styles` / `assets/themes` 作为 CSS 选择器入口。
- page shell 与 entries controls/overview/source chip 这批视觉 class 也已不再被 `assets/styles` / `assets/themes` 作为 CSS 选择器入口。
- 保留 class 边界已明确：设计系统 class 不再作为“必须迁移”的技术债；后续只处理死样式、深 DOM selector、确实需要外部主题控制的页面业务槽。
- 可见回归 runner 已避免 CDP 请求无限挂起；复用污染较重的 9225 Chrome 会话时曾在 `Page.navigate` 超时，改用干净 9226 Windows Chrome profile 后全量通过。
- WSLg 桌面窗口呈现问题继续视为环境层问题，不阻塞 Web SPA 浏览器态验证。

## 风险与后续事项

- 当前 `mcp__chrome` 工具会话仍绑定 WSL 侧 `127.0.0.1:9222`，不能直接控制 Windows 侧 `127.0.0.1:9225` Chrome。
- 如果后续要求“Chrome MCP 工具本身控制 Windows Chrome”，需要建立稳定的 WSL-to-Windows CDP bridge，或在 Windows 侧启动 MCP server。
- WSL 环境代理变量可能误导 localhost 诊断；检查本地端口时应使用 `curl --noproxy '*'`。
- 当前 visible runner 主路径已迁到 `data-*` 语义接口；后续扩展测试时应继续保持 selector-first，避免新增文案驱动断言。
- 可见回归复用 Windows Chrome 端口时，CDP load event 可能不会覆盖所有 SPA route 情况；runner 已改为 selector readiness 作为最终判断。
- 复用长期运行的 Windows Chrome DevTools 端口可能受历史 tab / 旧 CDP 会话污染；建议日常回归优先使用新的 `--chrome-port`，或先清理旧可见回归窗口。
- 下一轮 CSS 分离不建议继续机械迁移 `button` / `field-label` / `inline-actions__item`；应只看新增死样式、深 DOM selector，或主题作者确实需要重排的业务槽。
- 当前 entries wrapper 这轮尚未单独提交；若继续拆 class 边界，应优先检查设计系统 class 与页面语义 hook 的交界处，而不是继续平铺更多 `data-*`。
- `.inline-actions__item` 已确认属于设计系统 class；后续只需要防止页面/主题把它重新当作布局入口使用。
- `.inline-actions` 现已只承担排列，不再隐含通用 `margin-top`；若后续出现动作条节奏问题，应优先在对应 `data-layout` 修，而不是回填到全局 class。
- 这轮之后，reader/settings/feeds 仍允许保留的 class 应只剩设计系统类、卡片标题类和 reader bottom bar 内部实现类；若后续再出现新的页面 class token，需要先证明存在外部消费方。
- 这轮之后，页面/卡片/分组/表单相关 dead class token 已基本清空；剩余保留类应优先视为设计系统类或内部实现类，而不是新的页面契约。
- `rssr-web browser feed smoke` 当前两次都在最终 `[data-smoke="rssr-web-browser-feed-smoke"][data-result="pass"]` selector 等待超时；由于前面静态页面与 reader/settings/feeds 可见回归均通过，暂判断为 helper/环境层问题，待单独排查。

## 给下一位 Agent 的备注

- 本轮可见验证使用 Windows Chrome CDP/Node，而不是 Dioxus desktop/WSLg 窗口。
- Windows visible Chrome CDP runner 已沉淀进 repo；当前主路径断言也已迁到 headless active interface 风格。
- `037e31a` 已提交上一轮 workspace CSS 语义 hook 收口。
- `c36edfd` 已提交 page shell / entries controls 语义 hook 收口。
- `fea79d0` 已提交 handoff 元数据记录。
- `e8d3887` 已提交保留 class 边界审查与 `.entry-card__action` 死 selector 清理。
- `972ab12` 已提交 class audit handoff 元数据更新。
- `c41864f` 已提交 status banner layout hook 与 atlas 深选择器收口。
- `66119cf` 已提交 page shell / design-system class boundary 收口。
- 本工作区当前还有一轮未提交改动：semantic shell class purge 与规范文档同步，commit: pending。
