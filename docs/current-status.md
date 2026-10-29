# 维护状态

核对日期：2026-10-28。正式版本基线：`v0.2.1` → `dbfbf8965f4bdfa6463f33c50d5fbf6c2fcd15ee`；Rust / CSS 产品代码与此前专项验收的 `a9608056` 相同，正式附件另做身份与升级核验。本文件汇总当前事实，不替代具体设计契约或逐次验收证据；经源码核实的后续事项维护在[当前待办](./maintenance-backlog.md)。

## 主线与已发布版本

| 项目 | 当前事实 | 证据 |
| --- | --- | --- |
| 正式发布 | v0.2.1，2026-10-28 发布；14 个公开附件与实际 CI 文件及 GitHub SHA256 全部相符 | [Release](https://github.com/Develata/RSS-Reader/releases/tag/v0.2.1)、[正式发布记录](./handoffs/2026-10-28-v0.2.1-release.md) |
| 最近合并 | PR #35 文档与原生证据、#34 setup-python v7、#36 历史待办核实；各自合并前 37 项成功 / 1 项跳过 | [PR 收尾记录](./handoffs/2026-10-28-historical-followup-audit.md)、[PR #36](https://github.com/Develata/RSS-Reader/pull/36) |
| 发布基线自动化 | dbfbf896 的 CI、Pages、Docker 成功；Release 最终 13 项成功 / 1 项复用跳过，四平台 Web 包各 6 项、容器 9 项检查通过 | [CI](https://github.com/Develata/RSS-Reader/actions/runs/40681852013)、[Pages](https://github.com/Develata/RSS-Reader/actions/runs/40681852004)、[Docker](https://github.com/Develata/RSS-Reader/actions/runs/40681852052)、[Release](https://github.com/Develata/RSS-Reader/actions/runs/40683428344) |
| 正式原生产物 | Windows 正式 GUI 启动、5000 篇单库升级及关闭通过；Android 正式 APK 在隔离模拟器覆盖安装、初始化与预置旧 schema 单库对照通过，APK/AAB 证书与 v0.2.0 相同 | [正式产物验收及边界](./handoffs/2026-10-28-v0.2.1-release.md)、[原生证据](./testing/evidence/2026-10-28-v0.2.1-release/native.json) |
| 原生交互专项 | 此前 Windows 27 项、Android 真机 28 项，以及快捷键、重启偏好与 Windows v0.2.0 升级通过；Android 当时使用 debug 外壳，未把其结果当作正式 APK 全流程复测 | [此前验收及边界](./handoffs/2026-10-28-release-readiness.md)、[覆盖矩阵](./testing/release-ui-coverage-matrix.md) |
| Penpot | rev 370：55 张画板及命名版本回读正常；桌面 / 手机目录代表抽查通过，未改设计 | [设计基准](./design/penpot-code-baseline.md)、[抽查证据](./testing/evidence/2026-10-28-release-readiness/penpot.json) |
| 后续维护 | 正式产物身份待办 M03 已关闭；剩余 6 类维护事项按实测与风险推进 | [当前待办](./maintenance-backlog.md) |

阅读工作区、目录和返回行为改进已进入 v0.2.1。发布成功与正式附件核验不等于所有设备、旧库和输入方式完成验收；平台覆盖限制见正式发布记录。

## 当前维护重点

1. HTML 的 Marker/属性慢点与后台图片轮转、候选载荷约束已在 `codex/parse-image-performance` 实现；2026-10-29 创建 [PR #38](https://github.com/Develata/RSS-Reader/pull/38)，`a8791026` 的远端检查 37 成功 / 1 预期跳过，见[CI 回执](./handoffs/2026-10-29-parse-image-pr-ci.md)。这是尚未合并、未发布的工作分支；实现与边界见[本轮记录](./handoffs/2026-10-28-parse-image-performance.md)、[处理预算](./design/reader-processing-budgets.md)，剩余单 token 预算、304 续排和规模观察继续列在[当前待办](./maintenance-backlog.md)。
2. 真实 Android 旧库搬迁与正式签名包真机升级仍需专项验收；模拟器中的覆盖安装和预置 Windows 旧 schema 夹具不替代这两项。
3. 系统 IME、辅助技术、macOS/Linux GUI 完整交互与其他设备覆盖按明确场景补验；其余历史段落继续逐批核实，归档不代表全部结案。

## 不应从历史结论推导的保证

- App 内的写入排序不提供跨浏览器或跨设备的操作序列保证。
- Android 构建成功不等于真机全流程通过；已有设备验收只适用于其记录的包与场景。
- Penpot 状态快照不模拟真实网络、持久化、滚动和所有状态组合。
- 历史审查、旧分支通过结果、文档提交不能替代当前产品源码的验收。

最近的执行依据见[交接入口](./handoffs/README.md)；重新验证后更新本页的日期、基线及证据链接。
