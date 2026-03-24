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
<!-- MANUAL ADDITIONS END -->
