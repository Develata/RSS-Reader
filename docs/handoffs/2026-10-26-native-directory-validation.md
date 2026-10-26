# 目录可靠性、时间文章层级与最新原生验收

- 日期：2026-10-26
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 起始 HEAD：`aff3c7e6`
- 相关 commit：源码候选随本记录提交，提交前为 `pending`；后续原生证据单独补记
- 相关 tag / release：N/A
- 状态：`draft`，浏览器及 Rust 验证完成，原生包与性能验收进行中

## 工作摘要

用户授权继续最新 Windows / Android 验收与目录性能定位，并要求修复前先评估架构边界。随后指出时间目录仍停留在日期，要求展示到文章。本轮先修复这两个功能缺口，再用同一源码候选构建并验收原生包，不迁入 v0.2 主线。

## 影响范围

- `rssr-app` 前端目录 transport、adapter、view、分组投影及回归脚本。
- 共享前端影响 Web、Windows / Linux / macOS、Android；当前已验 Web，原生待补。
- 无新增 crate、依赖、数据库或 application/domain 职责变化。

## 关键变更

- 根因：原 `while let Ok(fact) = eval.recv::<Fact>()` 将消息解码失败等同于通道关闭，Rust 退出而 JS 仍在等待。修改前 Chrome 注入未知 kind 后 `whenSettled` 明确超时。
- `directory/transport.rs` 独立处理 Eval 错误分类：单条 Serialization 错误回复 `{rejected:true}` 并继续；通道关闭或拒绝回复发送失败则终止。JS 在私有单飞通道中释放该次等待，保留排队意图，后续正常更新重新观测。不重试损坏载荷，不更改控制器状态规则。
- 时间目录补为月份 → 日期 → 文章，保留日期跳转；复用来源目录的文章组件、`(index,id)` 校验和全局分页。DOM 观测两种分组下的具体文章，继续只挂载展开路径。
- 目录浏览器回归对时间与来源分别验文章层级、键盘展开、同页多选保持、跨页定位与清空、历史记录和具体文章跟随。新增独立 transport fault-injection 验收，不在生产中增加测试接口。
- 现有 ignored Rust 投影测量补充 5000 条规模；这项微测量不含 I/O、WebView 渲染，不与真机耗时混为一谈。

## 验证与验收

### 已完成

- `cargo test -p rssr-app --locked`：154 unit + 4 integration 通过，1 个已有手动性能测量 ignored。
- `cargo fmt --all --check` 与 `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过。
- `dx build --platform web --package rssr-app --locked`：通过，现有 dx 0.7.10 / Dioxus 0.7.9 提示仍在。
- Chrome `rssr_small_viewport_assertions.mjs --directory-only true`：151 项通过，覆盖两种目录与桌面 / 窄窗口。
- Chrome `rssr_directory_transport_acceptance.cjs`：未知类型、错误字段类型、null、数组、缺失字段五类真实 Eval 消息注入后，损坏请求不展开分支，下一条正常请求成功；无 uncaught page errors。
- transport Rust 测试覆盖坏消息之后的正常状态更新、关闭接收端不重试、拒绝回复发送失败终止。
- 实际查看时间目录桌面截图，已显示当日文章标题；后续原生证据不借用浏览器结果。

### 证据与待验项

- 根目录：`target/native-validation-20261026/`。
- 已有：`before-directory-fault.json`、`chrome-directory/assertions.json`、`chrome-transport/transport-acceptance.json`、`app-tests.log`、`web-build.log`、`time-directory-desktop.png`。
- 手机 Xiaomi 12 / serial `caa0c283` 已连接；已停止测试包并备份原 files/cache 下 RSS-Reader 数据到 `android-before.tar`，SHA256 `F3C1993812693BA67A03AE2C9C78604D3CEFFD13717B7BFD041A77FDA0CE5B2A`。没有清除或覆盖数据。
- Windows / Android 最新包实际操作、真实输入法、大目录性能分解、原生数据前后核对待补。

## 结果、风险与后续事项

- 当前仅能确认源码、Rust 与 Chrome 层面的修复；尚不宣称原生验收通过或性能提升。
- 继续构建本源码候选，记录产物 SHA256、实际宿主与测量条件；性能证据不足时不直接引入虚拟化。
- 源码只在 E 盘当前阅读分支修改。保留无关 dirty / deleted / untracked 与其他 agent 的 Penpot 文档。

## 给下一位 Agent 的备注

- 本轮必须完成原生验收 / 性能定位，或明确记录真实阻碍与未验项后再交付；此记录的 draft 不是完成声明。
- Android 同包名同签名 `install -r`，保留数据；Windows 使用隔离测试副本和已有测试数据。不得触碰 C 盘受保护的旧基线工作区。
