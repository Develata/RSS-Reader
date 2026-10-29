# PR #40 后的原生前置模块化建议

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/native-back-policy`（现已迁到独立 E 盘工作树；只提交评估与交接文档）
- 评估源码 HEAD：`4101520c33266026747db867c33e2b13baea3d23`
- 相关 commit：`a641cee1` 为初次评估；后续接续路径与收尾记录见本文件 git log
- 相关 tag / release：无
- 状态：`draft`（源码评估完成，下一批尚未实施）

## 工作摘要

PR #40 已合并，四个页面的主要异步规则已完成本轮收口。下一批建议只独立系统返回和退出提示的纯 Rust 决策，保留现有宿主执行路径。该建议以当前源码为依据，不把“继续拆文件”当作原生接入的无限前置条件，也不宣称发现了新的运行缺陷。

## 接续背景

这项工作起初是为未来原生界面准备可复用的 Rust 规则，先从 Entries 的初始化和查询生命周期开始，随后按批处理写入/偏好、Reader 访问归属、Feeds 编辑与读取、Settings 编辑保存和异步回填。期间主线已调整为七 crate、Backend/LocalBackend、rusqlite 单库及逐字段 Settings 保存，因此 PR #40 的整合采用了主线架构和保存语义，旧分支的整份草稿替换与 BrowserStore 路径已经退役。

PR #40 的代码工作、冲突整合及最终 review 已结束，合并提交为 `ad5e8f31`；后续 main `4101520c` 的 CI 和 Docker 也已完成验证。`codex/native-back-policy` 是为了下一次 session 准备的本地接续分支，目前只有评估与交接文档，返回策略尚未实现。分支名不能作为已完成新一批重构的证据。

选择返回策略，是因为它仍将决策和宿主执行组合在同一个 hook 中，且已有明确的行为契约和历史跨线程修复可作约束；这是模块边界的改进建议，不是一个已经复现的新缺陷。用户确认本 session 只收尾交接并迁移到 E 盘，实际实现留给新 session。迁移、证据位置和清理限制见[工作树接续记录](./2026-10-29-pr40-worktree-handoff.md)。

## 影响范围

- 本次变更：本交接及交接索引，没有生产代码、测试、依赖或工作流改动。
- 下一批主要入口：[返回 hook](../../crates/rssr-app/src/hooks/use_mobile_back_navigation.rs)、[跨线程捕获门禁](../../crates/rssr-app/src/hooks/use_mobile_back_navigation/back_capture.rs)、[Android 输入桥](../../crates/rssr-app/src/hooks/use_mobile_back_navigation/android_bridge.rs)。
- 相关调用：Entries 的来源筛选/目录、Reader 的图片查看器、Feeds 的编辑关闭，以及 Settings 的返回入口。
- 目标平台：策略为普通 Rust；Android 系统返回是主要行为面。共用工具栏返回影响 Web、Windows 及其他原生宿主，应保留同步路径。
- 当前接续工作树：`E:/gitclone/RSS-Reader-worktrees/native-back-policy`。原评估发生于 `C:/Users/QQ/.codex/worktrees/pr40-main-integration/RSS-Reader`；E 盘主仓库的其他任务改动保持原样。

## 当前源码事实

- [原生前置边界](../design/native-ui-preparation.md)及[控件契约](../design/native-control-adapter-contracts.md)已记录 Entries 查询/写入、Reader 结果归属、Feeds 编辑/读取和 Settings 逐字段草稿/保存/回填规则。不要重复实施 10 月 27 日记录中的旧“下一批”。
- 阅读位置、目录控制器/窗口、下拉刷新和快捷键已有可独立测试的 Rust 模块。图片查看器已有 `Closed / Open` 状态，主要剩余 JS 是输入、同步默认动作、焦点、模态和滚动适配；不能仅以 JS 行数判断优先级。
- `use_mobile_back_navigation.rs` 仍在 Dioxus hook 中组合 dismiss 优先、history/fallback 选择、首页退出提示、提示代次、定时清除及恢复前台处理。
- `back_capture.rs` 已有跨线程原子 guard；`android_bridge.rs` 已有监听代次与容量 1 的输入合并测试。这些不是待重建模块。
- 返回按钮同步导航；Android 系统返回在导航前采集位置，最多等待 500 ms，采集失败仍继续。首页首次返回显示提示，5 秒内再次返回移入后台；恢复前台清除旧提示。
- `ui/shell.rs` 仍将恢复读取的合并、设置重读与 Dioxus/App scope 编排组合在一起，可在后续单独评估；本批不动。

## 建议顺序

| 顺序 | 范围 | 完成标准 |
| --- | --- | --- |
| 1 | 系统返回和退出提示策略 | 决策与提示状态能用普通 Rust 事件序列重放，现有 hook 只装配事实与副作用，当前交互保持。 |
| 2，待独立评估 | Shell 的恢复读取/启动设置读取规则 | 先确认具体耦合与收益，再选择一个入口提取；保留 App 作业所有权、只读刷新及逐字段设置边界，不做全局运行时重写。 |
| 3，接入具体原生控件时 | 宿主能力与正文呈现兼容 | 依据现有契约核对焦点、模态、文件、选择复制、IME、滚动、HTML/MathML 与自定义 CSS；以实际控件需求确定适配接口与样本。 |

后两项是后续候选和接入门禁，不是本批一并实施清单。原生正文如何保持 HTML/CSS 与主题能力尚需可评审方案；当前不选择或替换渲染器，不先建通用 host SDK。

## 下一批明确范围

1. 先记录现有输入到动作的顺序，再在现有 hook 子模块中提取具体私有 Rust 策略。输入是已确定的事实，输出是返回、展示/清除提示或退后台等动作；策略不依赖 `Signal`、`Navigator`、DOM、JNI 或真实计时器。
2. 各页面继续拥有自己的浮层状态与 dismiss gate。策略只接收“本次返回已被页面消费”的事实，来源编辑保存中消耗返回但不关闭的规则继续由 Feeds 决定；不建立全局浮层状态镜像。
3. 首页提示状态和代次只有一个所有者；超时作为显式事件输入，旧超时不能清除新一轮提示，恢复前台清除提示。真实 sleep、窗口恢复和退后台调用留在适配层，保留 5 秒行为。
4. history/fallback 的事实采集与执行留在当前适配器，保持 fallback 等于当前路由时不重复替换。Android 系统返回才走位置采集等待；工具栏点击继续同步导航。
5. 复用现有原子 guard、500 ms 位置采集上限、输入队列与监听代次。保留完成/失败/取消释放、等待时输入合并和返回前的路由检查；不把 gate 改回 thread_local，也不改变捕获顺序。
6. 本批不新增 crate、公开 API、通用页面框架、数据库迁移或新的业务状态副本。七 crate、Backend/LocalBackend、单库、正式 Web 同源服务和 Settings 的 stored → submitted → pending 规则保持。
7. 若核对出返回生命周期的新缺陷，先复现，再单列修复和回归证据；不要把推测的竞态或交互变化藏进等价提取。

## 验证与验收

### 本次实际执行

- `git status --short`、`git rev-parse HEAD`、`git ls-remote origin refs/heads/main`：评估开始时本地和远端 main 都为 `4101520c`，工作树干净。
- `rg` 与源码阅读：核对全局/仓库/pages 指令、功能边界、原生前置与控件契约、当前维护待办、返回 hook/门禁/输入桥、各页面调用及 Shell 编排；没有 `.codegraph`，未初始化索引。
- 提交前核对本文链接、索引、`git diff --check`、暂存路径和生产代码无差异；仅文档，不重跑 Cargo、浏览器、容器或设备测试。
- 主线 `4101520c` 的 CI 36/36、Docker 4 成功/1 预期跳过是上一轮同一会话的实际运行结果，链接和 artifact 身份见 [PR #40 最终 main 回执](https://github.com/Develata/RSS-Reader/pull/40)；不算本次评估重新运行，也不适用于下一批改动后的源码。

### 下一批实施验收

- 普通 Rust 测试不创建 `VirtualDom`：页面先消费、history/fallback、首页第一/第二次返回、超时后的再次返回、旧提示计时器、恢复前台、重复事件；测试以可观察动作与状态为准。
- 保留并运行 `back_capture` 和 `android_bridge` 现有回归；补必要的 hook 装配测试，核对捕获失败/超时、任务取消、在途重复输入、旧监听清理和路由变化后的等待处理。
- 运行 `cargo fmt --all --check`、`cargo test -p rssr-app --locked`、相称的 Clippy，以及 Web/Android 编译检查；记录实际执行结果，不照抄历史测试数量。
- 基线和候选使用相同隔离 fixture，复用现有浏览器/原生脚本核对工具栏返回、阅读位置恢复、图片/来源筛选/目录/来源编辑关闭优先级与保存中返回。脚本入口先核对当前文件和参数；旧 BrowserStore 脚本不恢复。
- 浏览器和 Windows WebView2/CDP 用于共用路径；Android 系统返回、首页两次返回、超时、恢复前台及连续阅读必须通过实际 Android 宿主（可用设备或模拟器）记录。直接使用 adb/CDP，保护用户数据，不以浏览器小视口或编译通过替代。若无可用目标，明确列为未覆盖，不能宣称 Android 验收完成。
- 模块内独立测试、适配器集成、真实宿主结果和性能结论分别记录；不从文件拆分推断速度或内存收益。

## 结果与风险

- 本次交付是下一批范围与接续入口；没有开始实现或修复产品行为。评估文档本地提交，不 push、打 tag 或发布。
- 返回功能曾有跨线程门禁问题，已有历史修复必须保留；新状态模型不能增加一次异步往返或让工具栏返回等待位置采集。
- 当前源码只做静态核对，没有复现新的 ABA、超时或设备缺陷；上述风险是验收重点，不是新缺陷结论。

## 给下一位 Agent 的备注

- 在 `E:/gitclone/RSS-Reader-worktrees/native-back-policy` 复用当前 `codex/native-back-policy` 分支；先重新核对 HEAD、dirty 状态与最新 main，再开始有界实现。不要在保留的 C 盘历史 checkout 或 E 盘主仓库启动本批，也不要向已合并的 PR #40 追加新实现。
- 优先阅读本记录、[前端命令契约](../design/frontend-command-reference.md)、[原生控件契约](../design/native-control-adapter-contracts.md)与 [Android 连续返回修复](./2026-10-25-android-repeated-back.md)。历史记录仅用于了解原因，以当前源码和契约为准。
- 完成后 review、相称验收、更新长期契约中实际变化的部分和 `docs/handoffs/`，只暂存本任务文件并本地提交。新的 push、PR 和发布不属于本份接续范围。
