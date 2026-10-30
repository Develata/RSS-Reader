# 原生界面前置整理：目录控制器与阅读快捷键

- 日期：2026-10-26
- 作者 / Agent：Codex；Claude CLI 只读评审
- 分支：`feat/reading-workspace-20261024`
- 实施基线 HEAD：`14c8fea4`；期间其他任务提交了纯文档 `822bcbc7`，本次不修改其内容。
- 相关 commit：与本记录同批提交，记录生成时 `pending`；通过 `git log -1 -- docs/handoffs/2026-10-26-native-ui-preparation.md` 定位。
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

用户要求继续优化，并推进未来原生 Rust 界面之前的模块化。源码盘点确认阅读位置、下拉刷新、来源草稿已有纯 Rust 控制器；本轮补齐目录重置和回复决策的独立测试边界，并抽出阅读快捷键策略。沿用当前视觉和交互，不选新渲染框架、不新增 crate、不迁入 v0.2 主线。

## 影响范围

- `pages/entries_page/directory.rs` / `directory/controller.rs` / `directory/view.rs`。
- `hooks.rs`、`hooks/use_reader_shortcuts.rs`、`hooks/reader_shortcut_policy.rs`。
- 阅读位置浏览器验收脚本、前端命令文档、原生前置边界说明。
- 共享前端影响 Web / Windows / Android / Linux / macOS；本轮运行验证为 Windows Chrome，另做 Windows 主机测试和 Web / Android 编译检查。
- 无存储、迁移、配置格式、业务命令、CSS、依赖或 workflow 变化。目录及阅读位置 JS 适配器保持原样。

## 关键变更

### 目录控制器

- 迁出路径、状态、事实与命令。`reset` 集中代次重置；`receive` 一次校验 epoch/sequence，统一决定状态更新、导航页、对齐、手动滚动恢复和回复。
- 过期事实仍收到带当前 epoch、原请求 sequence 的确认，不产生副作用，避免单条在途桥接等待不结束。
- 重复观测仅更新序号，不发布渲染状态；必须对齐但无状态变化时仍能发出对齐命令。可见性重复报告在有效且手动浏览时仍请求恢复滚动，不额外要求 revision 改变。
- Dioxus 外层保留 presenter 身份、生命周期、epoch 同步及既有“派发页码 → 按需发布 → 回复”顺序。视图只读控制器访问器，不复制整份文章列表或触发新查询。
- 原有 8 个状态序列测试迁入控制器并经过完整 `receive` 入口；新增 6 个回复/重置/协议用例。外层仍保留 presenter 身份测试。

### 阅读快捷键

- 普通 Rust 策略接收中立键值并输出四种动作；页面适配继续执行导航和 session 命令。
- 先排除修饰键和输入法组词，再延迟读取键值，去掉 `to_string().to_lowercase()`。仍保留 Dioxus 的键值取得成本，未测量或宣称可感知的性能收益。
- 大小写 M/F、左右方向键、重复按键、默认行为和快捷键作用域保持原样。3 个新测试覆盖策略、排除时不读取键值，以及 Dioxus 值映射。
- 浏览器专项增加实际 M/F 大小写操作和修饰键/IME 排除回归。正向操作使用 Chrome 键盘输入；排除项使用明确的 DOM 键盘事件事实，避免浏览器查找框抢焦点，不等于物理输入法/软键盘实测。

### 评审与设计记录

- Claude 实际只读会话 `38a7acc7-01a8-47e6-90ec-35fbc28a48c1`：候选评审、目录复核、最终快捷键复核三次成功，无工具权限拒绝。仅开放 Read/Glob/Grep，plan 模式、严格 MCP 配置，不允许编辑或 Shell。
- 双方确认价值在于把原桥接中的重置、回复和导航决策纳入纯测试，不把文件移动称为新的 JS 迁移。保留 DOM 的同步事件、测量、焦点和滚动职责。
- 采纳 Claude 提出的过期 Visibility 测试缺口，补充手动且可见时的重复序号和旧 epoch，确认不会错误恢复滚动。最终静态复核无具体阻塞项；Claude 没有执行测试。
- 新增 `docs/design/native-ui-preparation.md`，列出已完成模块、宿主适配职责、剩余 JS 归因/测量边界和验证入口。

## 验证与验收

### 修改前基线

- `cargo test -p rssr-app --locked`：142 单元 + 4 集成通过，1 项既有忽略。
- Chrome 目录契约：144 项通过。
- 阅读位置专项：360 / 1280 × 800 通过，连续滚动与搜索编辑期间正文布局读取均为 0。

### 修改后验证

- `cargo test -p rssr-app --locked`：151 单元 + 4 集成通过，1 项既有忽略。目录专项在补强过期 Visibility 断言后另跑 15 项全部通过。
- `dx build --platform web --package rssr-app --locked`：通过；仍有既有 dx 0.7.10 / Dioxus 0.7.9 版本提示。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过。初次独立 Cargo 检查没有找到 C 编译器；随后仅为该进程设置现有 NDK 28.2.13676358 的 `CC_aarch64_linux_android`、`CXX_aarch64_linux_android`、`AR_aarch64_linux_android` 和目标 linker。编译器使用 `aarch64-linux-android24-clang.cmd`，与仓库 min_sdk=24 对齐；没有安装软件或修改全局环境。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all --check`：通过。
- Chrome 目录契约重跑：144 项通过。此后目录生产代码未变；仅补强 Rust 测试、增加独立快捷键策略。
- 增强后的阅读位置专项：360 / 1280 × 800 全部通过，包含 M/F 大小写、四类修饰键、组词、切文、浏览器返回、原生捕获事件和顶部/底部恢复；无未捕获页面错误。
- 已查看桌面来源文章目录及手机浮层截图；沿用既有布局和导航行为。
- 任务路径 `git diff --check`：通过。

使用已安装 Google Chrome 和隔离 fixture / 浏览器上下文，不读取或修改用户日常文章库。证据在忽略目录 `target/native-ui-prep-20261026/`：`before-*`、`after-directory/assertions.json`、`after-reading.log`、Rust/构建检查日志和三份 Claude JSON。

## 结果

目录重置/回复和快捷键规则已完成可独立测试的 Rust 边界，既有交互经上述验证。沿用用户授权提交并推送当前阅读分支，不合入 v0.2，不发布安装包。

## 风险与后续事项

- 本轮不是 WebView 替换，也不是新一轮性能基线；不据此宣称运行时内存或响应速度改善。
- 未重新生成、安装和操作 Windows / Android 原生包；Android 为编译检查，Linux/macOS 未运行。
- 目录未知消息类型仍沿用既有解码失败行为；协议恢复策略需单独设计和验证，本次不夹带变更。
- 后续原生输入/测量/滚动适配需按现有契约验证，尤其是滚动来源归因、真实 IME、系统返回、焦点与正文选择复制。

## 给下一位 Agent 的备注

- 明确使用 `E:\gitclone\RSS-Reader`，不要在 C 盘旧基线继续修改。
- 保留任务外 .agents/.specify 删除、既有交接修改和未跟踪设计文件；只提交上述任务文件。
- 自动审批拒绝过本地辅助抽取脚本执行，未给具体原因；未运行该脚本，随后采用明确文件补丁完成修改，辅助脚本已移除。
