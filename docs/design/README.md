# 设计文档

这里保存现行产品规则、架构边界和公开接口。核对日期：2026-10-28，main `a9608056`。实施记录和某次验收结果分别进入[交接目录](../handoffs/README.md)与[测试索引](../testing/README.md)。

## 现行契约

| 范围 | 入口 |
| --- | --- |
| 产品能力、阅读缓存、Rust / CSS 分工 | [功能设计哲学](./functional-design-philosophy.md) |
| 七 crate、正式 Web、统一 SQLite、Pages | [v0.2 架构](./v0.2-architecture.md)（已在 v0.2.0 发布） |
| Shell、命令、runtime、页面 session | [UI 分层](./ui-shell-bus-page-facade.md)、[前端命令接口](./frontend-command-reference.md) |
| 目录独立浏览、自动跟随、文章窗口 | [目录交互](./entries-directory-follow.md)、[原生界面前置边界](./native-ui-preparation.md) |
| 主题与稳定选择器 | [主题作者参考](./theme-author-selector-reference.md)、[CSS 分离检查](./css-separation-baseline-checklist.md) |
| Penpot 与实际代码的对应关系 | [当前代码设计基准](./penpot-code-baseline.md) |
| 应用服务职责 | [Use Case 收敛](./application-use-case-consolidation-plan.md)、[边界检查](./application-use-case-boundary-checklist.md) |
| 资料库兼容与交付形态 | [SQLite 迁移兼容](./sqlite-migration-compatibility.md)、[GUI / CLI 分发](./cli-gui-distribution.md) |

目录交互与文章级入口已在主线实现；平台未验收项仍见各契约。GUI / CLI 的独立分发已实施，Scoop 包仍是该文档列出的后续工作。

## 长期方向与历史计划

- [Headless Active Interface](./headless-active-interface.md)：长期方向和重构门禁，不意味着已更换渲染器。
- [v0.2 阶段 B2 契约](./v0.2-phase-b2-contracts.md)、[阶段 C 实施计划](./v0.2-phase-c-plan.md)：保留阶段决定与偏差记录；当前架构以 v0.2 架构和源码为准。
- [服务器模式设计入口](./rssr-web-server-mode.md)：包含历史方案的入口；现行部署见 [Web 部署](../deployment/web.md)。
- [静态 SPA 回归迁移](./web-spa-regression-server.md)：旧入口的退役说明；正式浏览器验收使用[同源服务器回归](../testing/server-web-regression.md)。
- [Penpot 历史草模](./reading-workspace-penpot-contract.md)：v4–v8 草模与旧连线，不替代当前画板。

新增或修改设计时应写清：已经实施的行为、尚未实施的方案、实际验证范围。不要只修改本索引中的状态而遗漏正文中的旧结论。
