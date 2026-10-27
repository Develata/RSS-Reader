# 阅读工作区复查：切页期间的偏好基线与文章写入

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`codex/review-workspace-20261027`
- 基线 HEAD：`0115eb9a6b1b888d1040765e3ba022160a4c598d`（PR #32 合并）
- 验证源码 HEAD / 修复 commit：`66d4a1f59576a6934d72fa3ea480a9d8155843c5`
- 交接记录：另行提交本文件，源码验证身份以上述修复 commit 为准。
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

对刚合并的阅读工作区做代码复查并修复两处切页竞态。GitHub PR #32 无新增 review；
基线 main 的 [CI](https://github.com/Develata/RSS-Reader/actions/runs/40559125142)、
[Pages](https://github.com/Develata/RSS-Reader/actions/runs/40559125180)、
[Docker](https://github.com/Develata/RSS-Reader/actions/runs/40559125208) 均成功。
这些远端结果只覆盖基线，不代表本次尚未推送的修复。

## 影响范围

- 模块：`rssr-app` 文章页面 session、批量回执文案、测试 Backend、AppShell 顺序边界说明。
- 平台：共享 Rust UI（Web、Windows / Linux / macOS、Android）；本轮运行了 Windows 主机单测和 Chrome Web 自动化。
- 额外影响：阅读工作区浏览器回归、前端行为契约。本轮不修改 schema、Backend 公共接口或工作流。

## 复查问题与关键变更

### P2：返回文章页可能以来源编辑前的地址保存偏好

来源编辑与偏好写入此前共享锁，但新页面 bootstrap 不等待此前写入。
来源 URL 修改尚未完成时返回文章页，可以读取旧选择，再把包含旧 URL 的筛选快照排在编辑后保存。
新页面现在先经过相同写入边界再读取偏好；读取期间释放锁，仍通过请求代次丢弃迟到结果。

### P2：切页取消已经接受的单篇／批量文章操作

单篇已读、收藏及批量已读／归档此前使用页面作用域任务，卸载页面会丢弃正在等待的 Backend 调用。
现在这些写入统一由 App 持有；当前页面仍在时回到其作用域应用回执，后续只读查询继续属于页面。
原页面已卸载时，全局提示报告成功、失败或结果未知，并重新读取当前页面；不自动重试未知写入。
关闭整个 App 仍结束客户端等待。批量回执文案共用一处函数，避免页面与全局提示分叉。

## 验证与验收

证据目录：`E:/gitclone/RSS-Reader/target/review-workspace-20261027/`。
复用先前独立构建缓存与服务器 fixture；`RSSR_BUILD_ID=pr32-integration-20261027` 仅用于这套本地
前后端匹配，不能单独作为本次源码身份。执行目录为本分支 worktree；`CARGO_BUILD_JOBS=1`。

### 自动化验证

- 修复前加入确定性延迟／卸载测试：23 通过、2 失败，分别验证提前读取与写入被取消，见 `tests-before-fix.log`。
- `cargo test -p rssr-app --locked`：278 passed、1 ignored（既有手工性能测量）；两组样式契约各 2 passed，见 `tests-after-fix.log`。
  新增四项生命周期测试，覆盖新页面读取顺序、离页后未知结果不重试、补充查询随原页销毁、关闭 App 结束等待。
- 旧 Web 构建的新增切页写入用例确实失败（等待写入响应超时），见 `browser-before.log`。
  同一轮来源返回用例通过；该项修复前反例由上述确定性 Rust 测试证明，不能把浏览器用例说成复现成功。
- `node --check scripts/browser/rssr_workspace_acceptance.cjs`：通过。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`：通过，见 `clippy.log`。
- DX 0.7.9：`dx build --platform web --package rssr-app --locked --release --debug-symbols false --verbose`：
  完整通过（127.41 s）；实际执行 Binaryen 129 `wasm-opt -Oz ... --strip-debug`，优化完成后生成 Web bundle，见 `web-release.log`。
- `node scripts/browser/rssr_workspace_acceptance.cjs`：16 组全部通过、页面异常 0，见 `browser-final.log`。
  新用例覆盖来源保存中浏览器返回、单篇已读／收藏、批量已读／归档写入中切页，最终读取真实服务器状态。
  初轮新归档断言误用 69 篇，夹具实际初始 72 篇；改为检查操作前总量恰好减少 1 篇，整套重跑通过。
- Chrome CDP 注入 150 ms 网络延迟后，`preferencesSurviveNavigation`、`reopeningDuringSourceEdit`、
  `articleWritesSurviveNavigation` 均通过、页面异常 0，见 `browser-latency.log`。
- `git diff --check`：通过。构建入口 JS／引用 WASM 哈希与源码身份保存在证据目录 `verification-receipt.json`。

### 手工验收

- 未执行 Android 实机或桌面应用全流程手工验收。自动化浏览器及共享 Rust 测试不替代实机验收。

## 结果

两处复查问题已修复并通过本轮验证；修复已本地提交，尚未推送或触发本分支远端 CI。
本记录随独立文档提交交付；当前工作分支为 `codex/review-workspace-20261027`。

## 风险与后续事项

- App 内顺序边界不提供跨浏览器、跨设备的全局写入锁；原有服务器语义保持不变。
- 关闭应用不能保证已经发送的服务器写入被撤销；结果未知按现有契约重新读取，不自动重复提交。
- 本轮没有重跑全部七个 crate 的完整测试，改动集中于 UI session 与其测试；全 workspace 的 Clippy 检查已通过。

## 给下一位 Agent 的备注

- 入口：`pages/entries_page/session.rs` 的 `spawn_bootstrap` 与 `spawn_article_mutation`。
- 回归入口：`scripts/browser/rssr_workspace_acceptance.cjs` 的 `reopeningDuringSourceEdit` 与 `articleWritesSurviveNavigation`。
- 工作目录：`C:/Users/QQ/.codex/worktrees/integrate-reading-workspace/RSS-Reader`；保留原主检出中的任务外修改。
- 不要将基线 main 的远端 CI 成功作为此修复分支的 CI 结果。
