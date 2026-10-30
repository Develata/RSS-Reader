# 返回导航与退出提示策略提取

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/native-back-policy`
- 起点 HEAD：`faf4c21102de25de5d6cbaf77e97d2706e5a8856`
- 产品基线及本次两次核对的远端 main：`4101520c33266026747db867c33e2b13baea3d23`
- 相关 commit：本记录与实现同次本地提交；提交前 `commit: pending`，提交后以本文件 Git 历史定位，精确 SHA 另记本地 `final-receipt.json`
- 相关 tag / release：无；不 push、不发布、不修改已合并的 PR #40
- 状态：`draft`（实现、自审、Rust/Web 验证及跨平台构建完成；实际原生宿主验收受执行审批阻止）

## 工作摘要

按[上一批范围](./2026-10-29-native-preparation-next-batch.md)在指定 E 盘工作树提取返回决策、首页提示状态/代次、超时和恢复前台规则。保持当前交互与页面所有权，没有据此认定或修复新的产品运行缺陷。未扩展到 Shell、渲染器、业务层、数据库、公开接口或新 crate。

## 影响范围

- 工作树：`E:/gitclone/RSS-Reader-worktrees/native-back-policy`。没有切换、清理或提交 `E:/gitclone/RSS-Reader` 的其他任务改动；C 盘旧工作树清理不在本批范围。
- 生产代码：`hooks/use_mobile_back_navigation.rs`、子模块 `policy.rs` 和 `back_capture.rs`。
- 平台：Web/Windows/其他原生端共用的工具栏返回；Android 系统返回与首页提示。实际运行证据仅为本轮正式 Web。
- 更新长期模块说明、前端命令说明、验收入口与交接索引。没有依赖、锁文件、样式、页面组件或 workflow 变更。

## 关键变更

- `policy.rs` 为私有普通 Rust 模块，不依赖 Signal、Navigator、DOM、JNI 或真实计时器。`Navigation` 选择 history、不同 fallback 或停留；`BackPolicy` 接收已消费、首页、导航、带代次超时及恢复前台事件，输出动作。
- 策略独占提示状态和代次；`BackAdapter` 只投影可见性到既有 `Signal<bool>`。页面显示信号不参与返回决策。首页首次返回开始 5 秒提示，再次返回退后台；旧超时不清除新提示，恢复前台清除提示。
- 页面 dismiss 仍在采集导航事实之前执行。Entries 的来源筛选/目录优先级、Reader 图片查看器、Feeds 保存中消费返回但不关闭的 gate 均未迁移或复制。
- 工具栏同步调用原路由操作。Android 系统返回在策略确认可导航后取得原子 guard，原位置采集函数继续执行 500 ms 上限；完成时检查路由并重新解析导航目标。
- `back_capture::finish_capture` 将原异步等待装配为可注入 Future 的窄函数；不保存新的业务状态。guard 覆盖等待和导航，失败、超时或取消后仍可再次取得；原跨线程实现与输入桥代次保持不变。
- 新增 12 项回归：7 项无 VirtualDom 的策略事件序列、4 项采集适配测试、1 项 Dioxus 提示投影测试。连同原 gate/输入桥 4 项，返回模块共有 16 项通过。

## Review

- 自审逐项对照旧 hook 的副作用顺序：dismiss → 判定首页/导航 → guard → 位置采集 → 当前路由校验 → 重新导航；首页提示状态先更新，再启动计时器或退后台。
- 确认 history 优先、相同 fallback 不 replace、首页不沿 history、保存中的返回不会穿透、过期超时不改变新提示、重复恢复前台幂等。
- `git diff 4101520c --` 对 `android_bridge.rs`、`ui/reading_position.rs` 及整个 `pages/` 为空；500 ms、原子内存序、容量 1 队列、监听代次、旧监听清理和页面关闭 gate 原样保留。
- 未发现需要单列修复的产品问题；这是本 agent 自审，不是独立交叉审查。实际 Android 调度、JNI/Activity、WebView2 交互仍需下述宿主验收。

## 验证与验收

原始证据根目录：`target/back-policy-20261029/`，均为本批新运行或新准备的材料；未将此前复制的 4719 个文件当成本批通过结果。紧凑回执见 [result.json](../testing/evidence/2026-10-29-native-back-policy/result.json)。

| 验证 | 实际结果 | 证据 |
| --- | --- | --- |
| `cargo test -p rssr-app --locked` | 373 通过、0 失败、1 既有忽略 | `app-tests.log`；主单元 368 + 集成 2 + 样式契约 3 |
| `rustc --test --edition 2024 target/back-policy-20261029/policy-tests.rs` 及运行产物 | 7/7，通过不链接 Dioxus 的普通 Rust 包装验证策略模块 | `policy-tests.rs`、`policy-tests.exe` |
| `cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过 | `clippy.log` |
| `cargo fmt --all --check`、`git diff --check` | 通过 | 提交前再次核对 |
| `cargo build -p rssr-app --locked` | Windows 基线与候选构建通过；未启动原生窗口 | `baseline-windows-build.log`、`candidate-windows-build.log` |
| `dx build --locked --platform web --package rssr-app --release --debug-symbols false` | 基线与候选正式 Web 构建通过，不含 pages-demo | `baseline-web-build.log`、`candidate-web-build.log` |
| `node scripts/browser/rssr_workspace_acceptance.cjs` | 基线/候选分别 18/18，无页面异常 | `web-baseline-workspace-matched.log`、`web-candidate-workspace.log` |
| `node target/back-policy-20261029/web-return-compare.cjs` | 基线/候选分别 6/6，1280×844、390×844，无页面/控制台异常 | `web-{baseline,candidate}-return/acceptance.json` |
| `node scripts/browser/rssr_directory_window_boundaries.cjs` | 基线/候选分别 11/11；含筛选后末页阅读返回，保留第 6 页、来源选择和文章相对位置，0 页面异常 | `web-{baseline,candidate}-boundaries/acceptance.json` |
| `dx build --locked --platform android --package rssr-app --target aarch64-linux-android --debug-symbols false` | 两份独立包名 debug APK 构建通过；未安装、未运行 | `baseline-android-build-retry.log`、`candidate-android-build-verified.log` |

Web 使用现有真实 rssr-web/SQLite harness，每次新建隔离资料库。设置 `NODE_PATH` 为当前 Codex runtime 包目录，`CHROME_BIN` 为本机 Chrome，`RSSR_WEB_BIN` / `PUBLIC_DIR` 为对应产物，`ARTIFACT_DIR` 为上述独立结果目录。脚本、产物和三个生产源文件的 SHA-256 记录在 `identity.json`。

返回专项覆盖：三个连续工具栏往返、Reader 直接加载后的 fallback、来源筛选及小视口目录关闭、来源编辑关闭、Feeds/Settings 页面。11 组同路径截图中 9 组逐像素相同；两组来源编辑截图分别有 1015/539 个不同像素，来自回环 fixture 端口和实时刷新时间，已目视核对，无布局变化。完整像素统计见 `visual-comparison.json`，不宣称所有截图逐像素相同。

### 构建和测试装配问题

- 首次 Playwright 默认查找的 headless Chromium 不存在；改用已安装的 `C:/Program Files/Google/Chrome/Application/chrome.exe`，没有安装新浏览器。
- 首次基线工作区写入被 `build_mismatch` 拒绝：基线页面 ID 为 `dev-faf4c21102de-dirty-7397074f0232`，当时服务器为候选 ID `dev-faf4c21102de-dirty-0d479f1a558c`。为基线服务器显式设置匹配的 `RSSR_BUILD_ID` 并重建，候选使用另一份匹配服务器；两个包请求头均由专项脚本断言。未关闭版本门禁。初次失败保留在 `web-baseline-workspace/`。
- Android 构建在 `target/` 内的源码快照进行，只改测试快照的 bundle identifier 和 Kotlin BuildConfig 别名为 `io.github.develata.rssreader.backpolicytest`，不覆盖日常包。首次 Kotlin 仍取到旧别名而失败，重建解决；没有将测试包名写入产品源码。
- 候选首次复制源文件保留旧 mtime，Cargo 复用了基线 APK；核对哈希发现后刷新三个复制文件的 mtime 并重建，确认 depfile 包含 `policy.rs`、源码与候选逐文件哈希相同、APK 哈希不同。仅 `candidate-android-build-verified.log` 和最终 `candidate.apk` 计入候选构建证据。
- dx 0.7.10 对锁定 Dioxus 0.7.9 的版本提示，以及 Android 既有 `LOCAL_DATA_DIR_NAME` 未使用警告仍存在；两次有效打包退出码均为 0。未借机升级依赖或改其他模块。

### 实际原生宿主未覆盖及审批阻塞

`adb devices -l` 发现在线设备 `caa0c283` / `2201123C`（cupid），不是缺少设备。自动审批先拒绝“生成隔离资料库并启动独立 Windows EXE”的组合命令，返回 `blocked by policy`；再单独准备隔离资料库时返回 `approval required by policy, but AskForApproval is set to Never`。命令均未执行，没有更具体的拒绝原因，没有改用别的入口绕过。

因此本批没有启动 Windows 原生宿主、安装 Android 测试包或修改设备数据；没有宣称 WebView2/CDP、Android 系统返回、退后台、恢复前台或实际 500 ms 等待验收完成。这里只完成 Windows/Android 构建和普通 Rust/适配器测试；Web 小视口不替代 Android。设备包和测试资料库没有运行，不存在本批需要恢复的用户资料库。

已准备的 `target/back-policy-20261029/rssr_back_navigation_acceptance.cjs` 仅通过 Node 语法检查，**尚未实际验证**，不作为已验证的仓库运行器提交。解除执行审批后须先完成同一隔离 fixture 的 Windows/Android 基线，再执行候选；复核脚本和进程/APK 身份，不直接套用历史参数。

## 当前状态、风险与后续

- 实现及上述 Rust/Web 检查完成，本地保存本批代码与交接；实际原生验收仍是未完成项，所以交接状态为 draft，不能标记为完整原生交付或发布就绪。
- 待补：真实 Android 首页两次返回、5 秒过期、旧超时、新提示、恢复前台、图片/来源/目录关闭优先级、保存中返回、连续阅读、失败/超时后继续导航、取消后的再次返回；Windows 用相同隔离库补真实 WebView2 路径。
- 本次适配器超时测试注入已超时 Future，不代表实际设备测得 500 ms；宿主保留原函数中的 500 ms 上限。没有性能测量或改善结论。
- 未运行全 workspace、远端 CI、Docker、Linux/macOS GUI、系统文件选择器、真实 IME 或辅助技术测试。这些不作为本批通过范围。
- 相关 test-server 8119 及计划原生端口 9291/9299 收尾时无监听；保留本批证据、源码快照、编译缓存和 APK/EXE，不清理旧 C 盘材料。

## 给下一位 Agent 的备注

从本文件、`policy.rs` 和 `back_capture.rs` 进入；保持独立策略与页面状态所有权，不把未覆盖的宿主结果写成通过。继续使用本 E 盘工作树和分支，先核对本地提交/dirty 状态及 `identity.json`；本批没有 push，不应向 PR #40 追加实现。
