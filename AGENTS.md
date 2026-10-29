# RSS-Reader 开发指南

当前实现说明，核对日期：2026-10-28（v0.2.1 已发布，源码 tag 指向 `dbfbf896`）。历史功能计划见 `specs/`；以当前源码、锁文件和下述文档为准。当前验证范围与维护重点见 `docs/current-status.md`，经源码核实的维护事项见 `docs/maintenance-backlog.md`。

## 当前技术栈

- Rust 稳定版（Edition 2024）
- Dioxus / dioxus-router 0.7.9
- tokio
- rusqlite（捆绑 SQLite）
- reqwest
- feed-rs
- quick-xml
- serde / serde_json
- tracing

## 项目结构

```text
crates/
├── rssr-api/
├── rssr-app/
├── rssr-cli/
├── rssr-web/
├── rssr-application/
├── rssr-domain/
└── rssr-infra/

build-support/   # 构建脚本共用的片段（构建标识）
assets/
migrations/
migrations_content/
docs/
scripts/
tests/
specs/
```

## 常用命令

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo run -p rssr-app --locked`
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`
- `bash scripts/run_rssr_web_browser_smoke.sh`（正式 Web 同源开发与手工检查）
- `bash scripts/run_server_web_regression.sh --suite acceptance --long`（服务器模式验收）
- `dx serve --platform web --package rssr-app --features pages-demo --locked`（仅静态演示）

## 代码风格

- 生产代码统一使用 Rust
- UI 与业务逻辑分层，避免在 UI 中直接写 SQL、HTTP 或解析逻辑
- 保持本地优先、仅配置同步、避免过度抽象
- 性能敏感路径优先减少无意义 clone、分配和异步复杂度

## 当前架构与入口

- 七个 crate：UI、CLI 和 Web 部署服务为入口；application 统一用例语义，domain 定义模型与端口，`rssr-api` 定义界面与用例之间的 `Backend` 接口，infra 实现平台适配与 `LocalBackend`。
- 原生端使用单个 SQLite 资料库 `library.sqlite3`（rusqlite），实现与基线 schema 在 `crates/rssr-infra/src/store/`；根目录的 `migrations/`、`migrations_content/` 冻结为 v0.1.x 历史，只供旧库导入核对。
- 正式 Web 的 `HttpBackend` 经同源 `/api/v1` 读写 `rssr-web` 的 `LocalBackend` 与服务器 `library.sqlite3`；浏览器不保存文章库，正式 wasm 不含 SQLite 或 `memory-store`。登录由服务器负责，已删除本地浏览器口令门禁与 `/feed-proxy`。
- `pages-demo` 通过 `memory-store` 复用共享 SQL 仓储与单连接内存 SQLite，每次加载预抓取 RSS，资料库操作重载即重置；旧 BrowserStore 已退役，在线订阅、WebDAV 与自动刷新禁用。正式 Web 仍可保存设备偏好，以及在重新加载或登录前保留本标签页的设置草稿（`sessionStorage`）；这些不构成离线文章库。
- 页面通过 facade/session 派发 `UiCommand`；runtime 只经 `Arc<dyn Backend>` 调用，页面不直接执行 I/O。
- 用户操作见 `docs/user-guide.md`，边界见 `docs/design/functional-design-philosophy.md`，UI 契约见 `docs/design/frontend-command-reference.md` 与 `docs/design/theme-author-selector-reference.md`。
- 变更和验证证据见 `docs/handoffs/`，发布检查见 `docs/testing/release-ui-regression-checklist.md`；历史 spec 的完成勾选不能代替当前验证。

<!-- MANUAL ADDITIONS START -->
## Agent 测试与验收方式

- 默认优先直接运行可重复的自动化测试：复用 Rust 回归、现有验收脚本及脚本化 UI 测试；浏览器交互优先使用 Playwright / CDP，Windows 原生端优先连接实际应用的 WebView2 / CDP。
- 优先由脚本完成操作、断言、截图、录像及必要的几何 / 逐帧记录；已有自动化接口能覆盖的场景，不反复使用 computer use 点击窗口或截图。
- 仅在自动化接口无法覆盖的系统弹窗、原生输入行为或必要的系统级视觉核对中使用 computer use，并尽量集中完成。
- 原生验收应运行实际原生宿主并使用隔离测试资料库；构建成功或普通浏览器测试不能代替原生交互通过。交接记录注明运行环境、实际结果、证据位置与未覆盖范围。

## 本地提交授权

- 用户已授权：任务完成并做完相称验证后，可按职责分批进行本地 commit，无需逐次确认。
- 仅提交该任务范围内已理解的改动，保留任务外 dirty / staged / untracked 内容；在 handoff 中记录提交与验证结果。
- 此授权不包含 push、打 tag、发布或触发远端工作流；用户后续明确要求不提交时，以该要求为准。

## Agent 交接记录要求

- 每次 agent 完成一次可交付工作后，MUST 在 `docs/handoffs/` 新增或更新一份固定格式的交接记录。
- 记录文件名 MUST 使用 `YYYY-MM-DD-<slug>.md` 格式，除非该次工作明确归并到同日已有记录。
- 记录内容 MUST 至少包含：
  - 工作摘要与背景
  - 受影响模块与平台
  - 关键代码/文档/workflow 变更
  - 已执行的验证/验收命令与结果
  - 当前状态、风险、待跟进项
  - 相关 commit、tag 或 worktree 状态
- 如果该次工作尚未提交，记录中 MUST 明确写出 `commit: pending` 或等价状态。
- 未补 `docs/handoffs/` 记录的工作，不应视为完整交付。
- 记录规范与模板以 `docs/handoffs/README.md` 和 `docs/handoffs/TEMPLATE.md` 为准。

## Agent 架构护栏

- 当即将提出或推进的设计 / 计划出现以下任一信号时，MUST 先做严谨的保守分析，再立刻向交互人员明确提出，不得静默继续推进：
  - 代码严重分叉
  - infra 架构被污染，平台差异回流到 application / domain 主语义
  - 前后端大规模迁移或职责重分配（纯后端内部重构、纯前端内部重构除外）
  - 明显违背 `docs/design/functional-design-philosophy.md`
- 对上述几类方案，agent 默认应持保守甚至负面倾向；只有在收益、边界和迁移成本被充分论证后，才可建议继续。
- 讨论此类方案时，必须优先说明：
  - 哪些核心语义仍保持统一
  - 哪些差异被限制在 infra / adapter / host capability 层
  - 哪些迁移是新增能力，哪些迁移只是为了弥补设计错误
  - 如果不做该方案，当前更小、更稳的替代路径是什么
<!-- MANUAL ADDITIONS END -->
