# 维护状态

核对日期：2026-10-30。近期目标是让 WebView 成为可替换的界面实现，让未来 Rust 原生界面复用同一交互规则；JS 行数仅作辅助指标。返回、App 设置读取和 Android 系统主题文件导入已进入 main。Reader 的规则独立性、宿主入口、快捷键定义及模块归属通过 PR #50–#52 合入 main `dbbaf390`；界面仍使用 Dioxus / WebView。正文、资源与锚点的[兼容边界](./design/reader-rendering-boundary.md)已核实，尚未替换渲染器。

列表密度选项和紧凑样式、仓库 `.agents/` 的 9 份旧 skill 已经由 PR #48 合入 main；旧密度参数仅暂留 0.2.x 兼容，0.3.x 破坏性删除已批准但不在本批实施。Android 导入验收发现的开头 BOM 呈现缺陷已在旧包复现后独立修复，见[当前交接](./handoffs/2026-10-30-android-theme-import.md)。

本页只汇总当前事实；具体规则见[设计索引](./design/README.md)，未完成工作见[当前待办](./maintenance-backlog.md)，每次验证的版本和环境见[交接入口](./handoffs/README.md)。

## 三个不同的版本状态

| 范围 | 当前事实 | 证据与边界 |
| --- | --- | --- |
| 已发布版本 | v0.2.1，tag 固定在 `dbfbf896` | [正式发布记录](./handoffs/2026-10-28-v0.2.1-release.md)：14 个附件身份、Windows 正式包与 Android 模拟器升级等当时验收；不能替代后续源码验收 |
| main 的产品整合基线 | `dbbaf390`，包含 PR #38–#52；E 盘接续工作树已快进到 main | #50 → #51 → #52 的合并树分别与已验证的 head 相同。合并后本地 app 420 通过 / 1 忽略、独立规则 24 通过，Docker 容器 9 项、Linux 包 6 项及 Pages 构建/部署通过；主线 CI 与精确版本见[整合回执](./handoffs/2026-10-30-reader-integration.md) |
| 本轮后续提交 | main 上的正文兼容契约、状态及交接文档；没有后续运行代码变更 | Git 中的产品源码继续与 `4d0aab02` 相同；Windows、Android 真机及正式 Web 的实际回执见[同产品树的宿主记录](./handoffs/2026-10-30-reader-shortcut-cohesion.md)。最终文档 head 的 CI 单独核对，不把同树证明写成重新执行了真机验收 |

main `4101520c` 的 CI 36/36、Docker 构建和容器验收、容器 9 项及 Linux 包 6 项成功，是上一轮已留存的历史验收；[E 盘交接](./handoffs/2026-10-29-pr40-worktree-handoff.md)记录了报告身份和复制核验。这些结果不作为 `62113c4f` 返回策略通过原生验收的证明。

## 已完成的主线整合

- PR #38 已合入正文解析热点修复与后台图片队列约束，合并提交 `6e99706c`；不再是待合并的性能工作。单 token 取消、304 续排及规模观察等边界仍在[当前待办](./maintenance-backlog.md)。
- PR #40 已合入 Entries 初始化、查询、写入与偏好，Reader 访问归属，Feeds 编辑与读取，以及 Settings 编辑、保存、异步回填等规则。保留七 crate、`Backend` / `LocalBackend`、rusqlite 单库、正式 Web 同源服务和 Settings 逐字段保存及 `stored → submitted → pending` 回填。
- PR #39、#41–#45 已完成目录入口、固定标题、二级页面层级、主题预览、目录顺序及相关验收/文档整合。目录的 Windows 原生专项证据绑定 `0692c924`，见[原生复测](./handoffs/2026-10-29-directory-native-verification.md)。
- PR #46 已合入返回导航/退出提示策略及此前授权的文档与维护资料整合；Windows / Android 返回专项仍以[其实际宿主回执](./handoffs/2026-10-30-native-back-validation.md)的包和场景为准。
- PR #47 已合入 App 设置读取生命周期；其原始 Rust/Web/Windows 验证见[实现交接](./handoffs/2026-10-30-shell-settings-lifecycle.md)。本轮另补 Android 主题选择专项，不能扩大为该批全部 Android 生命周期验收。
- PR #50–#52 已按顺序合并：规则独立性 `7edb118d`、宿主入口 `5d51025a`、快捷键定义与模块归属 `dbbaf390`。分支保留用于溯源，当前无这三批的待合并实现；正文兼容要求见[专门契约](./design/reader-rendering-boundary.md)。
- 阅读位置、目录控制器与窗口、下拉刷新、快捷键、图片查看器已有 Rust 规则；继续复用这些模块，不建立全局浮层状态副本或通用页面框架。模块边界见[原生界面前置说明](./design/native-ui-preparation.md)。

## 现在还要做什么

1. **保留已收口的规则边界。** App 设置读取已经审查、CI 和合并；不再列为候选。主题选择的现有行为已完成专项，不继续扩大 Shell 提取或重复实现系统弹窗。
2. **维持已完成的分支与资料收口。** 历史验收资料和 Claude 配置已在工作树外备份并校验，5 个旧工作树及 3 个分支已退役，见[清理与恢复记录](./handoffs/2026-10-30-branch-consolidation.md)。保留当前候选与主 checkout 的既有改动，不向已合并的 PR #40 追加实现，不发布。
3. **处理下一条具体依赖。** Reader 三批已整合，正文内容/资源/锚点与 HTML/DOM 的当前边界已明确。下一实现聚焦命令装配对页面 `support.rs` 的正文选择引用，保留候选顺序与清洗政策，并独立验证同一生产函数。实际原生正文渲染器仍需验证主题、选择复制、IME 和无障碍；没有第二种实现前不新增通用框架、crate 或全局宿主接口。见[下一批范围](./design/reader-rendering-boundary.md#下一批建议)。

## 分支与工作树快照

截至本批核对：**8 个本地分支、2 个工作树、7 个真实远端分支**。接续工作树现为 main；远端其余为 codex/native-back-policy、codex/shell-settings-lifecycle、codex/android-theme-import、codex/reader-rule-boundary、codex/reader-host-boundary、codex/reader-shortcut-definition。本批不新增或删除分支，不新建工作树，不把 `origin/HEAD` 算作分支。

本地保留 main、上述六个已合并的 codex 分支和主 checkout 的 `feat/reading-workspace-20261024`。主 checkout 的 HEAD、状态、index 与 41 个文件/缺失路径继续保留；另一任务的 JavaScript 审计文档不纳入提交。历史备份与恢复方式见[清理及恢复清单](./handoffs/2026-10-30-branch-consolidation.md)。

## 验证结论的边界

- 历史记录仅证明其中指定的源码、包、设备及场景；发布、构建、普通浏览器和真实原生宿主验收分开记录。
- Android 正式签名包真机升级、真实旧数据搬迁，以及 macOS/Linux GUI、系统 IME、辅助技术和更广设备交互仍未全面覆盖。
- Penpot 是设计基准，不模拟网络、持久化、滚动和全部状态组合；已有抽查见[设计基准](./design/penpot-code-baseline.md)。
- App 内的写入排序不提供跨浏览器或跨设备的全局操作序列保证。
