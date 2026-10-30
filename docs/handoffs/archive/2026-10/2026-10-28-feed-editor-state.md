# 来源编辑状态提取与保存中原生取消修复

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 当前 HEAD（实施基线）：`ee5582c12351d90d5fc7e0f01098ad9125005993`
- 相关 commit：与本记录同批本地提交；记录生成时 `commit: pending`。提交后用 `git log -1 -- docs/handoffs/2026-10-28-feed-editor-state.md` 定位。
- 相关 tag / release：N/A；未 push、打 tag 或发布。
- 状态：`validated`

## 工作摘要

在 Feeds 查询与刷新提示提取之后，将来源编辑的加载、草稿、保存与关闭规则收拢为独立 Rust 状态模块。建立基线时发现保存中 Esc 绕过关闭 gate 的既有问题，同批以原生 cancel 事件接回原 Rust 入口修复，符合既有前端契约。

## 影响范围

- Rust：`feeds_page/edit_state.rs`、state/reducer/session/facade、编辑组件及装配；共用 `components/dialog.rs` 增加原生取消处理。
- 平台：共用代码影响 Web、Windows、Android、Linux、macOS；实跑 Web 与隔离 Windows WebView2，Android 为编译覆盖。
- 测试/文档：Feeds 验收运行器增加 `--scenario edit`，新增编辑场景模块；更新模块边界、页面职责与命令参考。
- 无新依赖、crate、schema、业务用例、宿主 capability 或 workflow 变更。

## 关键变更

### 唯一编辑状态与适配职责

- 原 `edit_feed_id / edit_generation / edit_details / edit_busy / edit_error` 五字段迁入 `FeedEditState`，原位置不保留副本；嵌入原 `FeedsPageState`，不新增 signal 或长期草稿。
- 打开、关闭和保存准入，加载结果的 ID/代次/阶段校验，草稿修改与保存失败重试均由纯 Rust 模块处理。保存成功只返回页面应发布成功反馈的事实；命令、确认状态、reload 与页面消息仍在 reducer。
- 保留原值与边界：代次环绕；只有精确空标题成为 None；URL/标题校验仍在 application；保存只复制请求字段；成功不再分配代次；保存结果仍仅按 ID/代次准入，不额外加 busy 条件。
- facade / dialog 复用统一的 `is_saving` 判定。session 仍先同步转移状态，再构造命令 Future；Dioxus 任务作用域、快照查询与 Shell 刷新职责不变。
- 新增 8 个纯状态测试、2 个页面 reducer 测试、1 个 Dioxus session 测试；原编辑测试改为通过状态方法准备场景。覆盖旧成功/失败、同来源重开、重复结果、保存中关闭/编辑/重开/重复提交、原始草稿值、失败重试、环绕、成功仅重载一次、命令构造前 gate 已发布。

### 基线发现的行为缺陷及最小修复

- 改前 Web 运行时，保存按钮禁用让焦点移至 BODY；随后 Esc 的 keydown 目标为 BODY，未经过 dialog 的键盘处理，浏览器发出的 cancel 未阻止。原生弹窗 `open=false`，但 DOM 仍挂载且保存按钮仍 disabled，保存任务尚未完成。
- 证据：`baseline-web/acceptance.json`、`baseline-probe.log`。这是改前缺陷，不把这条基线记为通过，也不把修复称为所有行为完全未变。
- 共用 Dialog 增加 typed `oncancel`：阻止默认关闭后调用原 `on_close`，由 Rust gate 决定是否能关闭；不新增 JS 状态或键盘映射。原组词 Escape 的键盘阻止逻辑保留。
- 候选 Web 用真实 Web Lock 暂停写入，令焦点离开控件后按真实 Esc，确认弹窗仍打开且 gate 仍在；释放后成功保存。重复 submit、失败重试和重开回填同时验证。

## 验证与验收

证据目录：`target/feed-edit-20261028/`，产物/数据库/profile 均为忽略文件，未提交。

### 自动化验证

- `cargo fmt --all --check`：通过，`fmt.log`。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过，`clippy.log`。
- `cargo test -p rssr-app --locked feeds_page`：42 passed / 0 failed，`feeds-tests.log`。
- `cargo test --workspace --locked`：537 passed / 0 failed / 2 ignored，`workspace-tests.log`。忽略项为原有手动性能探针。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过，`wasm-check.log`。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过，`android-check.log`，NDK 28.2.13676358 / API 24 clang 与 llvm-ar。
- 两个变更的 CJS 文件 `node --check`、本批范围 `git diff --check`：通过。
- `node scripts/browser/rssr_feeds_lifecycle_startup_test.cjs target/feed-edit-20261028/startup`：3/3 通过，启动失败报告与资源回收未回退。

### 构建与运行

- 基线复用 `2b551537` 已验证产物；`2b551537..ee5582c1` 仅有脚本/文档变化。重新核对源码状态、Web 引用链与 EXE 哈希，`baseline-identity.json`。
- Web 构建：`target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false`；Windows：`cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`。两者成功，`web-build.log` / `native-build.log`。
- Web 设置 `NODE_PATH=C:/Users/QQ/node_modules`、`PUBLIC_DIR=<该版本 public>`，运行 `rssr_feeds_lifecycle_acceptance.cjs --scenario edit --artifact-dir <目录>`。候选 `candidate-web/acceptance.json` 两视口 2/2 通过。改前完整 Web 流程在已记录的保存中 Esc 缺陷处失败；正常窗口的同路径视觉另行记录，未伪称该基线全通过。
- Windows 通过 `run-native.ps1 -Variant baseline-complete|candidate` 启动独立复制 EXE/profile/5000 篇复制 fixture，按哈希确认目标；调用同运行器与 `--scenario edit --cdp ... --database ... --feed-origin http://127.0.0.1:9277`。基线与候选各 2/2 通过。第一次基线的导入确认断言读得过早，改为等待具体提示后重新跑完整基线，失败日志保留在 `native-baseline/`。
- 场景：编辑取消不落库、重复 URL 保存失败保留草稿并允许重试、名称/URL 保存及重开、使用原名、清理删除/导入确认。最终来源 ID/URL/远端名称/自定义名称恢复原值；全部文章标记与数量保持（Web 72、Windows 5000）。不声称更改 URL 的刷新 generation 或缓存验证器也未变化。
- 候选默认 Feeds 生命周期流程 `candidate-lifecycle/acceptance.json` 两视口 2/2 通过，覆盖刷新提示、busy 与新增/删除后的快照，复用原验收步骤。
- 复用 `rssr_workspace_acceptance.cjs` 的 `dialogComposition`，基线与候选均通过来源编辑和来源筛选的真实 Chromium composition + Esc 检查；`baseline-extra/acceptance.json`、`candidate-extra/acceptance.json`，由本地 `extra-web.cjs` 调用。
- 正常运行的 pageErrors / consoleErrors 均为空。Web Locks 延迟保存与失焦原生 cancel 的精确场景只在 Web 跑；Windows 正常编辑流程不作这条精确调度的实机证明。

### 视觉和身份

- 两视口均为 1536×844 / 390×844，检查初始编辑弹窗实际图片与尺寸；基线、候选专项及候选编辑验收截图逐字节一致。宽屏 SHA-256 `4ABDB89D1C40276D711F2B02E6BB36C5FF8CE7E4A419338BF5F82BD977262583`；窄屏 `C95BA08AF9AE4EBEF1E95E059E5E6B0C7EB6C7BAB7C79BA29F8CC1F53E204309`，见 `visual-equivalence.json`。
- 基线 WASM `9905a36017e8fd0c64c762368a4cb3330d3096d7aa3bbd3a3cc28b32abf16b50`；候选 `d53771b051df8097e8abbd6ad169dca532f029fd46d2ed02900658f210236b34`，实际引用链 `candidate/web-identity.json`。
- 基线 EXE `8AD0653A863ED11FF9625D23962D6CDE255EB84E045C85B6FF4B88A72ED92A54`；候选 `5BB314912F694B0ADDFDD822681437C4E9B41F2CBA8BFDF7F40BE35588AB1CDE`，运行副本见各 `native-*/identity.json`。
- 最终源码与场景脚本 SHA-256 清单 `validation-identity.json`。默认运行器仍支持原场景名与参数，新增 edit 分支。

### 手工与未执行范围

- 使用真实无头 Chrome、隔离 Windows WebView2 的 DOM / CDP 和只读 SQLite 核对；不是人工逐项点击。隐藏原生窗口截图不作为前台视觉证据。
- 未运行 Android 设备系统返回、硬件输入法、Windows 前台视觉、Linux/macOS 原生 UI 或托管 `rssr-web` 服务端模式。Chromium CDP composition 不能代替硬件输入法验收。
- 迟到回复与加载中关闭由纯状态 / reducer / Dioxus 测试覆盖，未在设备上人为控制这些时序；不声明性能收益。

## 结果

- 编辑状态边界、测试、文档与交接完成；额外修复了基线实测发现的保存中原生 cancel 缺陷。
- 可按本批范围本地提交，提交身份见开头定位命令。

## 风险与后续事项

- 不把保存结果的准入条件改成另一套阶段规则；如要改变 busy / 草稿回填语义，需作为后续行为变更单独设计。
- 共用 Dialog 的 native cancel 必须继续经 Rust 关闭 gate；不能只检查键盘事件，也不能在 JS 镜像保存状态。
- 下一阶段可先做一次页面边界收口，盘点 Settings 加载/同步与主题文件交互中的剩余耦合，再决定下一批；避免为统一接口而不断细拆已经独立的纯规则。

## 给下一位 Agent 的备注

- 从 `edit_state.rs` 与 reducer 的 Edit intent 分支进入；检查 `components/dialog.rs` 的原生 cancel 与键盘 composition 两条路径。
- 保留本批最初的 `initial-status.txt`；任务外 `.agents/skills` / `.specify` 删除、两份 10/03 handoff 和未跟踪文档均不纳入提交。
- 未访问用户数据库或外部订阅；仅本地提交，无 push、合并、tag 或发布。
