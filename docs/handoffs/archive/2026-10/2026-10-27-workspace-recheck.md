# 工作区复测：恢复读取、离页写入与目录截图修复

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`codex/review-workspace-20261027`
- 复测基线 HEAD：`2b2c9888`
- 相关 commit：`66d4a1f5`（上一轮写入生命周期）、`8de67b8d`（职责拆分）、`6afed8f9`（本轮截图修复）、`95d75723`（本轮功能修复）；本记录单独提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按用户要求复测前两轮文章页保存、切页与会话重构，扩展到整个 Rust 工作区、桌面和手机尺寸的浏览器交互及正式 Web 构建。结合 Claude 的只读复核，独立复现并修复偏好恢复覆盖筛选、补充读取丢失设置、阅读标记与打开订阅记录在离页时被取消的问题；另修复目录回归脚本的空白手机截图。

## 影响范围

- 模块：Entries session 的读取／写入边界、Reader session 的显式标记写入、测试 Backend、浏览器回归和目录截图入口。
- 平台：Windows 本机验证；Chrome 驱动正式 Web 包及隔离的 `rssr-web` 测试资料库。
- 额外影响：前端命令契约、会话维护说明和交接文档；不修改存储格式、Backend 公共接口、依赖或 workflow。

## 关键变更

### 恢复读取使用完整的只读刷新

- 单篇已读和批量操作完成后使用 `reread()`。之前的 `bootstrap()` 会在偏好尚未恢复时直接覆盖筛选，已恢复时又只查询订阅，顶替正在进行的完整设置读取。
- 复用 reducer 已有的 `recovered_edits` 规则，保留用户在偏好不可用期间选择的筛选；恢复本身不保存它们。完整读取也保留了被其替换的设置刷新语义。
- 新增单篇／批量各两项原生回归。修复前均失败：筛选从 UnreadOnly 变回 All；远端每页 13 条未生效，仍是 100 条。真实浏览器也复现“仅未读”被清除。

### 显式写入在离页后完成

- Reader 的已读／收藏写入改由 App 持有；回执仍检查原 `entry_id + load_generation`，原访问结束则使用全局提示并刷新当前页面。关闭 App 仍结束等待，未知结果不重试。正文读取及自动图片本地化继续由页面持有。
- “记住打开的订阅”移入 Entries 的 `workspace.rs`，同样由 App 持有，快速返回不取消已接受的记录操作。删除因此无调用者的通用页面任务包装器，保留仍被 Settings 使用的 intent 投影助手。
- 新增两个 Reader 原生测试及一个打开订阅测试，覆盖离页、重新挂载、未知结果、单次调用及关闭 App。正确使用页面 scope 的 Reader 测试在修复前明确记录 `abandoned=1`；浏览器旧包的标记请求也在离页后收不到成功回执。

### 目录视觉证据使用实际视口

- 原脚本用 CDP 设置视口和设备像素比，却用保留默认视口的 Playwright `page.screenshot()` 取图。
- 360 × 800、DPR 3 的两张手机截图实际成为 1280 × 720 的空白图（各 4,259 字节）；同轮 150 条 DOM／交互断言通过，不能据此声称视觉验收通过。
- 改用现有 `server_page.mjs` 的 `capturePage`，通过同一 CDP session 截图，并保留同名 HTML 方便核对。没有新增一套截图工具。
- 修复后时间／来源目录手机截图均为 1080 × 2400，已直接查看，目录与正文正常呈现；桌面截图也已查看。目录 150 条行为断言重跑通过。

### 回归入口说明

- 修正生命周期脚本中过时的“no server”注释；实际使用正式 Web 包、同源服务器和隔离测试资料库。

## 验证与验收

证据目录：`E:/gitclone/RSS-Reader/target/recheck-workspace-20261027/`。

### 自动化验证

- 最终 `cargo test --workspace --locked`：退出 0；62 组结果，980 passed、0 failed、4 ignored，见 `workspace-tests-final.log`。跳过项为已有的投影性能测量、ticket 性能测量、需外部语料的正文校准和正文压力计时。
- `cargo test -p rssr-app --locked`：285 passed、0 failed、1 ignored，另有 4 项样式契约通过；新增的 7 项原生回归全部通过，见 `app-after-fix.log`。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过，见 `clippy-final.log`。
- `cargo fmt --all --check`、`git diff --check`：通过。
- DX 0.7.9：`dx build --platform web --package rssr-app --locked --release --debug-symbols false --verbose`：功能修复后完整通过（128.36 s）；Binaryen 129 实际执行 `wasm-opt -Oz --strip-debug` 并完成优化，见 `web-release-final.log`。
- `node --test scripts/browser/api_traffic.test.mjs scripts/browser/server_acceptance.test.mjs`：10 passed、0 failed。
- `node scripts/browser/rssr_entries_lifecycle_acceptance.cjs`：新包上的 8 个场景通过，包含 1536 px／390 px 的搜索、刷新与返回；页面异常和非预期 console error 均为 0，见 `lifecycle-final.log`。
- `node scripts/browser/rssr_workspace_acceptance.cjs`：新包上的 17 个交互场景及无页面异常断言，共 18 项通过，见 `workspace-browser-final.log`。新增场景分别覆盖单篇／批量的偏好恢复、Reader 已读／收藏在离页后完成。
- `node scripts/browser/rssr_directory_offline_regression.cjs`：最终新包上 150 条行为断言通过、页面异常 0，见 `directory-final.log`。旧空白截图只保留为失败证据。
- 三个改动脚本的 `node --check`：通过。
- 首次生命周期脚本因漏传 `CARGO_TARGET_DIR` 无法找到 fixture binary（ENOENT）；补齐环境后重跑通过。它是启动配置问题，没有计入产品失败。
- 加入回归测试后：`app-before-fix.log` 记录 279 passed、6 failed、1 ignored。Reader 测试最初从 App scope 触发，改成真实页面 scope 后单独再次复现离页取消，见 `reader-before-fix.log`。
- 浏览器先失败证据：`browser-before-fix.log` 的筛选重置；`reader-browser-before-verified.log` 的 Reader 写入回执超时。早期 Reader 脚本定位底栏时错误地将其视作 article 子元素，纠正为独立底栏后才记录上述确认复现；该定位错误不算产品缺陷。
- 最终源码身份、日志与当前入口 JS／引用 WASM 的 SHA-256 记录在证据目录的 `verification-receipt.json`。

构建使用 `CARGO_BUILD_JOBS=1` 和已有忽略目录 `E:/gitclone/RSS-Reader/target/pr-integration-20261027/build`。`RSSR_BUILD_ID=pr32-integration-20261027` 仅用于匹配隔离服务器／fixture 的兼容标识，源码身份仍以本记录 Git 提交为准。

### 人工核对与交叉复核

- 已直接查看旧空白截图与修复后的时间／来源目录手机截图、桌面截图；最终新包的这三张图片与已查看版本 SHA-256 完全一致，两张手机图均为 1080 × 2400，桌面图为 1280 × 800。
- 按用户许可调用本机 Claude Code，只开放 Read／Grep／Glob、禁用修改及外部工具。调用在约 15 分 49 秒后自行完成，未被终止；session `2725973b-e4e5-4f55-a750-4903d1bf2153`，原始回复为 `claude-review.json`。其 F1/F2 及 S1/S2 中的显式标记／打开订阅路径经独立复现后修复；没有把静态意见直接当作测试结论。
- Claude 提出的批量 generation 忙碌状态隐患当前不可达，没有修改现有规则；自动图片本地化是随阅读访问执行的后台工作，维持页面生命周期。
- 未执行 Android 实机、原生桌面 GUI 全流程或本分支远端 CI；本地测试不替代这些验收。

## 结果

本轮确认的问题已修复，最终测试与 Web release 构建通过；截图与功能修复分别本地提交，工作记录单独提交。没有推送或触发远端 CI。

## 风险与后续事项

- 保存排序仍只协调当前 App 实例，不提供跨浏览器的操作序列保证。
- 本轮不宣称性能收益或所有平台完成实机验收。
- 之前的目录手机截图为空白，后续交接使用 `directory-final/` 的最终新包证据；`directory-fixed/` 只记录截图修复阶段。

## 给下一位 Agent 的备注

- 会话边界先读 `crates/rssr-app/src/pages/entries_page/session/README.md`。
- CDP 设置视口时也用 CDP 取截图；不要把 Playwright 默认视口的截图当成当前移动视口。
- 工作目录为本 managed worktree；`E:/gitclone/RSS-Reader` 主检出的任务外改动保持原样，仅复用了其忽略的构建／证据目录。
