# 测试与回归文档

这个目录只保留两类长期文档：

- 可重复执行的检查清单和 smoke 入口
- 长期维护的验证矩阵、环境限制和重建计划

单次执行结果、临时报表和阶段性结论，优先写入：

- `target/**/summary.md`
- `docs/handoffs/`

不要再把一次性测试报告长期堆在这里。

## 快速导航

### 入口索引

- [主线验证矩阵](./mainline-validation-matrix.md)
- [发布前 UI 回归清单](./release-ui-regression-checklist.md)
- [发布前 UI 覆盖矩阵](./release-ui-coverage-matrix.md)
- [手工回归测试清单](./manual-regression.md)

### Web / 浏览器 smoke

- [`rssr-web` 浏览器手工 Smoke](./rssr-web-browser-smoke.md)
- [`rssr-web` 浏览器自动 Feed Smoke](./rssr-web-browser-feed-smoke.md)
- [`rssr-web` 代理 Feed Smoke](./rssr-web-proxy-feed-smoke.md)
- [Static Web 浏览器手工 Smoke](./static-web-browser-smoke.md)
- [Static Web `/reader` 主题矩阵 Smoke](./static-web-reader-theme-matrix.md)
- [Static Web 小视口 Smoke](./static-web-small-viewport-smoke.md)
- [Windows Chrome 可见窗口回归](./windows-chrome-visible-regression.md)
- [Chrome MCP 目标浏览器](./chrome-mcp-target.md)

### 重构 / 约束

- [Headless 重构视觉等价验收](./headless-refactor-equivalence.md)
- [环境限制索引](./environment-limitations.md)
- [Contract Harness 重建计划](./contract-harness-rebuild-plan.md)

### 用户故事手工模板

- [US1 手工验证清单：订阅、列表与阅读](./manual/us1-reading-checklist.md)
- [US1 性能检查模板：刷新与阅读](./manual/us1-performance-checklist.md)
- [US2 手工验证清单：筛选、搜索与快捷键](./manual/us2-interaction-checklist.md)
- [US2 性能检查模板：大数据量筛选与状态切换](./manual/us2-performance-checklist.md)
- [US3 手工验证清单：配置交换与导入导出](./manual/us3-config-exchange-checklist.md)
- [US3 边界验证模板：配置交换与主题生效](./manual/us3-boundary-checklist.md)
- [最终验收模板：MVP](./manual/final-acceptance-checklist.md)

## 推荐使用方式

### 日常改动

- 页面、主题、设置或阅读链路有改动后，先走 [手工回归测试清单](./manual-regression.md)。
- 涉及浏览器入口时，优先选一个固定 smoke 脚本，不要临时手搓环境。

### 发布前

- 先看 [发布前 UI 覆盖矩阵](./release-ui-coverage-matrix.md)，确认哪些已经自动化、哪些还要人工补。
- 再按 [发布前 UI 回归清单](./release-ui-regression-checklist.md) 执行。
- 本次如果涉及真实阅读页、多主题或小视口，额外补：
  - `bash scripts/run_static_web_reader_theme_matrix.sh`
  - `bash scripts/run_static_web_small_viewport_smoke.sh`

### 主线验证 / CI 补查

- 先看 [主线验证矩阵](./mainline-validation-matrix.md)。
- 遇到失败先查 [环境限制索引](./environment-limitations.md)，不要把环境问题直接记成代码回归。

## 维护规则

- 新文档如果是长期入口，文件名应体现“清单 / 矩阵 / smoke / 计划”。
- 新文档如果只是某次执行结果，不应放在 `docs/testing/` 根目录。
- 过时报告应及时删除，避免 README 和主线矩阵继续引用旧结论。
