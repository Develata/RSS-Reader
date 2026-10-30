# 维护状态

核对日期：2026-10-30。近期目标是让 WebView 成为可替换的界面实现，让未来 Rust 原生界面复用同一交互规则；JS 行数仅作辅助指标。返回、App 设置读取和 Android 系统主题文件导入已进入 main。Reader 已按批次完成规则独立性、宿主入口和快捷键定义去重，保持现有行为；界面仍使用 Dioxus / WebView。

列表密度选项和紧凑样式、仓库 `.agents/` 的 9 份旧 skill 已经由 PR #48 合入 main；旧密度参数仅暂留 0.2.x 兼容，0.3.x 破坏性删除已批准但不在本批实施。Android 导入验收发现的开头 BOM 呈现缺陷已在旧包复现后独立修复，见[当前交接](./handoffs/2026-10-30-android-theme-import.md)。

本页只汇总当前事实；具体规则见[设计索引](./design/README.md)，未完成工作见[当前待办](./maintenance-backlog.md)，每次验证的版本和环境见[交接入口](./handoffs/README.md)。

## 三个不同的版本状态

| 范围 | 当前事实 | 证据与边界 |
| --- | --- | --- |
| 已发布版本 | v0.2.1，tag 固定在 `dbfbf896` | [正式发布记录](./handoffs/2026-10-28-v0.2.1-release.md)：14 个附件身份、Windows 正式包与 Android 模拟器升级等当时验收；不能替代后续源码验收 |
| 最新 main | `4cef074c`；本地 main 已快进到同一提交 | PR #38–#49 已合并；PR #49 head 与合并 main 的 CI 各 36/36，main Pages / Docker workflow 成功。合并树与 head `b03ebc4e` 相同，见[整合回执](./handoffs/2026-10-30-android-theme-import.md#后续整合回执2026-10-30) |
| 待合并的前一批 | `codex/reader-rule-boundary`，[PR #50](https://github.com/Develata/RSS-Reader/pull/50)，head `ff628ed0`、产品 `82beaf52` | 规则独立性与宿主验证见[前批交接](./handoffs/2026-10-30-reader-rule-boundary.md)；最新 head CI 36/36 和 Pages 成功，PR 仍 open，未在本批合并 |
| 待合并的宿主批次 | `codex/reader-host-boundary`，[PR #51](https://github.com/Develata/RSS-Reader/pull/51)，head `6a5b91b4`、产品 `9e0eb45b`；base 为 PR #50 分支 | CI 首轮 36/36；app 418 通过 / 1 忽略，同一 23 项规则独立编译通过。Windows 基线/候选各 6 组、Android 真机各 3 组、正式 Web 各 2 组，完整范围见[宿主交接](./handoffs/2026-10-30-reader-host-boundary.md)。仍 open，本批未合并 |
| 当前工作分支 | `codex/reader-shortcut-definition`，[PR #52](https://github.com/Develata/RSS-Reader/pull/52)，产品 `ddb7045e`；base 为 PR #51 分支 | app 420 通过 / 1 忽略，同一 24 项规则独立编译通过；Windows 基线/候选各 8 组、Android 真机各 4 组、正式 Web 各 4 组。最终 22 对截图中 21 对相同，余下一对的两个 7 像素差异状态均在两个版本出现。详见[快捷键交接](./handoffs/2026-10-30-reader-shortcut-definition.md)；远端 CI 以最终 head Checks 为准，base 非 main 不触发 Pages |

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
3. **按替换成本推进下一批。** Reader 规则、宿主入口和快捷键定义已完成有界提取。下一步先按依赖顺序收口 PR #50–#52，再明确正文内容/资源/锚点与 HTML/DOM 渲染边界。独立原生设置面板仍是候选，不优先于上述步骤；没有第二种实现前不新增通用框架、crate 或宿主接口。见[长期方向和本批边界](./design/native-ui-preparation.md)。

## 分支与工作树快照

截至本批推送后核对：**8 个本地分支、2 个工作树、7 个真实远端分支**。本批新增并推送 `codex/reader-shortcut-definition`；其余远端为 main、codex/native-back-policy、codex/shell-settings-lifecycle、codex/android-theme-import、codex/reader-rule-boundary、codex/reader-host-boundary。复用原工作树，不删除分支或新建工作树，不把 `origin/HEAD` 算作分支。

本地保留 main、已合并的 `codex/native-back-policy` / `codex/shell-settings-lifecycle` / `codex/android-theme-import`、待合并的 `codex/reader-rule-boundary` / `codex/reader-host-boundary`、当前 `codex/reader-shortcut-definition` 和主 checkout 的 `feat/reading-workspace-20261024`。主 checkout 的 HEAD、状态、index 与 41 个文件/缺失路径已复核保留；另一任务的 JavaScript 审计文档不纳入提交。历史备份与恢复方式见[清理及恢复清单](./handoffs/2026-10-30-branch-consolidation.md)。

## 验证结论的边界

- 历史记录仅证明其中指定的源码、包、设备及场景；发布、构建、普通浏览器和真实原生宿主验收分开记录。
- Android 正式签名包真机升级、真实旧数据搬迁，以及 macOS/Linux GUI、系统 IME、辅助技术和更广设备交互仍未全面覆盖。
- Penpot 是设计基准，不模拟网络、持久化、滚动和全部状态组合；已有抽查见[设计基准](./design/penpot-code-baseline.md)。
- App 内的写入排序不提供跨浏览器或跨设备的全局操作序列保证。
