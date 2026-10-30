# 返回策略接续复核与宿主验收阻塞

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/native-back-policy`
- 本轮核对 HEAD：`7a0403e0778ca590458a6347a4372bdb414961ea`
- 产品实现：`62113c4fc181e245bc077dcd46eda56a969d7b96`
- 最新远端 main：`4101520c33266026747db867c33e2b13baea3d23`
- 相关 commit：本记录提交前 `commit: pending`；提交后以本文件 Git 历史定位
- 相关 tag / release：无
- 状态：`draft`（代码复核及本轮 17 项 Rust 测试通过；实际宿主验收仍被执行审批阻止）

## 工作摘要

在用户指定的 `E:/gitclone/RSS-Reader-worktrees/native-back-policy` 接续[返回策略提取](./2026-10-29-native-back-policy.md)。工作树起始干净，HEAD 与远端候选一致；最新 main 未变化。本轮先补实际宿主验收的准备与代码复核，没有扩大到 Shell 或原生控件替换。

## 影响范围与关键变更

本轮只补文档与[复核回执](../testing/evidence/2026-10-30-native-back-validation/result.json)，没有修改产品源码、依赖、构建配置或验收运行器。对照 `62113c4f`，当前 `crates/`、`assets/`、`Cargo.toml`、`Cargo.lock` 没有新增差异。

代码自审未发现本次提取引入的可操作问题，覆盖以下边界；实际 JNI、WebView2、Android Activity 运行未覆盖，也没有独立 agent review：

- 工具栏仍同步选择 history、不同 fallback 或停留；Android 导航采集完成后仍核对路由并重新解析目标。
- 页面继续拥有浮层与门禁：Entries 来源筛选先于目录，Reader 先关闭图片查看器，Feeds 保存中消费返回且不关闭。页面、阅读位置采集及 Android 桥接源码相对 main 未改动。
- 首页提示由私有 Rust 策略持有，signal 仅投影；5 秒提示、旧超时不能清除新提示、恢复前台清除提示均有事件序列测试。
- 跨线程原子 guard、输入合并、监听代次和取消释放保留。真实采集函数仍设置 500 ms 上限；单元测试不构成设备上的等待时间测量。

## 验证与验收

| 本轮执行 | 结果 |
| --- | --- |
| `cargo test -p rssr-app --locked use_mobile_back_navigation` | 16 passed，0 failed，0 ignored；包括策略、Dioxus 提示投影、捕获及桥接测试 |
| `cargo test -p rssr-app --locked dismiss_edit_consumes_back_without_leaving_a_save_in_flight` | 1 passed，0 failed；来源编辑保存门禁 |
| `git diff --exit-code 4101520c HEAD -- crates/rssr-app/src/pages crates/rssr-app/src/ui/reading_position.rs crates/rssr-app/src/hooks/use_mobile_back_navigation/android_bridge.rs` | 通过；所列适配职责保持主线实现 |
| `Get-FileHash -Algorithm SHA256` 对照原 `identity.json` | 7 个已有文件及 3 个产品源码全部匹配；包含基线/候选 EXE、APK 和待验证运行器 |
| `aapt dump badging` | 两个 APK 均为可调试的独立包 `io.github.develata.rssreader.backpolicytest`，入口 `dev.dioxus.main.MainActivity` |
| `adb -s caa0c283` 设备与包查询 | 真机 `2201123C` / cupid 在线；独立测试包未安装 |
| `target/branch-consolidation-20261030/check-documentation.ps1`、新增 JSON 解析及 `git diff --check` | 通过；322 份 Markdown、1474 个本地链接、15 个锚点，201 份归档保持不变 |
| 真实 Windows / Android 操作 | 未执行，原因如下 |

### 本轮自动审批拒绝

尝试的命令为：

```powershell
python scripts/seed_reading_native.py target/back-policy-native-20261030/seed --fixture-dir target/back-policy-native-20261030/fixture --fixture-base http://127.0.0.1:9299 --entries 100 --paragraphs 120
```

命令准备在当前工作树下创建新的隔离测试资料库和本机 RSS 夹具，自动审批返回 `approval required by policy, but AskForApproval is set to Never`，没有更具体原因。命令未启动，随后 `Test-Path target/back-policy-native-20261030` 为 `False`。本轮没有重试被拒绝的动作，也没有改用其他入口绕过；没有启动原生 EXE、安装 APK、创建端口转发或修改设备资料库。

## 当前状态、风险与后续

- 返回策略仍是“实现及 Rust/Web 历史检查完成，实际原生宿主验收待完成”。本轮新增 17 项 Rust 通过回执不能替代宿主验收；已有 EXE/APK 的哈希匹配也不等于运行通过。
- 后续须在允许上述隔离夹具初始化的执行环境中，先复核并运行 `target/back-policy-20261029/rssr_back_navigation_acceptance.cjs` 的基线，再用相同夹具验候选。该脚本依旧仅经语法检查，不能当作已验证运行器推广。
- 待验内容保持原批范围：真实 Android 系统返回、首页两次返回与超时、旧计时器/恢复前台、浮层优先级、保存门禁、连续阅读与位置恢复、捕获失败/超时/取消后的导航，以及 Windows WebView2 路径。
- 本轮未重跑完整 workspace、Clippy、跨平台构建、正式 Web、CI 或发布验收；没有新产品代码需要扩大回归范围。合并、发布与后续模块提取仍按各自授权和验收推进。
- 保留两个工作树与三个本地分支。主 checkout 仍在 `020f9d40`；其 40 条既有状态、index 哈希、20 个现存文件哈希和 22 个缺失路径均与清理前快照一致。

## 给下一位 Agent 的备注

继续使用本 E 盘工作树，先读本记录和[原实现交接](./2026-10-29-native-back-policy.md)。不要将执行环境拒绝解释为缺设备或产品缺陷；也不要为绕开宿主验收而扩大本批重构。没有新的运行缺陷复现，保持现有等价提取。
