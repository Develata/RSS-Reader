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

### 当日后续重构：App shell 再向语义壳收一层

- `AppNav` 不再自己持有导航与搜索的交互细节，而是开始明确消费 `ui/shell.rs` 中的 `AppNavShell`：
  - `nav_hidden`
  - `show_nav`
  - `hide_nav`
  - `entry_search`
  - `submit_search`
  - `focus_search`
- 这样 `app.rs` 里的导航壳继续变薄，更接近“默认语义 DOM + shell state”的结构，而不是直接编排导航行为。
- 同时补了一轮 DOM 语义整理，避免把字段型接口误标成动作：
  - 顶部搜索框增加 `data-field="entry-search"`
  - 文章页组织方式选择器从条件 `data-action` 改成稳定 `data-field="entry-grouping-mode"`
  - “显示已归档文章”开关从 `data-action` 改成 `data-field="show-archived"`
- 这一轮的目标不是视觉变化，而是让：
  - `App shell`
  - `page shell`
  - CSS / 自动化 / future headless interface
  共享同一套更稳定的语义接口。

### 当日后续文档收口：把目标架构与当前实现正式对齐

- 新增设计文档：
  - `docs/design/ui-shell-bus-page-facade.md`
- 这份文档不再讲“未来应该怎么做”，而是明确记录当前主线已经落成的实现边界：
  - `ui/shell`
  - `UiCommand / UiRuntime / UiIntent`
  - `page facade`
  - semantic page shell
- 同时把设计文档索引和旧目标文档接起来：
  - `docs/design/README.md`
  - `docs/design/headless-active-interface.md`
  - `docs/design/frontend-command-reference.md`
- 这样后续继续推进 `headless active interface + CSS 完全分离 + infra` 时，不再只有愿景文档和接口清单，而有一份明确的“当前实现版边界说明”作为正式基线。

### 当日后续收口：阅读筛选字段语义继续去动作化

- `crates/rssr-app/src/components/entry_filters.rs` 里原先仍把阅读筛选项暴露为动作接口：
  - `filter-unread`
  - `filter-read`
  - `filter-starred`
  - `filter-unstarred`
- 这类接口本质上是在表达持续筛选状态，不是触发一次性副作用，因此继续收成稳定字段接口：
  - `data-field="read-filter-unread"`
  - `data-field="read-filter-read"`
  - `data-field="starred-filter-starred"`
  - `data-field="starred-filter-unstarred"`
  - `data-field="entry-source-filter"`
- 同时更新 `docs/design/frontend-command-reference.md`：
  - 从 `data-action` 清单移除这组筛选项
  - 把当前稳定 `data-field` 接口系统化列出
- 这一步的意义不是改视觉，而是继续确保：
  - `data-action` 只表示真正的命令语义
  - `data-field` 只表示持续输入或选择状态
  - CSS 与自动化不再需要根据旧命名猜测某个输入到底是字段还是动作

### 当日后续收口：补 `data-state` 作为默认状态语义面

- 为了继续推进 `headless active interface + CSS 完全分离 + infra`，开始把一些原先主要靠 class
  名、按钮文案或 disabled 状态表达的默认状态，收成稳定 `data-state`：
  - `status-banner` 直接暴露 tone：
    - `info`
    - `error`
    - `success`
  - 设置页保存按钮：
    - `pending`
    - `idle`
  - WebDAV 下载按钮：
    - `confirm`
    - `idle`
  - 主题卡片：
    - `active`
    - `inactive`
  - 配置导入按钮：
    - `confirm`
    - `idle`
  - 已保存订阅区：
    - `empty`
    - `populated`
  - 删除订阅按钮 / 卡片：
    - `confirm`
    - `idle`
  - 阅读页：
    - 页面级 `error / loaded`
    - 正文 `html / text`
    - 上一篇 / 下一篇 `available / unavailable`
    - 已读按钮 `read / unread`
    - 收藏按钮 `starred / unstarred`
- 同时同步更新 `docs/design/frontend-command-reference.md`：
  - 新增“状态接口”小节
  - 把 `data-state` 的角色从隐含约定变成正式设计接口

### 当日后续重构：全局 UI 命令面第一刀

- 在 `crates/rssr-app/src/ui/` 下新增最小全局 UI 层骨架：
  - `mod.rs`
  - `commands.rs`
  - `snapshot.rs`
  - `runtime.rs`
- 这一刀的目标不是引入新的大 store，而是先建立统一的：
  - `UiCommand`
  - `UiIntent`
  - `UiRuntime`
- `App()` 根壳不再直接执行：
  - `AppServices::shared()`
  - `load_settings()`
  - `ensure_auto_refresh_started()`
- 这些初始化动作现在改走：
  - `UiCommand::LoadAuthenticatedShell`
  - `UiIntent::AuthenticatedShellLoaded`
- `StartupPage` 也不再在页面函数里直接做“读取设置 -> 决定启动路由 -> 跳转”，而是改走：
  - `UiCommand::ResolveStartupRoute`
  - `UiIntent::StartupRouteResolved`
- 这一步的意义不是减少几行代码，而是把最后两个明显的 UI 壳层直连 service 点先拔掉，为后续把页面退化成 headless active interface 的语义壳打基础。

### 当日后续重构：entries / reader / feeds 接入统一 UiRuntime

- `entries_page`：
  - 删掉旧的 `queries.rs`
  - 页面 runtime 不再直接触碰 `AppServices`
  - `Bootstrap / LoadEntries / ToggleRead / ToggleStarred / SaveBrowsingPreferences` 全部改为先映射到 `UiCommand`
  - 再由 `UiRuntime` 回灌 `EntriesPageIntent`
- `reader_page`：
  - `LoadEntry / ToggleRead / ToggleStarred` 三类 effect 也改成统一映射到 `UiCommand`
  - `reader_page/runtime.rs` 不再自己保存一套独立 service 调用与正文选择流程
  - 正文装配、状态消息与 reload intent 现在由 `UiRuntime` 统一产出
- `feeds_page`：
  - 删掉旧的 `dispatch.rs`
  - 删掉旧的 `queries.rs`
  - `LoadSnapshot / AddFeed / Refresh / Remove / ImportExport` 这组原先分散在 `queries + dispatch` 的 service 调用统一提升到 `UiRuntime`
  - 页面本地 runtime 现在只负责：
    - `FeedsPageEffect -> UiCommand` 映射
    - 剪贴板读取这类浏览器局部能力
    - `UiIntent -> FeedsPageIntent` 转发
- 到这一步为止，`entries / reader / feeds` 三个主页面已经进入同一类总线形状：
  - 页面 session 发 page-local effect
  - page-local runtime 做薄映射
  - `UiRuntime` 负责真正的 service / application 调用
  - 再回灌 page-local intent
- 也就是说，页面不再各自保存第二套“直接碰 service 的 runtime 语义”，已经开始朝“页面只是 active shell，行为统一由全局 runtime 总线承接”的方向收口。

### 当日后续重构：settings_page service 路径并入 UiRuntime

- 在 `entries / reader / feeds` 已经接入 `UiRuntime` 之后，继续把 `settings_page` 相关的真实 service 路径也统一并入总线：
  - `settings_page/runtime.rs` 的主加载链
  - `settings_page/save/runtime.rs` 的保存链
  - `settings_page/sync/runtime.rs` 的 WebDAV push/pull 链
- 为此扩展了全局 UI 命令面：
  - `UiCommand::SettingsLoad`
  - `UiCommand::SettingsSaveAppearance`
  - `UiCommand::SettingsPushConfig`
  - `UiCommand::SettingsPullConfig`
- 同时引入 `UiIntent::SettingsPage(SettingsPageIntent)`，让设置页现有的 page-local intent 可以直接作为总线回灌目标，而不需要再保留一套平行的 runtime 返回协议。
- 这次并没有把 `open_repository()` 这类纯前端壳行为硬塞进总线：
  - 该动作不涉及 `AppServices`
  - 仍保留在 `SettingsPageSession`
  - 这样总线只负责真正需要 application / infra 的命令，边界更干净
- 完成这一步之后，四个主页面的真实 service 访问都已经有了同一归宿：
  - `entries_page`
  - `reader_page`
  - `feeds_page`
  - `settings_page`
- 页面侧现在保留的 runtime 更多只是：
  - page-local effect 到 `UiCommand` 的薄映射
  - 浏览器局部能力（例如 clipboard）
  - page-local intent 的回灌
- 也就是说，`rssr-app` 正在从“每页各自一套半独立 runtime”转向：
  - 页面只保留语义壳和局部交互接口
  - `UiRuntime` 统一承接真实行为
  - CSS 后续可以在不破坏行为层的前提下更彻底接管布局与显示

### 当日后续重构：继续压薄 page-local runtime

- 在四个主页面都已经接到 `UiRuntime` 之后，继续做第二轮收口，不再保留那些只会“把 page-local effect 映射成 `UiCommand`”的空壳 runtime。
- `entries_page`：
  - 直接在 `EntriesPageSession` 内派发：
    - `UiCommand::EntriesBootstrap`
    - `UiCommand::EntriesLoadEntries`
    - `UiCommand::EntriesToggleRead`
    - `UiCommand::EntriesToggleStarred`
    - `UiCommand::EntriesSaveBrowsingPreferences`
  - 删除 page-local 的：
    - `effect.rs`
    - `runtime.rs`
- `reader_page`：
  - `ReaderPageSession` 现在直接派发：
    - `UiCommand::ReaderLoadEntry`
    - `UiCommand::ReaderToggleRead`
    - `UiCommand::ReaderToggleStarred`
  - 删除 page-local 的：
    - `effect.rs`
    - `runtime.rs`
- `settings_page`：
  - 主页面加载不再通过 `SettingsPageEffect / SettingsPageRuntime`
  - `SettingsPageSession` 直接派发 `UiCommand::SettingsLoad`
  - `save/session.rs` 与 `sync/session.rs` 也直接派发：
    - `UiCommand::SettingsSaveAppearance`
    - `UiCommand::SettingsPushConfig`
    - `UiCommand::SettingsPullConfig`
  - 删除：
    - `settings_page/effect.rs`
    - `settings_page/runtime.rs`
    - `settings_page/save/effect.rs`
    - `settings_page/save/runtime.rs`
    - `settings_page/sync/effect.rs`
    - `settings_page/sync/runtime.rs`
- `feeds_page`：
  - 保留 page-local 的 reducer / state / session
  - 但不再维持额外的 `commands.rs` 和 `runtime.rs`
  - reducer 直接产出两类 effect：
    - `Dispatch(UiCommand)`
    - `ReadFeedUrlFromClipboard`
  - 这样 `feeds_page` 本地层现在只保留浏览器局部动作（clipboard）这一类总线外行为
- 到这一步为止，页面本地层和全局总线的边界更明确了：
  - 全局 bus 负责所有 application / infra 行为
  - 页面本地层只负责：
    - page-local state / reducer
    - 语义 intent
    - 极少量浏览器局部能力
- 这比前一轮“page-local runtime + UiRuntime”的混合态又更接近目标：
  - 页面趋向纯语义壳
  - CSS 可以继续接管结构和显示
  - `UiRuntime` 成为稳定的行为承接面

### 当日后续重构：继续压薄 page-local runtime

- 在 `UiRuntime` 已经接住四个主页面的真实 service 路径之后，继续做第二步收口：
  - 不再满足于“page-local runtime 只是薄映射”
  - 直接让 page session 发 `UiCommand`
  - 删掉已经退化成中转壳的本地 `effect/runtime`
- 这次被进一步压薄的页面/子能力包括：
  - `entries_page`
  - `reader_page`
  - `settings_page`
  - `settings_page/save`
  - `settings_page/sync`
- 具体变化：
  - `EntriesPageSession` 直接发：
    - `UiCommand::EntriesBootstrap`
    - `UiCommand::EntriesLoadEntries`
    - `UiCommand::EntriesToggleRead`
    - `UiCommand::EntriesToggleStarred`
    - `UiCommand::EntriesSaveBrowsingPreferences`
  - `ReaderPageSession` 直接发：
    - `UiCommand::ReaderLoadEntry`
    - `UiCommand::ReaderToggleRead`
    - `UiCommand::ReaderToggleStarred`
  - `SettingsPageSession` 直接发：
    - `UiCommand::SettingsLoad`
  - `SettingsPageSaveSession` 直接发：
    - `UiCommand::SettingsSaveAppearance`
  - `SettingsPageSyncSession` 直接发：
    - `UiCommand::SettingsPushConfig`
    - `UiCommand::SettingsPullConfig`
- 删除的页面本地中间层：
  - `entries_page/effect.rs`
  - `entries_page/runtime.rs`
  - `reader_page/effect.rs`
  - `reader_page/runtime.rs`
  - `settings_page/effect.rs`
  - `settings_page/runtime.rs`
  - `settings_page/save/effect.rs`
  - `settings_page/save/runtime.rs`
  - `settings_page/sync/effect.rs`
  - `settings_page/sync/runtime.rs`
- 这样做之后，页面本地层的职责进一步收缩成：
  - 语义 DOM
  - page-local reducer / intent（仍然保留局部 UI 状态整理）
  - 少量浏览器局部能力
  - `UiCommand` 的直接分发
- 从结构上看，这一步比“引入全局总线”更重要，因为它真正去掉了“总线下面再藏一层 page-local runtime 适配器”的残余中间层。

### 当日后续重构：feeds_page 收平到与其它页面同层

- 在 `entries / reader / settings` 都已经去掉本地 `effect/runtime` 之后，继续把 `feeds_page` 也收平：
  - 删除 `feeds_page/effect.rs`
  - `reduce_feeds_page_intent(...)` 不再产出 `FeedsPageEffect`
  - 现在直接产出 `UiCommand`
  - `FeedsPageSession` 直接用 `spawn_projected_ui_command(...)` 分发命令并回灌 `FeedsPageIntent`
- 这意味着 `feeds_page` 不再是“四个主页面里唯一还保留本地 runtime 壳层”的例外。
- 剪贴板读取也已经被总线化：
  - `UiCommand::FeedsReadFeedUrlFromClipboard`
  - 由 `UiRuntime` 统一处理，再回灌 `FeedsPageIntent::FeedUrlChanged` 或错误状态
- 做完这一步后，四个主页面的结构已经更接近同一形态：
  - page-local session
  - page-local reducer / intent（管理局部 UI 状态）
  - 直接发 `UiCommand`
  - `UiRuntime` 统一承接真实行为

### 当日后续整理：ui helper 收口

- 既然页面已经统一改成：
  - `visit_ui_command`
  - `collect_projected_ui_command`
  - `spawn_projected_ui_command`
  这三类主入口
- 那么 helper 层里未被实际消费的：
  - `spawn_visited_ui_command`
  - `apply_projected_ui_command` 的公开 re-export
  也一并清掉，避免总线刚建立就留下死 helper。

### 当前验证与限制补充

- 本轮 UI 总线扩展后的验证：
  - `cargo fmt --all`
  - `cargo check -p rssr-app`
  - `cargo check -p rssr-app --target wasm32-unknown-unknown`
  - `cargo check -p rssr-app --target aarch64-linux-android`
  - `git diff --check`
- 这次尝试补新的 Chrome MCP 页面实测时，MCP 侧仍返回：
  - `Transport closed`
- 因此本轮没有新增浏览器自动化验收记录；当前浏览器侧验证受限于 MCP transport，而不是代码编译或页面路由本身失败。
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

## 追加：页面会话收口与 browser state 写放大优化

### settings themes 保存语义并回页面 save session

- 主题实验室里原先绕过页面保存链的“即时保存 / 回滚”逻辑，现已收回 [save/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/session.rs)。
- 关键代码调整：
  - [save/effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/effect.rs) 的 `SaveAppearance` 改为显式携带 `success_message`。
  - [save/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/runtime.rs) 不再硬编码单一成功提示，而是接受调用方传入的主题相关文案。
  - [save/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/session.rs) 增加 `save_with_message(...)`，让主题实验室与“保存设置”按钮共享一条保存链。
  - [theme_apply.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/theme_apply.rs) 删除了直接调 service 并手写回滚的旧路径，只负责更新 `draft / preset_choice`，随后把真正的写入交给 `SettingsPageSaveSession`。
  - [lab.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/lab.rs)、[presets.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/presets.rs)、[theme_io.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/theme_io.rs) 统一改成围绕 `SettingsPageSession + SettingsPageSaveSession` 工作。
  - [appearance.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/appearance.rs) 明确把 `save_session` 作为页面级能力向主题模块传递。
- 结果：
  - 主题实验室不再维护第二套“设置写入”语义。
  - 当前所有设置写入都会经过统一的校验、保存、成功提示与失败回滚链路。

### feeds_page 去掉旧 Outcome / Patch 中间层

- 订阅页原先虽然已经有 `intent / reducer / effect / runtime / session` 外壳，但命令执行结果仍然通过旧的 `FeedsPageCommandOutcome + FeedsPageUiPatch` 回灌 reducer。
- 这层旧 DTO 现已完全移除：
  - [commands.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/commands.rs) 只保留 `FeedsPageCommand`。
  - [dispatch.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/dispatch.rs) 现在直接返回显式 `FeedsPageIntent` 列表，例如：
    - `SetStatus`
    - `ConfigTextExported`
    - `OpmlTextExported`
    - `PendingConfigImportSet`
    - `PendingDeleteFeedSet`
    - `BumpReload`
  - [intent.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/intent.rs) 不再保留 `CommandCompleted(...)`。
  - [reducer.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/reducer.rs) 不再解释 `outcome.patch.*`，只处理明确结果。
  - [runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/runtime.rs) 现在把 `execute_command(...)` 直接展开成 `Vec<FeedsPageIntent>`。
- 结果：
  - `feeds_page` 从“新壳包旧 DTO”回到了单一状态流。
  - 订阅页的 local workspace 结构和 `entries / reader` 更接近了。

### entries / reader / feeds 的 effect 调度风格继续靠齐

- [entries_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/session.rs) 新增私有 `apply_runtime_outcome(...)`，不再在 `spawn_effect(...)` 中直接散开 bindings。
- [reader_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/session.rs) 从原先每个动作各写一段 `spawn(async move { ... })`，收成和 `entries / feeds` 一致的 `spawn_effect(...)` + `apply_runtime_outcome(...)`。
- 结果：
  - 三个页面都更接近“页面壳只发动作，session 统一调度 effect，runtime 只产出 intent，reducer 只负责本地状态变化”的模式。
  - 当前仍然没有引入全局统一命令层；这一层依然刻意推迟。

### browser persisted-state 写放大第一轮收窄

- 问题背景：
  - 原先 browser adapter 对高频小更新（已读 / 收藏 / `last_opened_feed_id`）也会锁住整份 `PersistedState`，clone 整体，再全量写回 `rssr-web-state-v1`。
- 本轮新增 sidecar key：
  - `rssr-web-app-state-v1`
  - `rssr-web-entry-flags-v1`
- 关键实现位于 [state.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/state.rs)：
  - `load_state()` 会在加载主状态后叠加 sidecar：
    - `last_opened_feed_id`
    - entry 的 `is_read / is_starred / read_at / starred_at / updated_at`
  - `save_state_snapshot(...)` 仍然保存主快照，但也会同步刷新 sidecar。
- [adapters.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/adapters.rs) 中高频路径已改成窄写入：
  - `set_read(...)` → `save_entry_flag_patch(...)`
  - `set_starred(...)` → `save_entry_flag_patch(...)`
  - `save_last_opened_feed_id(...)` → `save_app_state_slice(...)`
  - `clear_last_opened_feed_if_matches_impl(...)` → `save_app_state_slice(...)`
- 同时更新了三份 wasm browser harness 的 localStorage 清理逻辑，让测试会把 sidecar key 一起清理干净。

## 本轮补充验证

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-infra`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `cargo test -p rssr-infra --test test_subscription_contract_harness`：通过
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_subscription_contract_harness --no-run`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_config_exchange_contract_harness --no-run`：通过
- `git diff --check`：通过

### Chrome 实际回归

- 使用 Chrome MCP 在 `http://127.0.0.1:8081/entries` 下进入 web 应用。
- 当前浏览器环境已有本地 auth 配置，但没有现成明文密码；本轮回归通过读取 `rssr-web-auth-config-v1` 并按 [web_auth.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/web_auth.rs) 当前算法生成会话 token 写回 `sessionStorage` 的方式完成本地解锁，未改动仓库代码。
- 实际结果：
  - `/entries`：正常进入，显示导航、标题、“显示筛选与组织”按钮和空状态文案。
  - `/feeds`：正常进入，统计卡片、添加订阅、配置交换与 OPML 区块正常渲染。
  - `导出配置`：正常生成 JSON，并出现“已导出配置包 JSON。”状态提示。
  - `/settings`：正常进入，阅读外观、主题实验室、主题预设、WebDAV 同步卡片正常渲染。
  - `ThemePresetSections -> Atlas Sidebar`：点击后出现“已应用示例主题：Atlas Sidebar。”状态提示，说明主题预设已经成功走进统一 save session。
  - console：未出现新的 `error` / `warn`。

## 当前判断

- `settings themes` 的保存语义分叉已收口。
- `feeds_page` 的旧 `Outcome / Patch` 中间层已删除，订阅页不再停留在半迁移状态。
- `entries / reader / feeds` 的 effect 调度风格已经明显靠齐，但仍然不建议此时再往上抽全局命令面。
- browser persisted-state 写放大已经完成第一轮优化；高频小写入不再每次重刷整份主状态，后续如继续深挖，再评估 feed metadata 或 settings 是否也值得更细颗粒持久化。

## 追加：进一步去掉残留双轨

### entries / reader 去掉 bindings 中间层

- [entries_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/session.rs) 与 [reader_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/session.rs) 现在不再通过 `bindings.rs` 把 runtime outcome 二次转发到 reducer。
- 两页都直接在 session 内：
  - `spawn_effect(...)`
  - `execute_*_effect(...)`
  - 遍历 `outcome.intents`
  - 直接 `dispatch_*_intent(...)`
- 对应地：
  - [entries_page/bindings.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/bindings.rs) 已删除
  - [reader_page/bindings.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/bindings.rs) 已删除
  - [entries_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs) 与 [reader_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/mod.rs) 不再声明 `bindings` 模块
- 结果：
  - `entries / reader / feeds` 三页都回到更一致的单一局部状态流。
  - 之前那层“session 内已统一调度，但仍绕一层 bindings”的过渡味道已经去掉。

### browser persisted-state 不再在主状态序列化 sidecar 已接管字段

- [state.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/state.rs) 中以下字段已改成只在内存里保留，不再参与主状态 `rssr-web-state-v1` 的序列化/反序列化：
  - `PersistedState::last_opened_feed_id`
  - `PersistedEntry::is_read`
  - `PersistedEntry::is_starred`
  - `PersistedEntry::read_at`
  - `PersistedEntry::starred_at`
- 这些字段现在的唯一持久化来源是 sidecar：
  - `rssr-web-app-state-v1`
  - `rssr-web-entry-flags-v1`
- 同时，`entry flags` sidecar 不再重复持久化 `updated_at`，减少与主 entry 内容快照的重叠。
- 结果：
  - browser storage 里不再存在“主状态也写一份，sidecar 再写一份”的显式双轨字段。
  - 主状态负责内容主体，sidecar 负责高频状态切片，职责边界更清楚。

## 这一轮后的判断

- 之前最明确的双轨已经被清掉：
  - `settings themes` 保存语义双轨：已消失
  - `feeds_page` 旧 `Outcome / Patch` 双轨：已消失
  - `entries / reader` bindings 中间层：已消失
  - browser 主状态与 sidecar 的重复持久化字段：已消失
- 当前剩下的结构差异主要是**有意设计差异**，不再是历史兼容包袱：
  - `settings_page` 仍然是“组合壳 + save/sync/themes 子 session”，这是刻意保持的页面边界
  - 还没有引入全局统一命令面，这也是刻意延后，而不是过渡残留

## 追加：继续按重构优先级收口

### settings_page 主加载链 session 化

- 本轮新增：
  - [effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/effect.rs)
  - [intent.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/intent.rs)
  - [runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/runtime.rs)
- [session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/session.rs) 不再直接在 `load()` 里调用 `AppServices::shared().await` 和 `services.load_settings().await`。
- 现在改成：
  - 页面壳触发 `session.load()`
  - session 只发 `SettingsPageEffect::LoadSettings`
  - runtime 执行副作用
  - runtime 返回 `SettingsPageIntent`
  - session 统一 `dispatch(...)`
- 新增了页面级 helper：
  - `apply_loaded_settings(...)`
  - `restore_settings(...)`
  - `set_status(...)`
- 结果：
  - 设置页主加载链不再是“页面壳 + session 直调 service”的半旧模式。
  - 页面级状态写回入口比之前统一。

### settings_page sync 与页面 session 的边界统一

- [sync/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/runtime.rs) 不再返回：
  - `status_message`
  - `status_tone`
  - `imported_settings`
  这类 ad-hoc 结构。
- 现在它直接产出 `Vec<SettingsPageIntent>`：
  - `SettingsLoaded(...)`
  - `SetStatus { ... }`
- [sync/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/session.rs) 也不再手动改：
  - `preset_choice`
  - `draft`
  - `theme.settings`
  - `status`
- 现在的边界变成：
  - `sync session` 只负责自己的局部状态，例如 `pending_remote_pull`
  - 页面级设置、主题、状态提示统一回到 `SettingsPageSession`
- 同时 [save/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/session.rs) 也改成复用页面 session 的恢复与加载逻辑，不再自己散改 `theme/draft/preset_choice`

### entries_page 的 workspace hook 再收一层

- 之前 [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs) 里有 4 条独立加载 resource：
  - `remember_last_opened_feed`
  - `load_preferences`
  - `load_feeds`
  - `load_entries`
- 本轮把前三条合并成一个更清楚的 workspace 启动 effect：
  - [effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/effect.rs) 新增 `Bootstrap { feed_id, load_preferences, load_feeds }`
  - [session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/session.rs) 新增 `bootstrap(...)`
  - [runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/runtime.rs) 统一执行：
    - remember last feed
    - load preferences
    - load feeds
- 现在页面壳只保留：
  - 一条 bootstrap resource
  - 一条 entries resource
  - 一条 preferences save effect
- 结果：
  - `entries_page` 的 hook 编排明显更薄
  - workspace 生命周期入口更明确

### browser state 第二轮 slice 化

- 本轮把 browser state 从“单个 `PersistedState` + 隐式 sidecar 覆盖”进一步收成了显式复合结构：
  - [state.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/state.rs)
  - 新增 `BrowserState { core, app_state, entry_flags }`
- 主状态现在只承担核心内容：
  - `feeds`
  - `entries` 主体内容
  - `settings`
  - `next_feed_id`
  - `next_entry_id`
- `last_opened_feed_id` 已正式移出主状态，进入：
  - `PersistedAppStateSlice`
- entry flags 也已正式移出主 entry 结构，进入：
  - `PersistedEntryFlagsSlice`
  - `PersistedEntryFlag`
- [PersistedEntry](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/state.rs) 现在不再携带：
  - `is_read`
  - `is_starred`
  - `read_at`
  - `starred_at`
- [query.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/query.rs) 和 [adapters.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters/browser/adapters.rs) 已改成围绕 `BrowserState` 工作：
  - feed / entry / settings 仓储改读 `core`
  - app-state adapter 改读写 `app_state`
  - read/starred 改读写 `entry_flags`
- wasm browser harness 也同步改到了新的显式 slice 结构：
  - [wasm_refresh_contract_harness.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/tests/wasm_refresh_contract_harness.rs)
  - [wasm_subscription_contract_harness.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/tests/wasm_subscription_contract_harness.rs)
  - [wasm_config_exchange_contract_harness.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/tests/wasm_config_exchange_contract_harness.rs)
- 结果：
  - browser state 已不再停留在“逻辑 slice 化、内存结构仍然混在一起”的半步状态
  - adapter/query/tests 的边界都更清晰

## 这一轮验证

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app -p rssr-infra`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `cargo test -p rssr-infra --test test_subscription_contract_harness --test test_config_exchange_contract_harness`：通过
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_subscription_contract_harness --test wasm_config_exchange_contract_harness --test wasm_refresh_contract_harness --no-run`：通过
- `git diff --check`：通过

### Chrome 实际回归

- 使用 Chrome MCP 对 web 端做了实际检查。
- 结果：
  - `/entries`：正常进入，标题与空状态正常显示
  - `/feeds`：正常进入，统计卡片、订阅输入与配置交换区正常显示
  - `/settings`：正常进入
  - 在设置页点击主题预设 `Newsprint`：
    - 页面出现“已应用示例主题：Newsprint。”
    - 说明 `themes -> save session -> page session` 这条统一链路实际可用
  - console：没有新的 `error` / `warn`

## 本轮判断

- `settings_page` 的页面级加载链和 `sync` 边界已经明显收口，不再各自私下直写页面状态。
- `entries_page` 的 workspace hook 已比之前更像单一工作台生命周期入口。
- browser state 已从“主状态 + sidecar 逻辑覆盖”推进到显式 slice 结构，长期维护成本更低。
- 到这里，继续只做 page-local runtime 已经开始接近过渡态；如果后续目标是 `headless active interface + CSS 完全分离 + infra`，就应开始往统一 UI 命令面推进。

## 追加：全局 UI 命令面第一刀

### 背景与目标

- 在页面层一致性审查后，发现真正还在 UI 壳层直接碰 service 的核心点主要是：
  - [App()](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/app.rs)
  - [StartupPage](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
- 为了把 UI 继续往 `headless active interface` 推，而不是无限细化各页 local runtime，本轮先落了最小全局总线骨架。

### 本轮新增

- 新增 [ui/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/mod.rs)
- 新增 [ui/commands.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/commands.rs)
- 新增 [ui/snapshot.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/snapshot.rs)
- 新增 [ui/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime.rs)
- [main.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/main.rs) 已注册新 `ui` 模块

### 最小总线骨架

- 当前只先定义两条命令：
  - `UiCommand::LoadAuthenticatedShell`
  - `UiCommand::ResolveStartupRoute`
- 当前只先落两类 snapshot / intent：
  - `AuthenticatedShellSnapshot`
  - `StartupRouteSnapshot`
  - `UiIntent::{AuthenticatedShellLoaded, StartupRouteResolved, SetStatus}`
- [ui/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime.rs) 现在是这条最小统一入口：
  - 接收 `UiCommand`
  - 调 `AppServices`
  - 产出 `UiIntent`

### App 根壳接入

- [App()](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/app.rs) 不再在认证通过后直接：
  - `AppServices::shared()`
  - `load_settings()`
  - `ensure_auto_refresh_started()`
- 现在改成：
  - `execute_ui_command(UiCommand::LoadAuthenticatedShell)`
  - 再应用 `UiIntent::AuthenticatedShellLoaded(...)`
- 结果：
  - 根壳不再直接碰 service 初始化细节
  - settings hydrate 和 auto refresh 启动都收进了最小 UI runtime

### StartupPage 接入

- [StartupPage](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs) 不再直接：
  - `AppServices::shared()`
  - `load_settings()`
  - `load_last_opened_feed_id()`
  - `list_feeds()`
  - 自己拼 startup route
- 现在改成：
  - `execute_ui_command(UiCommand::ResolveStartupRoute)`
  - 再处理：
    - `UiIntent::StartupRouteResolved(...)`
    - `UiIntent::SetStatus { ... }`
- 结果：
  - 启动页也从“页面壳自己决定启动导航”收进了统一 UI runtime

### 这一刀后的判断

- 这还不是完整“全局命令总线”，只是第一批最小切口。
- 但现在已经有了：
  - 统一 UI command 入口
  - 统一 UI runtime 执行入口
  - 统一 UI snapshot / intent 回灌形式
- 这意味着后续继续迁：
  - `entries`
  - `reader`
  - `feeds`
  - `settings`
  就不需要再新起一套 page-local 风格，而可以逐步吸收到同一总线。

## 追加验证

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

### Web 实测说明

- 本轮原计划使用 Chrome MCP 继续验证 `/` 启动页与 authenticated shell。
- 但在执行 MCP 导航前，Chrome MCP transport 会话中断，工具侧返回 `Transport closed`，因此本轮未能补做新的浏览器实测。
- 当前可以确认的是：
  - 代码层编译通过
  - bus 第一刀只影响 `App()` 与 `StartupPage`
  - 其余页面未被进一步改写

## 追加：继续压薄 page-local runtime

### 背景与目标

- 在四个主页面都接到 `UiRuntime` 之后，页面层仍然保留了一层“本地 runtime 包 bus”的薄转发。
- 这会让页面接口停在：
  - `page-local runtime + UiRuntime`
- 而不是继续退化成更接近：
  - `state + reducer + session + bus`
- 本轮目标就是继续压薄这一层，让页面更接近纯语义壳。

### 本轮变更

- `entries_page`
  - 删除 [effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/effect.rs)
  - 删除 [runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/runtime.rs)
  - [session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/session.rs) 现在直接派发 `UiCommand`
- `reader_page`
  - 删除 [effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/effect.rs)
  - 删除 [runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/runtime.rs)
  - [session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/session.rs) 现在直接派发 `UiCommand`
- `feeds_page`
  - 删除 [commands.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/commands.rs)
  - 删除 [runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/runtime.rs)
  - [effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/effect.rs) 现在只保留：
    - `LoadSnapshot`
    - `Dispatch(UiCommand)`
    - `ReadFeedUrlFromClipboard`
  - 也就是页面本地只保留浏览器剪贴板这种 bus 外局部能力
- `settings_page`
  - 删除 [effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/effect.rs)
  - 删除 [runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/runtime.rs)
  - 删除 [save/effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/effect.rs)
  - 删除 [save/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/runtime.rs)
  - 删除 [sync/effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/effect.rs)
  - 删除 [sync/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/runtime.rs)

### 统一 bus 接口继续收口

- [ui/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime.rs) 不再返回额外的 `UiRuntimeOutcome`
- 现在统一就是：
  - `UiCommand -> Vec<UiIntent>`
- [ui/snapshot.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/snapshot.rs) 给 `UiIntent` 增加了页面级投影 helper：
  - `into_authenticated_shell_loaded`
  - `into_startup_route_resolved`
  - `into_entries_page_intent`
  - `into_reader_page_intent`
  - `into_feeds_page_intent`
  - `into_settings_page_intent`
  - `into_status`

### 当前判断

- 到这里，`UiRuntime` 已经是四个主页面共同的真实行为承接面。
- 页面本地层现在主要只剩：
  - `state`
  - `intent`
  - `reducer`
  - `session`
  - 极少数浏览器局部能力
- 这比之前更接近目标中的：
  - `headless active interface + CSS 完全分离 + infra`

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `git diff --check`：通过

### 当日后续收口：page facade 命名统一

- 在四个 page facade 上继续做了一轮高价值统一，不再保留“同义不同名”的边界词汇：
  - `status()` 统一收成 `status_message()`
  - `feeds_page`：
    - `pending_config_import()` -> `is_config_import_pending()`
    - `feed_count()` -> `total_feed_count()`
    - `entry_count()` -> `total_entry_count()`
  - `settings_page`：
    - `pending_save()` -> `is_save_pending()`
    - `pending_remote_pull()` -> `is_remote_pull_pending()`
  - `entries_page`：
    - `archived_count()` -> `archived_entry_count()`
  - `reader_page`：
    - `previous_action_target()` -> `previous_entry_target()`
    - `next_action_target()` -> `next_entry_target()`
- 这轮不是简单改名，而是在把 facade 真正收成更稳定的 page boundary 词汇：
  - `status_message`
  - `status_tone`
  - `is_*_pending`
  - `total_*`
  - `*_entry_target`
- 相应页面与 section 也同步改成消费新命名：
  - `reader_page/mod.rs`
  - `feeds_page/mod.rs`
  - `feeds_page/sections/config_exchange.rs`
  - `settings_page/appearance.rs`
  - `settings_page/sync/mod.rs`
  - `entries_page/controls.rs`
  - `hooks/use_reader_shortcuts.rs`

### 当日后续收口：将展示策略继续推入 facade

- 继续把页面壳里手写的展示判断收回 facade，不让页面自己拼接：
  - `has_status_message()`
  - `save_button_label()`
  - `remote_pull_button_class()`
  - `remote_pull_button_label()`
  - `config_import_button_class()`
  - `config_import_button_label()`
  - `previous_entry_button_class()`
  - `next_entry_button_class()`
  - `read_toggle_icon()`
  - `read_toggle_label()`
  - `starred_button_class()`
  - `starred_toggle_icon()`
- 受影响边界：
  - `entries_page/facade.rs`
  - `reader_page/facade.rs`
  - `feeds_page/facade.rs`
  - `settings_page/facade.rs`
  - `reader_page/mod.rs`
  - `feeds_page/mod.rs`
  - `feeds_page/sections/config_exchange.rs`
  - `settings_page/appearance.rs`
  - `settings_page/sync/mod.rs`
  - `entries_page/controls.rs`
- 这轮之后，页面和 section/card/control 继续从“自己决定壳层展示策略”退化成“消费 facade 提供的默认语义策略”，更接近：
  - `headless active interface`
  - `CSS 完全分离`
  - `infra` 承担真实行为

### 当日后续收口：空状态与删除确认策略继续进入 facade

- 继续把仍然留在页面里的默认文案和危险态策略收回 facade：
  - `entries_page`：
    - `empty_entries_message()`
    - `archived_entries_state_message()`
    - `archived_entries_message()`
  - `feeds_page`：
    - `empty_feeds_message()`
    - `remove_feed_button_class(feed_id)`
    - `remove_feed_button_label(feed_id)`
- 页面和 section 不再自己决定：
  - “没有文章 / 没有订阅”空状态文案
  - “确认删除 / 删除订阅”按钮文案
  - 删除确认态对应的危险按钮样式
- 受影响文件：
  - `entries_page/facade.rs`
  - `entries_page/session.rs`
  - `entries_page/controls.rs`
  - `entries_page/mod.rs`
  - `feeds_page/facade.rs`
  - `feeds_page/sections/saved.rs`

### 当日后续收口：settings themes 也开始优先消费 facade

- `settings_page/themes` 里原本还留着比较明显的默认展示和选择策略：
  - 当前主题是否激活
  - 主题卡片 class
  - “当前已选 / 使用这套主题”按钮文案
  - “载入所选主题”按钮内部对 `none / custom / builtin` 的分支判断
  - `custom_css` 的直接读取与同步
- 这轮将其继续回收到 `SettingsPageFacade`：
  - `custom_css()`
  - `set_custom_css(...)`
  - `apply_selected_theme()`
  - `apply_builtin_theme(...)`
  - `clear_custom_css(...)`
  - `is_theme_preset_active(...)`
  - `theme_card_class(...)`
  - `theme_apply_button_class(...)`
  - `theme_apply_button_label(...)`
  - `remove_theme_preset(...)`
- `themes/presets.rs` 和 `themes/lab.rs` 现在优先通过 facade 获取这些策略和动作，不再自己判断当前主题卡片状态。
- 同时删掉了 `themes/theme_apply.rs` 里已经被 facade 吸收掉的旧 helper，避免留下新的死代码。

## 追加：把 entries_page 继续收成 facade 动作口边界

### 背景

- 前一轮已经给四个主页面都补上了 facade，但 `entries_page` 的控件区、目录区、文章卡片仍然直接碰：
  - `EntriesPageSession`
  - `session.dispatch(...)`
  - `session.toggle_*`
- 这意味着 `entries_page` 还是没完全达到“组件拿 facade，而不是拿 session”的边界要求。

### 本轮变更

- [entries_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/facade.rs)
  - 新增动作口：
    - `set_controls_hidden`
    - `set_grouping_mode`
    - `set_show_archived`
    - `set_read_filter`
    - `set_starred_filter`
    - `set_selected_feed_urls`
    - `toggle_directory_source`
    - `toggle_read`
    - `toggle_starred`
  - 并为 facade 本身补了 `Clone`

- [entries_page/controls.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/controls.rs)
  - 控件区不再直接 `session.dispatch(...)`
  - 目录区不再直接拿 `EntriesPageSession`
  - 现在统一通过 `EntriesPageFacade` 动作口操作

- [entries_page/cards.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/cards.rs)
  - 文章卡片不再直接拿 `EntriesPageSession`
  - 已读/收藏切换改走 facade

- [entries_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
  - 卡片渲染和目录渲染改成直接传 facade
  - 页面壳进一步从 “session + presenter + snapshot” 退向 “facade + 语义 DOM”

### 当前判断

- `entries_page` 现在比 `reader_page` / `feeds_page` 更接近统一的 facade 边界了。
- 页面组件树里最容易泄漏局部 session 的三个位置：
  - controls
  - directory
  - cards
  已经统一走 facade。
- 这一步没有引入新业务逻辑，纯粹是在继续压薄页面边界。

### 本轮验证

- `cargo check -p rssr-app`：通过
- `commit`：pending

## 追加：标准化 entries/settings facade 边界命名

### 背景

- 在前几轮 facade 收口后，四个主页面已经都有 page facade，但一致性还不够：
  - `reader_page` / `feeds_page` 已经开始转向明确的 accessor
  - `entries_page` 仍然保留 `ui/session/snapshot/presenter` 原始字段暴露
  - `settings_page` 的 facade 字段本身也还是公开的
- 这意味着 facade 的语义边界已经形成，但命名和访问习惯还不统一。

### 本轮变更

- [entries_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/facade.rs)
  - 把 `ui/session/snapshot/presenter` 收成私有字段
  - 补出更完整的 accessor：
    - 搜索/筛选相关：
      - `entry_search`
      - `set_entry_search`
      - `read_filter`
      - `starred_filter`
      - `selected_feed_urls`
    - 页面展示相关：
      - `controls_hidden`
      - `grouping_mode`
      - `show_archived`
      - `archive_after_months`
      - `status`
      - `status_tone`
      - `entries_is_empty`
      - `visible_entries_is_empty`
      - `visible_entries_len`
      - `archived_count`
    - presenter 快照相关：
      - `source_filter_options`
      - `group_nav_items`
      - `time_grouped_entries`
      - `source_grouped_entries`
      - `directory_months`
      - `directory_sources`
      - `expanded_directory_sources`

- [entries_page/controls.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/controls.rs)
  - 不再直接解构：
    - `facade.ui`
    - `facade.snapshot`
    - `facade.presenter`
  - 改成统一走 facade accessor

- [entries_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
  - 分组渲染和目录渲染不再直接解构 `snapshot/presenter`
  - 统一改成走 facade
  - 同时把分组循环改成按引用遍历，避免在 facade slice 模式下误移动内部向量

- [settings_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/facade.rs)
  - `page/save/save_snapshot/sync/sync_snapshot` 改成私有字段
  - 现在 `settings_page` facade 也明确只通过方法暴露边界

### 当前判断

- 到这里，四个主页面的 facade 形态已经更接近统一：
  - 原始字段不再是主要读取面
  - 读路径通过 accessor
  - 写路径通过动作口
- 也就是说，page boundary 现在已经比前几轮更接近：
  - `snapshot accessors`
  - `action slots`
 这个统一心智模型。

### 本轮验证

- `cargo check -p rssr-app`：通过
- `git diff --check`：通过
- `commit`：pending

## 追加：把 reader/feeds facade 继续收成只读快照边界

### 背景

- 在 `entries_page` 和 `settings_page` 收口之后，`reader_page` 与 `feeds_page` 仍然还存在一类较明显的耦合：
  - 组件虽然已经拿 facade
  - 但仍然大量直接解构 `facade.snapshot.*`
- 这意味着页面壳和 section 仍然依赖底层 state 结构细节，而不是依赖 facade 的稳定只读边界。

### 本轮变更

- [reader_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/facade.rs)
  - 补出只读访问口：
    - `shortcuts`
    - `title`
    - `source`
    - `published_at`
    - `body_text`
    - `body_html`
    - `status`
    - `status_tone`
    - `error`
    - `is_read`
    - `is_starred`
    - `navigation_state`
  - `session` / `snapshot` 不再继续作为页面层主要读取面

- [reader_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/mod.rs)
  - 页面壳不再直接解构 `snapshot`
  - 阅读正文、上下文分页、底部操作条都改成优先读 facade

- [feeds_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/facade.rs)
  - 补出只读访问口：
    - `feed_url`
    - `config_text`
    - `opml_text`
    - `pending_config_import`
    - `feeds`
    - `feed_count`
    - `entry_count`
    - `status`
    - `status_tone`

- [feeds_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/mod.rs)
  - 顶部统计卡和状态 banner 改成直接消费 facade 只读口

- [feeds_page/sections/compose.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/compose.rs)
  - 新增订阅输入框不再直接读 `snapshot.feed_url`

- [feeds_page/sections/config_exchange.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/config_exchange.rs)
  - JSON / OPML 输入框和导入确认态改成读 facade

- [feeds_page/sections/saved.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/saved.rs)
  - 已保存订阅列表不再直接读 `snapshot.feeds`

### 当前判断

- 到这里，`reader_page` 和 `feeds_page` 的 facade 已经更像真正的只读边界，而不只是“把 session 和 snapshot 打包一下”。
- 这一步和前两轮一起看，四个主页面都已经在往统一的：
  - 只读快照
  - 明确动作口
 这个 page boundary 模式靠拢。

### 本轮验证

- `cargo check -p rssr-app`：通过
- `commit`：pending

## 追加：把 settings_page facade 收成“值 + 动作口”边界

### 背景

- `settings_page` 虽然已经有 facade，但前一轮仍然更多是在透传：
  - `draft_signal`
  - `preset_choice_signal`
  - `status_signal`
  - `status_tone_signal`
- 这意味着：
  - `preferences`
  - `themes`
  - `sync`
  这些 section 虽然表面上在用 facade，实际上仍然在直接操作页面内部 `Signal`。

### 本轮变更

- [settings_page/facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/facade.rs)
  - facade 现在补出更明确的值/动作口：
    - `draft`
    - `update_draft`
    - `preset_choice`
    - `set_preset_choice`
    - `set_status`
    - `endpoint`
    - `remote_path`
    - `pending_remote_pull`

- [settings_page/preferences.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/preferences.rs)
  - `ReadingPreferencesSection` 不再接 `Signal<UserSettings>`
  - 改成直接接 `SettingsPageFacade`
  - 主题/密度/启动视图/刷新间隔/归档阈值/字体缩放都改成走 `update_draft`

- [settings_page/appearance.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/appearance.rs)
  - 外观卡片不再单独把 `draft Signal` 往下传
  - 继续只暴露 facade

- [settings_page/sync/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/mod.rs)
  - WebDAV 卡片不再直接读 `sync_snapshot` 字段
  - 改成使用 facade 的：
    - `endpoint`
    - `remote_path`
    - `pending_remote_pull`

- [settings_page/themes/theme_apply.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/theme_apply.rs)
  - 主题应用辅助函数不再直接碰 draft/preset/status `Signal`
  - 统一走：
    - `update_draft`
    - `set_preset_choice`
    - `set_status`

- [settings_page/themes/theme_io.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/theme_io.rs)
  - 主题文件导入导出状态反馈也统一走 facade

- [settings_page/themes/lab.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/lab.rs)
  - 主题实验室不再直接取 signal 进行改写
  - 文本输入、导入、导出、应用都改成走 facade 动作口

- [settings_page/themes/presets.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/presets.rs)
  - 主题预设选择、主题卡片应用/移除也都改成走 facade

### 当前判断

- `settings_page` 现在比上一轮更像真正的 page boundary：
  - section 不再直接摸页面内部 signal
  - facade 不再只是“session 打包层”，而是开始承担：
    - 设置值读取
    - 设置写入动作
    - 壳层状态反馈
- 这一步和 `entries_page` 一样，目标都是让组件进一步退化成：
  - 语义 DOM
  - 少量局部展示逻辑
  - facade 动作口消费方

### 本轮验证

- `cargo check -p rssr-app`：通过
- `commit`：pending

## 追加：把页面生命周期编排继续收成通用 facade helper

### 背景

- 前几轮已经把页面的真实行为层并到 `UiRuntime`，但 `entries / reader / feeds / settings` 仍然各自在页面壳里写：
  - `use_resource(use_reactive!(...))`
  - `use_effect(use_reactive!(...))`
- 这些代码虽然已经不直接碰 service，但仍然是重复的生命周期机械代码，不利于继续往“纯语义壳”推进。

### 本轮变更

- [ui/helpers.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/helpers.rs)
  - 新增：
    - `spawn_ui_command`
    - `spawn_projected_ui_command`
    - `use_reactive_task`
    - `use_reactive_side_effect`
  - 页面和子 session 不再自己包 `spawn(async move { ... })`，也不再到处散落 `use_resource/use_effect` 的样板。

- [entries_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/session.rs)
  - [reader_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/session.rs)
  - [settings_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/session.rs)
  - [feeds_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/session.rs)
  - [settings_page/save/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/session.rs)
  - [settings_page/sync/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/session.rs)
  - 这些 session 现在统一通过 `ui/helpers` 发 bus，不再各自维护异步投影胶水。

- 页面 workspace 入口也继续变薄：
  - [entries_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
  - [reader_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/mod.rs)
  - [feeds_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/mod.rs)
  - [settings_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/mod.rs)
  - 这些页面现在统一通过 `use_reactive_task/use_reactive_side_effect` 描述“需要哪些语义动作”，而不是直接写 hook 机械代码。

### 当前判断

- 这一步不是引入更大的抽象，而是把已经稳定下来的 page-local bus 样板继续集中。
- 到这里，页面层比上一轮更接近：
  - 语义 DOM
  - 局部 state
  - reducer
  - 极薄的 session / facade
- 下一轮如果要继续往前推，就可以开始考虑更明确的 `headless page facade model`，而不是先处理更多重复钩子。

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

## 追加：把 facade 从“对象打包层”推进成“动作口 + 快照边界”

### 背景

- 上一轮已经让 `entries / reader / feeds / settings` 都有了 facade，但大部分 facade 还只是：
  - `session`
  - `snapshot`
  - `presenter`
  的对象打包层。
- 页面和 section 仍然容易直接伸手去拿底层 `session` 方法，这还不够像稳定的 headless interface。

### 本轮变更

- `reader_page`
  - [facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/facade.rs)
    - 新增动作口：
      - `previous_action_target`
      - `next_action_target`
      - `toggle_read`
      - `toggle_starred`
  - [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/mod.rs)
    - 底部阅读操作条已改成优先调 facade，而不是直接调 session

- `feeds_page`
  - [facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/facade.rs)
    - 新增动作口：
      - `set_feed_url`
      - `set_config_text`
      - `set_opml_text`
      - `add_feed`
      - `refresh_all`
      - `export/import config`
      - `export/import opml`
      - `refresh_feed`
      - `remove_feed`
      - `paste_feed_url`
      - `is_delete_pending_for`
  - [sections/compose.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/compose.rs)
  - [sections/config_exchange.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/config_exchange.rs)
  - [sections/saved.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/saved.rs)
    - 这些 section 现在优先消费 facade 动作口，不再显式依赖 `FeedsPageSession`

- `settings_page`
  - [facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/facade.rs)
    - 新增动作口和只读入口：
      - `draft_signal`
      - `preset_choice_signal`
      - `status`
      - `status_tone`
      - `status_signal`
      - `status_tone_signal`
      - `open_repository`
      - `save`
      - `save_with_message`
      - `pending_save`
      - `set_endpoint`
      - `set_remote_path`
      - `push`
      - `pull`
  - [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/mod.rs)
    - 根页面开始直接走 facade 的状态与动作口
  - [appearance.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/appearance.rs)
  - [sync/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/mod.rs)
    - 卡片层现在优先调用 facade
  - [themes/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/mod.rs)
  - [themes/lab.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/lab.rs)
  - [themes/presets.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/presets.rs)
  - [themes/theme_apply.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/theme_apply.rs)
  - [themes/theme_io.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/themes/theme_io.rs)
    - 主题相关动作已经从“显式拿 page/save session”收成“先走 facade，再由 facade 落到页面底层句柄”

### 当前判断

- 到这里，facade 已经不只是 page-local workspace 的包装器，而开始具备真正的外部边界价值：
  - section / card / page 先看 facade
  - facade 暴露快照与动作口
  - session 继续向内退居为实现细节
- 这比前一轮更接近你要的：
  - `headless active interface`
  - 页面是默认语义壳
  - bus/runtime/infra 承担真实行为

### 本轮验证

- `cargo fmt --all`：通过
- `git diff --check`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过

## 追加：起第一版 headless page facade model

### 背景

- 前几轮已经把页面真实行为收进 `UiRuntime`，也把生命周期和 bus 投影样板集中到了 `ui/helpers`。
- 但 `entries / reader / feeds` 页面本身仍然在消费：
  - `session`
  - `snapshot`
  - `presenter`
  这类分散对象，页面壳依旧在自己拼 view model。

### 本轮变更

- `entries_page`
  - 新增 [facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/facade.rs)
  - [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
    - workspace 现在直接返回 `EntriesPageFacade`
    - 页面本身不再自己组装 `session + state_snapshot + presenter`
  - [controls.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/controls.rs)
    - 控制区开始直接消费 `EntriesPageFacade`

- `reader_page`
  - 新增 [facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/facade.rs)
  - [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/mod.rs)
    - workspace 现在直接返回 `ReaderPageFacade`

- `feeds_page`
  - 新增 [facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/facade.rs)
  - [mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/mod.rs)
    - workspace 现在直接返回 `FeedsPageFacade`
  - [sections/compose.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/compose.rs)
  - [sections/config_exchange.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/config_exchange.rs)
  - [sections/saved.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/sections/saved.rs)
    - section 不再各自回头从 session 读快照，而是消费 facade 的只读数据 + 动作口

### 当前判断

- 这一步还不是最终的统一 page facade 框架，但已经把最成熟的三个页面推进到了：
  - workspace 返回明确 facade
  - 页面/section 优先消费 facade
  - 本地 session 继续退居动作口
- 这比之前更接近：
  - 语义壳
  - facade/view model
  - bus/runtime/infra 承担真实行为

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `git diff --check`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过

## 追加：把 settings_page 也纳入组合式 facade

### 背景

- `entries / reader / feeds` 已经起了第一版 facade，但 `settings_page` 仍然由根页面把：
  - `page session`
  - `save session`
  - `sync session`
  分散交给各个卡片。
- 这会让组合页继续成为例外，不利于把四个主页面都统一到“页面暴露 facade、行为沉到 bus/runtime”的模型里。

### 本轮变更

- 新增 [facade.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/facade.rs)
  - `SettingsPageFacade`
  - 统一承载：
    - `page`
    - `save`
    - `save_snapshot`
    - `sync`
    - `sync_snapshot`

- [settings_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/mod.rs)
  - 根页面现在统一组装 `SettingsPageFacade`
  - 再把 facade 分发给各卡片

- [appearance.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/appearance.rs)
  - 不再自己创建 save session，而是直接消费 facade

- [sync/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/mod.rs)
  - 不再自己创建 sync session，而是直接消费 facade

### 当前判断

- 到这里，四个主页面都已经具备了“workspace 返回/暴露 page facade”的趋势。
- `settings_page` 仍然是组合页，但不再是架构例外；它只是组合式 facade，而不是散落的多个本地工作台。

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `git diff --check`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过

## 追加：收口启动型多投影场景

### 背景

- 上一轮已经把页面 session 的 bus 投影样板收到了统一 helper。
- 但 `StartupPage` 这种启动型场景仍然需要对同一批 intents 做双重处理：
  - 处理 startup route
  - 处理 fallback status
- 这类场景不适合简单套 `collect_projected_ui_command`，否则会重复执行命令。

### 本轮变更

- [ui/helpers.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/helpers.rs) 新增 `visit_ui_command`
  - 语义是：
    - 单次执行 `UiCommand`
    - 顺序访问每个 `UiIntent`
    - 由调用方自行决定多投影处理
- [ui/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/mod.rs) 已导出该 helper
- [entries_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs) 的 `StartupPage` 改成使用 `visit_ui_command`
  - 不再直接碰 `execute_ui_command`
  - 但仍保留 route/status 的双重处理逻辑

### 当前判断

- 到这里，`StartupPage` 这种启动壳也已经被统一纳入 bus helper 体系。
- 页面层直接触碰 `execute_ui_command` 的必要性进一步下降。
- 这一步的重点不是新增能力，而是把“多投影但单次执行”的启动场景也收进统一心智模型。

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

## 追加：把 WebAuthGate 与 AppNav 交互继续迁入 ui/shell

### 背景

- 在引入 `AppShellState` 与 shell facade 后，根组件里仍然保留两类明显的壳交互实现：
  - `AppNav` 的搜索提交、搜索聚焦、导航显隐交互
  - `WebAuthGate` 的表单状态、默认用户名填充、登录/初始化提交
- 这些都属于 UI shell boundary，不应该继续散在 `app.rs`。

### 本轮变更

- [ui/shell.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/shell.rs)
  - `AppShellState` 新增：
    - `submit_search`
    - `focus_search`
  - 新增：
    - `WebAuthGateShell`
    - `use_web_auth_gate_shell`
  - `WebAuthGateShell` 现在负责：
    - username/password/status/status_tone
    - 默认用户名填充
    - `NeedsSetup/NeedsLogin` 提交逻辑
    - 登录成功后的过渡处理
- [ui/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/mod.rs)
  - 导出 `use_web_auth_gate_shell`
- [app.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/app.rs)
  - `AppNav` 不再自己决定：
    - 搜索提交跳转
    - 搜索框 focus 跳转
    - 导航显隐副作用
  - `WebAuthGate` 不再自己维护：
    - username/password/status/status_tone
    - 默认用户名 effect
    - setup/login 提交流程
  - 这些现在都改成消费 `ui/shell` 暴露的壳状态与壳方法

### 当前判断

- 到这里，`app.rs` 已经更接近真正的根壳：
  - theme provider
  - shell provider
  - shell facade 挂载
  - 语义渲染分支
- `ui/shell` 现在已经承接：
  - 全局壳状态
  - 启动/认证壳 facade
  - 顶部导航/搜索壳交互
  - Web 本地认证壳交互
- 这说明当前架构已经不只是 page-local runtime 收口，而是开始形成更完整的 shell boundary。

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

## 追加：把 WebAuthGate 与 AppNav 交互迁入 ui/shell

### 背景

- 在引入 `AppShellState` 和 shell facade 后，根组件里仍然保留两块明显的壳交互实现：
  - `AppNav` 的搜索提交、搜索聚焦、顶部导航显隐交互
  - `WebAuthGate` 的用户名/密码表单状态、默认用户名填充、登录/初始化提交
- 这些都不属于页面业务，也不应该继续散在 `app.rs` 里。

### 本轮变更

- [ui/shell.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/shell.rs)
  - `AppShellState` 新增：
    - `submit_search`
    - `focus_search`
  - 新增：
    - `WebAuthGateShell`
    - `use_web_auth_gate_shell`
  - `WebAuthGateShell` 现在负责：
    - 用户名/密码 signal
    - 默认用户名填充
    - 状态文案与 tone
    - `NeedsSetup/NeedsLogin` 的提交逻辑
    - 登录成功后的过渡处理
- [app.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/app.rs)
  - `AppNav` 不再自己决定：
    - 搜索提交跳转
    - 搜索框 focus 时跳转
    - 导航显隐副作用
  - `WebAuthGate` 不再自己维护：
    - username/password/status/status_tone
    - 默认用户名 effect
    - setup/login 提交流程
  - 这些现在都改成消费 `ui/shell` 暴露的壳方法与壳状态
- [entries_page/controls.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/controls.rs)
  - 继续通过 `AppShellState` 统一读写全局搜索词

### 当前判断

- 到这里，`app.rs` 已经更接近真正的根壳：
  - provider
  - shell facade 挂载
  - 语义渲染分支
- `ui/shell` 已经开始承接三类职责：
  - 全局壳状态
  - 启动/认证壳 facade
  - 顶部导航/搜索这类全局壳交互
- 这说明当前架构已经从“页面 local runtime 收口”进入了更明确的：
  - shell boundary 收口
  - command/query/runtime facade 收口

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

## 追加：把全局壳状态迁入 ui/shell

### 背景

- 在引入 shell facade 之后，`App()` 仍然自己持有一组全局壳状态实现：
  - 全局搜索词
  - 顶部导航显隐
  - localStorage 持久化
- 这些状态虽然不是业务状态，但仍属于 UI shell boundary，不应该继续留在根组件里。

### 本轮变更

- [ui/shell.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/shell.rs) 现在新增：
  - `AppShellState`
  - `use_app_shell_state`
  - `entry_search / nav_hidden` 的 localStorage 持久化实现
  - `set_entry_search / show_nav / hide_nav` 等壳层方法
- [ui/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/mod.rs) 已导出：
  - `AppShellState`
  - `use_app_shell_state`
- [app.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/app.rs)
  - 删除原本的：
    - `AppUiState`
    - `initial_entry_search`
    - `remember_entry_search`
    - `initial_nav_hidden`
    - `remember_nav_hidden`
  - 现在直接：
    - `let shell = use_app_shell_state();`
    - `use_context_provider(|| shell);`
  - `AppNav` 也改成只消费 `AppShellState`
- [entries_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
  - workspace 读取搜索词改成消费 `AppShellState`
- [entries_page/controls.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/controls.rs)
  - `EntryFilters` 的搜索输入也改成通过 `AppShellState` 的壳层方法更新

### 当前判断

- 到这里，根组件已经不再自己保存“全局壳状态实现细节”。
- `App()` 更接近：
  - 提供 theme context
  - 提供 shell context
  - 挂载 shell bus
  - 输出语义 DOM
- 这一步对 `headless active interface + CSS 完全分离 + infra` 很关键，因为它继续把：
  - 页面结构
  - 壳状态
  - 行为调度
  分别往更稳定的边界收。

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

## 追加：收口入口壳层的 shell facade

### 背景

- 在 `visit_ui_command` 之后，`StartupPage` 和 `App()` 虽然已经不再直接碰 service，但仍然各自手写一段启动资源逻辑：
  - 认证壳负责 server gate + authenticated shell hydrate
  - 启动页负责 startup route + fallback status
- 这些都属于更高一层的 UI shell boundary，不应该继续散在页面/根组件里。

### 本轮变更

- 新增 [ui/shell.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/shell.rs)
  - `use_authenticated_shell_bus`
  - `use_startup_route_bus`
- [ui/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/mod.rs) 已导出上述 shell helper
- [app.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/app.rs)
  - 根组件现在直接挂 `use_authenticated_shell_bus(auth, settings)`
  - 不再自己维护认证探测后的 shell hydrate 流程
- [entries_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
  - `StartupPage` 现在直接挂 `use_startup_route_bus(...)`
  - 不再自己持有 startup route 解析 resource

### 当前判断

- 到这里，`App()` 与 `StartupPage` 都已经从“手写启动壳逻辑”进一步退化成“挂载 shell facade”的入口。
- 页面和根组件继续朝：
  - 语义壳
  - bus facade
  - CSS 呈现层
 这条线推进。
- `ui` 层现在已经不只是命令与 snapshot，还开始承担明确的 shell boundary。

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：通过
- `git diff --check`：通过

## 追加：收口页面 session 的 bus 投影 helper

### 背景

- 在 bus 接口压到 `UiCommand -> Vec<UiIntent>` 之后，页面 `session` 里还残留一批重复样板：
  - 执行 `execute_ui_command(...)`
  - 遍历 intents
  - 用某个 `into_*_intent` 投影
  - 再分发到本地 reducer
- 这些逻辑本身没有业务差异，只是重复的 bus 消费胶水。

### 本轮变更

- 新增 [ui/helpers.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/helpers.rs)
  - `collect_projected_ui_command`
  - `apply_projected_ui_command`
  - `apply_projected_ui_intents`
- [ui/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/mod.rs) 已统一导出这些 helper
- 下列页面 session / 壳层已切到统一 helper：
  - [app.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/app.rs)
  - [entries_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/session.rs)
  - [reader_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/session.rs)
  - [feeds_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/session.rs)
  - [settings_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/session.rs)
  - [settings_page/sync/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/session.rs)
  - [settings_page/save/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/session.rs)

### 当前判断

- 页面层现在不只是“行为经由 bus”，而且“消费 bus 的样板”也开始集中到了统一 helper。
- 这一步的意义不是再造新抽象，而是把重复胶水收掉，让页面层更接近：
  - 语义 DOM
  - 局部 state
  - reducer
  - 最少量 session
- `StartupPage` 仍保留手动处理，是因为它同时要处理：
  - startup route
  - fallback status
  对同一批 intents 做双重投影，硬套 helper 反而会把命令执行两次。

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `git diff --check`：通过

## 追加：继续收口 bus 接口与页面局部能力

### 背景

- 上一轮虽然已经把主页面都推到 `UiRuntime` 上，但 bus 接口仍然还带一层 `UiRuntimeOutcome { intents }` 包装。
- 同时 `feeds_page` 还残留最后一处本地浏览器副作用：
  - 剪贴板读取 fallback

### 本轮变更

- [ui/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime.rs) 去掉 `UiRuntimeOutcome`
  - 现在统一就是：
    - `UiCommand -> Vec<UiIntent>`
- [app.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/app.rs)
  - [entries_page/mod.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/mod.rs)
  - [entries_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/entries_page/session.rs)
  - [reader_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/reader_page/session.rs)
  - [feeds_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/session.rs)
  - [settings_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/session.rs)
  - [settings_page/save/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/save/session.rs)
  - [settings_page/sync/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/settings_page/sync/session.rs)
  这些消费方都改成直接处理 `Vec<UiIntent>`，不再先拆 `.intents`

- `feeds_page` 的最后一处本地浏览器能力也推到了总线：
  - [ui/commands.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/commands.rs) 新增 `UiCommand::FeedsReadFeedUrlFromClipboard`
  - [ui/runtime.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/ui/runtime.rs) 负责执行剪贴板读取
  - [feeds_page/effect.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/effect.rs) 不再保留 `ReadFeedUrlFromClipboard`
  - [feeds_page/intent.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/intent.rs) 不再保留 `ClipboardReadCompleted`
  - [feeds_page/session.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/feeds_page/session.rs) 删掉本地 `document::eval(...)` 剪贴板逻辑

### 当前判断

- 到这里，`feeds_page` 也不再自己执行浏览器异步能力，而是只发 bus command。
- 页面本地层更接近：
  - `state`
  - `reducer`
  - `session`
  - 语义 DOM
- 这比上一轮又往 `headless active interface + CSS 完全分离 + infra` 目标收了一步。

### 本轮验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `git diff --check`：通过
