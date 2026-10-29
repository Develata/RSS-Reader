# 测试与回归文档

这个目录的入口文档分为两类：

- 可重复执行的检查清单和 smoke 入口
- 长期维护的验证矩阵、环境限制和重建计划

单次执行结果、临时报表和阶段性结论，优先写入：

- `target/**/summary.md`
- `docs/handoffs/`

一次性报告不堆放在根目录。`evidence/` 和 `baselines/` 保留有源码身份的发布证据与测量基线，不因实现更新而删除；引用时必须注明对应版本。最新主线状态见[维护状态](../current-status.md)。

## 快速导航

### 入口索引

- [主线验证矩阵](./mainline-validation-matrix.md)
- [发布前 UI 回归清单](./release-ui-regression-checklist.md)
- [发布前 UI 覆盖矩阵](./release-ui-coverage-matrix.md)
- [手工回归测试清单](./manual-regression.md)
- [原生旧库升级验收](./native-upgrade-verification.md)
- [原生宿主阅读基线](./native-reading-baseline.md)：Windows / Android 的隔离数据、来源身份和测量边界。
- [正文解析与图片队列证据](./evidence/2026-10-28-parse-image/README.md)：固定夹具耗时、输出一致性及有界队列回归；发布后的独立工作分支。
- [发布 UI runner](./release-ui-runner.md)：统一入口、构建复用与阶段结果。

### Web / 浏览器 smoke

- [服务器模式浏览器回归（统一入口）](./server-web-regression.md)：regression、theme-matrix、feed-smoke、acceptance；长验收加 `--long`。
- [Web 服务包与容器验收](./web-distribution.md)：四平台服务包、真实旧页面、镜像复用与旧认证卷迁移。

- [`rssr-web` 浏览器手工 Smoke](./rssr-web-browser-smoke.md)
- [`rssr-web` 浏览器自动 Feed Smoke](./rssr-web-browser-feed-smoke.md)
- [旧静态入口的迁移说明](./static-web-browser-smoke.md)
- [Web 阅读页 `/entries/:entry_id` 主题矩阵 Smoke](./static-web-reader-theme-matrix.md)
- [Web 小视口 Smoke](./static-web-small-viewport-smoke.md)
- 正式 Web 多浏览器可见性、只读刷新与草稿：统一入口的 `acceptance`；原生与演示共用 SQL 契约，浏览器执行器另验。
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
  - `bash scripts/run_server_web_regression.sh --suite theme-matrix`
  - `bash scripts/run_server_web_regression.sh --suite regression`

### 主线验证 / CI 补查

- 先看 [主线验证矩阵](./mainline-validation-matrix.md)。
- 遇到失败先查 [环境限制索引](./environment-limitations.md)，不要把环境问题直接记成代码回归。

## 维护规则

- 新文档如果是长期入口，文件名应体现“清单 / 矩阵 / smoke / 计划”。
- 新文档如果只是某次执行结果，不应放在 `docs/testing/` 根目录。
- 过时执行报告从当前入口移入有日期的历史索引，保留原始事实与证据；删除文件须有明确理由和授权。README 与主线矩阵应链接当前契约，不能继续把旧报告当成当前通过结论。
