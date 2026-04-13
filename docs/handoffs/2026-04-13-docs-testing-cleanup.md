# 文档整理：CSS 基线与测试目录

- 日期：2026-04-13
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：b6cdee1
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

整理并收紧了 CSS 分离基线文档与 `docs/testing/` 目录结构，把一次性测试结果从长期文档入口中剥离，并将多份手工检查文档改回可重复使用的模板。

## 影响范围

- 模块：
  - `docs/design/css-separation-baseline-checklist.md`
  - `docs/testing/README.md`
  - `docs/testing/mainline-validation-matrix.md`
  - `docs/testing/headless-refactor-equivalence.md`
  - `docs/testing/manual/*.md`
- 平台：
  - Web
  - desktop
  - 文档流程
- 额外影响：
  - 测试文档分层
  - 手工验收模板
  - 旧报告清理

## 关键变更

### CSS 分离基线重写

- 将原先偏过程性、重复性较高的 `css-separation-baseline-checklist` 重写为长期维护口径。
- 明确：
  - 已稳定的 `data-*` 公开接口
  - 可保留的 class 边界
  - `reader-body-html` 等允许例外
  - 审查命令与下一轮判断规则

### `docs/testing` 重新分层

- 重写 [README.md](/home/develata/gitclone/RSS-Reader/docs/testing/README.md)，按“入口索引 / 浏览器 smoke / 重构约束 / 用户故事模板”分类。
- 明确单次执行结果应落在 `target/**/summary.md` 或 `docs/handoffs/`，不再长期堆在 `docs/testing/` 根目录。
- 删除过时单次报告：
  - `docs/testing/global-browser-regression.md`
- 同步更新仍引用该旧报告的文档：
  - `docs/testing/mainline-validation-matrix.md`
  - `docs/testing/headless-refactor-equivalence.md`

### 手工模板去历史化

- 将以下文档从“带固定日期和历史结论的记录”改回可复用模板：
  - `docs/testing/manual/us1-reading-checklist.md`
  - `docs/testing/manual/us1-performance-checklist.md`
  - `docs/testing/manual/us2-interaction-checklist.md`
  - `docs/testing/manual/us2-performance-checklist.md`
  - `docs/testing/manual/us3-config-exchange-checklist.md`
  - `docs/testing/manual/us3-boundary-checklist.md`
  - `docs/testing/manual/final-acceptance-checklist.md`
- 新模板统一补充了：
  - 适用范围
  - 执行前记录
  - 标准化结果表
  - 通过标准

## 验证与验收

### 自动化验证

- `rg -n "global-browser-regression|全局浏览器回归报告" docs/testing docs/design -S`：通过
  - 活跃测试与设计文档中已无残留引用

### 手工验收

- 文档结构复核：通过
  - `docs/testing/README.md` 入口分类与当前目录结构一致
  - `docs/testing/manual/*.md` 已回到长期模板口径
  - `docs/design/css-separation-baseline-checklist.md` 已压缩为基线维护文档

## 结果

- `docs/testing` 目录的长期入口与一次性结果已基本分离。
- CSS 分离清单更适合作为后续审查基线，而不是历史流水账。
- 后续若再新增单次测试结果，应该优先写到 `target/**/summary.md` 或 `docs/handoffs/`。

## 风险与后续事项

- `docs/testing/manual-regression.md` 仍保留旧式“结果区块”写法，后续可以再统一成与新模板一致的记录格式。
- `specs/001-minimal-rss-reader/tasks.md` 仍把部分手工模板描述成“记录结果”；如后续继续整理规格文档，可同步改成“维护模板 / 执行记录入口”。

## 给下一位 Agent 的备注

- 如果继续清理测试文档，先看：
  - `docs/testing/README.md`
  - `docs/testing/manual-regression.md`
  - `docs/testing/release-ui-regression-checklist.md`
- 如果继续推进 CSS 语义收口，先看：
  - `docs/design/css-separation-baseline-checklist.md`
