# 维护状态

核对日期：2026-10-28。代码与流程基线：main `da8521e23b16079294dd43f4cfe9a1f7d5024cdc`；Rust / CSS 产品代码与已验收的 `a9608056` 相同。本文件汇总当前事实，不替代具体设计契约或逐次验收证据；经源码核实的后续事项维护在[当前待办](./maintenance-backlog.md)。

## 主线与已发布版本

| 项目 | 当前事实 | 证据 |
| --- | --- | --- |
| 正式发布 | v0.2.0，2026-10-23 发布；源码 tag 与之后的 main 分开记录 | [Release](https://github.com/Develata/RSS-Reader/releases/tag/v0.2.0)、[发布记录](./handoffs/2026-10-23-v0.2.0-release.md) |
| 最近合并 | PR #35 文档与原生证据、PR #34 setup-python v7；各自合并前 37 项成功 / 1 项跳过 | [PR 收尾记录](./handoffs/2026-10-28-historical-followup-audit.md) |
| main 自动化 | da8521e2 的 CI、Pages、Docker 已成功；额外四平台服务包各 6 项、容器 9 项检查通过 | [CI](https://github.com/Develata/RSS-Reader/actions/runs/40676313625)、[Pages](https://github.com/Develata/RSS-Reader/actions/runs/40676313551)、[Docker](https://github.com/Develata/RSS-Reader/actions/runs/40676313862)、[四平台](https://github.com/Develata/RSS-Reader/actions/runs/40677169921) |
| 原生交互 | 本轮 Windows 27 项、Android 真机 28 项通过；补充快捷键、重启偏好与 Windows v0.2.0 升级检查 | [本轮验收及边界](./handoffs/2026-10-28-release-readiness.md)、[覆盖矩阵](./testing/release-ui-coverage-matrix.md) |
| Penpot | rev 370：55 张画板及命名版本回读正常；桌面 / 手机目录代表抽查通过，未改设计 | [设计基准](./design/penpot-code-baseline.md)、[抽查证据](./testing/evidence/2026-10-28-release-readiness/penpot.json) |
| 下一版准备 | v0.2.1 草稿已整理；尚未打 tag、构建正式签名附件或发布 | [发布草稿与执行条件](./releases/v0.2.1-draft.md) |

后续主线包含阅读工作区、目录和返回行为改进；这些改动尚未发布为新版本。CI 成功不等于正式安装包已经发布，也不等于所有设备和输入方式完成验收。

## 当前维护重点

1. 按[当前待办](./maintenance-backlog.md)推进：先复现 HTML 预算盲区、测量后台图片任务公平性，不重做已被新架构替代的旧事项。
2. 当前基线的远端检查已完成；合入本轮维护清单后确定 v0.2.1 的最终 tag 提交与发布操作。
3. 对实际发布包补正式签名、版本、附件哈希及升级校验；已验证测试包和 Web 服务包不能代替正式 GUI / Android 附件。
4. 旧库设备导入、系统 IME、辅助技术与其他设备覆盖按明确场景补验；其余历史段落继续逐批核实，归档不代表全部结案。

## 不应从历史结论推导的保证

- App 内的写入排序不提供跨浏览器或跨设备的操作序列保证。
- Android 构建成功不等于真机全流程通过；已有设备验收只适用于其记录的包与场景。
- Penpot 状态快照不模拟真实网络、持久化、滚动和所有状态组合。
- 历史审查、旧分支通过结果、文档提交不能替代当前产品源码的验收。

最近的执行依据见[交接入口](./handoffs/README.md)；重新验证后更新本页的日期、基线及证据链接。
