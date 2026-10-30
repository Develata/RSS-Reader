# 返回策略复核与 Windows / Android 宿主验收

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/native-back-policy`
- 本轮起始 HEAD：`8477977af9fe762e037dae5e90e55805aa0ae630`；重启前复核起点为 `7a0403e0`
- 产品实现：`62113c4fc181e245bc077dcd46eda56a969d7b96`
- 最新远端 main：`4101520c33266026747db867c33e2b13baea3d23`，本日重新回读未变化
- 相关 commit：本文与验收工具在同一交付提交；提交前 `commit: pending`，提交后以本文件 Git 历史定位
- 相关 tag / release：无
- 状态：`validated`（限下述返回专项；候选尚未合入 main，不构成发布验收）

## 工作摘要

在指定的 E 盘工作树补齐返回策略的实际宿主验收。Windows 11 / WebView2 和 Android 15 真机均对照 main 基线与候选包运行，使用同一合成资料库。Windows 每版本两视口共 4 组检查、Android 每版本 6 组检查及 4 个采集子结果通过；Windows 13 组、Android 9 组截图逐像素一致。没有修改产品代码，也没有复现本批提取引入的行为变化。

结构化结果和原始报告摘要见[验收回执](../testing/evidence/2026-10-30-native-back-validation/result.json)。原始截图、日志、运行资料库及诊断失败保留在本地 `target/back-policy-native-20261030/`。原实现与先前构建/Web 检查见[10 月 29 日记录](./2026-10-29-native-back-policy.md)，历史结果不计作本次新测试。

## 影响范围与关键变更

- 新增 [CDP 返回验收运行器](../../scripts/browser/rssr_back_navigation_acceptance.cjs)，连接真实 Windows / Android 宿主，先验证进程、包、CDP 和 fixture 身份。Windows 验同步工具栏、浮层、连续阅读及位置恢复；Android 另用实际系统返回和 Activity 前后台切换。
- 新增 [Android 保存门禁夹具](../../scripts/native_back_save_lock.rs)。独立 Rust 小工具链接现有 SQLite 静态库，不新增 crate 或产品依赖。仅打开测试包内经过内容验证的现有合成库，持有真实写锁，收到释放/EOF 或 12 秒到期便回滚，不提交内容。
- 更新当前状态、维护待办、原生前置设计/验收入口和本交接；记录运行器诊断失败及修正，不将其归为产品修复。
- `crates/`、`assets/`、`Cargo.toml`、`Cargo.lock` 相对 `62113c4f` 无差异；页面、阅读位置采集和 `android_bridge.rs` 相对 main 也保持不变。

## 代码与运行器 review

本轮自审没有发现返回提取的新可操作问题。私有策略独占返回选择、首页提示与代次；页面保留浮层优先级和保存门禁；工具栏保持同步；Android 的 500 ms 捕获上限、原子 guard、输入合并、监听代次和取消释放仍使用既有职责。没有独立 agent review。

运行器在基线调试中修正了四类问题，失败文件均保留：

1. 目录关闭后节点仍在 DOM 中，应断言隐藏而非不存在（`windows-baseline-01`）。
2. 编辑来源只写 SQLite，不抓取新地址；等待慢 HTTP 响应不能制造保存中状态（`android-baseline-01`）。改用真实写锁后，返回保留禁用编辑框，释放锁后保存完成。
3. Android IPC 方法赋值未提供可靠注入证据，初轮 `android-baseline-02` 的失败/超时及取消结果作废，不作为这些路径的通过凭据。改为替换可用的 window IPC 绑定，兼容字符串/对象消息，并断言每次注入实际命中；中间错误记录在 `android-baseline-03`。原生日志随后确认一次通信错误和一次真实 500 ms 等待到期。
4. Windows 的 IPC 绑定不能按 Android 方式替换（`windows-baseline-03`）。最终仅 Android 注入回复故障，Windows 观察捕获事件，不改其 IPC。

最终 Android 基线为 `android-baseline-04`（运行器 SHA256 `ddbb3e5f…`）；候选为 `android-candidate-01`（`404af138…`）。二者之间只给 Windows 加了平台分支，Android 执行路径相同。最终 Windows 基线/候选也使用 `404af138…`；完整脚本、包和报告散列见回执。

## 验证与验收

### 环境与身份

| 项目 | 本轮实际值 |
| --- | --- |
| Windows | Windows 11，10.0.26200；WebView2 156.0.4314.50；1280×844、390×844 |
| Android | 真机 `2201123C` / cupid，Android 15 / API 35；WebView 156.0.8078.47；CSS 宽度 392 |
| 独立测试包 | `io.github.develata.rssreader.backpolicytest`；Activity `dev.dioxus.main.MainActivity` |
| 基线源码 | main `4101520c33266026747db867c33e2b13baea3d23` |
| 候选源码 | `62113c4fc181e245bc077dcd46eda56a969d7b96`，原生包源码散列重新匹配 |
| 合成库 | 100 篇、6 个本机来源；SHA256 `529eed9823d1a4ec207ec3c725e467feabb0cade7c6e694a22b7097195383ef3` |
| 原生包身份 | EXE/APK 完整散列在 `target/back-policy-20261029/identity.json` 及本次回执；Android 安装后再次在设备上算包散列 |

### 执行结果

| 命令 / 场景 | 本轮结果与边界 |
| --- | --- |
| `cargo test -p rssr-app --locked use_mobile_back_navigation` | 同日重启前 16 passed，0 failed，0 ignored；普通策略、Dioxus 投影、捕获与桥接 |
| `cargo test -p rssr-app --locked dismiss_edit_consumes_back_without_leaving_a_save_in_flight` | 同日重启前 1 passed；来源保存门禁 |
| `cargo fmt --all --check`、`node --check scripts/browser/rssr_back_navigation_acceptance.cjs` | 通过 |
| `run-windows.ps1 -Variant baseline -RunName windows-baseline-04` 与 `-Variant candidate -RunName windows-candidate-02` | 各 4/4 组通过；真实 WebView2、两视口，每视口连续 3 次阅读/同步工具栏返回、位置恢复、图片/来源/目录/编辑浮层 |
| `run-android.ps1 -Variant baseline -RunName android-baseline-04` 与 `-Variant candidate -RunName android-candidate-01` | 各 6/6 组、4/4 个采集子结果通过；实际 adb 系统返回、保存中消费返回、采集失败/超时、三次输入合并、路由取消、首页提示、退后台与恢复 |
| 两平台报告的 `pageErrors` / `consoleErrors` | 最终四次运行均为空；原生日志的警告另列下方，不能写成零日志警告 |
| `compare-native.py <baseline> <candidate> <output>` | Windows 13/13、Android 9/9 图片相同，0 个像素变化；不推断其他页面/设备视觉 |
| 主 checkout 保护复核 | 40 条状态、index SHA256、20 个现存文件散列不变；22 个缺失路径仍缺失 |
| 文档链接、JSON 解析、`git diff --check` | 结果写入本次结构化回执 |

两个 PowerShell 启动器和图像比较器位于本次 `target/` 证据根目录。它们记录包、进程、隔离 profile/库和 CDP 身份，逐次使用新运行目录；原始失败不覆盖。Rust 小工具的本次编译方式：

```powershell
$ndkBin = Join-Path $env:ANDROID_HOME 'ndk/28.2.13676358/toolchains/llvm/prebuilt/windows-x86_64/bin'
rustc --edition 2024 --target aarch64-linux-android -C "linker=$ndkBin/aarch64-linux-android24-clang.cmd" -L native=target/aarch64-linux-android/android-dev/build/libsqlite3-sys-f80900288a184be8/out target/back-policy-native-20261030/native_back_save_lock.rs -o target/back-policy-native-20261030/native-back-save-lock
```

这些工具链目录是本机本次构建位置，复跑时须重新定位 NDK 和当前 `libsqlite3-sys` 产物，不能照抄旧 build hash。运行器的通用输入和隔离要求见[验收入口](../testing/native-control-acceptance.md)。

### 计时的准确含义

采集事件到 Reader DOM 离开的单次样本：基线正常约 49 ms、注入失败约 49 ms、扣留回复约 549 ms；候选正常约 40–44 ms、失败约 52 ms、扣留回复约 544 ms。两者扣留回复均确认注入命中，原生日志均报告 `Elapsed` 后导航。500 ms 是 Rust 捕获等待上限；上面的 DOM 延迟包含宿主/界面调度，不能解读为系统按键全链路严格不超过 500 ms，也不是性能提升结论。

取消检查在捕获后约 143 / 112 ms 改变页面，早于超时；旧请求不再导航，下一次返回继续工作。首页提示寿命约 5048 / 5059 ms；再次返回退后台、恢复后清除、旧计时器不清除新提示均实际通过。

## 审批阻塞与安装经过

重启前原始 fixture 命令被拒绝，错误为 `approval required by policy, but AskForApproval is set to Never`，当时没有执行或换入口绕过。后经用户授权排查、修改配置并收窄规则：旧规则对所有 Python 文件执行和 Start-Process 强制 prompt，与 never 会话相冲突。相关 agent-config 本地提交为 `8b7014b2`，26 个规则用例通过；该独立配置仓库未 push，备份在 `C:/Users/QQ/.codex/backups/20261030-101537-approval-policy/`。

用户重启后，本会话实际策略仍显示 never，磁盘设置为 on-request；两者不能混为一谈。本次不继续扩大配置修改，已验证收窄后的原始命令和原生进程启动可以执行：

```powershell
python scripts/seed_reading_native.py target/back-policy-native-20261030/seed --fixture-dir target/back-policy-native-20261030/fixture --fixture-base http://127.0.0.1:9299 --entries 100 --paragraphs 120
```

首次安装基线被手机返回 `INSTALL_FAILED_USER_RESTRICTED`，用户确认后重试成功；随后候选 `adb install -r` 成功。两次成功安装都只涉及独立测试包，日常 RSS-Reader 未安装/重置/写入。

## 当前状态、风险与后续

- 本批返回策略在所列宿主和场景内完成验收，可进入后续候选审查；尚未创建新 PR、合并、打 tag 或发布，不向已合并 PR #40 追加实现。用户已允许本候选分支的本地提交与 push。
- 基线和候选的来源保存日志均有 Dioxus `__copy_value_hoisted` 警告；本次未复现保存后错误导航、崩溃或数据丢失。记录为既有观察，不把它变成本批产品修复。启动刷新早于夹具 HTTP 服务监听的六条失败也属测试启动顺序，不证明刷新缺陷。注入的通信错误/超时日志则为预期证据。
- 采集错误/扣留回复是实际宿主上的受控 CDP 注入，未证明真实设备 IPC 故障发生率。单元测试承担取消前未轮询、旧监听注销等无法仅靠 UI 操作完整枚举的分支。
- 未覆盖正式签名包升级、旧 Android 数据目录搬迁、系统 IME、辅助技术、多设备以及 macOS/Linux GUI；合成库预置存储标记不作为迁移证据。
- 没有新产品代码，未重复完整 workspace/Clippy、跨平台构建、正式 Web、CI 或发布验收。当前已发布版本仍为 v0.2.1，main 未变化。
- 测试结束后 Windows 进程已正常关闭，Android 测试包已停止；本轮 9291/9292/9299 无监听，Android 本轮 forward/reverse 已移除，既有 9243/9252/9262 forwards 和 8111 reverse 保留。候选测试 APK、合成库及诊断副本留存，日常资料库不受影响。
- 保留 2 个工作树、3 个本地分支；主 checkout `020f9d40` 的既有改动保持原样。旧 C 盘目录不在本次处理范围。

## 给下一位 Agent 的备注

继续使用 E 盘候选工作树，先读[当前维护状态](../current-status.md)及本记录。返回策略本批已收口，下一步应审查候选与主线的整合，不应重新把审批阻塞或缺设备列为当前未完成项。原生控件接入仍未开始；任何新的提取都须另选明确收益与边界，继续复用既有 Rust 模块。
