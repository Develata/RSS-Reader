# 2026-04-09 交接汇总

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：见当日提交时间线
- 相关 commit：3318d5e, 2937232, 92fb6e1, 07391a3, 4d11787, 4d0d270, 9cfde8e, aede229, e7a14ad, 8cf8b53, 049a8ab, c9c13fa, a592d40, 781bf1a, 682da79, d8b046d, f1bc8a8, 30bb036, 722e5ad, 01a0365, 0047269, b8bde41, b9de79f, 0ccd7b3
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

本日主线从 headless 接口清理转入测试与契约重建：继续补齐文章页显式 session、规范 `data-action` / `data-field` / `data-nav` 边界、吸收 `zheye-mainline-stabilization` 中值得保留的测试文档与产品修复，并把 refresh / subscription / config exchange 三条 contract harness 在 host 与 wasm/browser 两条执行线上全部重建出来。

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
- `b8bde41` `docs: consolidate handoff logs by day`
- `b9de79f` `refactor: organize page modules into local workspaces`
- `0ccd7b3` `refactor: deepen page session boundaries`

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/entries_*`
  - `crates/rssr-app/src/pages/settings_*`
  - `crates/rssr-app/src/pages/settings_page_themes/`
  - `crates/rssr-app/src/pages/feeds_page_sections/compose.rs`
  - `crates/rssr-app/src/pages/reader_page_support.rs`
  - `crates/rssr-infra/tests/`
  - `scripts/run_wasm_*`
  - `.github/workflows/ci.yml`
  - `docs/testing/`
  - `docs/handoffs/`
- 平台：
  - Web
  - Windows
  - macOS
  - Linux
  - Android
  - CI / browser runner
- 额外影响：
  - docs
  - testing
  - contract
  - governance

## 关键变更

### 来源记录

- `2026-04-09-command-surface-naming-alignment-step1.md`
- `2026-04-09-settings-field-action-boundary.md`
- `2026-04-09-field-action-boundary-step2.md`
- `2026-04-09-command-surface-cleanup-step2.md`
- `2026-04-09-testing-docs-and-reader-html-fallback.md`
- `2026-04-09-feeds-paste-fallback.md`
- `2026-04-09-contract-harness-rebuild-plan.md`
- `2026-04-09-refresh-contract-harness-step1.md`
- `2026-04-09-wasm-refresh-harness-base.md`
- `2026-04-09-wasm-refresh-harness-step2.md`
- `2026-04-09-wasm-browser-runner-and-ci.md`
- `2026-04-09-subscription-contract-harness-step1.md`
- `2026-04-09-subscription-contract-harness-step2.md`
- `2026-04-09-config-exchange-contract-harness.md`
- `2026-04-09-wasm-contract-runner-matrix.md`
- `2026-04-09-contract-harness-browser-strategy.md`

### Headless 接口继续收口

- 文章页补成显式 `EntriesPageSession`。
- 主题预设命名、字段/动作接口边界、阅读页导航命名进一步统一。
- `data-field` 与 `data-action` 的语义边界被明确拉开，容器/展示位不再误标为动作接口。
- 这轮不是简单改名字，而是在为 `headless active interface` 的稳定选择器做“去视觉化”和“去 DOM 位置耦合”：
  - 主题预设动作不再把预设名和布局位置编码进 `data-action`
  - 持续输入值不再伪装成命令
  - `data-nav` 只保留真正的导航语义

### 吸收 `zheye-mainline-stabilization` 的高价值遗产

- 新增长期测试文档：
  - `docs/testing/mainline-validation-matrix.md`
  - `docs/testing/environment-limitations.md`
- 将阅读页 HTML-like fallback 手动移植到 `crates/rssr-app/src/pages/reader_page_support.rs`。
- 将 feed 输入框 paste fallback 手动移植到 `crates/rssr-app/src/pages/feeds_page_sections/compose.rs`。
- 到这一天为止，对 `zheye-mainline-stabilization` 的处理原则已经定型：
  - 不直接 merge 分支
  - 不直接 cherry-pick 大型重构提交
  - 只手动吸收当前主线仍缺的修复、测试文档和 contract 测试方向

### 当日后续修复：CI 红点收口

- 修复 `rssr-web` 中 `auth_state_file_prefers_userprofile_when_home_is_missing` 的跨宿主机断言：
  - 旧断言把 Windows 风格路径分隔符写死为 `\\`
  - 在 Linux runner 上 `PathBuf::join` 会生成 `C:\\Users\\rssr/.rssr-web-auth.json`
  - 新断言改为基于 `PathBuf::from(...).join(DEFAULT_AUTH_STATE_FILE_NAME)`，与真实实现一致
- 为 wasm browser harness runner 补了 `webdriver.json` 能力配置，显式加入：
  - `--headless=new`
  - `--no-sandbox`
  - `--disable-dev-shm-usage`
  - `--disable-gpu`
  - `--remote-allow-origins=*`
  - `--window-size=1280,720`
- runner 现在在 `crates/rssr-infra` 下执行 `wasm-bindgen-test-runner`，确保能拾取同目录的 `webdriver.json`
- 本机 WSL2 仍受 `chromedriver bind() failed: Cannot assign requested address (99)` 限制，但 GitHub Actions 上此前的 `404 + SIGKILL` 路径已经被 runner 配置缺失所对齐修补

### 当日后续修复：wasm browser runner 根因定位与修补

- 在本地 Linux CI 容器中成功稳定复现了 GitHub Actions 同样的失败路径：
  - `wasm-bindgen-test-runner` 进入 `Visiting http://127.0.0.1:...`
  - 随后出现 `driver status: signal: 9 (SIGKILL)` 与 `Error: http status: 404`
- 后续通过 `chromedriver --verbose` 缩小问题，确认不是 contract harness 本身失败，也不是单纯 Chrome/ChromeDriver 版本过新，而是 Chrome 进程启动时直接崩溃：
  - `chrome_crashpad_handler: Permission denied (13)`
- 根因是：
  - `scripts/setup_chrome_for_testing.sh` 用 Python 解压 Chrome for Testing zip
  - 顶层可执行文件权限没有完整保留下来
  - 之前只对 `chrome` 与 `chromedriver` 做了 `chmod +x`
  - 但 Chrome 实际还会启动 `chrome_crashpad_handler`，导致 session 创建阶段直接崩溃
- 修复内容：
  - `scripts/setup_chrome_for_testing.sh` 改为对 `chrome-${platform}` 与 `chromedriver-${platform}` 顶层文件统一 `chmod +x`
  - `scripts/run_wasm_contract_harness.sh` 为每次执行分配独立 `--user-data-dir`
  - 并显式把 `google-chrome` binary 写入 `webdriver.json`
- 修复后，在本地 Linux 容器里，按接近 CI 的执行路径，三条 wasm browser harness 都已实际跑绿：
  - refresh
  - subscription
  - config exchange

### 当日后续修复：Docker 发布节流

- 调整 `.github/workflows/docker.yml` 的语义边界：
  - `main` 分支日常 push 仍会执行 Docker build 与 runtime smoke
  - 但不会再登录 GHCR，也不会真正 push image
  - 只有 `refs/tags/v*` 才会执行 metadata、registry login 与 image push
- 这样保留了“验证 Docker 镜像可构建、可运行”的持续信心，同时避免每次主线 push 都生成额外的 GHCR 版本噪音

### 当日后续补充：本地 Linux CI 容器基座

- 新增 `Dockerfile.ci-local`
  - 基于 `ubuntu:24.04`
  - 预装 Rust stable、`wasm32-unknown-unknown` target、`wasm-bindgen-cli`
  - 安装与当前 `CI` workflow 接近的系统依赖和 headless Chrome 运行库
- 新增 `scripts/run_ci_local_container.sh`
  - 支持一键启动交互式容器
  - 支持 `--rebuild`
  - 支持 `--cmd '<command>'` 一次性执行测试命令
  - 默认挂载 cargo registry/git/target volume，并开启 `--shm-size=2g`
  - 默认使用 `--network host`，尽量贴近 browser runner 与 CI 的网络行为
- 这套容器不是为了替代 GitHub Actions，而是为了在本地更接近 `ubuntu-latest` 地复现：
  - `cargo test -p rssr-web`
  - `cargo test -p rssr-infra --test ...`
  - `cargo test -p rssr-infra --target wasm32-unknown-unknown --test ... --no-run`
  - `scripts/setup_chrome_for_testing.sh`
  - `scripts/run_wasm_*_contract_harness.sh`

### 当日后续修复：本地 CI 容器 `--cmd` 非交互执行

- `scripts/run_ci_local_container.sh` 最初统一使用 `docker run -it`。
- 这在交互式 shell 下正常，但在脚本化或当前这类非交互执行环境里，会直接报：
  - `the input device is not a TTY`
- 修复后脚本改成两条分支：
  - 无 `--cmd` 时，继续使用 `-it` 进入交互式容器
  - 有 `--cmd` 时，去掉 TTY 参数，按非交互 one-shot 命令执行
- 这次修复不改变镜像内容、不改变 volume/network/shm 配置，只修正容器启动模式与实际使用场景不匹配的问题。
- 修复后已确认：
  - `bash scripts/run_ci_local_container.sh --cmd 'echo ok'`
  - `bash scripts/run_ci_local_container.sh --cmd 'cargo test -p rssr-web -- --nocapture'`
  均能正常执行

### 当日后续重构：页面模块目录化与 workspace 结构统一

- 将 `rssr-app` 页面层里已经成型的局部 workspace/session 结构，正式从“平铺文件”收成“目录模块”：
  - `crates/rssr-app/src/pages/entries_page/`
  - `crates/rssr-app/src/pages/reader_page/`
  - `crates/rssr-app/src/pages/settings_page/`
- `entries_page` 现在统一落在一个显式模块下：
  - `mod.rs`
  - `bindings.rs`
  - `cards.rs`
  - `controls.rs`
  - `effect.rs`
  - `groups.rs`
  - `intent.rs`
  - `presenter.rs`
  - `queries.rs`
  - `reducer.rs`
  - `runtime.rs`
  - `session.rs`
  - `state.rs`
- `reader_page` 也从平铺的 `reader_page_*` 文件统一成目录模块：
  - `mod.rs`
  - `bindings.rs`
  - `effect.rs`
  - `intent.rs`
  - `reducer.rs`
  - `runtime.rs`
  - `session.rs`
  - `state.rs`
  - `support.rs`
- `settings_page` 则进一步收成更接近能力边界的层次：
  - `appearance.rs`
  - `preferences.rs`
  - `save/{mod,effect,runtime,session,state}.rs`
  - `sync/{mod,effect,runtime,session,state}.rs`
  - `themes/{mod,lab,presets,theme_apply,theme_io,theme_preset,theme_validation}.rs`
- `feeds_page` 也不再保留“单文件页面 + 邻接 sections 目录”的旧形状，而是统一为：
  - `feeds_page/mod.rs`
  - `feeds_page/{bindings,commands,dispatch,queries}.rs`
  - `feeds_page/sections/{mod,compose,config_exchange,saved,support}.rs`
- `crates/rssr-app/src/pages.rs` 不再继续充当大号平铺注册表，页面内核逻辑回收到各自模块目录内部。
- 这轮重构不改变页面行为，不引入新功能，目标是统一页面内核的物理结构，使已经形成的 session/workspace 模式更可维护、更可搜索，也让后续继续收薄 view shell 时不需要在几十个 `*_page_*` 平铺文件中来回跳转。
- 这轮完成后，`entries_page` / `feeds_page` / `reader_page` / `settings_page` 四个页面模块都已经进入同一类目录模块形状；后续如果继续统一页面模式，可以直接沿目录模块继续，而不必再先做一次物理文件重排。

### 当日后续重构：feeds/settings view shell 再收薄

- 在目录化完成后，继续把 `feeds_page` 与 `settings_page` 页面的壳层入口再往内部收一层：
  - `crates/rssr-app/src/pages/feeds_page/session.rs`
  - `crates/rssr-app/src/pages/settings_page/session.rs`
- `feeds_page` 的页面壳不再自己组装整组信号并直接调用 snapshot 查询函数，而是通过 `FeedsPageSession` 统一管理：
  - `feed_url`
  - `config_text`
  - `opml_text`
  - `pending_config_import`
  - `pending_delete_feed`
  - `feeds`
  - `feed_count`
  - `entry_count`
  - `status`
  - `status_tone`
  - `load_snapshot()`
- `settings_page` 的页面壳不再直接持有：
  - `draft`
  - `preset_choice`
  - `status`
  - `status_tone`
  - `AppServices::shared() -> load_settings()`
  - 打开 GitHub 仓库的错误处理
- 这些入口现在统一落到 `SettingsPageSession`：
  - `load()`
  - `open_repository()`
  - 以及对 `theme / draft / preset_choice / status` 的访问器
- 这轮没有把设置页改成新的大状态机，也没有改变 `save` / `sync` / `themes` 的能力边界；目标只是进一步减少 view shell 中零散的状态线头和服务初始化逻辑，让页面壳更接近“只组装 session 与 section”。

### 当日后续重构：feeds 完整 local session / settings 状态边界再收口

- `feeds_page` 继续从“页面壳 + 一组散 signal + bindings patch”收成更完整的 local session/workspace：
  - 新增 `crates/rssr-app/src/pages/feeds_page/state.rs`
  - 删除旧的 `crates/rssr-app/src/pages/feeds_page/bindings.rs`
  - `FeedsPageSession` 现在直接持有 `Signal<FeedsPageState>`
- `FeedsPageState` 统一承载：
  - `feed_url`
  - `config_text`
  - `opml_text`
  - `pending_config_import`
  - `pending_delete_feed`
  - `feeds`
  - `feed_count`
  - `entry_count`
  - `status`
  - `status_tone`
  - `reload_tick`
- `FeedsPageSession` 现在直接负责：
  - `load_snapshot()`
  - `apply_snapshot(...)`
  - `apply_command_outcome(...)`
  - `set_feed_url / set_config_text / set_opml_text`
  - `add_feed / refresh_all / refresh_feed / remove_feed`
  - `export_config / import_config / export_opml / import_opml`
  - 粘贴失败时的状态错误回写
- 订阅页 section 也改成直接消费 `FeedsPageSession`：
  - `sections/compose.rs`
  - `sections/config_exchange.rs`
  - `sections/saved.rs`
- 这一步的意义是：
  - 订阅页不再依赖“页面壳构造 signal，再由 bindings 回补 patch”的旧模式
  - 命令结果、状态提示、reload tick 和输入状态都回到同一个本地 workspace
  - 后续如果继续把订阅页推进到和文章页/阅读页更接近的状态机结构，已经不需要先做一次中间抽象迁移

- `settings_page` 这一步则继续收边界，而不是强推成大状态机：
  - `AppearanceSettingsCard` 现在直接接收 `SettingsPageSession`
  - `WebDavSettingsCard` 现在也直接接收 `SettingsPageSession`
  - 页面壳不再把 `theme / draft / preset_choice / status / status_tone` 五根线手动往下散
  - `SettingsPageSession` 现在明确承担：
    - 初始设置加载
    - GitHub 仓库打开失败时的状态提示
    - 向子卡片暴露同一组页面级状态入口
- 这里刻意没有把设置页变成统一 reducer/store，因为当前设置页仍然更像“多个独立工具卡片”的组合页；这次收口的目标是缩小边界，而不是为了统一形式去强造更重的页面内核。

### 当日后续重构：entries/reader workspace helper 与 settings 子卡片围绕页面 session

- `entries_page` 的 view shell 再收薄一层：
  - 将 `remember_last_opened_feed`、偏好加载、订阅加载、文章加载与浏览偏好保存这些 `use_resource/use_effect` 组合统一收进私有 `use_entries_page_workspace(...)`
  - `entries_page/mod.rs` 现在在页面壳里只拿三样东西：
    - `EntriesPageSession`
    - `EntriesPageState`
    - `EntriesPagePresenter`
  - 这一步没有改变文章页行为，但把“页面装配”和“异步 wiring”拆得更清楚，后续如果继续收 `entries_page`，可以先改 helper，而不必先碰渲染骨架
- `reader_page` 做了同类收口：
  - 新增私有 `use_reader_page_workspace(...)`
  - 将 `state/session/reload/load` 与快捷键绑定周边的资源加载集中到 helper 中
  - `ReaderPage` 组件本体继续只保留：
    - 导航
    - 正文/状态展示
    - 上下篇与未读跳转操作
- `settings_page` 的内部卡片边界也进一步统一到了 `SettingsPageSession`：
  - `SettingsPageSaveSession` 不再单独持有 `theme/draft/preset_choice/status/status_tone` 五条裸状态线，而是显式持有 `SettingsPageSession`
  - `SettingsPageSyncSession` 同样改为显式持有 `SettingsPageSession`
  - `ThemeSettingsSections`、`ThemeLabSection`、`ThemePresetSections` 改成直接消费 `SettingsPageSession`
  - `AppearanceSettingsCard` 与 `WebDavSettingsCard` 因而不再负责把页面级状态逐根下传，只负责创建自己的局部 save/sync state
- 这轮重构的实际收益是：
  - `settings_page` 的 themes/save/sync 三块不再围绕一串裸 signal 相互耦合
  - `entries_page` / `reader_page` 的 view shell 中剩余的加载逻辑被压回私有 workspace helper
  - 页面层的“壳”与“会话内核”分界更接近同一种形状，后续继续重构时更容易保持一致

### 当日后续重构：feeds_page 补齐 intent / reducer / effect / runtime

- `feeds_page` 在完成目录化和 local session 之后，又继续向 `entries/reader` 的形状靠拢了一步：
  - 新增 `crates/rssr-app/src/pages/feeds_page/intent.rs`
  - 新增 `crates/rssr-app/src/pages/feeds_page/reducer.rs`
  - 新增 `crates/rssr-app/src/pages/feeds_page/effect.rs`
  - 新增 `crates/rssr-app/src/pages/feeds_page/runtime.rs`
- 这轮没有重写 UI，也没有改 `commands/dispatch/queries` 的业务语义，而是把它们重新挂到显式的会话流里：
  - `FeedsPageIntent` 统一表达：
    - 本地字段变化
    - 加载请求
    - 各类订阅/配置交换动作请求
    - clipboard paste 请求
    - runtime 回灌结果
  - `reduce_feeds_page_intent(...)` 负责：
    - 更新 `FeedsPageState`
    - 派生是否需要执行 `LoadSnapshot / ExecuteCommand / ReadFeedUrlFromClipboard`
  - `execute_feeds_page_effect(...)` 负责：
    - 调 `load_feeds_page_snapshot()`
    - 调现有 `execute_command(...)`
    - 在浏览器端读取剪贴板文本
  - `FeedsPageSession` 则不再直接做“查询 + 命令 + patch 回填”的混合控制器，而是负责：
    - `dispatch_intent(...)`
    - `spawn_effect(...)`
    - 对 section 暴露稳定的页面动作接口
- `feeds_page/mod.rs` 也继续收薄：
  - 新增私有 `use_feeds_page_workspace()`
  - 把 `state/session/reload resource` 压回 helper
  - 页面壳只消费 `session + snapshot`
- 订阅输入框的 paste fallback 也正式进入这条 effect 链，不再从 section 里直接起一段异步 clipboard 读取逻辑。
- 到这一步为止，`feeds_page` 虽然还不像 `entries_page` 那样有完整 presenter/reducer 风格的页面内核，但已经具备了：
  - 显式 state
  - 显式 intent
  - 显式 effect/runtime
  - 围绕 session 的统一入口
  后续如果继续推进，不需要再先做一次中间态迁移。

### Contract harness 规划与重建

- 新增 `docs/testing/contract-harness-rebuild-plan.md`，明确不直接复制旧分支 harness，而按当前主线重建。
- `refresh contract harness`：
  - host/sqlite baseline 已建。
  - wasm/browser baseline 已建，并扩展为真实断言。
- `subscription contract harness`：
  - host/sqlite baseline 已建。
  - wasm/browser baseline 已建。
- `config exchange contract harness`：
  - host/sqlite baseline 已建。
  - wasm/browser baseline 已建。

### refresh contract 线的重建细节

- host / sqlite baseline 覆盖：
  - updated feed metadata + entries
  - not modified
  - failed refresh
  - `refresh_all` 聚合结果
- wasm/browser baseline 先建立基座，再扩成真实断言：
  - `list_targets`
  - `RefreshCommit::NotModified`
  - `RefreshCommit::Updated`
  - `RefreshCommit::Failed`
  - localStorage 写回

### subscription contract 线的重建细节

- host / sqlite baseline 覆盖：
  - URL 规范化与去重
  - 删除订阅时的软删除
  - `purge_entries = true` 时 entry 清理
  - `last_opened_feed_id` 命中与不命中两种清理语义
- wasm/browser baseline 覆盖同一组契约，并把：
  - `BrowserFeedRepository`
  - `BrowserEntryRepository`
  - `BrowserAppStateAdapter`
  正式纳入 contract 线

### config exchange contract 线的重建细节

- host / sqlite baseline 覆盖：
  - JSON config roundtrip
  - 导入时清理被移除 feed 的 entries 与 `last_opened_feed_id`
  - OPML import 的 URL 规范化
  - remote push/pull 契约
- wasm/browser baseline 覆盖：
  - browser state 导出 JSON
  - import 时清理被移除 feed 的 entries 与 `last_opened_feed_id`
  - remote pull roundtrip
  - localStorage 写回

### wasm browser runner 与 CI

- 新增通用 runner：
  - `scripts/run_wasm_contract_harness.sh`
- 新增三个薄 wrapper：
  - `run_wasm_refresh_contract_harness.sh`
  - `run_wasm_subscription_contract_harness.sh`
  - `run_wasm_config_exchange_contract_harness.sh`
- `.github/workflows/ci.yml` 中的 `wasm-browser-contract` 改为 matrix，同时覆盖 refresh / subscription / config exchange 三条 contract 线。
- 本机执行策略也在这一天固定下来：
  - `cargo test ... --target wasm32-unknown-unknown --test <harness> --no-run`
  - 再通过 `wasm-bindgen-test-runner` 执行单个 `.wasm`
  - 不以 `wasm-pack test` 作为当前仓库的正式入口

### handoff 整理

- 将 `docs/handoffs/` 中 `2026-04-06` 到 `2026-04-09` 的碎片记录按日期归并为单日汇总。
- 这次整理不是单纯删文件，而是把原始记录里的：
  - 提交顺序
  - 关键文件
  - 自动化验证
  - 手工验收
  - 风险判断

## 验证与验收

- `cargo fmt --all`
- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo check -p rssr-app --target aarch64-linux-android`
- `git diff --check`

结果：

- 页面目录化与模块重命名后，host / wasm / Android 三个目标都编译通过。
- `feeds/settings` view shell 再收薄后，host / wasm / Android 三个目标继续保持通过。
- `feeds_page` 收成完整 local session/workspace、`settings_page` 页面级状态入口再收口之后，host / wasm / Android 三个目标继续保持通过。
- `git diff --check` 通过，没有引入格式或空白错误。
- 这轮未改 UI 行为与业务逻辑，重点验收的是模块结构调整后跨目标路径、`include_str!` 相对路径、以及 session/workspace 内部引用是否仍然正确。

## 当前状态 / 风险 / 待跟进

- 状态：`validated`
- 当前目录化已经完成第一轮收口，但还没有继续把 `feeds_page` 也改成完全一致的目录模块。
- 当前风险主要不是功能回退，而是：
  - 后续如果继续改页面结构，需要保持 `pages.rs` 入口和内部 re-export 一致
  - `settings_page` 目前虽然已目录化，但它仍然是“组合壳 + save/sync/themes 子能力”，不是像 `entries_page` 那样的完整单页状态机；这是刻意保留的边界，不应误当成未完成状态去强行统一
- 建议下一步：
  - 若继续沿重构线推进，优先评估是否把 `feeds_page` 也收成显式目录模块
  - 或者转去做一轮更细的 view shell 收薄，继续减少页面壳中零散的 `use_resource/use_effect`
 重新汇入日汇总，避免目录干净了但信息丢失。

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `cargo check -p rssr-infra`：通过
- `cargo test -p rssr-infra --test test_refresh_contract_harness`：通过
- `cargo test -p rssr-infra --test test_subscription_contract_harness`：通过
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_subscription_contract_harness --no-run`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_config_exchange_contract_harness --no-run`：通过
- `cargo test -p rssr-web`：通过
- `git diff --check`：通过
- `bash -n scripts/run_wasm_contract_harness.sh scripts/run_wasm_refresh_contract_harness.sh scripts/run_wasm_subscription_contract_harness.sh scripts/run_wasm_config_exchange_contract_harness.sh`：通过
- CI workflow YAML 解析：通过
- `docker.yml` 触发条件与 push gate 代码级复核：通过
- `bash -n scripts/run_ci_local_container.sh`：通过
- `bash scripts/run_ci_local_container.sh --cmd 'echo ok'`：通过
- `bash scripts/run_ci_local_container.sh --cmd 'cargo test -p rssr-web -- --nocapture'`：通过
- `bash scripts/run_ci_local_container.sh --cmd 'bash scripts/setup_chrome_for_testing.sh >/dev/null; bash scripts/run_wasm_refresh_contract_harness.sh'`：通过
- `bash scripts/run_ci_local_container.sh --cmd 'bash scripts/setup_chrome_for_testing.sh >/dev/null; bash scripts/run_wasm_refresh_contract_harness.sh; bash scripts/run_wasm_subscription_contract_harness.sh; bash scripts/run_wasm_config_exchange_contract_harness.sh'`：通过

### 手工验收

- Chrome MCP：文章页已读切换、阅读页快捷键、设置页输入与保存：通过
- `wasm-pack test --headless --chrome crates/rssr-infra --test wasm_refresh_contract_harness`：失败，但失败原因为当前 crate 下 native-only 集成测试也被一起按 wasm 编译，不是 wasm harness 本身错误
- WSL2 本地 browser runner：受 `chromedriver bind() failed: Cannot assign requested address (99)` 限制，真实浏览器执行依赖 GitHub Actions
- Chrome for Testing / chromedriver 安装与版本对齐：通过
- `bash scripts/run_wasm_refresh_contract_harness.sh`：仍为 `env-limited`，但当前输出已进入 WSL2 绑定异常路径，不再是缺少 browser capability 配置的未知失败
- Linux CI 容器中的真实 browser 执行：
  - `run_wasm_refresh_contract_harness.sh`：通过
  - `run_wasm_subscription_contract_harness.sh`：通过
  - `run_wasm_config_exchange_contract_harness.sh`：通过

## 结果

- `zheye-mainline-stabilization` 中最核心的三条 contract 线，已经在当前 `main` 上完成重建。
- headless 接口命名与字段/动作边界进一步清晰。
- 当前主线已经具备统一的 wasm browser harness runner 与 CI matrix。
- 本地 `Dockerfile.ci-local` + `run_ci_local_container.sh` 现在既能支持交互式排障，也能支持脚本化单命令测试执行。
- wasm browser harness 的 `404 + SIGKILL` 根因已经被定位并在本地 Linux 容器中验证修复。
- 到此可以更准确地说：
  - `zheye` 分支的主功能与架构价值，已经基本被当前 `main` 手动吸收
  - 剩余重点不再是“还缺哪条主线”，而是观察远端 CI 是否把三条 wasm contract 线全部跑绿

## 风险与后续事项

- `entries_page` 在目录化与 workspace 收薄后，一度出现 `/entries` 登录后白屏。根因不是资源 404，也不是旧 localStorage 兼容，而是页面工作区里把 `EntryQuery` 和“保存浏览偏好”这两条副作用链建立在了 `session.snapshot()` 的隐式读取上；在 wasm 下这会把资源刷新和状态写回耦合成主线程卡死。
- 目前已把 `entries_page` 的加载与保存触发改成显式依赖：
  - `LoadEntries` 直接消费在 view shell 外部推导好的 `EntryQuery`
  - `SaveBrowsingPreferences` 直接消费显式传入的偏好快照
  - `remember_last_opened_feed` / `load_preferences` 也改成以 `feed_id` 为边界的显式 reactive 挂载
- Chrome MCP 复测结果：
  - 全新隔离浏览器上下文下，先完成本地登录，再进入 `http://127.0.0.1:8081/entries`
  - 修复前：页面线程挂死，`take_snapshot` / `evaluate_script` 超时，用户视角是纯白屏
  - 修复后：页面正常渲染出顶部导航、文章标题、“显示筛选与组织”按钮，以及空状态文案“没有可显示的文章，先去订阅页添加并刷新 feed。”
- `wasm-pack test` 不是当前仓库的正式 wasm harness 入口；若要强行支持，需要额外拆分 native-only integration tests。
- WSL2 本地环境仍无法可靠执行 `chromedriver` 真实浏览器链路，需以 GitHub Actions 结果为准。
- 后续更值得做的是观察远端 `wasm-browser-contract` matrix 是否全绿，并在必要时针对 CI 差异修补脚本或环境。
- 这次 runner 修复主要面向 GitHub Actions 的 Chrome 会话稳定性；真正是否彻底收口，仍需下一次远端 `wasm-browser-contract` matrix 结果确认。
- Docker workflow 现在仍会在 `main` push 时运行一次完整 smoke；如果未来还想进一步节流，可以再评估是否把这条 workflow 只留给 tag 与手动触发。
- 本地 CI 容器当前是“最小可复用基座”，还没有把全部 GitHub Actions job 包装成统一入口脚本；后续如长期使用，可以再加一层 `scripts/run_ci_local_checks.sh`。

## 给下一位 Agent 的备注

- 如果继续 contract harness 方向，先看 `docs/testing/contract-harness-rebuild-plan.md` 与 `scripts/run_wasm_contract_harness.sh`。
- 如果有人再次尝试 `wasm-pack test` 直跑 `crates/rssr-infra`，先读 `docs/testing/environment-limitations.md`，不要误判为 harness 回归。
- 如果要继续追 `zheye` 分支遗产，优先级已经明显下降；更高优先级是：
  - 看 `wasm-browser-contract` matrix 的真实结果
  - 再决定是否需要为 CI 差异补环境脚本
