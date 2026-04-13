# 2026-04-12 Daily Rollup

- 日期：2026-04-12
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：2b9f947
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成一轮以 `ui/runtime -> application use case` 收敛为主轴的跨模块整理，并同步收紧 feeds/settings 页面行为、browser adapter 边界和 Web 相关 helper 的职责分布。

## 影响范围

- 模块：
  - `crates/rssr-app/src/ui/runtime/*`
  - `crates/rssr-app/src/bootstrap/*`
  - `crates/rssr-app/src/pages/feeds_page/*`
  - `crates/rssr-app/src/pages/entries_page/*`
  - `crates/rssr-app/src/pages/settings_page/*`
  - `crates/rssr-app/src/ui/shell_browser.rs`
  - `crates/rssr-app/src/web_auth_browser.rs`
  - `crates/rssr-application/src/*`
  - `crates/rssr-infra/src/application_adapters/browser/*`
  - `docs/design/*`
- 平台：
  - Linux
  - Web
  - wasm32
  - desktop
  - CLI
- 额外影响：
  - `rssr-web browser feed smoke`
  - release UI regression
  - browser boundary review

## 关键变更

### Application Use Case 收敛

- 将页面级 workflow 继续从 UI runtime 收进 application 层，落地或扩展：
  - `EntriesWorkspaceService`
  - `EntriesListService`
  - `ReaderService`
  - `FeedsSnapshotService`
  - `SettingsSyncService`
  - `SettingsPageService`
  - `ShellService`
  - `StartupService`
- `StartupService` 统一解析“启动到全部文章还是上次打开订阅”，UI shell 只负责 route 映射。
- `ImportExportService` 的导出时间来源改为 `ClockPort` 注入，移除 `rssr-application` 内部 wasm 条件分支和对 `js-sys` 的直接依赖。

### Feeds / Settings 页面行为硬化

- feeds 页的 add / refresh-feed / refresh-all 路径从裸 `Result<()>` 收敛为结构化 outcome。
- UI runtime 不再通过错误字符串判断“订阅已保存但首次刷新失败”或“单订阅刷新失败但状态已写回”。
- 删除订阅、配置导入、WebDAV 下载配置等危险操作的确认门禁回收到 reducer / state 生命周期，避免陈旧确认态残留。
- 系统剪贴板读取迁入 host capability，runtime 不再直接绑定浏览器 `document::eval` 细节。

### Browser / Web 边界整理

- `browser/state.rs` 和 `browser/adapters.rs` 按职责拆分，保留原有导入面。
- `browser/feed.rs` 完成前两阶段收敛：
  - request policy / proxy URL / fallback 判定拆到 `feed_request.rs`
  - login shell / HTML body 识别拆到 `feed_response.rs`
- 页面或壳层里的 browser-specific helper 下沉到局部文件：
  - `shell_browser.rs`
  - `web_auth_browser.rs`
  - `theme_file_io.rs`
  - `browser_interactions.rs`
  - entries page `clock` helper
- 当日边界审查结论明确：
  - `shell_browser.rs`
  - entries page browser helper
  - theme file IO
  已处于合理壳层终点；后续更值得继续的是 `browser/feed.rs` 与 browser persisted state。

### 测试与断言整理

- 清理低价值字符串断言和测试 warning：
  - config package codec 错误断言
  - `feed_service` 错误断言
  - `test_browser_state_seed_contracts` warning
- 多轮 release UI regression 持续覆盖 Web shell / settings / browser feed smoke，作为当天 UI 与 browser 调整后的主门禁。
- 本 rollup 已吸收并替代当日原先 30 份拆分 handoff；若需逐项追溯，需从 git 历史查看 2026-04-12 当日记录。

## 验证与验收

### 自动化验证

- `cargo fmt`：通过
- `cargo fmt --check`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-application`：通过
- `cargo check -p rssr-cli`：通过
- `cargo test -p rssr-app`：通过
- `cargo test -p rssr-application`：通过
- `cargo test -p rssr-infra`：通过
- `cargo test --workspace`：通过
- `cargo clippy --workspace --all-targets`：通过，部分轮次仍有既有 warning
- `bash scripts/run_release_ui_regression.sh --no-serve --with-rssr-web --port <varies> --web-port <varies> --log-dir target/release-ui-regression/<varies>`：通过，多轮执行
- `git diff --check`：通过

### 手工验收

- 独立 UI 点击回归：未执行
- browser 边界静态审查：通过
  - `crates/rssr-app/src/ui/shell_browser.rs`
  - `crates/rssr-app/src/pages/entries_page/browser_interactions.rs`
  - `crates/rssr-app/src/pages/settings_page/themes/theme_file_io.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/application_adapters/browser/state/*`
- release UI regression 产物审阅：通过
  - `target/release-ui-regression/*/summary.md`

## 结果

- 本次整理对应的当日交付可以视为可合并基线。
- UI runtime 对业务流程、错误语义和浏览器细节的直接耦合进一步下降。
- feeds 页刷新与新增订阅路径不再依赖错误字符串协议，Web 可见行为基线保持稳定。
- 本文件已作为 2026-04-12 唯一保留的 handoff 入口。

## 风险与后续事项

- 当日新引入的 `ShellService`、`SettingsPageService` 属于阶段性 page-shaped façade，后续仍需要继续审查其长期语义是否稳定。
- `browser/feed.rs` 当时只完成 request / response 侧收敛，parse normalization 尚未完全收口。
- browser persisted state 仍是后续 Web 演化热点；如果状态模型继续增长，需要优先复核持久化切片边界。

## 给下一位 Agent 的备注

- 继续看 04-12 的 application 收敛，先从这些入口读：
  - `crates/rssr-application/src/startup_service.rs`
  - `crates/rssr-application/src/entries_workspace_service.rs`
  - `crates/rssr-application/src/reader_service.rs`
  - `crates/rssr-application/src/feeds_snapshot_service.rs`
- 继续看 04-12 的 Web/browser 路径，先从这些入口读：
  - `crates/rssr-app/src/ui/runtime/feeds.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_request.rs`
  - `crates/rssr-infra/src/application_adapters/browser/feed_response.rs`
- 若需要逐项恢复原拆分 handoff 细节，只能从 git 历史检出 2026-04-12 当日已删除记录。
