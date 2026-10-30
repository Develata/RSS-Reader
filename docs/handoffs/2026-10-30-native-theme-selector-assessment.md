# PR #47 整合与主题模式选择宿主评估

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：复用 `codex/shell-settings-lifecycle`
- 工作树：`E:/gitclone/RSS-Reader-worktrees/native-back-policy`
- 产品源码：实现 `14f334d92711a526c3b7d0126b75f7651ae4e904`；被验收的快照 `f38c3ba8b59387cf4beeb59bae5dab97e43bdfd9`
- 当前 main / 产品 HEAD：`663ef4ee5d19458c815a6921f0e418321cf75464`
- 本批脚本与文档 commit：`c3dc5ddbdda4e279439fd9194e66b75cee2ae596`；后续回执提交仅补本条状态
- 相关 tag / release：N/A；不打 tag、不发布
- 状态：`validated`；PR #47 已合并，主题控件评估完成并已推送当前分支；本次评估后续提交尚未另开 PR / 合入 main

## 工作摘要

按上一批建议核对主题模式选择是否适合第一批新增原生控件，并将已经完成的 App 设置读取候选独立合入 main。实际宿主证明 Android 已使用原生列表弹窗，Windows 使用 Chromium 控件；本轮未确认需要修改产品的缺陷，也没有得到重写这三个选项的明确收益。

因此交付宿主评估、可复跑测试与证据，保持产品代码和界面行为。方案比较及下一设计的完成标准见[主题选择评估](../design/native-theme-selector-assessment.md)。前置规则提取在此收口；独立原生设置面板仍是后续设计候选，未批准整页替换、新框架、crate 或数据库迁移。

## 影响范围

- 新增 `scripts/browser/rssr_theme_selector_windows.mjs`、其 UIA PowerShell 助手和 `rssr_theme_selector_android.mjs`。
- 更新维护状态、待办、原生前置边界、适配/验收入口和文档索引；补上一批 PR #47 合并回执。
- 无产品 Rust、Kotlin、JS/CSS、Cargo 依赖或 workflow 变更。未增加业务状态、宿主回调或公共 API。
- 实际平台：Windows 11 / WebView2 `156.0.4314.50`；一台 `2201123C` Android 15 / API 35，WebView `156.0.8078.47`。

## PR #47 整合

[PR #47](https://github.com/Develata/RSS-Reader/pull/47) 的 head 为 `f38c3ba8`、base 为 `6a75fb83`。对照旧 Shell 与私有策略复核启动确认、合并读取、页面读取归属、存储 revision、错误提示和 effect 订阅；没有发现待修问题。GitHub review 和 review threads 均为空，不宣称有独立审查人的批准。

- [CI #325](https://github.com/Develata/RSS-Reader/actions/runs/41098808743)：36/36 成功。
- [Pages demo #220](https://github.com/Develata/RSS-Reader/actions/runs/41098808839)：build 成功、deploy 按 PR 条件跳过。
- jobs 接口出现 ReadTimeout/HTTP 502 后，改用 `gh pr checks 47 --repo Develata/RSS-Reader --json name,state,link` 获取完整 checks；38 条中 36 条属于 CI，另有 Pages build/deploy。原始回执见[checks](../testing/evidence/2026-10-30-native-theme-selector/pr47-checks.json)。没有用首屏 30 条推定全部通过。
- 合并前再次核对 head/base、mergeable 和未解决 review，使用 expected head 合并。结果 `663ef4ee`；`git rev-parse f38c3ba8^{tree} 663ef4ee^{tree}` 同为 `941dd1b245befd30c5ddb930e946f193402c15c5`。
- 当前工作分支和本地 main 快进到合并提交。CI 结论绑定 PR head，未声称合并提交另跑过相同 CI。

## 宿主评估与规则

1. Windows UIA 的主题 ComboBox 为 `framework=Chrome`、`nativeHandle=0`，弹出三个 ListItem。实际 EXE 中 CDP Esc 关闭弹层并保留选择框焦点；CDP 键盘选择只更新草稿/预览。
2. Android UIAutomator 记录原生 `ListView` / `CheckedTextView`，adb 点击产生 trusted input/change。系统返回键取消弹窗，保留 Settings 页面、草稿和存储值；没有导航或退后台。
3. Windows 注入外部 SQLite 设置变更和 DOM focus/visibility/online：pending 主题覆盖新的 stored 主题，未编辑密度字段接受回填，回填不产生输入。
4. 两端都用真实保存命令写库，只有主题字段改变；离页后重开仍为已保存值。继续使用现有 `stored → submitted → pending`，不建立宿主草稿副本。

Windows 宿主可提供窗口和菜单 API，不代表 OS 控件能直接参与 DOM 布局。覆盖式 Win32 ComboBox 或 Android View 还需 DPI、滚动、裁剪、z-order、焦点和自定义 CSS 的明确处理。本轮未把这些成本藏进“单字段适配”，也没有添加第二套选择器。

## 验证与验收

### 本轮命令与结果

所有命令在上述 E 盘工作树执行。本地原始资料位于 `target/native-theme-assessment-20261030/`；可随 Git 回读的紧凑记录为[result.json](../testing/evidence/2026-10-30-native-theme-selector/result.json)。

| 命令 / 入口 | 实际结果 |
| --- | --- |
| `node scripts/browser/rssr_theme_selector_windows.mjs --app-bin target/shell-settings-20261030/bin/rssr-app.exe --seed-dir target/back-policy-native-20261030/seed --artifact-dir target/native-theme-assessment-20261030/windows-reviewed-02 --port 9495` | 4/4 通过；真实 EXE + 独立库/profile + UIA + CDP；有取消、选择仅预览、回填和显式保存 |
| `node target/native-theme-assessment-20261030/run-android.mjs android-reviewed` | 调用已提交 Android 运行器，3/3 通过；独立测试包、真机列表、实际返回键/点击、预览、保存重开 |
| `node --check scripts/browser/rssr_theme_selector_windows.mjs` 及 Android 对应脚本 | 通过；UIA 助手已在实际运行中执行 |
| 文档链接/锚点/JSON 检查、`git diff --check` | 325 份 Markdown / 1502 个本地链接与锚点检查无新增错误；两份新 JSON 有效，diff 检查通过 |
| 本轮 Rust 产品代码测试 | 未重复执行；本轮产品树未改，PR #47 当前 head 已完成 CI；上一批 Rust/Web/Windows 的具体命令及范围保留在原交接 |

### 产物身份

- Windows EXE 复用上一批已核对的当前产品产物，SHA256：`dc05668809d3aad7a9e96079c0437ec24a3eabccc1228fbae5aea0c02fce0d3a`。
- Android 从 `git archive f38c3ba8` 的独立副本构建，只改测试包 identifier 和 Kotlin BuildConfig 包引用；未改工作树生产配置。APK SHA256：`f5b967ebed0e701a655cf9cd019f1f85a053d1a483382b92eccb8d7bb66e8add`。
- 构建命令：`dx build --locked --platform android --package rssr-app --target aarch64-linux-android --debug-symbols false`。进程环境指定现有 JDK 21、Android SDK、NDK `28.2.13676358` 和工作树 target；未改全局配置。
- Android `aapt dump badging` 核对 identifier 为 `io.github.develata.rssreader.backpolicytest`，Activity 为 `dev.dioxus.main.MainActivity`。`adb install -r -t` 成功，运行前再次核对安装包哈希。
- Android 最终 seed SHA256：`13a5449fa88b4fd9332670ca7bff4834c9ea740e906e825751ad48a487ebca0a`。两端均为 100 篇/6 个本机来源，长周期刷新 525600、合法归档月数 6。脚本哈希和输入方式在紧凑回执中。

### 调试失败与最终证据的区别

- JDK 26 的 `jlink` 转换失败，见 `android-build.log`；改用已安装 JDK 21 后构建成功，见 `android-build-jdk21.log`。这是构建环境问题，未修改产品绕过。
- Windows 前两次尝试被前台归属 guard / UIA SetFocus 拒绝，未向其他应用发送按键。最终脚本使用 CDP 按键，启动时仅显示本测试窗口而不激活；隐藏窗口导致空 UIA 树的尝试也保留为失败，未降低断言。
- Windows 早期保存失败来自旧夹具的归档月数 0，违反既有领域校验；改用合法值后保存通过。另一次后台 `Page.captureScreenshot` 超时；最终 Windows 断言使用 UIA 和实际存储，不宣称物理键盘或系统截图验收。
- Android 初版驱动遗漏设备的 `topResumedActivity=` 字段、对异步重开页面使用非空安全断言，以及一次 UIAutomator 快照失败，均分别留有失败目录。
- 准备 Android 夹具时，一次临时 SQL 更新遗漏 key 条件，误写了合成库的 `app_state_v2`；日志明确报 unknown field `theme`。已保留该错误 seed，重新复制原始夹具并只修改 `user_settings`，最终日志不再有该错误。这是本轮测试准备错误，不是产品数据缺陷；没有操作用户库。
- 最终通过目录是 `windows-reviewed-02/` 和 `android-reviewed/acceptance/`。前面的 `windows-*`、`android-*` 探索目录不能代替最终结果。没有复现到需要单列产品修复的行为变化。

## 保全与当前状态

- 安装前先停止测试包，保存原 APK 和完整 `files.tar`，校验 archive、解包及原库。原 APK SHA256：`256390897d0d6a7ba6219ad90457365175fd1883b5c50c3efe1433d432751366`；tar SHA256：`e2643d0b5103ca1acccf141b7d7eb11f77d15215cacd6e89bff2f38ffa836e6b`。
- `android-before/` 保留完整备份；33 个文件中，32 个保留数据文件与设备上的原目录/改名保全目录逐一散列一致。测试包重新安装/运行后 `profileInstalled` 由 Android profile installer 更新，其原文件仍在备份中；不把它写成全包数据完全不变。
- 每次测试把本测试包的上一份合成库保留为 `files/native-theme-<run>-before`，没有删除旧库。结束后停止测试包，只移除本轮 CDP forward；既有 3 条 forward 和 1 条 reverse 与开工记录一致。测试包保留本轮 APK 和最终合成库。
- 日常 `io.github.develata.rssreader` 未安装、启动、停止或修改数据；前后包路径/哈希一致，SHA256 `32111b52c2396e53cd7afba5578f2b0b9aa2b9ee96e6024a061b0b4d4ee6fa8e`。
- 主 checkout HEAD、40 条状态、index 哈希及 42 个文件/缺失路径与旧保全快照一致。见本地 `primary-preservation.json`。历史备份、Claude 配置、旧 C 盘目录未改。
- 仍为 4 个本地分支、2 个工作树、3 个远端分支；本轮没有增删分支，也没有向 PR #40/#46 追加实现。
- `git push origin HEAD:codex/shell-settings-lifecycle` 已推送评估提交 `c3dc5ddb`，随后 `git ls-remote --heads` 核对远端为同一完整 SHA。后续回执仍只更新该分支，不改 main，不发布。最终节点在会话交付中报告。

## Review、风险与后续事项

自查重点是运行器只进入指定隔离宿主、CDP/PID/包身份、截图/按键前台归属、脚本与产物散列、设置逐字段保存、失败回执和旧资料保留。未发现待修问题；这不是独立第三方 review。

尚未覆盖 Windows 物理键盘/IME/屏幕阅读器、Android 手势返回/TalkBack/其他设备、macOS/Linux GUI；Windows 外部回填测试是受控输入，Android 不承接该项结论。没有性能对比或正式包升级结论。启动刷新访问本机夹具失败未作为主题检查通过项。

下一步建议先为 Windows 独立原生设置面板的小样写出具体设计，明确主题选择、取消、保存与现有 session 的关系、窗口归属和 CSS 兼容，再决定是否实施。不能把本轮“现有 Android 弹窗是原生的”扩大成原生界面迁移已完成。

## 给下一位 Agent 的备注

先读[维护状态](../current-status.md)和[评估结论](../design/native-theme-selector-assessment.md)，再按[复跑入口](../testing/native-control-acceptance.md)核对当前设备、包、分支与证据。工作树名称仍是 `native-back-policy`，实际分支复用 `codex/shell-settings-lifecycle`；旧路径名称不表示返回策略尚未完成。
