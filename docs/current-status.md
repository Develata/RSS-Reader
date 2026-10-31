# 维护状态

核对日期：2026-10-31。近期目标是让 WebView 成为可替换的界面实现，让未来 Rust 原生界面复用同一交互规则；JS 行数仅作辅助指标。返回、App 设置读取和 Android 系统主题文件导入已进入 main。Reader 的规则独立性、宿主入口、快捷键定义及模块归属通过 PR #50–#52 合入 main `dbbaf390`，并发修复通过 PR #53 合入 `ed0714d3`；界面仍使用 Dioxus / WebView。正文、资源与锚点的[兼容边界](./design/reader-rendering-boundary.md)已核实，尚未替换渲染器。

列表密度选项和紧凑样式、仓库 `.agents/` 的 9 份旧 skill 已经由 PR #48 合入 main；旧密度参数仅暂留 0.2.x 兼容，0.3.x 破坏性删除已批准但不在本批实施。Android 导入验收发现的开头 BOM 呈现缺陷已在旧包复现后独立修复，见[当前交接](./handoffs/2026-10-30-android-theme-import.md)。

本页只汇总当前事实；具体规则见[设计索引](./design/README.md)，未完成工作见[当前待办](./maintenance-backlog.md)，每次验证的版本和环境见[交接入口](./handoffs/README.md)。

此前在 main `0c5e31a6` 复现的两类 Reader 并发问题已修复并合入：切文后迟到的图片激活打开旧图片，以及同种标记写入的旧响应覆盖新状态。合并树与已验收的 `21f3468a` 完全相同；候选的本地 app 428 项、独立规则 27 项、正式 Web 6 项、Windows WebView2 8 项及 Android 真机 4 项结果见[修复交接](./handoffs/2026-10-30-reader-race-fixes.md)。合并提交的新 main CI 36/36、Reader Web 6/6、Docker 容器 9 项及 Linux 包 6 项、Pages 构建/部署均通过，证据单列于[合并交接](./handoffs/2026-10-31-reader-race-merge.md)；不将合并前真机结果写成重新验收。

## 三个不同的版本状态

| 范围 | 当前事实 | 证据与边界 |
| --- | --- | --- |
| 已发布版本 | v0.2.1，tag 固定在 `dbfbf896` | [正式发布记录](./handoffs/2026-10-28-v0.2.1-release.md)：14 个附件身份、Windows 正式包与 Android 模拟器升级等当时验收；不能替代后续源码验收 |
| main 的整合基线 | `b5b072b1`，已合并 PR #54–#56；正文选择归属和两类偏好测试门禁修正已整合 | 新 main CI 36/36、浏览器适配 13/13、工作区 18/18、Reader Web 6/6、容器 9 项、Linux 包 6 项及 Pages 均通过，见[合并回执](./handoffs/2026-10-31-workspace-cancel-merge.md) |
| 当前接续工作树 | E 盘 `codex/main-regression-20261031` 基于 main `60b4c6f1`；本地已修复 Reader 读取恢复后旧错误提示残留，尚未合入 main | 修复前 Rust/Web 均复现；候选 app 445 项、独立规则 37 项、正式 Web 8 项与 7 个故障场景、Windows 宿主 8 项通过，Android 仅编译，见[本批交接](./handoffs/2026-10-31-reader-reread-recovery.md) |

main `4101520c` 的 CI 36/36、Docker 构建和容器验收、容器 9 项及 Linux 包 6 项成功，是上一轮已留存的历史验收；[E 盘交接](./handoffs/2026-10-29-pr40-worktree-handoff.md)记录了报告身份和复制核验。这些结果不作为 `62113c4f` 返回策略通过原生验收的证明。

## 已完成的主线整合

- PR #38 已合入正文解析热点修复与后台图片队列约束，合并提交 `6e99706c`；不再是待合并的性能工作。单 token 取消、304 续排及规模观察等边界仍在[当前待办](./maintenance-backlog.md)。
- PR #40 已合入 Entries 初始化、查询、写入与偏好，Reader 访问归属，Feeds 编辑与读取，以及 Settings 编辑、保存、异步回填等规则。保留七 crate、`Backend` / `LocalBackend`、rusqlite 单库、正式 Web 同源服务和 Settings 逐字段保存及 `stored → submitted → pending` 回填。
- PR #39、#41–#45 已完成目录入口、固定标题、二级页面层级、主题预览、目录顺序及相关验收/文档整合。目录的 Windows 原生专项证据绑定 `0692c924`，见[原生复测](./handoffs/2026-10-29-directory-native-verification.md)。
- PR #46 已合入返回导航/退出提示策略及此前授权的文档与维护资料整合；Windows / Android 返回专项仍以[其实际宿主回执](./handoffs/2026-10-30-native-back-validation.md)的包和场景为准。
- PR #47 已合入 App 设置读取生命周期；其原始 Rust/Web/Windows 验证见[实现交接](./handoffs/2026-10-30-shell-settings-lifecycle.md)。本轮另补 Android 主题选择专项，不能扩大为该批全部 Android 生命周期验收。
- PR #50–#52 已按顺序合并：规则独立性 `7edb118d`、宿主入口 `5d51025a`、快捷键定义与模块归属 `dbbaf390`。分支保留用于溯源，当前无这三批的待合并实现；正文兼容要求见[专门契约](./design/reader-rendering-boundary.md)。
- PR #53 已合入图片访问隔离和同种标记写入门禁，合并提交 `ed0714d3`。没有新增全局浮层状态、跨页写入队列或后台业务语义分叉；复现与新增回归见[修复交接](./handoffs/2026-10-30-reader-race-fixes.md)。
- PR #54 已将正文选择归属整理合入 `465fead9`；PR #55 已将偏好测试完成门禁修正合入 `79d8711d`。前者的运行规则保持不变，后者仅涉及测试和文档；主线验收见[对应合并回执](./handoffs/2026-10-31-workspace-response-merge.md)。
- PR #56 已将提前取消不能充当 EOF 的测试修正合入 `b5b072b1`，没有修改产品保存逻辑；新的 main 检查与具体产物见[合并回执](./handoffs/2026-10-31-workspace-cancel-merge.md)。
- 阅读位置、目录控制器与窗口、下拉刷新、快捷键、图片查看器已有 Rust 规则；继续复用这些模块，不建立全局浮层状态副本或通用页面框架。模块边界见[原生界面前置说明](./design/native-ui-preparation.md)。

## 现在还要做什么

1. **保留已收口的规则边界。** App 设置读取已经审查、CI 和合并；不再列为候选。主题选择的现有行为已完成专项，不继续扩大 Shell 提取或重复实现系统弹窗。
2. **维持已完成的分支与资料收口。** 历史验收资料和 Claude 配置已在工作树外备份并校验，5 个旧工作树及 3 个分支已退役，见[清理与恢复记录](./handoffs/2026-10-30-branch-consolidation.md)。保留接续工作树与主 checkout 的既有改动，不向已合并的 PR #40 追加实现，不发布新版本。
3. **先收口本地修复，再按具体复用阻碍安排下一批。** PR #54–#56 均已整合；新的 Reader 读取提示修复已完成本地验证，待进入主线审查，见[本批交接](./handoffs/2026-10-31-reader-reread-recovery.md)。后续模块提取由实际依赖问题驱动；原生正文渲染器仍须验证主题、选择复制、IME 和无障碍，见[后续边界](./design/reader-rendering-boundary.md#下一批建议)。

## 分支与工作树快照

PR #56 刚合并时的历史快照：**12 个本地分支、2 个工作树、11 个真实远端分支**，当时接续工作树为 `main`。后续补充回归已在同一工作树新建 `codex/main-regression-20261031`，本批继续使用；不把历史快照当作实时分支数量。合并时远端为 main、codex/native-back-policy、codex/shell-settings-lifecycle、codex/android-theme-import、codex/reader-rule-boundary、codex/reader-host-boundary、codex/reader-shortcut-definition、codex/reader-race-fixes、codex/reader-body-boundary、codex/workspace-response-completion、codex/workspace-response-cancel，不把 `origin/HEAD` 算作分支。

当时本地保留 main、上述十个已合并的 codex 分支和主 checkout 的 `feat/reading-workspace-20261024`。本批没有清理分支、工作树或证据。主 checkout 的 HEAD、状态、index 与 41 个文件/缺失路径由本轮独立快照核对；另一任务的 JavaScript 审计文档不纳入提交。历史备份与恢复方式见[清理及恢复清单](./handoffs/2026-10-30-branch-consolidation.md)。

## 验证结论的边界

- 历史记录仅证明其中指定的源码、包、设备及场景；发布、构建、普通浏览器和真实原生宿主验收分开记录。
- Android 正式签名包真机升级、真实旧数据搬迁，以及 macOS/Linux GUI、系统 IME、辅助技术和更广设备交互仍未全面覆盖。
- Penpot 是设计基准，不模拟网络、持久化、滚动和全部状态组合；已有抽查见[设计基准](./design/penpot-code-baseline.md)。
- App 内的写入排序不提供跨浏览器或跨设备的全局操作序列保证。
