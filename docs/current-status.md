# 维护状态

核对日期：2026-10-30。当前处于**原生界面接入前的模块化收口阶段**：页面交互与异步规则逐步独立为可测试的 Rust 模块，界面仍使用 Dioxus / WebView，尚未接入原生控件。文档整合、全部分支审计、旧引用及旧工作树清理已完成；当前优先补返回策略的宿主验收，不继续扩大重构范围。

本页只汇总当前事实；具体规则见[设计索引](./design/README.md)，未完成工作见[当前待办](./maintenance-backlog.md)，每次验证的版本和环境见[交接入口](./handoffs/README.md)。

## 三个不同的版本状态

| 范围 | 当前事实 | 证据与边界 |
| --- | --- | --- |
| 已发布版本 | v0.2.1，tag 固定在 `dbfbf896` | [正式发布记录](./handoffs/2026-10-28-v0.2.1-release.md)：14 个附件身份、Windows 正式包与 Android 模拟器升级等当时验收；不能替代后续源码验收 |
| 最新 main | `4101520c`；本地 main 已快进到同一提交 | PR #38–#45 均已合并；[PR #38 回执](./handoffs/2026-10-29-parse-image-pr-ci.md)、[PR #40 最终审查](./handoffs/2026-10-29-pr40-final-review-merge.md)、[设计与目录整合](./handoffs/2026-10-29-completed-design-cleanup.md) |
| 候选分支 | `codex/native-back-policy`，返回策略实现 `62113c4f`，基于上述 main；随后仅整理文档 | 本轮已获推送授权，待远端回执，尚未进入 main。Rust / Web 检查及 Windows / Android 构建已完成；实际 Windows / Android 宿主交互未完成，见[返回策略交接](./handoffs/2026-10-29-native-back-policy.md) |

main `4101520c` 的 CI 36/36、Docker 构建和容器验收、容器 9 项及 Linux 包 6 项成功，是上一轮已留存的历史验收；[E 盘交接](./handoffs/2026-10-29-pr40-worktree-handoff.md)记录了报告身份和复制核验。这些结果不作为 `62113c4f` 返回策略通过原生验收的证明。

## 已完成的主线整合

- PR #38 已合入正文解析热点修复与后台图片队列约束，合并提交 `6e99706c`；不再是待合并的性能工作。单 token 取消、304 续排及规模观察等边界仍在[当前待办](./maintenance-backlog.md)。
- PR #40 已合入 Entries 初始化、查询、写入与偏好，Reader 访问归属，Feeds 编辑与读取，以及 Settings 编辑、保存、异步回填等规则。保留七 crate、`Backend` / `LocalBackend`、rusqlite 单库、正式 Web 同源服务和 Settings 逐字段保存及 `stored → submitted → pending` 回填。
- PR #39、#41–#45 已完成目录入口、固定标题、二级页面层级、主题预览、目录顺序及相关验收/文档整合。目录的 Windows 原生专项证据绑定 `0692c924`，见[原生复测](./handoffs/2026-10-29-directory-native-verification.md)。
- 阅读位置、目录控制器与窗口、下拉刷新、快捷键、图片查看器已有 Rust 规则；继续复用这些模块，不建立全局浮层状态副本或通用页面框架。模块边界见[原生界面前置说明](./design/native-ui-preparation.md)。

## 现在还要做什么

1. **补返回策略实际宿主验收。** Windows 使用真实 WebView2 / CDP；Android 使用隔离测试包和资料库验证系统返回、首页 5 秒提示、恢复前台、位置采集失败/超时。上一轮被自动执行审批阻止，未运行这些场景；设备存在和编译成功均不等于已验收。
2. **收口候选。** 文档整合和全部分支审计完成后，已将历史验收资料和 Claude 本地配置复制到工作树外并校验，退役 5 个旧工作树及对应的 3 个分支，见[清理与恢复记录](./handoffs/2026-10-30-branch-consolidation.md)。用户已授权推送当前候选；新 PR 和合并仍待宿主验证及后续授权，不向已合并的 PR #40 追加实现，不发布。
3. **后续工作按具体收益选择。** 原生控件接入尚未开始；没有批准 Shell 重构、渲染器替换、新 crate 或数据库迁移。旧库升级、更多设备与输入方式等持续维护事项保留在待办，不要求先完成所有历史风险才结束本批。

## 分支与工作树快照

截至清理完成：**3 个本地分支、2 个工作树**；远端暂为 main，候选分支待本批推送回执。不把 `origin/HEAD` 当作分支。此前的 4 条失效登记和本次 5 个旧工作树均已清理。

保留 main、`codex/native-back-policy` 和主 checkout 的 `feat/reading-workspace-20261024`。主 checkout `E:/gitclone/RSS-Reader` 的 40 条既有状态、暂存区和已记录文件散列保持原样；当前返回策略工作树继续保留。旧源码由已验证的 Git bundle 保全，4004 个历史验收文件、Claude 配置及 5 个 Linux 链接元数据另有独立备份，详见[清理及恢复清单](./handoffs/2026-10-30-branch-consolidation.md)。

## 验证结论的边界

- 历史记录仅证明其中指定的源码、包、设备及场景；发布、构建、普通浏览器和真实原生宿主验收分开记录。
- Android 正式签名包真机升级、真实旧数据搬迁，以及 macOS/Linux GUI、系统 IME、辅助技术和更广设备交互仍未全面覆盖。
- Penpot 是设计基准，不模拟网络、持久化、滚动和全部状态组合；已有抽查见[设计基准](./design/penpot-code-baseline.md)。
- App 内的写入排序不提供跨浏览器或跨设备的全局操作序列保证。
