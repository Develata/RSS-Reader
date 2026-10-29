# 目录突跳修复：Windows 原生复测

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/directory-native-verification`
- 当前代码 HEAD：`0692c92465b25a059d083a977b513d9f8e088355`（PR #43 合并后的 main）
- 相关 commit：`a3dbd63e`（顺序模型修正）；验收文档随本文件所属的独立本地提交归档
- 相关 tag / release：N/A；未替换用户安装
- 状态：`validated`（本机 Windows 原生交互通过）

## 工作摘要

应用户要求，继续验证原视频中“同日不同来源交界处，小幅滚动导致目录突然远距离往返”的问题。本次从已合并源码重新构建并实际启动 Windows 原生应用，在隔离 SQLite 资料库中重复该动作，逐帧检查目录位置，并用 Windows 系统滚轮交叉核验。所测场景均通过，补齐此前原生启动未执行的验收缺口。

## 影响范围

- 模块：验收文档与索引；本次没有产品代码、依赖、存储或 Penpot 变更。
- 平台：本机 Windows 原生 Dioxus 宿主，WebView2 `156.0.4314.50`。
- 工作区：`C:/Users/QQ/.codex/worktrees/design-refinements/RSS-Reader`。
- 测试程序与资料库位于下述证据目录；未使用用户实际资料库或替换已安装应用。

## 关键变更

### 可追溯的运行产物

- 原生目标页为 `http://dioxus.index.html/`，不是本地 Web 开发服务器。
- 实际窗口客户区 `1280 × 900`、DPR `1`、浅色主题；未用设备模拟替代原生窗口。
- 运行程序：`C:/Users/QQ/Documents/Codex/2026-10-29/directory-jump/native-main/rssr-app.exe`。
- SHA-256：`4E195F7F90E538C3FC4543CD7AB643ED663E21A916574B718BA0150DF9E63348`。
- 启动回执 `native-main/identity.json` 记录源码、文件散列与 PID；确认其 WebView2 子进程属于本次应用。
- 夹具含 6 个来源、5000 篇同日交错文章；当前查询为 2000 篇、每页 50 篇。复现边界为 Alpha 的 `4951` 与 Bravo 的 `5000`，修复后目录下标相邻（8、9）。

### 验收工具与证据

- 复用仓库 `scripts/browser/cdp_session.mjs` 连接本次原生 WebView2 的调试端口，输入滚轮、检查真实 DOM、记录动画帧与原生客户区画面。
- 另用 Windows 系统输入向所属原生窗口发送正反向滚轮，并用 Windows 窗口截图检查结果；页面收到两个 `isTrusted = true` 的滚轮事件。
- 本次脚本保存在证据目录，未新增生产调试开关。测试后正常关闭该原生窗口，确认所属进程退出且 `9253` 端口不再监听。

## 验证与验收

### 自动化验证

以下 `$evidence` 指 `C:/Users/QQ/Documents/Codex/2026-10-29/directory-jump`；命令使用 PowerShell。

- 设置 `CARGO_TARGET_DIR=E:/gitclone/RSS-Reader/target/parse-image-20261028`、`RSSR_BUILD_ID=0692c92465b25a059d083a977b513d9f8e088355` 后执行 `cargo build -p rssr-app --locked`：通过。
- `node "$evidence/native-directory-regression.mjs" "$evidence/native-main/verified"`：9/9 验收检查通过，0 个记录到的页面异常。
- `node "$evidence/native-directory-supplement.mjs" "$evidence/native-main"`：6/6 补充检查通过。
- 系统滚轮前后执行 `node "$evidence/native-os-wheel.mjs" start/finish "$evidence/native-main/verified"`：通过；两个系统滚轮事件、6563 个采样帧，目录最大帧间位移 0px。
- `git diff --check`：通过。

| 场景 | 实际结果 |
| --- | --- |
| 原视频来源交界处 ±40px 往返 24 次 | 1244 个采样帧，目录最大帧间位移 0px；当前项在 4951 / 5000 间正确切换 |
| 快速反向滚轮 40 次 | 318 个采样帧，目录最大帧间位移 0px，持续处于自动跟随模式 |
| 跨虚拟窗口正反向逐项定位 72 次 | 最大帧间位移 48px；目标可见、身份正确、选中状态保留 |
| 标题和虚拟窗口 | 原视频动作中标题 Y 坐标始终 64px，挂载 19 项，同时保留完整 2000 项目录引用 |
| 手动目录滚动后关闭重开 | 滚动位置 804px → 804px；补充 3 次关闭重开也一致，选择保留 |
| 手动浏览后重新滚动主列表 | 恢复自动跟随，目标位于标题下方可见区域 |
| 系统滚轮交叉检查 | 当前项经过 4951 → 5000 → 4994 后返回 4951，目录没有突跳 |

### 视觉检查与原始证据

- 检查实际 Windows 窗口截图：目录标题、当前项与主列表对应，目标未被标题遮挡。静态截图只证明布局；滚动稳定性由逐帧轨迹和重复输入共同支持。
- 最终完整验收目录：`$evidence/native-main/verified/`。
- `native-acceptance.json`：环境、9 项结果、异常列表；`native-boundary.json`、`native-rapid.json`、`native-walk.json`：动作状态与逐帧轨迹；`native-os-wheel.json`：系统输入与轨迹。
- `windows-directory-boundary.mp4`：170 个原生客户区采集帧按实际时间间隔编码，时长 6.20 秒；`native-window.png`：包含 Windows 标题栏的原生窗口截图。
- 补充检查位于 `$evidence/native-main/native-supplement.json`；构建日志为 `$evidence/native-main-build.log`。
- 早期 Playwright 截图超时，原生 CDP 截图与 Windows 窗口截图成功。首次完整检查的关闭重开步骤点击了客户区外的隐藏按钮位置，随后窗口被最小化，导致补充脚本等待超时；修正验收脚本的可见点击区域检查及 DOM 完成等待后，完整重跑 9/9 通过。早期失败文件仍保留在 `native-main/`，最终结果以 `verified/` 为准；此过程没有修改产品源码。

## 结果

- 已确认修复后的 Windows 原生应用在本次原视频复现场景中没有再次发生目录突跳；快速反向、跨虚拟窗口、手动浏览与关闭重开检查同时通过。
- 与先前 Web 红灯对照及顺序不变量测试合并判断，证据支持共享分组模型修正已在 Windows 宿主生效，无需追加滚动延时或补偿补丁。
- 本次只归档验收文档；PR #43 的既有源码修复已经合并，未发布新版本。
- PR #43 描述已补充本次原生验收结果，替换此前原生交互未执行的状态。
- 本次验收文档独立本地提交，未推送该文档分支；源码合并状态以 PR #43 为准。

## 风险与后续事项

- 本次为本机 Windows、上述 WebView2、DPR 1、浅色桌面窗口；未扩展为 Android、Safari、多显示器 / DPI、真实触摸惯性或辅助技术验收。
- 本次只运行修复后的原生产物；旧版本的失败对照为此前 Web 产物，不宣称完成旧 Windows 二进制的对照测试。
- 动画帧数据用于检验目录位置变化，不是操作系统合成器帧率或性能基准。没有将主动跟随所需的 48px 正常位移计为异常跳跃。
- 本次使用隔离测试资料库，没有以用户真实订阅内容覆盖所有输入组合。

## 给下一位 Agent 的备注

- 算法背景与此前红绿对照见 [阅读顺序模型修正](./2026-10-29-directory-reading-order.md)。PR：<https://github.com/Develata/RSS-Reader/pull/43>。
- 原始录像、截图、JSON、隔离资料库和临时验收脚本均保留在本机证据目录，未提交大体积产物到仓库。
- 文档位于独立验收分支；不要把本次复测误认为安装升级、版本发布或全部平台验收。
