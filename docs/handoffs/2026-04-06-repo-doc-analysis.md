# 仓库与文档梳理交接

- 日期：2026-04-06
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：194bc17
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

基于仓库 README、`docs/`、`specs/`、GitHub 仓库主页与关键源码，实现了一次“文档优先、源码复核”的整体梳理，确认 RSS-Reader 的真实产品边界、核心运行链路、持久化策略与部署包装层职责。

## 影响范围

- 模块：
  - `README.md`
  - `docs/README.md`
  - `docs/design/`
  - `docs/testing/`
  - `docs/roadmaps/`
  - `specs/001-minimal-rss-reader/`
  - `crates/rssr-app/`
  - `crates/rssr-cli/`
  - `crates/rssr-domain/`
  - `crates/rssr-infra/`
  - `crates/rssr-web/`
- 平台：
  - Windows
  - macOS
  - Linux
  - Web
  - Android
  - Docker
- 额外影响：
  - docs
  - handoff

## 关键变更

### 仓库理解与证据核验

- 已核验 GitHub 仓库主页：`https://github.com/Develata/RSS-Reader`
- 已阅读根 README、文档索引、设计文档索引、测试索引、Android 路线图、功能 spec、plan、quickstart、data model、tasks。
- 已复核关键源码，确认文档中的核心说法与以下实现一致：
  - 四层 Rust workspace 结构
  - native SQLite + web 本地序列化状态双后端
  - 订阅抓取、解析、去重写入、阅读、状态切换、配置交换
  - `rssr-web` 仅作为 Web 部署包装层

### 本次识别到的实现要点

- native 默认数据库位置来自可执行文件目录下的 `RSS-Reader/rss-reader.db`；Android 走 `HOME` 基础目录。
- Web 端不走 SQLite，而是浏览器内本地持久化状态与自动刷新逻辑。
- 阅读正文边界仍然限定在 feed 已提供内容；HTML 会做净化；图片本地化在 native/Android 由后台任务补偿，不阻塞刷新主流程。
- `rssr-web` 通过环境变量配置登录、静态目录、cookie、session、限流与 `/feed-proxy`，并对生产环境施加更严格启动约束。
- `rssr-app` 内置的 Web 本地登录只用于 loopback 场景保护浏览器本地数据，不应与 `rssr-web` 的服务端登录混淆。

## 验证与验收

### 自动化验证

- `git remote get-url origin`：通过
- `Get-Content README.md`：通过
- `Get-Content docs/README.md`：通过
- `Get-Content docs/design/functional-design-philosophy.md`：通过
- `Get-Content docs/design/frontend-command-reference.md`：通过
- `Get-Content docs/testing/manual-regression.md`：通过
- `Get-Content specs/001-minimal-rss-reader/spec.md`：通过
- `Get-Content specs/001-minimal-rss-reader/plan.md`：通过
- `Get-Content specs/001-minimal-rss-reader/quickstart.md`：通过
- `Get-Content specs/001-minimal-rss-reader/data-model.md`：通过
- `Get-Content specs/001-minimal-rss-reader/tasks.md`：通过
- `Get-Content migrations/0001_initial.sql`：通过
- `Get-Content crates/rssr-cli/src/main.rs`：通过
- `Get-Content crates/rssr-app/src/bootstrap/native.rs`：通过
- `Get-Content crates/rssr-app/src/bootstrap/web.rs`：通过
- `Get-Content crates/rssr-app/src/web_auth.rs`：通过
- `Get-Content crates/rssr-web/src/main.rs`：通过
- `Get-Content crates/rssr-web/src/auth/config.rs`：通过
- `Select-String` 对关键配置/默认值/路径/测试覆盖的源码定位：通过
- `cargo fmt --all`：未执行
- `cargo test --workspace`：未执行

### 手工验收

- 仓库主页、README、docs 与 specs 的一致性复核：通过
- 文档主张与关键源码落点交叉核验：通过
- 应用实际运行、GUI 交互、Docker 启动：未执行

## 结果

- 本次交付是“认知与文档梳理型”交付，不涉及业务代码改动。
- 当前可以据此继续做更深的模块分析、用户指南编写、或针对某一链路继续审查实现细节。

## 风险与后续事项

- `specs/` 与 `docs/` 对产品边界描述较完整，但部分旧路径表述仍带有 `tests/manual` 历史痕迹，实际手工文档当前位于 `docs/testing/manual/`。
- 当前只完成了文档与源码核验，未重新跑自动化测试，也未重做桌面/Web/Android 实机验收。

## 给下一位 Agent 的备注

- 如果要理解产品边界，先看 `docs/design/functional-design-philosophy.md` 与 `specs/001-minimal-rss-reader/spec.md`。
- 如果要理解运行时主链路，先看 `crates/rssr-app/src/bootstrap/native.rs`、`crates/rssr-app/src/bootstrap/web.rs`、`crates/rssr-infra/src/db/entry_repository.rs`、`crates/rssr-infra/src/parser/feed_parser.rs`。
- 如果要理解部署包装层，先看 `crates/rssr-web/src/main.rs` 与 `crates/rssr-web/src/auth/config.rs`。
