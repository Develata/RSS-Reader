# RSS-Reader 开发指南

由所有功能计划自动生成。最后更新：2026-03-24

## 当前技术栈

- Rust 稳定版（Edition 2024）
- Dioxus
- dioxus-router
- tokio
- sqlx
- reqwest
- feed-rs
- quick-xml
- serde / serde_json
- tracing

## 项目结构

```text
crates/
├── rssr-app/
├── rssr-application/
├── rssr-domain/
└── rssr-infra/

assets/
migrations/
tests/
specs/
```

## 常用命令

- `cargo fmt`
- `cargo clippy --workspace --all-targets`
- `cargo test --workspace`
- `cargo run -p rssr-app`

## 代码风格

- 生产代码统一使用 Rust
- UI 与业务逻辑分层，避免在 UI 中直接写 SQL、HTTP 或解析逻辑
- 保持本地优先、仅配置同步、避免过度抽象
- 性能敏感路径优先减少无意义 clone、分配和异步复杂度

## 最近变更

- `001-minimal-rss-reader`：新增极简个人 RSS 阅读器 MVP 的规格、计划、数据模型、契约和快速开始文档

<!-- MANUAL ADDITIONS START -->
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
<!-- MANUAL ADDITIONS END -->

## Active Technologies
- Rust 稳定版（Edition 2024） + Dioxus、dioxus-router、tokio、sqlx、reqwest、feed-rs、quick-xml、serde、serde_json、thiserror、anyhow、tracing、time、url (001-minimal-rss-reader)
- 桌面端和 Android 使用本地 SQLite；Web 使用浏览器本地持久化状态（当前实现为 `localStorage` 序列化）；配置交换使用本地配置文件与 OPML/JSON 导入导出文件 (001-minimal-rss-reader)
- Rust 稳定版（Edition 2024） + Dioxus 0.7.3、dioxus-router 0.7.3、tokio、sqlx、reqwest、feed-rs、quick-xml、serde、serde_json、thiserror、anyhow、tracing、time、url (001-minimal-rss-reader-followup-2)
- 桌面端使用本地 SQLite；Web 使用浏览器本地持久化状态（当前实现为 `localStorage` 序列化）；配置交换使用本地配置文件与 OPML/JSON 导入导出文件 (001-minimal-rss-reader-followup-2)

## Recent Changes
- 001-minimal-rss-reader: Added Rust 稳定版（Edition 2024） + Dioxus、dioxus-router、tokio、sqlx、reqwest、feed-rs、quick-xml、serde、serde_json、thiserror、anyhow、tracing、time、url
