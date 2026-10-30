# Android 连续阅读返回修复

- 日期：2026-10-25
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 修复基线 HEAD：`df750c5d`
- 相关 commit：修复与本记录同次提交（提交前 commit: pending；最终提交见本文件 Git 历史）
- 相关 tag / release：N/A
- 状态：`validated`（本修复范围）

## 工作摘要与影响范围

在原生宿主性能基线中发现并复现 Android 第二次阅读返回失效。旧基线 `730d29e3` 与当前 `df750c5d` 均受影响；停止把失败轮次纳入性能统计，先修复当前分支。只调整前端 Rust 的 Android 返回并发标记，不改变路由、正文位置策略、存储格式、Windows/Web 导航或业务层职责。

## 根因与关键变更

- 实机触摸轨迹证实第二次按钮收到 trusted pointer/touch/click 事件，仍停在阅读页。再次点击或经首页重新打开文章不能恢复，冷启动可恢复。
- 临时诊断构建记录：点击申请在 `ThreadId(15)`，完成释放在 `ThreadId(2)`。原 `thread_local Cell<bool>` 清除了另一个线程的副本，因此点击线程仍看到 busy=true。
- 改为进程共享 `AtomicBool` gate；取得独占 guard 后开始异步位置采集，完成、超时、路由变化或任务取消均由 Drop 释放。容量、500ms 采集上限、路由校验和 overlay 优先规则保持原样。
- 添加跨线程连续申请/释放，以及未轮询任务取消的回归测试。诊断日志仅存在临时构建，最终代码已移除。
- 其余阅读位置 `POSITIONS` 的保存/恢复仍由 Dioxus 任务与渲染入口访问；没有为此引入新的全局业务状态或 crate。

## 验证与验收

- `cargo test -p rssr-app --locked use_mobile_back_navigation`：4 项通过（2 项新增 gate 测试，2 项已有 Android bridge 测试）。
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`、`cargo fmt --all --check`、`cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `dx build --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false`：通过；JDK 21，现有 dx 0.7.10 / Dioxus 0.7.9 版本提示未消除。
- 测试 APK：`target/reading-baseline-20261025/b/rssr-android-back-fixed.apk`，SHA256 `BB757E310E773E038A1F47D8C881813309138C31CDBCEBD130CAC7A453811843`。同包名同签名覆盖安装成功，不是正式签名发布包。
- 小米 12 / Android 15 实机：连续 5 次按钮触摸返回成功；每次下滚 1000 CSS px，重新进入精确恢复到 1000 / 2000 / 3000 / 4000 px。紧接着 3 次系统返回均成功，未重启应用。记录：`android-fixed-back-qa.json`。
- 失败轨迹与临时诊断日志保留在 ignored 的 `target/reading-baseline-20261025/android-*-back-*-diagnostic.json`、`android-back-thread-probe.log`。

## 当前状态、风险与后续

- 尚未将性能基线等同于发布验收；主线集成按用户选择另行处理。
- 基准旧版保持原始代码，不能为了跑通循环而替它修复。Android A/B 公平对照改为每个冷进程只测一次阅读往返；修复后的连续往返另外验收并记录。
- 覆盖安装不卸载、不清数据；性能采样结束后由基线交接记录最终双库和偏好复核。

## 给下一位 Agent 的备注

- 入口：`hooks/use_mobile_back_navigation/back_capture.rs`，不要重新改成 thread_local 防重入标记。共享标记仅保存忙碌与否，不存储页面/文章数据；Android 当前是单 Activity / 单阅读窗口。
- 独立 CI `40216754357` 的 24 项成功对应修复前 `df750c5d`；本修复以以上本地检查与精确 APK 实机结果为证，不借用旧 CI 身份。
- 与 v0.2 主线的冲突集成和 SQL 迁移换行兼容问题见同日基线交接，均未在本修复中处理。
