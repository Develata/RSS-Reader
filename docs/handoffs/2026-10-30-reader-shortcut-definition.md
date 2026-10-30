# Reader 快捷键定义与同步输入边界

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/reader-shortcut-definition`
- 基线 HEAD：`6a5b91b460a112543bd5154e9b86705b83421748`（PR #51）
- 产品 commit：`ddb7045e9066f6a8b9d0c987dc5f6b2916636689`
- 文档/证据 commit：本记录所在提交，可用 `git log -- docs/handoffs/2026-10-30-reader-shortcut-definition.md` 查阅
- PR：[快捷键去重 #52](https://github.com/Develata/RSS-Reader/pull/52)，base 为 `codex/reader-host-boundary`
- 相关 tag / release：无
- 状态：`validated`（本地实现与所列验收；两轮 CI 失败及独立测试修正见下文，最终 head CI 另核）

## 工作摘要

承接 [Reader 宿主入口](./2026-10-30-reader-host-boundary.md)，消除 Rust 动作映射、宿主按键表与切文位置采集按键表的重复维护。用一份普通 Rust 绑定定义产生当前宿主配置，并共用同步输入过滤；保留原捕获/冒泡阶段、默认动作、监听归属和任务生命周期。没有确认新的产品缺陷或混入行为修复。

继续复用 `E:\gitclone\RSS-Reader-worktrees\native-back-policy`。main 仍为 `4cef074c`；PR #50 head `ff628ed0`、PR #51 head `6a5b91b4` 均 open。本批从 #51 建立依赖分支与 PR，只审查本次增量，不合并或发布。

## 影响范围与关键变更

- `hooks/reader_shortcut_policy.rs`：`BINDINGS` 为唯一的按键/动作定义，`captures_position` 标识导航动作；`resolve` 接收普通逻辑键字符串，保留 ASCII 字母大小写、命名键精确匹配、未知键和修饰/组字的惰性排除。
- `reader_page/webview/`：桥接启动时从 Rust 定义生成私有只读配置，与 `shortcut_input.js` 一起进入各自 Eval；不创建全局注册、额外握手或状态镜像。按键处理时不重新生成配置。
- `reading_position.rs/.js` 与快捷键监听共用上述投影和过滤；捕获阶段仅测量导航键，仍忽略 `defaultPrevented`，先于目标节点及冒泡；动作派发仍在冒泡阶段检查该标志并同步阻止默认动作。测量及两个桥接的原作用域、序号、恢复和清理保持不变。
- 新 `reader_shortcut_scenarios.cjs` 由已有原生运行器与正式 Web 运行器共同调用，使用原 `--reader-only true --reader-host true`。现有 `reading-workspace` CI 自动涵盖新场景，没有新增作业。
- 平台涉及 Web、桌面和 Android；七 crate、Backend、数据库、正文/图片、CSS、路由及 session 状态不变。不推广为全局键盘框架，不新增依赖或原生渲染器。

长期契约见[快捷键定义与同步边界](../design/native-ui-preparation.md#reader-快捷键定义与同步边界2026-10-30)。

## 验证与验收

本地证据在工作树的 `target/reader-shortcut-definition-20261030/`，仓库保留[紧凑回执](../testing/evidence/2026-10-30-reader-shortcut-definition/receipt.json)。不以 PR #51 的历史验收代替本次运行。

### 普通 Rust、编译与静态检查

| 命令 | 实际结果 |
| --- | --- |
| `cargo test -p rssr-app --locked` | 420 通过、1 忽略：391 unit + 24 生产规则 harness + 2 主题 + 3 样式；新采集分类测试在 unit 和 harness 各运行一次，不按两项独有覆盖计数 |
| `python scripts/check_reader_rule_boundary.py --target-dir target/reader-shortcut-definition-20261030/rules-build` | 同一 24 项生产规则独立编译通过；只链接现有 API/domain/serde，无 Dioxus/web-sys/Wry/Tao |
| `cargo fmt --all --check`；`cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过 |
| `cargo build -p rssr-app -p rssr-web --locked`；`cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 |
| dx 0.7.9 `build --platform web --package rssr-app --locked --release --debug-symbols false` | 正式 Web 构建通过 |
| dx 0.7.9 `build --platform android --package rssr-app --target aarch64-linux-android --locked` | 独立源码和 target 构建通过；既有 `LOCAL_DATA_DIR_NAME` 未用警告保留 |
| `node --check` 三份 CJS 与 `shortcut_input.js`；`git diff --check` | 通过 |
| 本地 Markdown 链接/锚点、证据 JSON 检查 | 331 份 Markdown、1566 个相对链接；本批无新增失效链接/锚点，变更文档与 JSON 检查通过，见 `documentation-check.json` |

### 实际宿主与正式 Web

| 平台 / 版本 | 基线 / 候选 | 产品提交时的回执目录（后续脚本修正见下文） |
| --- | --- | --- |
| Windows WebView2 `156.0.4314.50`，1280/390 | 各 8/8 | `windows-baseline-02` / `windows-candidate-02`；初轮 `-01` 亦均通过，顺序复跑用于核对截图差异 |
| Android 15 真机 `caa0c283`，WebView `156.0.8078.47`，实际宽度 392 | 各 4/4 | `android-shortcuts-baseline-01` / `android-shortcuts-candidate-01`；图片关闭使用实际系统返回 |
| 正式 Web，Windows Chrome `155.0.8059.40`，1280/390 | 各 4/4 | `web-baseline-01` / `web-candidate-01`；真实同源服务器与新 SQLite 夹具 |

三端均先运行旧包基线，再对候选使用相同脚本、路径和夹具。最终 `pageErrors` / `consoleErrors` 均为空。除原图片/焦点/滚动、M/F 保留正文、输入排除、切文与返回、三种卸载检查外，新增每视口：

- 70 个边界：repeat/composing、Alt/Ctrl/Meta/Shift、预先 preventDefault、目标停止传播、input/textarea/select、可编辑祖先/嵌套/显式 false、已打开 dialog、未知键。
- 10 个有效键值：左右方向、J/K/M/F 的大小写；目标节点收到事件时已经采集导航位置，`dispatchEvent` 返回前已经阻止默认动作。M/F 不扫描正文，实际等待写入状态改变再恢复。六个导航键值在各宿主视口均实际切文并返回恢复位置。
- 离开 Reader 后的输入不再阻止默认动作或扫描正文。探针包装原 `Element.querySelectorAll` 统计正文扫描，保留原测量；合成 KeyboardEvent 和受控停止传播不是物理键盘或系统 IME 实测。

调用分别为 `run-windows.ps1 -Variant baseline|candidate -RunName <目录>`、`run-android.ps1` 同名参数，以及 `node scripts/browser/rssr_reader_host_acceptance.cjs`。正式 Web 用 `RSSR_WEB_BIN`、`PUBLIC_DIR`、`ARTIFACT_DIR` 选择配套产物，`RSSR_FIXTURE_BIN` 可指定夹具程序；所有运行目录在 target 下新建。任务内脚本记录具体启动/构建/保全命令，稳定约束见[验收入口](../testing/native-control-acceptance.md)。

### 视觉核对与保留记录

首轮 22 对截图有 4 对 Windows 像素不同：69、1465、60785、7 像素。已查看原图，未发现可见内容或几何变化；相同包、脚本和夹具顺序复跑后，较大差异未重现。最终 21/22 对逐字节相同，余下 `390-image.png` 仅 7 个像素，范围 `[7,26,383,66)`；两份精确图片的 SHA256 在两轮中分别出现在基线与候选，证明该差异并非候选独有状态。

保留初轮与最终报告、全部原图、`receipt-initial.json`、`visual-differences-initial.json` 和 `visual-review.json`。没有调整产品或截图掩码来消除差异，没有宣称全部图片相同；绘制差异的根因未确认，不据此报告新产品缺陷。

### CI 失败与独立诊断

- `25d1ee89` 的 [CI run 41139300981](https://github.com/Develata/RSS-Reader/actions/runs/41139300981) 首轮 34 个作业成功，`reading-workspace` 与其汇总门禁失败。失败在既有 `preferenceOrdering` 第一组：最终 PUT 响应成功，但 15 秒内未观察到对应 `requestfinished`；尚未执行新增 Reader 场景。原始作业日志、失败 HTML/PNG 与完整作业清单保存在本批 target 的 `ci-initial-failure*`，下载第一次 EOF 的残留亦保留。
- 此签名曾在 PR #50 出现；本批再次按原顺序运行前置 9 场景，再连续 5 轮原偏好测试，均通过，没有请求失败或页面异常。Windows Chrome `155.0.8059.40` 与 CI Linux Chrome `156.0.8078.80` 不同；不能用本地未复现代替根因证明。
- 独立测试提交只给 `preferenceOrdering` 增加请求 ID、时间、请求/响应/结束/失败事件，并在断言失败时输出到既有 `run.log`；不改变请求、保存逻辑、断言、15 秒超时或重试规则。根因仍未确认，不报告产品缺陷已修复。最终 CI 回执按下文路径和 PR 当前 head 核对。
- 诊断改动后，`node --check scripts/browser/rssr_workspace_acceptance.cjs` 通过；使用同一正式 Web 产物与新隔离资料库运行完整 workspace **18/18**，无页面异常，日志为 `workspace-observability-01.log`。产品输入保持不变。

### Reader 返回位置的测试采样修正

- `2c50ba4b` 的 [CI run 41140712464](https://github.com/Develata/RSS-Reader/actions/runs/41140712464) 中，偏好测试和前置七套验收通过；Reader 1280 宽的全部检查与 390 宽的宿主生命周期通过，但后者快捷键返回断言期望 600、实际 700。首轮仍为 34 个作业成功、工作区与汇总门禁失败；完整归档在 `ci-diagnostic-failure/`。
- 原测试在读取按钮状态的多个 await **之前**记录 scrollY，随后才派发按键。受控诊断在这两点之间将实际视口滚动 100 像素：基线和候选均按实际派发位置 700 正确恢复到 700，但旧断言仍比较 600，二者都失败。该事件序列证明测试的取值边界不正确；CI 那次是否由尚未落定的初始滚轮事件触发，未被当时日志证明。
- 单独修正 `reader_shortcut_scenarios.cjs`：在同步 `dispatchEvent` 前立即记录 `positionY`，以此核对返回位置，并保留实际 `restoredY`。没有改变产品、超时或原 `toBeCloseTo(..., 0)` 的 0.5 像素精度。故障原脚本、基线/候选失败回执和受控诊断均保留。
- 修正后，同一受控序列的基线/候选各 **4/4**；正常正式 Web 基线/候选各 **4/4**，使用新隔离目录 `web-baseline-02` / `web-candidate-02`。没有重新构建产品，前述产物散列及产品输入仍适用。
- 修正后的正常原生候选复核：Windows **8/8**（`windows-candidate-03`），Android 真机 **4/4**（`android-shortcuts-candidate-02`），均无页面/控制台错误。测试包资料、日常 APK、adb 映射及主 checkout 再次核验保留。前述 22 对截图比较仍对应原表中的基线/候选，没有将后来截图冒充逐像素比较。

### 产物与数据保全

- Windows 基线 EXE 为前批候选产品 `9e0eb45b`，SHA256 `5e3c5e46bd9f45290049890debcbb76e7b605d8d0b43c76d8245d194cc3e96d3`；本批候选为 `67783946b9a22983b0b50c3d91fd93087568258ca953add5f09f3eeab58886fe`。
- Android 基线 APK 为 `54ca6d4644020b3c2a830bb90aef5d0a840cc073bf736dd1830ef13fc3e7fbd6`；候选 `reader-shortcuts-final.apk` 为 `6a191c33043e2c8835958095f4ff6f606a378b7190c41392ef1494b346fec912`。仅安装 `io.github.develata.rssreader.backpolicytest`，未清除资料。
- 核验 APK 内 `libmain.so`、生成 JNI 与本轮编译输出均为 `9627665a5b1c9799bb58408854a757ff0e5340c20f64f2fc723098b2549c076d`，依赖指向本轮独立源码。462 项产品输入清单实际 SHA256 为 `1c555050335e93b99eb1c15666f1761c3a6e3a06c4bf470c50a97ad6dbc81689`；build ID 为 `reader-shortcuts-6a5b91b4-1c555050335e`。仅测试包 identifier / BuildConfig typealias 在源码副本覆盖。
- Web 在构建候选前保存基线 public 及服务器 EXE，并逐文件散列；候选用本轮构建的 server/public。使用任务内 dx 0.7.9、进程级 JDK 21、NDK `28.2.13676358`，未修改全局工具配置。Android 缓存只复制到本轮独立 target，最终产物重新核验。
- 主 checkout 的 HEAD `020f9d40`、分支、index、status、41 个文件/缺失路径与开始快照一致。旧测试 APK 和原 6 个资料文件先备份、归档核验，每次测试后恢复并核对散列；日常 APK 及原有 adb forward/reverse 保持。候选测试包保留安装，原资料已恢复。

## 结果、风险与后续

本地规则、实际宿主和正式 Web 回归通过；代码自审未发现阻塞问题。适配器仍依赖 Dioxus/DOM，不声称整页可直接迁移或性能改善。物理键盘、系统 IME、辅助技术、macOS/Linux GUI、新渲染器和正式签名包升级未覆盖。

产品已本地提交并推送，PR #52 保持 open。当前为 8 个本地分支、2 个工作树、7 个真实远端分支；未清理分支、工作树或历史资料。最终 head 的 CI 结果留于本地 `ci-receipt-final.json` 并更新 PR 描述，以免仅记录 CI 又产生未验 head；Pages 不对非 main 目标 PR 触发。

下一步先按 #50 → #51 → #52 的依赖顺序收口，再梳理正文内容、图片资源、阅读锚点与 HTML/CSS、复制、IME、无障碍的兼容契约。不顺带实现候选渲染器、全局 Host trait、数据库迁移或新的原生设置整页。

## 给下一位 Agent 的备注

从 `reader_shortcut_policy.rs` 的绑定与动作开始，再读 `webview::shortcut_script` / `shortcut_input.js` 和两处监听。不要因过滤代码共用就移动捕获阶段、增加 defaultPrevented 门禁或等待异步动作决定默认行为。基线/候选脚本及原始回执均保留在本批 target，复跑使用新目录和隔离资料库。
