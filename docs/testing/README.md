# 测试与回归文档

这个目录收纳手工验证、回归检查与测试辅助说明。

## 当前文档

- [手工回归测试清单](./manual-regression.md)
- [发布前 UI 回归清单](./release-ui-regression-checklist.md)
- [`rssr-web` 浏览器手工 Smoke](./rssr-web-browser-smoke.md)
- [Static Web 浏览器手工 Smoke](./static-web-browser-smoke.md)
- [Static Web `/reader` 主题矩阵 Smoke](./static-web-reader-theme-matrix.md)
- [Static Web 小视口 Smoke](./static-web-small-viewport-smoke.md)
- [全局浏览器回归报告](./global-browser-regression.md)
- [Headless 重构视觉等价验收](./headless-refactor-equivalence.md)
- [主线验证矩阵](./mainline-validation-matrix.md)
- [环境限制索引](./environment-limitations.md)
- [Contract Harness 重建计划](./contract-harness-rebuild-plan.md)

## 建议怎么使用

- 每次大改阅读页、订阅页、设置页或主题系统之后：
  - 先过一遍 [手工回归测试清单](./manual-regression.md)
- 如果是发版前确认：
  - 先过 [发布前 UI 回归清单](./release-ui-regression-checklist.md)
  - 再补充路由、订阅刷新、阅读页导航、主题切换、设置持久化的专项确认
  - 静态 Web 如果要稳定进入真实阅读页，优先用 `run_static_web_browser_smoke.sh --seed reader-demo --next /entries/2`
  - 多主题下的真实阅读页，优先用 `run_static_web_reader_theme_matrix.sh`
  - 小视口关键路径，优先用 `run_static_web_small_viewport_smoke.sh`
- 如果是排查 Web 端问题：
  - 重点看 feed 刷新、浏览器 Console 和持久化行为
- 如果是逐模块推进 headless active interface 重构：
  - 先按 [Headless 重构视觉等价验收](./headless-refactor-equivalence.md) 走基线与复测
- 如果是执行 browser contract harness：
  - 先看 [环境限制索引](./environment-limitations.md)
  - 再按 [Contract Harness 重建计划](./contract-harness-rebuild-plan.md) 里的脚本入口执行
- 如果想在本地复现接近 GitHub Actions 的 Linux 测试环境：
  - 用仓库根目录的 `Dockerfile.ci-local`
  - 或直接执行 `scripts/run_ci_local_container.sh`

## 当前覆盖重点

这套回归清单当前更偏向：

- Web 端主要交互
- 阅读体验链路
- 主题切换与设置保存
- 导入导出与本地持久化
- headless 重构中的模块级视觉与体验等价验证

后续如果 Android 真机验收单独成型，建议继续在这个目录下新增独立的 Android 回归文档，而不是把所有平台混进同一份清单。
