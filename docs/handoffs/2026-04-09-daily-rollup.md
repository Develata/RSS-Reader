# 2026-04-09 交接汇总

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：d8b046d
- 相关 commit：3318d5e, 2937232, 92fb6e1, 07391a3, 4d11787, 4d0d270, 9cfde8e, aede229, e7a14ad, 8cf8b53, 049a8ab, c9c13fa, a592d40, 781bf1a, 682da79, d8b046d, pending
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
- `pending` 当前这轮 handoff 按日期整理与补细节

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

### 当日后续修复：Docker 发布节流

- 调整 `.github/workflows/docker.yml` 的语义边界：
  - `main` 分支日常 push 仍会执行 Docker build 与 runtime smoke
  - 但不会再登录 GHCR，也不会真正 push image
  - 只有 `refs/tags/v*` 才会执行 metadata、registry login 与 image push
- 这样保留了“验证 Docker 镜像可构建、可运行”的持续信心，同时避免每次主线 push 都生成额外的 GHCR 版本噪音

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
- 当前这次文档整理尚未提交，因此本条对应状态为 `commit: pending`。
- 这次整理不是单纯删文件，而是把原始记录里的：
  - 提交顺序
  - 关键文件
  - 自动化验证
  - 手工验收
  - 风险判断
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

### 手工验收

- Chrome MCP：文章页已读切换、阅读页快捷键、设置页输入与保存：通过
- `wasm-pack test --headless --chrome crates/rssr-infra --test wasm_refresh_contract_harness`：失败，但失败原因为当前 crate 下 native-only 集成测试也被一起按 wasm 编译，不是 wasm harness 本身错误
- WSL2 本地 browser runner：受 `chromedriver bind() failed: Cannot assign requested address (99)` 限制，真实浏览器执行依赖 GitHub Actions
- Chrome for Testing / chromedriver 安装与版本对齐：通过
- `bash scripts/run_wasm_refresh_contract_harness.sh`：仍为 `env-limited`，但当前输出已进入 WSL2 绑定异常路径，不再是缺少 browser capability 配置的未知失败

## 结果

- `zheye-mainline-stabilization` 中最核心的三条 contract 线，已经在当前 `main` 上完成重建。
- headless 接口命名与字段/动作边界进一步清晰。
- 当前主线已经具备统一的 wasm browser harness runner 与 CI matrix。
- 到此可以更准确地说：
  - `zheye` 分支的主功能与架构价值，已经基本被当前 `main` 手动吸收
  - 剩余重点不再是“还缺哪条主线”，而是观察远端 CI 是否把三条 wasm contract 线全部跑绿

## 风险与后续事项

- `wasm-pack test` 不是当前仓库的正式 wasm harness 入口；若要强行支持，需要额外拆分 native-only integration tests。
- WSL2 本地环境仍无法可靠执行 `chromedriver` 真实浏览器链路，需以 GitHub Actions 结果为准。
- 后续更值得做的是观察远端 `wasm-browser-contract` matrix 是否全绿，并在必要时针对 CI 差异修补脚本或环境。
- 当前这轮 handoff 目录整理尚未提交，因此“按日期汇总”本身仍处于 `pending` 状态。
- 这次 runner 修复主要面向 GitHub Actions 的 Chrome 会话稳定性；真正是否彻底收口，仍需下一次远端 `wasm-browser-contract` matrix 结果确认。
- Docker workflow 现在仍会在 `main` push 时运行一次完整 smoke；如果未来还想进一步节流，可以再评估是否把这条 workflow 只留给 tag 与手动触发。

## 给下一位 Agent 的备注

- 如果继续 contract harness 方向，先看 `docs/testing/contract-harness-rebuild-plan.md` 与 `scripts/run_wasm_contract_harness.sh`。
- 如果有人再次尝试 `wasm-pack test` 直跑 `crates/rssr-infra`，先读 `docs/testing/environment-limitations.md`，不要误判为 harness 回归。
- 如果要继续追 `zheye` 分支遗产，优先级已经明显下降；更高优先级是：
  - 看 `wasm-browser-contract` matrix 的真实结果
  - 再决定是否需要为 CI 差异补环境脚本
