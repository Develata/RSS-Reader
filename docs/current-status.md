# 维护状态

核对日期：2026-10-28。代码基线：main `a9608056e23864446a69881e38676598e56869d7`。本文件汇总当前事实与待办，不替代具体设计契约或逐次验收证据。

## 主线与已发布版本

| 项目 | 当前事实 | 证据 |
| --- | --- | --- |
| 正式发布 | v0.2.0，2026-10-23 发布；源码 tag 与之后的 main 分开记录 | [Release](https://github.com/Develata/RSS-Reader/releases/tag/v0.2.0)、[发布记录](./handoffs/2026-10-23-v0.2.0-release.md) |
| 最近合并 | PR #33：工作区偏好恢复、离页写入及 session 职责拆分 | [PR #33](https://github.com/Develata/RSS-Reader/pull/33)、[修复与验证](./handoffs/2026-10-27-workspace-recheck.md) |
| main 自动化 | CI、Pages、Docker 全部成功，均对应上面的 main 提交 | [CI](https://github.com/Develata/RSS-Reader/actions/runs/40579374354)、[Pages](https://github.com/Develata/RSS-Reader/actions/runs/40579374375)、[Docker](https://github.com/Develata/RSS-Reader/actions/runs/40579374505) |
| 原生交互 | 先前提交有 Windows / Android 专项结果；最新 main 的本轮实际交互验收待补 | [原生目录记录](./handoffs/2026-10-26-native-directory-validation.md)、[覆盖矩阵](./testing/release-ui-coverage-matrix.md) |
| Penpot | 最后记录的目录增量为 rev 369 / `badf7fae`；本轮将对最终 main 抽查 | [设计基准](./design/penpot-code-baseline.md) |

后续主线包含阅读工作区、目录和返回行为改进；这些改动尚未发布为新版本。CI 成功不等于正式安装包已经发布，也不等于所有设备和输入方式完成验收。

## 当前维护重点

1. 文档：校正当前状态、维护入口、归档发布前交接并修复链接，保留历史证据。
2. 原生验收：以明确源码构建，在隔离资料库验证切页后写入、筛选恢复、快速返回、快捷键和 Android 系统返回；设备、模拟器、浏览器结果分别记录。
3. 设计抽查：核对目录右侧箭头、5px 分支箭头、缩进和间距；通过 Penpot 版本回读与代表画板确认结果。
4. 补丁发布准备：完成更新说明、候选身份、安装包与升级检查，明确剩余设备和签名边界，再确定发布操作。

## 不应从历史结论推导的保证

- App 内的写入排序不提供跨浏览器或跨设备的操作序列保证。
- Android 构建成功不等于真机全流程通过；已有设备验收只适用于其记录的包与场景。
- Penpot 状态快照不模拟真实网络、持久化、滚动和所有状态组合。
- 历史审查、旧分支通过结果、文档提交不能替代当前产品源码的验收。

最近的执行依据见[交接入口](./handoffs/README.md)；重新验证后更新本页的日期、基线及证据链接。
