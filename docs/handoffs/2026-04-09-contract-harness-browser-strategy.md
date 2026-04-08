# contract harness browser strategy

- 日期：2026-04-09
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：e7a14ad
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`planned`

## 工作摘要

在落地 `refresh contract harness` 的 host / sqlite baseline 后，进一步明确 browser fixture 在当前主线里的实现边界，并把这一现实约束补进 contract harness 重建计划。

## 影响范围

- 模块：
  - `docs/testing/contract-harness-rebuild-plan.md`
- 平台：
  - host / sqlite fixture
  - wasm / browser fixture

## 关键结论

- 当前 `rssr-infra::application_adapters::browser` 仅在 `wasm32` 下导出。
- 因此旧分支里那种“同一份 host harness 同时跑 sqlite fixture 与 browser fixture”的结构，当前主线不能直接照搬。
- 正确路线应该是：
  - 先保留 host / sqlite harness 作为共享 use case 的基线
  - 再单独为 browser fixture 建 wasm/browser 测试基座

## 文档调整

- 在 `contract-harness-rebuild-plan.md` 中新增：
  - host / wasm 分层原则
  - refresh harness 当前进度
  - 推荐执行顺序修正
  - browser fixture 下一步的现实落点

## 已执行验证

- `git diff --check`

## 当前状态

- 仅完成策略澄清，未开始实现 browser / wasm harness。

## 风险与待跟进

- 如果后续决定为 `rssr-infra` 引入 wasm 测试基座，需要同步评估：
  - 依赖新增
  - CI 如何执行 wasm/browser 测试
  - browser localStorage / window 环境如何稳定初始化
