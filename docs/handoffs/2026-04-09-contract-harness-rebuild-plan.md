# contract harness rebuild plan

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：9cfde8e
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`planned`

## 工作摘要

基于当前 `main` 的结构，单独规划重建 `zheye-mainline-stabilization` 中有价值但已不适合直接移植的 3 份 contract harness 测试。

## 影响范围

- 模块：
  - `docs/testing/`
  - 后续将影响 `crates/rssr-infra/tests/`
- 平台：
  - SQLite / native fixture
  - browser persisted-state fixture

## 关键产出

- 新增长期规划文档：
  - `docs/testing/contract-harness-rebuild-plan.md`
- 更新测试文档入口：
  - `docs/testing/README.md`

## 规划结论

### 应重建的测试

- `refresh contract harness`
- `subscription contract harness`
- `config exchange contract harness`

### 不应直接复制的内容

- 旧分支里的原始 harness 文件
- 旧分支中依赖过时 browser 模型或旧 `bootstrap/web.rs` 结构的测试脚手架

### 推荐顺序

1. `refresh`
2. `subscription`
3. `config exchange`

## 已执行验证

- `git diff --check`

## 当前状态

- 仅完成规划，尚未开始实现新的 harness 测试文件。

## 风险与待跟进

- 当前 browser fixture 需要直接基于 `rssr-infra/src/application_adapters/browser/*` 组装，不能再偷用旧分支的临时模型。
- 第 3 阶段仍会受本地 WebDAV 测试环境限制影响，执行时需参考环境限制文档。
