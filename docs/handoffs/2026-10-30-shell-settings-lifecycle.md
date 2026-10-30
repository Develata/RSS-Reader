# PR #46 整合与 App 设置读取生命周期

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/shell-settings-lifecycle`
- 提取基线：`6a75fb835170589a14b31d8deff16ec3a678de97`
- 实现 commit：`14f334d92711a526c3b7d0126b75f7651ae4e904`（实现、测试、文档及首份证据）；后续回执提交仅补提交与推送状态
- 相关 tag / release：N/A；不发布
- 状态：`validated`；本记录原交付时为已推送候选；同日随后通过 PR #47 合并，见下方更新。未发布。

## 后续合并回执（2026-10-30）

[PR #47](https://github.com/Develata/RSS-Reader/pull/47) 将 head `f38c3ba8` 合入 main `663ef4ee`，两个 tree 相同。当前提交的 CI 36/36、Pages build 通过，deploy 按 PR 规则跳过，review 无遗留线程。完整回执及随后主题控件专项见[下一批交接](./2026-10-30-native-theme-selector-assessment.md)。下文“尚未 CI / 候选”等是原交付时状态；本轮 Android 主题专项不追认下文未执行的完整 Android 生命周期验收。

## 工作摘要

按用户确认的顺序，先将返回策略和此前完成的维护资料整合为独立 PR，再有界提取 App 级设置读取生命周期。没有发现需要单列修复的产品缺陷；本批保持既有行为，未扩大 Shell、渲染器或数据库职责。

[PR #46](https://github.com/Develata/RSS-Reader/pull/46) 的 head 为 `a3da94d0dc053f543a5c97cde03943aa154a4f49`，合并为 `6a75fb835170589a14b31d8deff16ec3a678de97`。合并前核对 head/base、mergeable、审查记录与线程均无待处理项；[CI #323](https://github.com/Develata/RSS-Reader/actions/runs/41090441523) 的 36/36 作业通过，[Pages demo #218](https://github.com/Develata/RSS-Reader/actions/runs/41090441643) 通过。自查未发现新增问题；没有声称有独立审查人的批准。合并树与该 head 的文件内容一致。

## 影响范围

- 产品仅 `crates/rssr-app/src/ui/shell.rs` 和新增私有 `ui/shell/settings_read_lifecycle.rs`；不新增 crate、依赖或公开 API。
- Windows / 正式 Web 实际运行；Android 仅本批交叉编译检查。七 crate、Backend / LocalBackend、rusqlite 单库及页面 `stored → submitted → pending` 不变。
- 新增可重复的 Windows CDP 设置专项 `scripts/browser/rssr_shell_settings_acceptance.mjs`，使用新建目录、复制的夹具库与独立 WebView2 profile，核对端口和进程归属。
- 复用 `E:/gitclone/RSS-Reader-worktrees/native-back-policy`，切出当前分支，未新建工作树。主 checkout 的 40 条状态、暂存区散列及 42 个文件/缺失路径核对均保持原样。

## 关键变更

- 生命周期拥有单个在途读取、启动确认、一次待补读、启动错误与带存储 revision 的重读错误。模式由后端启动确认决定，与读取成功与否分开。
- 自动读取在 Settings 页面打开时不进入队列；显式重试/恢复允许读取。在途触发合并为一次后续自动读取，完成时页面已打开则消费并放弃，不在关闭时重放。
- 旧成功不得覆盖后来的存储，等值存储同样推进 revision；成功结束旧重读错误。失败属于完成时的 revision，之后的存储隐藏该错误。页面存储立即隐藏启动错误并按既有路径完成恢复。
- Shell 仍在 App 作用域派发命令、收集事实、应用主题与发布 signal；恢复 effect 对策略 `peek`，只跟随主题存储变化。200 ms 触发合并、页面生命周期、后台刷新实现和实际任务取消规则不变。
- 策略 7 组普通 Rust 事件序列，保留 11 组 Shell 适配测试，增加 3 组并扩充在途合并测试，覆盖等值存储、失败恢复不自激重试、页面开启时消费排队请求及显式重试。

## 验证与验收

紧凑证据见 [result.json](../testing/evidence/2026-10-30-shell-settings-lifecycle/result.json)，原始日志、产物、隔离资料库和截图在 `target/shell-settings-20261030/`。基线复用上一批 `62113c4f` 的已核验 Windows / Web 产物；本次确认其全部产品构建输入与 `6a75fb83` 相同，并重新运行下列基线场景。候选来自本批最终源码，Web 页面和服务器均校验构建标识 `dev-6a75fb835170-dirty-a467516b802b`；提交后以文件散列对应身份，不把 dirty 构建标识冒充提交构建。

| 检查 | 实际结果 | 本地证据 |
| --- | --- | --- |
| 原始 Shell 基线 `cargo test -p rssr-app --locked ui::shell::tests -- --nocapture` | 11 通过 | `baseline-shell-tests.log` |
| `rustc --test --edition 2024 target/shell-settings-20261030/policy-tests.rs` 及产物 | 7/7，独立于 Dioxus | `policy-tests.log` |
| `cargo test -p rssr-app --locked` | 383 通过、1 既有忽略；378 单元 + 2 主题 + 3 样式契约 | `app-tests.log` |
| `cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过 | `clippy.log` |
| `cargo fmt --all --check`、`git diff --check` | 通过 | 提交前核验 |
| `dx build --locked --platform web --package rssr-app --release --debug-symbols false` | 正式 Web 构建通过，不启用 pages-demo | `web-build-final.log` |
| `cargo build -p rssr-web -p rssr-app --locked` | Windows 与匹配服务器通过 | `native-server-build-final.log` |
| `rssr_server_acceptance.mjs --only reread-triggers,settings-read-fails-at-start,startup-target-fails` | 基线/候选各 3/3，包括真实标签页切换、离线/恢复、失败重试；无非预期控制台错误 | `web-{baseline,candidate}-cdp/assertions.json` |
| `node scripts/browser/rssr_native_preparation_acceptance.cjs` | 基线/候选各 8/8，1536/390 视口；逐字段保存、回填、解析提示、主题文件；页面/控制台错误均 0 | `web-{baseline,candidate}-preparation/acceptance.json` |
| `node scripts/browser/rssr_shell_settings_acceptance.mjs --app-bin <对应 EXE> --seed-dir target/back-policy-native-20261030/seed --artifact-dir <新目录>` | 真实 Windows 11 / WebView2 156.0.4314.50，1280×900，基线/候选各 3/3；启动存储主题、外部改值后只读更新、Settings 重读保留 pending 及离页恢复存储主题 | `windows-baseline-03/acceptance.json`、`windows-candidate/acceptance.json` |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | 通过；NDK 28.2 / API 24。既有 infra `LOCAL_DATA_DIR_NAME` 未使用警告保留 | `android-check.log` |
| 文档本地链接与新 JSON 检查 | 1486 个本地链接，无新增或当前改动文档错误；两份证据 JSON 可解析 | `documentation-check.json` |

Windows 专项的外部改值通过测试库 SQLite 连接模拟，focus/visible/online 为发送给真实宿主的 DOM 事件，不声称覆盖 OS 前后台切换。本次没有新 Android 包安装或真机运行，也未重跑 Docker、Linux/macOS GUI、IME、辅助技术或正式包升级。

Windows 两个后续状态截图逐像素相同；启动图只有首页 R 图标区域（x=116–128，y=40–54）的 112 个像素不同，未观察到布局或主题差异，未单独确认这处像素差异的原因，不声称三张全部一致。截图与计数见 `windows-visual-comparison.json`。夹具的回环 feed 服务未启动，启动自动刷新的抓取失败日志在两版均存在；这些不是设置读取失败。

测试过程中保留了两类驱动问题的失败材料：Playwright 管理的 Chrome 自动启用焦点模拟，导致 `document.hidden` 等待超时；直接启动 Chrome、复用原 CDP 场景后 3/3 通过。新 Windows 驱动首轮把包装对象误传给 `pointerClick`，前两组通过后在点击前报错；改为传入 CDP client，并明确等待主题预览和使用合法长刷新间隔后，两版最终运行均通过。两者未改产品行为或放宽断言。

## Review 与结果

- 逐项对照旧代码与运行时回执顺序：启动确认独立、显式/自动准入、完成时页面归属、存储 revision、失败提示及 effect 订阅；没有待修的产品问题。
- 策略只持有规则状态与错误载荷，没有第二份 UserSettings；事件接收和主题应用在一次同步完成段内，随后才补读，App 任务作用域保持原样。
- 本批的已通过检查支持该范围的等价提取，不推断性能收益。新分支的远端 CI 尚未运行；PR #46 的 CI 只证明已合并的返回策略候选。
- `git push -u origin codex/shell-settings-lifecycle` 已成功，upstream 已建立；回执更新仍只推送同一候选分支。当前 4 个本地分支、2 个工作树、3 个真实远端分支（main、返回策略旧引用、新候选），未继续删除引用。main 保持 `6a75fb83`，当前工作树提交后干净。

## 风险与后续事项

前置模块化可以在此收口，下一步先评估**设置页的主题模式选择**作为首个最小原生控件候选：只有 System/Light/Dark 三个值，现有 Rust 草稿、预览、逐字段保存与回填可以直接复用，输入/程序回填/取消的边界小于目录或整页替换。

这仍是候选评估，未确定宿主控件嵌入方案或框架，未接入原生控件。落地前需核对 Windows/Android 的焦点、键盘、关闭/取消、程序回填不回送输入及真实宿主验收方式；选择后一次只接一类控件，不以此继续扩大 Shell 或数据库重构。原生文件选择器已存在，不作为本次新增成果。

## 给下一位 Agent 的备注

- 从本记录、[前置模块说明](../design/native-ui-preparation.md)、[适配契约](../design/native-control-adapter-contracts.md)继续；工作树目录名称保持不变，实际分支已改为 `codex/shell-settings-lifecycle`。
- 已合并的 PR #40、#46 均不追加本批实现。主 checkout 既有改动、历史备份、旧 C 盘目录不动；不打 tag、不发布。
- 本批 Android 是编译检查。上一批真实手机的返回专项证据仍绑定其原候选，不能写成新生命周期的真机通过。
