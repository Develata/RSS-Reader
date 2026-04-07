# Feeds 模块 Headless Phase 1

- 日期：2026-04-07
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：471ddc4
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

完成 `002-headless-active-interface` 的第一阶段订阅模块重构，把订阅页非显示性操作收敛为命令、查询、调度和绑定四层，同时保持当前页面视觉与交互体验不变。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/feeds_page.rs`
  - `crates/rssr-app/src/pages/feeds_page/`
  - `crates/rssr-app/src/pages/feeds_page_sections/compose.rs`
  - `crates/rssr-app/src/pages/feeds_page_sections/config_exchange.rs`
  - `crates/rssr-app/src/pages/feeds_page_sections/saved.rs`
  - `crates/rssr-app/src/bootstrap/web/feed.rs`
  - `crates/rssr-app/index.html`
- 平台：
  - Web
  - Windows
- 额外影响：
  - docs
  - validation

## 关键变更

### 订阅页命令面

- 新增 `FeedsPageCommand`，把新增订阅、刷新、导入导出、删除等操作建模为稳定命令语义。
- 新增 `FeedsPageSnapshot`，统一加载订阅页核心数据，避免页面分散拼接服务调用。
- 新增 `execute_feeds_page_command(...)` 调度层，将页面事件和服务执行解耦。
- 新增 `FeedsPageBindings`，统一负责状态回填、错误回填和命令执行结果映射。
- `compose.rs`、`config_exchange.rs`、`saved.rs` 不再直接调用服务，改为构造命令并应用调度结果。

### Web 抓取修复

- 修复 `crates/rssr-app/src/bootstrap/web/feed.rs` 中 Web 抓取路径污染原始 feed URL 的问题。
- 修复后 `/feed-proxy` 使用原始 feed URL，`_rssr_fetch` 仅保留在浏览器直连回退路径中。

### 浏览器验收支撑

- 新增 `crates/rssr-app/index.html` 作为临时 `trunk` Web 验证入口，用于在 Chrome MCP 中加载 `rssr-app` Web 构建产物并由 `rssr-web` 托管。

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `cargo check -p rssr-app`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo test --workspace`：通过

### 手工验收

- `Chrome MCP：登录 -> 进入 /feeds -> 添加 https://blog.rust-lang.org/feed.xml`：通过，页面显示 `订阅数 1`、`文章数 10`、`Rust Blog` 和成功状态文本
- `Chrome MCP：导出配置 -> 刷新此订阅 -> 验证导入确认流 -> 验证删除确认流`：通过，视觉布局和交互路径与重构前一致
- `Chrome MCP：桌面视图与移动窄屏视图对照`：通过，未观察到明显视觉偏离或布局错位

## 结果

- 当前订阅模块已经完成第一阶段 headless 化，核心行为不再直接绑在页面组件的服务调用上。
- 本次交付在真实浏览器下通过了前后等价验收，视觉与体验未发生预期外偏离。

## 风险与后续事项

- `crates/rssr-app/src/bootstrap/web/feed.rs` 当前行数已接近仓库约束上限，后续不宜继续向该文件追加逻辑。
- `crates/rssr-app/src/pages/feeds_page/dispatch.rs` 仍偏长，下一阶段适合继续拆分导入导出与订阅操作执行逻辑。
- 当前 Chrome MCP 验证依赖临时 `trunk` 入口，后续需要决定是否将其正式化为仓库标准 Web 验证入口。

## 给下一位 Agent 的备注

- 继续推进下一模块前，先看 `specs/002-headless-active-interface/` 和 `docs/testing/headless-refactor-equivalence.md`。
- 如果继续扩展订阅模块，优先从 `crates/rssr-app/src/pages/feeds_page/` 下的新命令层入口开始，而不是回到页面 section 中直接写服务调用。
