# 维护状态

核对日期：2026-10-28。代码基线：main `a9608056e23864446a69881e38676598e56869d7`。本文件汇总当前事实与待办，不替代具体设计契约或逐次验收证据。

## 主线与已发布版本

| 项目 | 当前事实 | 证据 |
| --- | --- | --- |
| 正式发布 | v0.2.0，2026-10-23 发布；源码 tag 与之后的 main 分开记录 | [Release](https://github.com/Develata/RSS-Reader/releases/tag/v0.2.0)、[发布记录](./handoffs/2026-10-23-v0.2.0-release.md) |
| 最近合并 | PR #33：工作区偏好恢复、离页写入及 session 职责拆分 | [PR #33](https://github.com/Develata/RSS-Reader/pull/33)、[修复与验证](./handoffs/2026-10-27-workspace-recheck.md) |
| main 自动化 | CI、Pages、Docker 全部成功，均对应上面的 main 提交 | [CI](https://github.com/Develata/RSS-Reader/actions/runs/40579374354)、[Pages](https://github.com/Develata/RSS-Reader/actions/runs/40579374375)、[Docker](https://github.com/Develata/RSS-Reader/actions/runs/40579374505) |
| 原生交互 | 本轮 Windows 27 项、Android 真机 28 项通过；补充快捷键、重启偏好与 Windows v0.2.0 升级检查 | [本轮验收及边界](./handoffs/2026-10-28-release-readiness.md)、[覆盖矩阵](./testing/release-ui-coverage-matrix.md) |
| Penpot | rev 370：55 张画板及命名版本回读正常；桌面 / 手机目录代表抽查通过，未改设计 | [设计基准](./design/penpot-code-baseline.md)、[抽查证据](./testing/evidence/2026-10-28-release-readiness/penpot.json) |
| 下一版准备 | v0.2.1 草稿已整理；尚未打 tag、构建正式签名附件或发布 | [发布草稿与执行条件](./releases/v0.2.1-draft.md) |

后续主线包含阅读工作区、目录和返回行为改进；这些改动尚未发布为新版本。CI 成功不等于正式安装包已经发布，也不等于所有设备和输入方式完成验收。

## 当前维护重点

1. 保持当前文档与契约同步；本轮已归档 147 份发布前交接、修复失效链接，见[维护记录](./handoffs/2026-10-28-docs-maintenance.md)。
2. 审查并合入本轮文档与验收证据，再确定 v0.2.1 的最终 tag 提交与发布操作。
3. 对未来实际发布包补正式签名、版本、附件哈希及升级校验；已验证测试包不能代替正式附件。
4. 继续保留真实系统 IME、Windows 前台性能和其他设备的覆盖边界，按后续需求专项验证。

## 不应从历史结论推导的保证

- App 内的写入排序不提供跨浏览器或跨设备的操作序列保证。
- Android 构建成功不等于真机全流程通过；已有设备验收只适用于其记录的包与场景。
- Penpot 状态快照不模拟真实网络、持久化、滚动和所有状态组合。
- 历史审查、旧分支通过结果、文档提交不能替代当前产品源码的验收。

最近的执行依据见[交接入口](./handoffs/README.md)；重新验证后更新本页的日期、基线及证据链接。
