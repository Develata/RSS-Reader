# 2026-04-09 交接汇总

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：`main`
- 状态：`active`
- HEAD：`713cdca` `refactor: add semantic layout slots for css decoupling`
- 说明：本文件已按日汇总标准压缩，保留提交时间线、关键变更、验证、风险与待跟进项；过程性细节以同日提交和设计文档为准。

## 工作摘要

- 本日工作主线有四条：
- 继续收 `headless active interface` 的前端接口边界。
- 完成 `zheye-mainline-stabilization` 中高价值内容的吸收与重建，尤其是三条 contract harness。
- 收口浏览器 / wasm runner、本地 Linux CI 容器和 Docker 发布策略。
- 继续推进 `CSS 完全分离 + infra`，让页面逐步退化成默认语义壳。

- 到当前为止，主线已经完成：
- `refresh / subscription / config exchange` 三条 contract harness 的 host 与 wasm/browser 重建。
- `ui/shell + UiCommand / UiRuntime / UiIntent + page facade` 的实现基线。
- 第一轮 CSS 状态接口迁移：
  - `data-state`
  - `data-variant`
  - `data-density`
- 第二轮 CSS 结构槽迁移已提交。
- 第三轮 CSS 语义收口正在工作区中，重点是继续去掉卡片头部对 `h3` 的依赖。
- 第三轮 CSS 语义收口正在工作区中，当前已继续推进：
  - 卡片头部统一迁到 `.card-title`
  - entries 分组头部统一迁到 `.group-header` 语义槽
  - feeds 页头从 `reading-header--feeds` 迁到 `page-section-header--feeds`

## 当日提交时间线

- `3318d5e` `refactor: add an explicit entries page session`
- `2937232` `refactor: align theme preset command naming`
- `92fb6e1` `refactor: separate field and action interfaces`
- `07391a3` `refactor: tighten command and navigation interfaces`
- `4d11787` `style: normalize settings page formatting`
- `4d0d270` `docs: add test matrix and restore reader html fallback`
- `9cfde8e` `fix: add feed compose paste fallback`
- `aede229` `docs: plan contract harness rebuild`
- `e7a14ad` `test: add refresh contract harness baseline`
- `8cf8b53` `test: add wasm refresh harness base`
- `049a8ab` `test: expand wasm refresh contract assertions`
- `c9c13fa` `test: add wasm browser harness runner`
- `a592d40` `test: add subscription contract harness baseline`
- `781bf1a` `test: add wasm subscription contract harness`
- `682da79` `test: add config exchange contract harness`
- `d8b046d` `test: unify wasm contract harness runner`
- `f1bc8a8` `fix: stabilize rssr-web auth test and wasm runner`
- `30bb036` `ci: only publish docker images for version tags`
- `722e5ad` `test: add local linux ci container`
- `01a0365` `fix: support non-interactive local ci runs`
- `0047269` `fix: resolve wasm browser runner issues and enhance Chrome setup script`
- `b9de79f` `refactor: organize page modules into local workspaces`
- `0ccd7b3` `refactor: deepen page session boundaries`
- `e5b917f` `refactor: thin page shells around local sessions`
- `94979a3` `refactor: add feeds page intent and effect flow`
- `3035fdc` `fix: stop entries page white screen loop`
- `7e0643c` `refactor: unify settings theme save flow`
- `8d55418` `refactor: complete feeds page state flow migration`
- `4cf99cf` `refactor: reduce browser state write amplification`
- `28272d3` `refactor: 移除 entries 和 reader 页面中的 bindings 中间层，简化状态流`
- `aac3c3b` `refactor: sessionize settings page load flow`
- `1d95183` `refactor: consolidate entries page workspace bootstrap`
- `bb43add` `refactor: split browser state into explicit slices`
- `dd27b53` `refactor: introduce global ui runtime skeleton`
- `55d85c5` `refactor: route reader and feeds pages through ui runtime`
- `dfff938` `refactor: route settings page through ui runtime`
- `ec59e93` `refactor: thin page sessions around ui bus`
- `66ffd4e` `refactor: share ui bus projection helpers`
- `5b95d4b` `refactor: move app shell and entries through shell facade`
- `b47ad11` `refactor: expose reader and feeds page facades`
- `4f7a7e8` `refactor: route settings facade through theme and sync shells`
- `81fca45` `refactor: expose entries facade action slots`
- `84ed0fe` `refactor: expose settings facade value and action slots`
- `964e283` `refactor: expose reader and feeds facade snapshots`
- `511b4f4` `refactor: standardize page facade boundaries`
- `ca1ed2e` `refactor: move app nav through shell facade`
- `497e63d` `refactor: standardize page facade semantics`
- `4427719` `docs: align shell bus facade architecture docs`
- `5e0d183` `refactor: expose stable ui state semantics`
- `799bd8d` `refactor: split ui bus by command family`
- `afd54bb` `refactor: move css state semantics to data attributes`

## 影响范围

- 业务模块：
- `crates/rssr-app`
- `crates/rssr-application`
- `crates/rssr-infra`
- `crates/rssr-web`

- 前端页面：
- `entries_page`
- `reader_page`
- `feeds_page`
- `settings_page`
- `ui/shell`
- `ui/commands`
- `ui/runtime`

- 测试与 CI：
- `crates/rssr-infra/tests`
- `scripts/run_wasm_*`
- `scripts/setup_chrome_for_testing.sh`
- `scripts/run_ci_local_container.sh`
- `.github/workflows/ci.yml`
- `.github/workflows/docker.yml`

- 文档：
- `docs/testing/*`
- `docs/design/*`
- `docs/handoffs/*`

- 平台：
- Web
- Desktop
- Android
- Linux CI
- wasm/browser runner

## 关键变更

### 1. 吸收 `zheye-mainline-stabilization` 的高价值内容

- 已明确不直接 merge 分支。
- 已吸收的高价值内容包括：
- `reader_page` 的 HTML-like fallback。
- feed 输入框 paste fallback。
- 测试矩阵与环境限制文档。
- 三条 contract harness 的思路与目标。

- 已明确不再继续吸收的内容包括：
- 旧的 web bootstrap 布局。
- 旧的 adapter 文件组织。
- 旧 handoff 原文。
- 已被当前主线覆盖的 application/use case 收束提交。

### 2. contract harness 重建完成

- 已完成三条主 contract 线：
- refresh
- subscription
- config exchange

- 每条线都已补齐：
- host / sqlite baseline
- wasm / browser baseline
- 统一 runner
- CI matrix 入口

- 现在 `zheye` 分支最核心的测试遗产，已经在当前 `main` 上重建完成。

### 3. wasm/browser runner 收口

- 解决了两类核心问题：
- `rssr-web` auth 单测在 Linux runner 上的路径断言错误。
- Chrome for Testing / chromedriver / `wasm-bindgen-test-runner` 的浏览器执行链不稳定。

- 关键修复包括：
- 为 runner 补 `webdriver.json` 能力。
- 显式传入：
  - `--headless=new`
  - `--no-sandbox`
  - `--disable-dev-shm-usage`
  - `--disable-gpu`
  - `--remote-allow-origins=*`
  - `--window-size=1280,720`
- 修复 Chrome for Testing 解压后顶层可执行权限不完整的问题。
- 为每次浏览器会话分配独立 `user-data-dir`。

- 本地 WSL2 仍受 `chromedriver bind() failed: Cannot assign requested address (99)` 限制。
- GitHub Actions 上的 `404 + SIGKILL` 路径已通过上述 runner 调整收口。

### 4. Docker 发布策略收紧

- `main` 日常 push：
- 继续 build
- 继续 runtime smoke
- 不再真正 push image

- `v*` tag：
- 才执行 registry login
- 才生成 metadata
- 才真正 push image

- 目标是避免日常主线 push 产生多余 GHCR 版本。

### 5. 本地 Linux CI 容器基座

- 新增：
- `Dockerfile.ci-local`
- `scripts/run_ci_local_container.sh`

- 用途：
- 在本地复现接近 `ubuntu-latest` 的测试环境
- 跑 host tests
- 跑 wasm `--no-run`
- 复现 browser runner 安装路径

- 该容器不是 GitHub Actions 替代品，而是本地复现辅助工具。

### 6. 页面模块目录化

- 页面层从平铺文件收成目录模块：
- `entries_page/`
- `reader_page/`
- `settings_page/`
- `feeds_page/`

- 这一步的价值不是功能变化，而是：
- 降低搜索成本
- 统一页面物理结构
- 为后续 session/workspace 收口提供稳定落点

### 7. 页面 session / workspace 收口

- `entries_page`
- 引入显式 session
- 收掉 bindings 中间层
- 收掉多余本地 runtime/effect

- `reader_page`
- 收口成 session + reducer + facade + bus
- 去掉旧 bindings/effect/runtime

- `feeds_page`
- 先补齐 `intent/reducer/effect/runtime`
- 再去掉旧 `CommandOutcome / UiPatch`
- 再继续收平到直接 dispatch `UiCommand`

- `settings_page`
- 先把 themes save 流并回 `SettingsPageSaveSession`
- 再让主加载链 session 化
- 再统一 `sync` 与页面 session 边界

### 8. browser state 双轨与写放大收口

- 已收掉的双轨：
- `settings themes` 保存双轨
- `feeds_page` 旧 patch/outcome 双轨
- `entries/reader` bindings 双轨

- browser persisted state 已做两轮收口：
- 第一轮：去掉高频字段全量写回
- 第二轮：显式切成：
  - `core`
  - `app_state`
  - `entry_flags`

- 当前 browser state 已不再是“主状态 + 隐式 sidecar 覆盖”的历史混合态。

### 9. 全局 UI 总线落地

- 已新增并逐步完善：
- `UiCommand`
- `UiRuntime`
- `UiIntent`
- `ui/shell`
- page facade

- 迁移顺序是：
- 先 `App()` 与 `StartupPage`
- 再 `entries`
- 再 `reader`
- 再 `feeds`
- 再 `settings`

- 之后继续做了第二轮收口：
- page-local runtime 只剩语义状态组织
- 真正的 service/application 调用统一由 `UiRuntime` 承担

### 10. facade / shell 边界标准化

- `entries/reader/feeds/settings` 四页都已经形成更一致的 facade 形状：
- 快照读取
- 默认动作入口
- 默认状态语义

- `AppNav` 也已并入 `ui/shell`
- 根壳继续变薄
- 页面继续向“默认语义壳”靠拢

### 11. 设计文档与实现文档对齐

- 新增：
- `docs/design/ui-shell-bus-page-facade.md`
- `docs/design/css-separation-baseline-checklist.md`

- 更新：
- `docs/design/README.md`
- `docs/design/headless-active-interface.md`
- `docs/design/frontend-command-reference.md`

- 结果是：
- 不再只有愿景文档
- 也有“当前实现版边界说明”

### 12. CSS 完全分离：第一轮状态接口迁移

- 已完成迁移：
- `.status-banner.info/.error` -> `data-state`
- `.button.secondary/.danger/.danger-outline` -> `data-variant`
- `.app-shell.density-compact` -> `data-density`
- `.theme-card.is-active` -> `data-state`
- `.entry-filters__source-chip.is-selected` -> `data-state`
- `.reader-bottom-bar__button.is-*` -> `data-state`

- 相关提交：
- `afd54bb` `refactor: move css state semantics to data attributes`

### 13. CSS 完全分离：第二轮结构槽迁移

- 当前工作区中已完成但尚未提交：
- 页面标题改成 `.page-title`
- 设置页页头补：
  - `.page-header__title`
  - `.page-header__actions`
- entries / feeds 顶部标题区补：
  - `.page-section-header`
  - `.page-section-title`
- 表单网格项补：
  - `.settings-form-grid__item`
- 行内动作项补：
  - `.inline-actions__item`
- 文章卡片动作项补：
  - `[data-slot="entry-card-action"]`
- 阅读列表项补：
  - `data-list-edge="start|middle|end|single"`

- 这一步已经清掉的旧结构依赖有：
- `.page h2`
- `.page-header h2`
- `.page-header .icon-link-button`
- `settings-form-grid > div`
- `inline-actions > *`
- `entry-card__actions > *`
- `.entry-card--reading:first-child`
- `.entry-card--reading:last-child`
- `.entry-card--reading + .entry-card--reading`
- `.page-entries .reading-header`

## 已执行验证

- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `cargo check -p rssr-infra`
- `cargo test -p rssr-web`
- `cargo test -p rssr-infra --test test_refresh_contract_harness`
- `cargo test -p rssr-infra --test test_subscription_contract_harness`
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_subscription_contract_harness --no-run`
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_config_exchange_contract_harness --no-run`
- `git diff --check`

## 浏览器与运行态验收

- Chrome MCP 在当日部分时段可用，曾用于验收：
- `/entries`
- `/feeds`
- `/settings`
- 主题预设切换与状态提示

- 但后续多轮重构期间，Chrome MCP 多次返回：
- `Transport closed`
- 因此后半段浏览器验收以：
  - 编译
  - runner
  - 接口 grep
  为主，而不是持续浏览器自动化。

- WSLg 桌面窗口问题已判断为平台兼容性问题，暂时搁置，不作为当前主线阻塞项。

## 当前状态

- 已提交到 `HEAD` 的主线状态：
- `zheye` 分支的核心内容已基本吸收完成。
- 四个主页面都已接到统一 UI bus。
- `ui/runtime` 与 `ui/commands` 已按命令族拆模块。
- 第一轮 CSS 状态接口迁移已提交。

- 当前工作区状态：
- 有未提交改动。
- 这批改动属于 CSS 完全分离第二轮结构槽迁移。
- 影响文件主要是：
  - `assets/styles/{shell,entries,workspaces,responsive}.css`
  - `crates/rssr-app/src/pages/entries_page/{mod.rs,cards.rs}`
  - `crates/rssr-app/src/pages/settings_page/{mod.rs,preferences.rs,sync/mod.rs,themes/{lab.rs,presets.rs}}`
  - `crates/rssr-app/src/pages/feeds_page/mod.rs`
  - `crates/rssr-app/src/pages/reader_page/mod.rs`
  - `docs/design/css-separation-baseline-checklist.md`

## 风险与限制

- `UiRuntime` 和 page facade 已经过第一轮拆分，但仍要继续防止：
- `ui/runtime/*` 重新膨胀成 God object
- facade 继续吸纳纯视觉判断

- WSL2 / WSLg 桌面问题仍未真正根治：
- 不是业务逻辑阻塞
- 但会影响本机桌面端体验验证

- Chrome MCP 在本轮后半段不可用：
- 浏览器自动化验收连续性受限

- CSS 完全分离仍未完成：
- 第一轮状态 modifier 已收掉
- 第二轮结构槽已完成但未提交
- 第三轮更深层的卡片头部 / 分组头部结构依赖仍待处理

## 待跟进项

- 优先级 P1：
- 提交当前工作区里的“CSS 完全分离第二轮结构槽迁移”

- 优先级 P2：
- 继续清第三批结构依赖，重点看：
  - `.feed-compose-card__header h3`
  - `.exchange-card h3`
  - `.settings-card h3`
  - `.entry-source-group__header`
  - `.entry-date-group__header`

- 优先级 P3：
- 继续做 CSS / DOM 语义审计，确保：
  - `data-page`
  - `data-action`
  - `data-field`
  - `data-nav`
  - `data-state`
  - `data-variant`
  - `data-density`
  真正成为长期稳定接口

## 结论

- 2026-04-09 的工作已经把仓库从“页面局部 runtime + 多处历史兼容痕迹”的状态，推进到：
- `ui/shell + UiCommand / UiRuntime / UiIntent + page facade`
- `headless active interface`
- `CSS 完全分离`
- `infra` 稳定承载真实行为

- 当前主线最关键的事情已经不再是继续吸收 `zheye` 分支，而是：
- 继续收 CSS 结构语义
- 避免 `ui/runtime` 与 facade 再次膨胀
- 让页面最终真正退化成默认语义壳
