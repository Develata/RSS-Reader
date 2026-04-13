# 2026-04-13 Daily Rollup

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：2b9f947
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成一轮以 application boundary baseline 固化、browser refresh contract 补强和 native/runtime 稳定性修复为主的跨层整理，并把前一阶段的 use case 收敛从“实现完成”推进到“规则、例外和验证入口明确”。

## 影响范围

- 模块：
  - `crates/rssr-application/src/*`
  - `crates/rssr-app/src/ui/runtime/*`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-app/src/pages/feeds_page/*`
  - `crates/rssr-app/src/pages/reader_page/*`
  - `crates/rssr-cli/src/main.rs`
  - `crates/rssr-infra/src/application_adapters/browser/*`
  - `crates/rssr-infra/src/application_adapters/refresh.rs`
  - `crates/rssr-infra/src/feed_normalization.rs`
  - `crates/rssr-infra/tests/*`
  - `docs/design/*`
  - `docs/testing/*`
- 平台：
  - Linux
  - desktop
  - Android
  - Web
  - wasm32
  - CLI
  - Docker / `rssr-web`
- 额外影响：
  - application boundary baseline
  - browser refresh source/store contracts
  - `rssr-web` smoke diagnostics
  - native refresh / reader stability

## 关键变更

### Application Boundary Baseline 固化

- 新增并回写：
  - `docs/design/application-use-case-boundary-checklist.md`
  - `docs/design/application-use-case-consolidation-plan.md`
  - `docs/README.md`
  - `docs/architecture-review-2026-04.md`
- 明确 `query` / `command` / `workflow` / `service` 的分类规则，补上：
  - `RefreshService` 与 `SubscriptionWorkflow` 的命名边界
  - `ImportExportService` 保留为 service 的条件
  - `SettingsSyncService` 的保留理由
  - native image localization worker 的 host-side 例外说明
- 删除无独立 application 语义的 façade：
  - `ShellService`
  - `SettingsPageService`
  - `EntryService`
- 保留但收窄 `AppStateService`，移除完整快照透传接口，只保留稳定语义字段读写。

### Query / Use Case 依赖方向继续收直

- 新增 `FeedCatalogService`，去掉 CLI 对 `FeedRepository` 的直接依赖。
- `StartupService`、`EntriesWorkspaceService`、`FeedsSnapshotService` 直接依赖 `FeedRepository::list_summaries()` 或 `get_feed()`，不再借道 `FeedService`。
- `FeedService` 收敛回订阅命令 use case，不再混合 feed summaries 查询。

### Browser Refresh Contract 与 Smoke 诊断补齐

- 补齐 browser refresh 的说明文档、harness 计划和 wasm contract 覆盖。
- `BrowserRefreshStore` 的 `get_target` / `commit` 语义进入 `wasm_refresh_contract_harness`。
- `BrowserFeedRefreshSource` 的这些逻辑被抽成可测试 helper 并覆盖：
  - body classification
  - status classification
  - request fallback
  - bad XML parse failure
- network / CORS failure 没有被硬塞进假 harness，而是明确归入真实 `rssr-web` smoke 与环境限制文档。
- feeds 页面 feed card 暴露稳定 `data-*` 诊断属性，`rssr-web browser feed smoke` 改为直接观察：
  - `data-refresh-state`
  - `data-last-fetched-at`
  - `data-fetch-error`
  以减少刷新完成但 smoke 仍等待文案变化的超时噪音。

### Infra / Native 稳定性修复

- native parser 与 browser parser 共用的 normalization / dedup / content hash 逻辑收敛到 `crates/rssr-infra/src/feed_normalization.rs`。
- 修复 `wasm_config_exchange_contract_harness` 在 wasm 浏览器测试环境里误用默认 `SystemClock` 的失败。
- 修复 native / CLI refresh 在“entries 已空但 validators 仍在”时被 `304 Not Modified` 锁死的坏状态，并放宽文件型 SQLite 连接池。
- native 自动刷新并发从 `4` 降到 `1`，日志改为输出完整 error chain，便于定位 SQLite 写锁等底层错误。
- reader 图片本地化增强对 lazy image、`srcset`、referer、UA 和常见占位图的兼容性，减少 broken image。
- 本 rollup 已吸收并替代当日原先 30 份拆分 handoff；若需逐项追溯，需从 git 历史查看 2026-04-13 当日记录。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo fmt --check`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-app`：通过
- `cargo test -p rssr-cli`：通过
- `cargo test -p rssr-infra`：通过
- `cargo check --workspace`：通过
- `cargo check -p rssr-web`：通过
- `bash scripts/run_wasm_refresh_contract_harness.sh`：通过
- `bash scripts/run_wasm_contract_harness.sh wasm_config_exchange_contract_harness`：通过
- `bash scripts/run_wasm_config_exchange_contract_harness.sh`：通过
- `bash scripts/run_rssr_web_browser_feed_smoke.sh --port <varies> --log-dir target/rssr-web-browser-feed-smoke/<varies>`：通过
- `bash scripts/run_rssr_web_proxy_feed_smoke.sh --skip-build --port <varies> --log-dir target/rssr-web-proxy-feed-smoke/<varies>`：通过
- `git diff --check`：通过

### 手工验收

- application baseline 文档与调用链审查：通过
  - `docs/design/application-use-case-boundary-checklist.md`
  - `docs/design/application-use-case-consolidation-plan.md`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
  - `crates/rssr-cli/src/main.rs`
- refresh empty cache recovery 手工回归：通过
  - `cargo run --quiet -p rssr-cli -- --database-url "sqlite:///tmp/rssr-nvidia-regression.db" add-feed https://blogs.nvidia.com/feed/`
  - 人工构造空 entries + 保留 validators 后执行 `refresh --feed-id`
- reader image localization 边界复核：通过
  - sanitizer 仅放开图片来源相关属性，未放开脚本或事件属性

## 结果

- 本次整理对应的当日交付可以视为可合并基线。
- application use case 收敛不再只是代码状态，已经形成可复用的判断规则、保留项和删除项。
- browser refresh 路径同时具备文档说明、wasm harness 覆盖和 `rssr-web` smoke 诊断三层验证面。
- native refresh 空缓存恢复、自动刷新日志诊断和 reader 图片 broken image 的用户感知问题得到缓解。
- 本文件已作为 2026-04-13 唯一保留的 handoff 入口。

## 风险与后续事项

- network / CORS failure 仍只由真实浏览器 smoke 与环境限制文档覆盖，没有与真实环境等价的 contract harness。
- native refresh / 图片本地化虽然缓解了 SQLite 竞争和超时问题，但并发上限、日志噪声和内容体积仍需继续观察。
- application naming baseline 只解决“今后如何判断”，不会自动清理现有全部历史命名包袱；后续调整仍应由真实边界压力驱动。

## 给下一位 Agent 的备注

- 继续看 04-13 的 application baseline，先从这些入口读：
  - `docs/design/application-use-case-boundary-checklist.md`
  - `docs/design/application-use-case-consolidation-plan.md`
  - `crates/rssr-application/src/composition.rs`
  - `crates/rssr-app/src/ui/runtime/services.rs`
- 继续看 04-13 的 browser refresh / smoke，先从这些入口读：
  - `crates/rssr-infra/tests/wasm_refresh_contract_harness.rs`
  - `crates/rssr-infra/src/application_adapters/refresh.rs`
  - `docs/testing/rssr-web-browser-feed-smoke.md`
  - `docs/testing/rssr-web-proxy-feed-smoke.md`
- 继续看 04-13 的 native 稳定性修复，先从这些入口读：
  - `crates/rssr-infra/src/feed_normalization.rs`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-app/src/pages/reader_page/support.rs`
- 若需要逐项恢复原拆分 handoff 细节，只能从 git 历史检出 2026-04-13 当日已删除记录。

---

## 增量交接（2026-04-13 架构体检）

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：b6cdee1
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

### 工作摘要与背景

按用户请求完成一次“全仓 Rust 架构清晰度与 infra 边界一致性”审查，覆盖 `domain/application/infra/app/cli/web` 的依赖方向、模块职责、组合入口和高复杂文件。

### 受影响模块与平台

- 模块：
  - `crates/rssr-domain/src/*`
  - `crates/rssr-application/src/*`
  - `crates/rssr-infra/src/*`
  - `crates/rssr-app/src/bootstrap/*`
  - `crates/rssr-app/src/ui/runtime/*`
  - `crates/rssr-cli/src/main.rs`
  - `crates/rssr-web/src/*`
  - `docs/design/*`
  - `docs/handoffs/2026-04-13-daily-rollup.md`
- 平台：
  - desktop
  - Android
  - Web
  - CLI
  - Docker / `rssr-web`

### 关键代码 / 文档 / workflow 变更

- 代码变更：无（本次为审查，不包含业务代码修改）。
- 文档变更：追加当前增量交接记录（本节）。
- workflow：无 CI/workflow 配置变更。

### 已执行验证 / 验收

#### 自动化验证

- `cargo check --workspace`：通过

#### 结构与边界核查（静态检索）

- `rg -n "use rssr_infra|rssr_infra::|sqlx::|reqwest::" crates/rssr-application/src crates/rssr-domain/src`：无匹配（应用层/领域层未反向依赖 infra 或基础设施库）
- `rg -n "use rssr_application|rssr_application::" crates/rssr-domain/src`：无匹配（domain 未依赖 application）
- `rg -n "AppServices::shared|use_cases\\(|rssr_infra|Sqlite|Repository" crates/rssr-app/src/pages crates/rssr-app/src/ui crates/rssr-app/src/hooks crates/rssr-app/src/components`：仅 `ui/runtime/services.rs` 命中（页面层未直接持有 infra/repository）

### 当前状态、风险、待跟进

- 当前状态：架构评审已完成，结论已同步给用户；代码库可编译。
- 主要风险：
  - `rssr-app` native/web 与 `rssr-cli` 存在 composition 与能力编排重复，后续变更成本偏高。
  - `rssr-web/src/auth.rs` 与 `rssr-infra/src/fetch/client.rs` 文件职责偏重，继续增长后可维护性风险升高。
- 待跟进：
  - 抽取共享 composition builder，减少三处启动入口重复装配。
  - 拆分高复杂文件（先从 `auth.rs` 的 HTML 输出与鉴权流程解耦开始）。

### 相关 commit / tag / worktree 状态

- commit：pending
- tag / release：N/A
- worktree：`main` 分支，当前包含本次 handoff 文档增量更新
