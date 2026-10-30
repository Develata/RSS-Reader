# 维护状态

核对日期：2026-10-30。当前处于**前置模块化已收口、原生控件边界已评估的阶段**：返回策略和 App 设置读取分别通过 PR #46、#47 合入 main。主题模式选择已完成 Windows / Android 实际宿主评估；Android 已有系统选择弹窗，尚无证据支持另写一套。界面仍使用 Dioxus / WebView，未接入新的原生控件或渲染器。

当前工作分支另按用户要求移除列表密度选项及紧凑样式，统一原默认间距；旧参数仅暂留 0.2.x 兼容，0.3.x 破坏性删除已批准。仓库 `.agents/` 中 9 份过时 Spec Kit skill 一并清理，见[本批交接](./handoffs/2026-10-30-density-retirement.md)。

本页只汇总当前事实；具体规则见[设计索引](./design/README.md)，未完成工作见[当前待办](./maintenance-backlog.md)，每次验证的版本和环境见[交接入口](./handoffs/README.md)。

## 三个不同的版本状态

| 范围 | 当前事实 | 证据与边界 |
| --- | --- | --- |
| 已发布版本 | v0.2.1，tag 固定在 `dbfbf896` | [正式发布记录](./handoffs/2026-10-28-v0.2.1-release.md)：14 个附件身份、Windows 正式包与 Android 模拟器升级等当时验收；不能替代后续源码验收 |
| 最新 main | `663ef4ee`；本地 main 已快进到同一提交 | PR #38–#47 已合并；PR #47 head 的 CI 36/36、Pages build 通过，deploy 按 PR 规则跳过。合并树与 head `f38c3ba8` 相同，见[本批交接](./handoffs/2026-10-30-native-theme-selector-assessment.md) |
| 当前工作分支 | 复用 `codex/shell-settings-lifecycle` 和原 E 盘工作树；主题选择评估已推送，本批增加列表密度退役与 `.agents/` 清理，尚未另开 PR / 合入 main | 先前主题选择评估属于其原源码；本批有新的候选构建与专项验收，以[退役交接](./handoffs/2026-10-30-density-retirement.md)为准 |

main `4101520c` 的 CI 36/36、Docker 构建和容器验收、容器 9 项及 Linux 包 6 项成功，是上一轮已留存的历史验收；[E 盘交接](./handoffs/2026-10-29-pr40-worktree-handoff.md)记录了报告身份和复制核验。这些结果不作为 `62113c4f` 返回策略通过原生验收的证明。

## 已完成的主线整合

- PR #38 已合入正文解析热点修复与后台图片队列约束，合并提交 `6e99706c`；不再是待合并的性能工作。单 token 取消、304 续排及规模观察等边界仍在[当前待办](./maintenance-backlog.md)。
- PR #40 已合入 Entries 初始化、查询、写入与偏好，Reader 访问归属，Feeds 编辑与读取，以及 Settings 编辑、保存、异步回填等规则。保留七 crate、`Backend` / `LocalBackend`、rusqlite 单库、正式 Web 同源服务和 Settings 逐字段保存及 `stored → submitted → pending` 回填。
- PR #39、#41–#45 已完成目录入口、固定标题、二级页面层级、主题预览、目录顺序及相关验收/文档整合。目录的 Windows 原生专项证据绑定 `0692c924`，见[原生复测](./handoffs/2026-10-29-directory-native-verification.md)。
- PR #46 已合入返回导航/退出提示策略及此前授权的文档与维护资料整合；Windows / Android 返回专项仍以[其实际宿主回执](./handoffs/2026-10-30-native-back-validation.md)的包和场景为准。
- PR #47 已合入 App 设置读取生命周期；其原始 Rust/Web/Windows 验证见[实现交接](./handoffs/2026-10-30-shell-settings-lifecycle.md)。本轮另补 Android 主题选择专项，不能扩大为该批全部 Android 生命周期验收。
- 阅读位置、目录控制器与窗口、下拉刷新、快捷键、图片查看器已有 Rust 规则；继续复用这些模块，不建立全局浮层状态副本或通用页面框架。模块边界见[原生界面前置说明](./design/native-ui-preparation.md)。

## 现在还要做什么

1. **保留已收口的规则边界。** App 设置读取已经审查、CI 和合并；不再列为候选。主题选择的现有行为已完成专项，不继续扩大 Shell 提取或重复实现系统弹窗。
2. **维持已完成的分支与资料收口。** 历史验收资料和 Claude 配置已在工作树外备份并校验，5 个旧工作树及 3 个分支已退役，见[清理与恢复记录](./handoffs/2026-10-30-branch-consolidation.md)。保留当前候选与主 checkout 的既有改动，不向已合并的 PR #40 追加实现，不发布。
3. **明确真正的原生表单边界。** 下一设计候选是 Windows 独立原生设置面板的小样，限主题选择、取消和显式保存。先具体说明窗口/session 所有权、CSS 兼容、回退与宿主验收；该设计尚未完成，也未批准整页替换、新框架、crate 或数据库迁移。详见[评估结论与完成标准](./design/native-theme-selector-assessment.md)。

## 分支与工作树快照

截至本批核对：**4 个本地分支、2 个工作树、3 个真实远端分支（main、codex/native-back-policy、codex/shell-settings-lifecycle）**。不把 `origin/HEAD` 当作分支。此前的 4 条失效登记和 5 个旧工作树均已清理；本次复用现有分支与工作树，没有再增删分支。

本地保留 main、已合并的 `codex/native-back-policy`、承载本次密度退役的 `codex/shell-settings-lifecycle` 和主 checkout 的 `feat/reading-workspace-20261024`。主 checkout `E:/gitclone/RSS-Reader` 的 40 条既有状态、暂存区及 42 个文件/缺失路径本批复核无变化；并行任务新增的 `2026-10-30-main-javascript-audit.md` 不属本批，也保留原状。旧源码由已验证的 Git bundle 保全，4004 个历史验收文件、Claude 配置及 5 个 Linux 链接元数据另有独立备份，详见[清理及恢复清单](./handoffs/2026-10-30-branch-consolidation.md)。

## 验证结论的边界

- 历史记录仅证明其中指定的源码、包、设备及场景；发布、构建、普通浏览器和真实原生宿主验收分开记录。
- Android 正式签名包真机升级、真实旧数据搬迁，以及 macOS/Linux GUI、系统 IME、辅助技术和更广设备交互仍未全面覆盖。
- Penpot 是设计基准，不模拟网络、持久化、滚动和全部状态组合；已有抽查见[设计基准](./design/penpot-code-baseline.md)。
- App 内的写入排序不提供跨浏览器或跨设备的全局操作序列保证。
