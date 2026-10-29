# 目录突跳：阅读顺序模型修正与职责复查

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/directory-follow-jump`
- 当前代码 HEAD：`a3dbd63e12e86da0f9cbe082fc2d5b46e6ac0bba`；基线：`8932a5ac317bfcb1af9b2d966e16ce3d989a9911`
- 相关 commit：`a3dbd63e`（实现、回归与契约）；本验收记录随下一文档提交归档
- 相关 tag / release：N/A；不发布版本、不替换用户安装
- 状态：`validated`（本地代码与浏览器验收完成；远端检查与合并回执见对应 PR）

## 工作摘要

用户视频显示，主列表在同一天两个来源的交界处小幅来回滚动，目录却突然远距离往返。用与视频相同的多来源同日数据复现后，确认根因是分页与分组投影不一致，而非固定标题的 CSS。按用户进一步要求，复查分组、全局文章身份、跟随状态和虚拟窗口的职责，修正在共享分组算法中缺失的页号维度。

## 影响范围

- 模块：`rssr-app/pages/entries_page/groups.rs`、presenter 回归、目录夹具与浏览器回归、CI、设计契约。
- 平台：共享 Rust UI（Web / Windows / Linux / macOS / Android）；实际运行验收为 Windows 上的 Chrome 与隔离真实 `rssr-web`。
- 无 application / domain / infra、后端接口、存储格式或迁移变更，无新增依赖。
- 工作区：`C:/Users/QQ/.codex/worktrees/design-refinements/RSS-Reader`。从当时 `origin/main` 建新分支；本地前一轮审查提交 `db4beb90` 保留在原分支，没有混入本修复。

## 关键变更

### 已确认的算法问题与修正

- 主列表先按完整查询下标分页，再在每页内按日期、来源组织；旧时间目录先对全量文章按来源分组，再拍平日期。两种运算不交换：当前页的 Alpha 尾篇和 Bravo 首篇之间，在目录中插入了其他页的全部 Alpha 文章。
- 5000 篇夹具（当前查询可见 2000 篇、每页 50 篇）中，`4951 → 5000` 在主列表相邻，旧目录却相隔数百项。主列表往返 40px，旧产物逐帧最大目录位移为 17969px。
- `group_date_sources` 统一以 `(全局页号, 来源名称, 来源 ID)` 建立来源段。同一来源可在全量树中跨页出现多个段，在当前页树中仍只有一个段。时间目录仅拍平共享树，不维护独立排序规则；主列表自身的分页和页内顺序保持一致。
- 分支不变量：**完整目录序列 = 各页主列表对应分支序列依次拼接**。此规则对时间和来源两种模式都测试。全局 `(index, id)` 仍负责文章身份及页码，目录分支内 index 仅用于虚拟窗口。
- 完整目录保留全部引用；没有把目录截成当前 50 篇，没有增加延时、平滑滚动或跳跃阈值来掩盖错误。

### 算法与架构复查

| 职责 | 当前拥有者与核对结果 |
| --- | --- |
| 文章身份、分页、分组顺序 | presenter / `groups.rs`；此次修正缺失的页号维度，目录视图不再需要单独计算排序 |
| 跟随 / 手动、展开集合、活动路径 | Rust `DirectoryState`；通过 epoch 与 sequence 拒绝旧上下文和乱序事实，不因目录滚动重算 presenter |
| DOM 输入来源、渲染完成、实际对齐 | `adapter.js`；目录自己的滚动在 `onScroll` 内返回，不冒充主列表输入；命令等到消费者提交指定 revision 才执行 |
| 虚拟窗口范围、可变行高 | Rust `Window`；主跟随的 reveal 只挂载目标行，目录 adapter 决定是否对齐；显式键盘定位及行高补偿由窗口 adapter 执行，后者检查当前 epoch 与 DOM 所有权 |

复查与运行证据支持修正共享分组模型，不支持为该症状重写整个跟随控制器或迁移前后端职责。仍有线性扫描（虚拟目标 ID 查找、行高前缀重建）和换页时的全量投影计算；它们与此次错误顺序不同，没有测量证据表明必须引入索引树或新的缓存层。本轮不宣称性能或内存提升。

分组仍为有界树映射，最坏时间 `O(N log N)`、空间 `O(N)`。页号加入分组键会增加跨页来源段的数量和临时元数据；最多按文章数增长，不复制正文。滚动时没有新增分组计算、IPC 或 DOM 常驻量。

### 回归与设计契约

- 加入具体多来源反例、无日期与绝对下标测试。
- presenter 一致性矩阵覆盖两种模式、7 种页大小（含 0 防御值）、4 种条目规模，同名来源、跨日、跨月、无日期、末页和全部文章恰好出现一次；换页不能改变完整目录的顺序。
- 新浏览器回归逐帧检查来源边界反复滚动，并穿过多个虚拟窗口再反向返回；覆盖桌面浅/深色、760px 平板、390px 手机、固定标题、选择保留、目标可见和末篇跨页定位。已加入 `ci-reading-workspace` 流程。
- 更新长期目录契约与模块 README。静态画板布局、字号与按钮位置没有变化；本轮不声称更新了 Penpot 动态交互。

## 验证与验收

### 自动化验证

- 红灯：旧源码上的新增多来源测试失败；旧运行产物上同一浏览器回归失败，最大帧间位移 17969px。
- `cargo test -p rssr-app --locked`：301 通过、1 忽略（既有微基准）。
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过。
- `cargo build -p rssr-app -p rssr-web --locked`：通过。
- dx 0.7.9 `build --platform web --package rssr-app --release --locked --debug-symbols false`：通过。
- `scripts/browser/rssr_directory_page_order_acceptance.cjs`：26 项通过，0 页面异常。四个视口/主题组合的原复现动作最大跳跃均为 0px；跨窗口正反向共 294 次逐行定位，最大帧间位移均为 48px。
- `scripts/browser/rssr_directory_offline_regression.cjs`：150 项通过。
- `scripts/browser/rssr_directory_window_acceptance.cjs`：26 项通过。
- `scripts/browser/rssr_directory_window_boundaries.cjs`：11 项通过。
- `scripts/browser/rssr_directory_window_review.cjs`：8 场景通过，包括迟到 reveal 不覆盖较新的手动滚动、隐藏后变宽、焦点及勾选保留。上述脚本无记录到的页面异常。
- `cargo fmt --all --check`、`git diff --check`、两个变更脚本的 `node --check`：通过。

### 手工验收与证据

- 用户视频按帧检查，核对发生跳动时的主列表文章和活动目录项。
- 实际浏览器截图检查：桌面与手机目标在固定标题下可见；逐帧证据用于判断跳动，静态截图本身不证明滚动稳定。
- 证据目录：`C:/Users/QQ/Documents/Codex/2026-10-29/directory-jump/`。`regression-before/` 保存旧产物失败；`model-final/` 保存最终模型和最终回归脚本的截图、每帧轨迹及验收 JSON；`model-offline_regression/`、`model-window_acceptance/`、`model-window_boundaries/`、`model-window_review/` 保存已有回归；`rust-tests-model.log`、`clippy-model.log`、`native-build-model.log`、`web-build-model.log` 保存命令结果。产物构建于代码提交前的同一工作树，内嵌 Git 基线仍为 `8932a5ac`；对应生产源码已提交为 `a3dbd63e`，远端 CI 按 PR HEAD 重新构建。
- Windows 原生隔离复测启动被自动审批拒绝，工具只给出 `blocked by policy`，因此未执行；不能将原生编译通过算作原生交互通过。Android、Safari、本机触摸/惯性及辅助技术此次未验收。

## 结果

- 已从共享分组模型修复可重复的目录突跳，本地 301 项 Rust 测试及 221 项浏览器断言/场景通过；可进入 PR 检查，不构成全平台发布验收。
- 不修改用户资料库、现有运行窗口或安装产物。

## 风险与后续事项

- 证明范围是上述分支顺序不变量和已执行的浏览器场景，不是所有平台及所有输入时序都无跳动。
- 保留来源/时间分组、跨页选择清空、手动浏览、恢复跟随和目录关闭重开语义。原生运行缺口须独立记账。
- 若未来测量发现几何补偿抢占新输入，应先补上能控制消息时序的失败复现，再审视滚动写入所有权；不能直接追加延时或重试循环。

## 给下一位 Agent 的备注

- 算法入口为 `groups.rs::group_date_sources`；跨组件不变量在 presenter 的 `every_directory_branch_is_the_concatenation_of_its_pages_reading_order`。
- 浏览器回归需指向同次构建的 Web bundle、`rssr-web` 与 fixture 工具；测试端口 8119，依次运行，不能并行启动多个夹具。
- 不把 `regression-after/` 的早期验证当最终模型证据；最终证据以 `model-final/` 及上述 `model-*` 回归目录为准。
