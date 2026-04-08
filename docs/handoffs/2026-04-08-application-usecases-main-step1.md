# application-usecases-main-step1

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：b235836
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

把 `audit/architecture-scan` 的架构评审文档引入当前主线，并从 `refactor/application-usecases-step1` 中移植第一刀最值的改动：把 native / CLI 中重复的订阅管理、刷新和配置交换编排开始收束到 `rssr-application`。

## 影响范围

- 模块：
  - `docs/architecture-review-2026-04.md`
  - `crates/rssr-application`
  - `crates/rssr-infra`
  - `crates/rssr-app/src/bootstrap/native.rs`
  - `crates/rssr-cli/src/main.rs`
- 平台：
  - desktop
  - CLI
  - Android（装配链编译路径受本机 NDK 缺失影响，未完成目标编译）
- 额外影响：
  - docs

## 关键变更

### 架构评审文档

- 从 `audit/architecture-scan` 引入 [architecture-review-2026-04.md](/home/develata/gitclone/RSS-Reader/docs/architecture-review-2026-04.md)，作为后续 use case 收束的评审依据和路线图。

### application use case 收束

- `FeedService` 吸收订阅 URL 规范化、删除订阅与可选文章清理语义。
- 新增 `RefreshService`，统一单 feed / 全量刷新流程，并通过应用层 port 接收刷新来源与持久化提交。
- 新增 `SubscriptionWorkflow`，统一：
  - 添加订阅并首次刷新
  - 删除订阅并清理 app-state
- `ImportExportService` 吸收：
  - OPML codec port
  - feed removal cleanup
  - 统一导入配置时的 feed 删除清理路径

### infra adapters

- `rssr-infra` 新增正式 adapter：
  - `InfraFeedRefreshSource`
  - `SqliteRefreshStore`
  - `SqliteAppStateAdapter`
  - `InfraOpmlCodec`
- 新增 adapter 测试，验证刷新提交会写入 feed metadata、entries 和 fetch state。

### 入口收薄

- native `bootstrap/native.rs` 改为装配 application service / workflow，而不是直接长期维护刷新与配置交换主流程。
- `rssr-cli` 改为调用共享 application service / workflow，而不是保留独立 refresh / remove / config-exchange 主流程。

## 验证与验收

### 自动化验证

- `cargo fmt --all`：通过
- `git diff --check`：通过
- `cargo check -p rssr-application`：通过
- `cargo test -p rssr-application`：通过
- `cargo check -p rssr-infra`：通过
- `cargo test -p rssr-infra --test test_application_refresh_store_adapter`：通过
- `cargo check -p rssr-cli`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：未完成，因当前环境缺少 `aarch64-linux-android-clang`

### 手工验收

- desktop 添加订阅 / 删除订阅 / 刷新 / 配置交换：未执行
- CLI 命令级回归：未执行

## 结果

- 第一刀收束已在当前 `main` 上落地，方向与 `audit` 文档一致。
- 共享 use case 已经成为 native / CLI 的新依赖支点，后续可以继续沿同一路径处理 wasm/web。

## 风险与后续事项

- 当前 `rssr-app` 的 Android/native 目标未在本机完整编译闭环，原因是缺少 Android toolchain，不是已知代码错误。
- wasm/web 仍保留原有本地状态与刷新实现，本轮刻意未动。
- 下一刀更自然的目标是：
  - 继续把 wasm/browser 逻辑外移为正式 adapter
  - 或补 native / CLI 的手工回归与 shared behavior 测试

## 给下一位 Agent 的备注

- 先看 [architecture-review-2026-04.md](/home/develata/gitclone/RSS-Reader/docs/architecture-review-2026-04.md)，再看这次新增的：
  - [refresh_service.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/refresh_service.rs)
  - [subscription_workflow.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/subscription_workflow.rs)
  - [application_adapters.rs](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/application_adapters.rs)
- 如果下一轮继续做 wasm/web，不要把 shared use case 再复制回入口层。
