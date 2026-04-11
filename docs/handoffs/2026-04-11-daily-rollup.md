# 2026-04-11 Daily Rollup

- 日期：2026-04-11
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：412631b
- 相关 commit：2189d8b / b914f4c / 2379557 / d3368b4 / 954b22a / 037e31a / c36edfd / fea79d0 / e8d3887 / 972ab12 / c41864f / 85c07f8 / 66119cf / 29b64df / 882a764 / 62a165e / 9202355 / 3ce6eb8 / 7140fd1 / 9460c4a / 58eaaf2 / 412631b
- 相关 tag / release：N/A
- 状态：`validated`

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
继续第三刀架构收口：把应用层服务装配下移成共享 `AppUseCases::compose(...)`，让 native / web / cli 共同复用同一套 use-case 组合骨架。
继续第四刀架构收口：让 `UiServices` 直接持有 `AppUseCases`，把 `AppServices` 再压回 host facade，仅保留自动刷新、刷新后处理和远端 config sync 这类 host 特有行为。
继续第五刀架构收口：把 host 特有行为显式拆成 capability，对 runtime 暴露 `auto_refresh`、`refresh`、`remote_config`，并把 native 正文图片本地化后台任务收成独立 worker。
继续第六刀架构收口：让 `UiServices` 只缓存 `use_cases + capability`，不再长期持有整块 `Arc<AppServices>`。
继续第七刀架构收口：把 runtime 依赖的 capability 进一步收成 `HostCapabilities` trait-object bundle，让 bootstrap 不再向 runtime 暴露具体 capability 实现类型。
完成 `.specify` 宪章 1.3.0 后的完整状态对齐、基线验证与 push 尝试；push 被 GitHub HTTPS 凭据阻止，代码侧无失败。
按新宪章补充 application use case 收敛架构计划，并落地第一刀：订阅生命周期由 `rssr-application::SubscriptionWorkflow` 统一承接，CLI/native/web 不再各自重组“添加后是否首次刷新”的业务分支。

## 影响范围

- 模块：`scripts/run_web_spa_regression_server.sh`、`scripts/run_windows_chrome_visible_regression.sh`、`scripts/browser/rssr_visible_regression.mjs`、`crates/rssr-app/src/pages/*`、`crates/rssr-app/src/ui/runtime/services.rs`、`crates/rssr-app/src/bootstrap/*`、`crates/rssr-domain/src/app_state.rs`、`crates/rssr-application/src/app_state_service.rs`、`crates/rssr-application/src/composition.rs`、`crates/rssr-infra/src/db/app_state_repository.rs`、`crates/rssr-infra/src/application_adapters/browser/*`、`assets/styles/*`、`assets/themes/*`、Web SPA 静态回归路径
- 平台：Windows Chrome、Web、WSL 开发环境
- 额外影响：browser regression workflow / handoff docs / config package v2 / browser app-state keyspace v2
- 额外影响：application use case 收敛计划 / CLI 添加订阅流程 / native-web 添加订阅后首次刷新流程 / pre-push 验证记录

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

### Shared Composition Primitive

- `crates/rssr-application/src/composition.rs` 新增 `AppUseCases::compose(...)`、`AppCompositionInput`、`AppStateServicesPort`，把 feed / entry / settings / app-state / refresh / subscription / import-export 的装配统一下沉到应用层。
- `native`、`web`、`cli` 现在都只负责选择 adapter 和 capability，再调用 `AppUseCases::compose(...)`，不再各自平行 new 一套主流程骨架。
- `SqliteAppStateAdapter` 同时实现 `AppStateRepository`，使 app-state 在 composition 层能作为单一依赖注入。

### Host Facade 收薄

- `crates/rssr-app/src/ui/runtime/services.rs` 现在直接持有 `AppUseCases`，entries / reader / feeds / settings / shell 的大多数操作直接走 use-case。
- `AppServices` 不再承担第二套通用 service API；目前只保留 `shared()`、`default_settings()`、`use_cases()`，以及自动刷新、刷新后图片本地化、远端 config sync 这类 host 特有行为。
- web 端 `bootstrap/web/exchange.rs` 只保留 push/pull remote config helper；纯导入导出已回到 `ImportExportService` 直接调用。

### Host Capability 显式化

- native / web 的 `AppServices` 现在显式暴露 `auto_refresh()`、`refresh()`、`remote_config()` capability，而不是再挂一组混合 service 方法。
- `crates/rssr-app/src/ui/runtime/services.rs` 已切到这些 capability：shell 只拿 `auto_refresh`，settings 只拿 `remote_config`，feeds 只拿 `refresh`。
- `crates/rssr-app/src/bootstrap/native.rs` 新增 `ImageLocalizationWorker`，把刷新后的正文图片本地化后台任务从 host facade 主体里抽离。
- `crates/rssr-app/src/bootstrap/web/refresh.rs` 已改为通过 `refresh()` capability 触发自动刷新，不再直接调 `AppServices::refresh_all()`。

### Runtime 持有面继续压薄

- `crates/rssr-app/src/bootstrap.rs` 现在对 runtime 侧只暴露统一的 `HostCapabilities` bundle 和对应 trait 接口，而不再暴露具体 capability 类型。
- `crates/rssr-app/src/ui/runtime/services.rs` 不再缓存 `Arc<AppServices>`；`UiServices::shared()` 只在初始化时取一次 host，然后缓存 `AppUseCases + capability`。
- page runtime 现在拿到的是明确的 host 能力对象，而不是整块 host facade，有利于下一步把 capability 接口进一步上提到 host adapter 层。

### Host Capability Bundle

- `crates/rssr-app/src/bootstrap.rs` 新增 `HostCapabilities`、`AutoRefreshPort`、`RefreshPort`、`RemoteConfigPort`，把 runtime 可见的 host 能力收成 trait-object bundle。
- native / web bootstrap 现在只在各自模块内部保留具体 `AutoRefreshCapability` / `RefreshCapability` / `RemoteConfigCapability` 实现，并通过 `host_capabilities()` 返回统一 bundle。
- `crates/rssr-app/src/ui/runtime/services.rs` 现在只依赖 `HostCapabilities`，不再直接 import bootstrap 内部的具体 capability 类型。
- native 自动刷新后台任务仍在模块内部使用具体 `RefreshCapability`，避免把 `tokio::spawn` 路径改成非 `Send` trait object。

### Pre-push Baseline And Push Attempt

- `cargo test --workspace`：通过；包含 `test_webdav_local_roundtrip`。
- `cargo check -p rssr-app --target aarch64-linux-android`：通过。
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --skip-build --port 8336 --web-port 18836 --log-dir target/release-ui-regression/20260411-codex-mainline-prepush`：通过。
- `git push origin main`：失败，原因是当前环境没有 GitHub HTTPS 用户名/凭据，报错为 `fatal: could not read Username for 'https://github.com': No such device or address`。
- `gh`：未安装，无法改用 GitHub CLI 认证路径。

### Application Use Case Consolidation Plan

- 新增 [application-use-case-consolidation-plan.md](/home/develata/gitclone/RSS-Reader/docs/design/application-use-case-consolidation-plan.md)。
- 计划按宪章 `1.3.0` 的顺序组织：先定义骨架边界，再列模块边界，再落到第一步实现逻辑。
- 收敛范围限定在当前 RSS 阅读器本体内的 subscription management / feed refresh / basic config exchange。
- 明确真相源：feeds 走 `FeedRepository`，entries/read-starred 走 `EntryRepository`，durable settings 走 `SettingsRepository`，workspace state 走 `AppStateRepository`，browser app-state keyspace 为 `rssr-web-app-state-v2`。
- 明确不下沉到 application 的 host 责任：auto-refresh loop、native image localization worker、WebDAV store construction、浏览器持久化细节和 UI 文案/呈现策略。
- 后续顺序建议：先收 refresh outcome summarization，再看 config exchange consolidation，最后补 contract/harness。

### Subscription Lifecycle Workflow

- [subscription_workflow.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/subscription_workflow.rs) 新增 `AddSubscriptionLifecycleInput` 与 `AddSubscriptionLifecycleOutcome`。
- `SubscriptionWorkflow::add_subscription_lifecycle(...)` 统一表达“添加订阅 + 可选首次刷新”。
- 兼容 helper `add_subscription(...)` 与 `add_subscription_and_refresh(...)` 继续保留，但内部委托给 lifecycle 方法，避免双主干。
- [native.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/bootstrap/native.rs) 与 [web.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/bootstrap/web.rs) 的 `RefreshCapability::add_subscription(...)` 改为调用 lifecycle 方法，host 只负责把首次刷新 outcome 翻译成现有用户可见结果。
- [main.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-cli/src/main.rs) 的 `add_subscription(...)` 改为把 `--skip-refresh` 映射为 `refresh_after_add`，首次刷新失败仍由 CLI 现有 exit/error 逻辑处理。
- `rssr-application` 新增直接覆盖 lifecycle skip-refresh 与 refresh-after-add 两个分支的测试。

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

### Shared Composition Primitive

- [composition.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/composition.rs)
  - 新增共享 `AppCompositionInput`、`AppStateServicesPort`、`AppUseCases::compose(...)`。
  - 组合入口统一负责装配 `FeedService`、`EntryService`、`SettingsService`、`AppStateService`、`RefreshService`、`SubscriptionWorkflow`、`ImportExportService`。
- [app_state_service.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/app_state_service.rs)、[entry_service.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/entry_service.rs)、[settings_service.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/settings_service.rs)、[import_export_service.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/import_export_service.rs)
  - 补齐 `Clone`，使组合后的应用层骨架可被 host facade 直接持有。
- [non_refresh.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/non_refresh.rs)
  - `SqliteAppStateAdapter` 现在同时实现 `AppStateRepository`，可作为单一 `app_state` 依赖传给组合入口。
- [native.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/bootstrap/native.rs)
  - 桌面端 bootstrap 不再直接手工拼 `FeedService` / `RefreshService` / `SubscriptionWorkflow` / `ImportExportService`，而是只负责选 SQLite / HTTP / parser / localizer 等 host adapter，然后调用 `AppUseCases::compose(...)`。
- [web.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/bootstrap/web.rs)
  - web bootstrap 同样改为只负责 browser state / reqwest client / browser refresh source / remote config store 等 host adapter 的选择，然后调用共享组合入口。
- [main.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-cli/src/main.rs)
  - CLI 初始化流程切到 `AppUseCases::compose(...)`，不再自己平行重建完整 service/workflow 装配链。

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
- `cargo check -p rssr-cli`：通过。
- `rg -n "AppUseCases::compose\\(|AppCompositionInput|AppStateServicesPort" crates/rssr-app crates/rssr-cli crates/rssr-application -S`：通过，native / web / cli 已共同接入共享 composition primitive。
- `cargo check -p rssr-app`：通过。
- `rg -n "self\\.inner\\.|AppServices::shared\\(|\\.use_cases\\(\\)" crates/rssr-app/src/ui/runtime crates/rssr-app/src/bootstrap -S`：通过，runtime 内只剩 `UiServices::shared()` 的单一 `AppServices::shared()` 入口，纯 use-case 读写已不再经由 `AppServices` 转发。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `rg -n "pub (async )?fn (ensure_auto_refresh_started|add_subscription|refresh_all|refresh_feed|push_remote_config|pull_remote_config)" crates/rssr-app/src/bootstrap crates/rssr-app/src/ui/runtime -S`：无命中，旧的 host 混合方法面已从 bootstrap/runtime 移除。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `rg -n "Arc<AppServices>|host: Arc<AppServices>|AppServices::shared\\(" crates/rssr-app/src/ui/runtime/services.rs crates/rssr-app/src/ui/runtime -S`：通过，`UiServices` 初始化时仅保留单一 `AppServices::shared()` 入口，后续 runtime port 已不再持有 `Arc<AppServices>`。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `rg -n "AutoRefreshCapability|RefreshCapability|RemoteConfigCapability|HostCapabilities|AppServices::shared\\(" crates/rssr-app/src/ui/runtime/services.rs crates/rssr-app/src/bootstrap.rs crates/rssr-app/src/bootstrap -S`：通过，runtime 已只依赖 `HostCapabilities` bundle；具体 capability 类型仅在 bootstrap 内部保留。

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
- `UiServices` 现在已处于“use cases + host capability”形态；`AppServices` 从全能 backend 退回到 host facade，但 native/web bootstrap 仍保留少量平台启动与后台任务责任，后续还可继续按 capability 拆薄。
- 当前 host facade 已不再暴露旧的混合 service 方法面；runtime 依赖已明确分成 use-case 读写与 host capability 两类入口。
- 当前 runtime 持有面已经进一步压到 `use_cases + capability`；剩余可继续抽象的部分主要在 capability 的共享接口层，而不是页面 runtime 本身。
- 当前 runtime 与 bootstrap 的接口面已进一步缩到 `AppUseCases + HostCapabilities`；页面层不再看到平台专属 capability 实现。

## 风险与后续事项

- 当前 `mcp__chrome` 工具会话仍绑定 WSL 侧 `127.0.0.1:9222`，不能直接控制 Windows 侧 `127.0.0.1:9225` Chrome。
- 如果后续要求“Chrome MCP 工具本身控制 Windows Chrome”，需要建立稳定的 WSL-to-Windows CDP bridge，或在 Windows 侧启动 MCP server。
- WSL 环境代理变量可能误导 localhost 诊断；检查本地端口时应使用 `curl --noproxy '*'`。
- 当前 visible runner 主路径已迁到 `data-*` 语义接口；后续扩展测试时应继续保持 selector-first，避免新增文案驱动断言。
- 可见回归复用 Windows Chrome 端口时，CDP load event 可能不会覆盖所有 SPA route 情况；runner 已改为 selector readiness 作为最终判断。
- 复用长期运行的 Windows Chrome DevTools 端口可能受历史 tab / 旧 CDP 会话污染；建议日常回归优先使用新的 `--chrome-port`，或先清理旧可见回归窗口。
- 当前 `AppServices` 仍同时承载 native/web host 启动、自动刷新调度与少量 capability bridge；下一步若继续架构线，应优先把这些能力显式拆成 host capability，而不是重新给 `AppServices` 加方法。
- native / web 目前的 capability 结构仍是平台内联定义；若继续推进，可再把 capability 共享接口抽到更明确的 host adapter 层，但不应重新收敛成一个新的全能 manager。
- `UiServices` 已切到统一 `HostCapabilities` bundle，不再依赖 native/web 具体 capability 类型；下一步若继续推进，重点应转到 bootstrap/native/web 本身的 host assembly 与 capability provider 抽取，而不是继续折腾 page runtime。
- 下一轮 CSS 分离不建议继续机械迁移 `button` / `field-label` / `inline-actions__item`；应只看新增死样式、深 DOM selector，或主题作者确实需要重排的业务槽。
- 当前 entries wrapper 这轮尚未单独提交；若继续拆 class 边界，应优先检查设计系统 class 与页面语义 hook 的交界处，而不是继续平铺更多 `data-*`。
- `.inline-actions__item` 已确认属于设计系统 class；后续只需要防止页面/主题把它重新当作布局入口使用。
- `.inline-actions` 现已只承担排列，不再隐含通用 `margin-top`；若后续出现动作条节奏问题，应优先在对应 `data-layout` 修，而不是回填到全局 class。
- 这轮之后，reader/settings/feeds 仍允许保留的 class 应只剩设计系统类、卡片标题类和 reader bottom bar 内部实现类；若后续再出现新的页面 class token，需要先证明存在外部消费方。
- 这轮之后，页面/卡片/分组/表单相关 dead class token 已基本清空；剩余保留类应优先视为设计系统类或内部实现类，而不是新的页面契约。
- `rssr-web browser feed smoke` 的旧 selector 超时已在本轮补充排查中修复；后续若再次超时，应先检查 helper 是否重新依赖页面 class 或旧 app-state key。

## 本轮补充：状态对齐、基线验证、smoke 排查、工程宪法评估

### 状态对齐

- 当前 HEAD 已从文档中的 `3ce6eb8` 对齐为 `7140fd1`。
- `7140fd1` 已提交 `UiServices` 从 `Arc<AppServices>` 压到 `use_cases + HostCapabilities` 的改动。
- 本轮补充修改尚未提交，commit: pending。

### 基线验证

- `cargo fmt --check`：初次失败，暴露 `7140fd1` 中 native/web capability 收口后的格式未落盘；已执行 `cargo fmt` 修正。
- `cargo check -p rssr-app`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `cargo check -p rssr-cli`：通过。
- `cargo test -p rssr-application`：通过，17 passed。
- `cargo test -p rssr-infra --test test_config_package_codec`：通过，7 passed。
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`：通过，4 passed。
- `cargo test -p rssr-infra --test wasm_config_exchange_contract_harness --target wasm32-unknown-unknown --no-run`：通过。
- `cargo test -p rssr-web`：通过，14 passed。
- `node --check scripts/browser/rssr_visible_regression.mjs`：通过。
- `bash -n scripts/run_web_spa_regression_server.sh`：通过。
- `bash -n scripts/run_rssr_web_browser_feed_smoke.sh`：通过。
- `bash -n scripts/run_windows_chrome_visible_regression.sh`：通过。
- `git diff --check`：通过。

### `rssr-web browser feed smoke` 超时排查

- 独立执行 `scripts/run_rssr_web_browser_feed_smoke.sh --skip-build --port 18901` 复现失败。
- 失败 DOM 显示 helper 已打开 `/feeds`、填入 feed URL、点击 `data-action="add-feed"`，但等待 `li.feed-card` 超时。
- 根因 1：`crates/rssr-web/src/smoke.rs` 仍使用已被移除的 `li.feed-card` 旧 class selector。
- 修复：helper 改为查询 `li[data-layout="feed-card"]`。
- 修复后执行 `scripts/run_rssr_web_browser_feed_smoke.sh --skip-build --port 18902`：通过。
- Windows visible regression 使用 `--skip-build` 后又在静态 `/entries` populated selector 超时。
- 根因 2：`scripts/run_web_spa_regression_server.sh` 的 reader-demo seed 仍写入 `rssr-web-app-state-v1`，但浏览器 adapter 已升级为 `rssr-web-app-state-v2`。
- 修复：静态 SPA helper 与 `docs/design/web-spa-regression-server.md` 同步为 `rssr-web-app-state-v2`。
- fresh build 后首次 Windows visible regression 在 `Page.navigate` 上超时；换全新 Chrome 端口/profile 后通过，符合既有 CDP 会话污染风险。
- 最终验证：`scripts/run_windows_chrome_visible_regression.sh --static-port 8334 --rssr-web-port 18834 --chrome-port 9244 --skip-build --slow-ms 100`：通过，summary 位于 `target/windows-chrome-visible-regression/20260411-codex-after-fresh-build-clean-port/summary.md`。
- 提交前复验：`scripts/run_rssr_web_browser_feed_smoke.sh --skip-build --port 18903 --log-dir target/rssr-web-browser-feed-smoke/20260411-codex-precommit-feed-smoke`：通过。
- 提交前复验：`scripts/run_windows_chrome_visible_regression.sh --static-port 8335 --rssr-web-port 18835 --chrome-port 9245 --skip-build --slow-ms 100 --log-dir target/windows-chrome-visible-regression/20260411-codex-precommit-visible-regression`：通过。

### 工程宪法评估

- 结论：值得部分融入，但不建议全文并入现有 `.specify/memory/constitution.md`。
- 适合吸收的部分：
  - 骨架与边界先于模块和实现。
  - 外部环境依赖必须通过 adapter/capability 进入核心逻辑。
  - 单一真相源、版本与迁移责任、失败路径、可观测性、幂等与并发时序应进入设计门禁。
  - 骨架级变更需要先出分析报告并由 USER 明确批准。
- 不适合直接全文纳入的部分：
  - 文本过长且抽象，容易把项目现有 RSS 专用宪章稀释成通用治理手册。
  - “严格固定顺序”和“最低成熟度”若照搬，会和当前 spec-kit 用户故事切片、快速验证、渐进交付节奏冲突。
  - “完美主义”措辞需要改写为“边界审查与收敛”，否则容易被误用为过度设计或延期落地理由。
  - 优先级排序里把工程复杂度控制放到次级，和项目现有“简单演进”“性能是产品特性”存在张力。
- 建议后续如果正式融入，应做 `.specify` 宪章 minor 版本升级，而不是替换全文：
  - 新增或扩展“骨架边界、单一真相源、失败可验证”原则。
  - 在 plan 模板中增加状态/数据/配置模型、adapter 边界、失败/迁移/回退检查。
  - 在 spec 模板中强化边界情况、非目标、版本迁移影响。
  - 在 tasks 模板中按风险自动生成迁移、观测、回归验证任务。

### `.specify` 宪章 minor 升级

- `.specify/memory/constitution.md` 从 `1.2.0` 升级到 `1.3.0`，最后修订日期更新为 `2026-04-11`。
- 原则 V 补充外部环境依赖必须通过 adapter、port 或 capability 进入核心逻辑。
- 新增原则 VII：`骨架边界，真相源，失败可验证`。
- 新原则明确：
  - 功能或架构变更必须先说明其落入的既有能力轴和模块边界。
  - 骨架级变更必须先提交分析并由 USER 明确批准。
  - 核心状态、数据和配置必须有唯一真相源。
  - 持久化、storage、同步链路、导入导出格式必须承担版本、迁移、兼容和回退责任。
  - 核心流程必须在设计阶段定义失败传播、用户可见结果、观测点、幂等/去重和验证方式。
- 同步 `.specify/templates/constitution-template.md`，新增第 6、7 原则占位示例。
- 同步 `.specify/templates/plan-template.md`，新增真相源/版本责任、adapter/capability 边界、失败与观测策略，以及骨架变更判断门禁。
- 同步 `.specify/templates/spec-template.md`，新增状态/版本/失败边界和 adapter/capability 边界。
- 同步 `.specify/templates/tasks-template.md`，新增迁移/回退、失败路径、观测、幂等/去重相关任务要求。
- `.specify/templates/checklist-template.md` 与 `.specify/templates/agent-file-template.md` 已复查，无强制变更。
- `.specify/templates/commands/` 当前不存在，无需同步。

### 主线完整验证与 pre-push 预检

- `cargo test --workspace`：通过；包括 `test_webdav_local_roundtrip`，本轮无 env-limited 项。
- `cargo check -p rssr-app --target aarch64-linux-android`：通过。
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --skip-build --port 8336 --web-port 18836 --log-dir target/release-ui-regression/20260411-codex-mainline-prepush`：通过。
- release UI summary 位于 `target/release-ui-regression/20260411-codex-mainline-prepush/summary.md`。
- 本轮主线验证覆盖 workspace 单测、rssr-app native/wasm/android check、rssr-web 单测、release UI 自动门禁、rssr-web HTTP smoke、rssr-web browser feed smoke、Windows visible Chrome 全量回归。

### Application Use Case 收敛首刀验证

- `cargo fmt --check`：通过。
- `git diff --check`：通过。
- `cargo test --workspace`：通过；`rssr-application` 当前为 19 tests，新增 lifecycle refresh / no-refresh 分支均通过。
- `cargo check -p rssr-cli`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android`：通过。
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port 8342 --web-port 18842 --log-dir target/release-ui-regression/20260411-codex-subscription-lifecycle`：通过；该轮重新构建 debug web bundle，自动化门禁、`rssr-web` HTTP smoke、`rssr-web browser feed smoke` 均通过。
- `bash scripts/run_static_web_small_viewport_smoke.sh --skip-build --port 8343 --log-dir target/static-web-small-viewport-smoke/20260411-codex-subscription-lifecycle-current-build`：通过；Chrome DBus warning 属于当前 WSL headless 环境噪音。
- `bash scripts/run_static_web_reader_theme_matrix.sh --skip-build --port 8344 --log-dir target/static-web-reader-theme-matrix/20260411-codex-subscription-lifecycle-current-build`：通过；Chrome DBus warning 属于当前 WSL headless 环境噪音。
- `bash scripts/run_windows_chrome_visible_regression.sh --static-port 8337 --rssr-web-port 18837 --chrome-port 9247 --slow-ms 100 --log-dir target/windows-chrome-visible-regression/20260411-codex-subscription-lifecycle`：未通过，失败点为 Windows CDP `Page.navigate` timeout；此前阶段静态 entries/feeds、主题矩阵、小视口 entries/feeds 已通过。
- `bash scripts/run_windows_chrome_visible_regression.sh --static-port 8338 --rssr-web-port 18838 --chrome-port 9248 --skip-build --slow-ms 100 --log-dir target/windows-chrome-visible-regression/20260411-codex-subscription-lifecycle-clean-port`：未通过，失败点为小视口 `/settings` 等待 `[data-page="settings"] [data-layout="theme-presets"]`。
- `bash scripts/run_windows_chrome_visible_regression.sh --static-port 8339 --rssr-web-port 18839 --chrome-port 9249 --skip-build --slow-ms 250 --log-dir target/windows-chrome-visible-regression/20260411-codex-subscription-lifecycle-clean-port-slow`：未通过，失败点仍为 Windows CDP `Page.navigate` timeout。
- 结论：本次代码路径由 workspace tests、native/wasm/android check、重新构建后的 release UI regression、`rssr-web browser feed smoke`、静态小视口 smoke 和 reader theme matrix 覆盖通过；Windows visible runner 当前保留为 env-limited/CDP flake，不作为阻断本次 application-layer 收敛的失败。

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
- `882a764` 已提交 app-state 拆分与 runtime 窄 port 收口。
- `62a165e` 已提交该轮 handoff 记录。
- `9202355` 已提交 shared composition primitive 接入应用层与 CLI。
- `3ce6eb8` 已提交 host facade 收薄与 runtime capability 化。
- `7140fd1` 已提交 `UiServices` 从 `Arc<AppServices>` 继续压到 `use_cases + HostCapabilities`，并把 capability 对外接口收成 trait-object bundle。
- `9460c4a` 已提交 `.specify` 宪章 `1.3.0` 升级、smoke helper selector/key 修复和格式化修正。
- `58eaaf2` 已提交主线 pre-push 验证记录。
- `412631b` 已提交 application use case 收敛计划与订阅生命周期 workflow 首刀。
- 状态对齐开始时工作区为 clean，分支状态为 `main...origin/main [ahead 72]`；本轮 push 尝试被 GitHub HTTPS 凭据阻止。
