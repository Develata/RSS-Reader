# 测试与回归文档

这个目录收纳手工验证、回归检查与测试辅助说明。

## 当前文档

- [手工回归测试清单](./manual-regression.md)
- [全局浏览器回归报告](./global-browser-regression.md)
- [Headless 重构视觉等价验收](./headless-refactor-equivalence.md)

## 建议怎么使用

- 每次大改阅读页、订阅页、设置页或主题系统之后：
  - 先过一遍 [手工回归测试清单](./manual-regression.md)
- 如果是发版前确认：
  - 优先看路由、订阅刷新、阅读页导航、主题切换、设置持久化
- 如果是排查 Web 端问题：
  - 重点看 feed 刷新、浏览器 Console 和持久化行为
- 如果是逐模块推进 headless active interface 重构：
  - 先按 [Headless 重构视觉等价验收](./headless-refactor-equivalence.md) 走基线与复测

## 当前覆盖重点

这套回归清单当前更偏向：

- Web 端主要交互
- 阅读体验链路
- 主题切换与设置保存
- 导入导出与本地持久化
- headless 重构中的模块级视觉与体验等价验证

后续如果 Android 真机验收单独成型，建议继续在这个目录下新增独立的 Android 回归文档，而不是把所有平台混进同一份清单。
