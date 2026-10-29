# 原生交互、Penpot 抽查与补丁发布准备

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`codex/docs-release-readiness-20261028`
- 构建源码：`67e1c2c299cf942ae972f1c5cab10d7f10bb98fe`
- 产品代码：与 main `a9608056e23864446a69881e38676598e56869d7` 相同；差异仅为文档
- 相关 commit：本记录所在提交；提交前为 `commit: pending`
- 相关 tag / release：已发布 v0.2.0；建议下一版 v0.2.1，尚未打 tag 或发布
- 状态：`validated`（以下明确范围）；正式发布产物待构建与验证

## 工作摘要

用户批准原生验收、设计抽查及补丁发布准备，并要求先整理 docs。文档维护已独立提交 `67e1c2c2`，见[整理记录](./2026-10-28-docs-maintenance.md)。本轮没有修改产品代码，补齐实际 Windows / Android 宿主证据、Penpot 代表画板复核和 v0.2.0 升级检查。

## 影响范围

- 模块：docs 当前状态、设计基准、Android 路线、发布草稿及验收证据。
- 平台：Windows WebView2、Xiaomi 12 / Android 15 API 35 / WebView 152；Penpot 当前代码文件。
- 额外影响：手机测试包覆盖安装；经用户明确授权新增独立单库。未清数据、未卸载、未修改旧双库、未使用用户日常 Windows 资料库。

## 关键变更与产物身份

- Windows：`cargo build -p rssr-app --release --locked`，单构建任务；独立程序副本与新建测试库。SHA256 `c2b972dbbb500a91b8fc4dcde7e0146d6f44b843b5376cdd26ad1b60c8e47877`。
- 夹具：同源码 `cargo build -p rssr-infra --example browser_fixture_library --release --locked`；`seed_reading_native.py` 生成 6 个来源、5000 篇文章和 24 篇长文，每篇长文 120 段、4 张内嵌图片。UI 依当前查询契约只展示最新 2000 篇，不能把它误写为数据丢失。
- Android：Dioxus CLI 0.7.9、JDK 21、NDK 28.2.13676358，`dx build --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false`；运行 `prepare_android_bundle.py` 后在生成工程执行 `gradlew.bat assembleDebug --no-daemon --console=plain`，全部成功。本轮用 `dx build` 生成测试工程；正式 workflow 使用 `dx bundle`，本轮不替代该发布路径的验收。
- APK SHA256 `32111b52c2396e53cd7afba5578f2b0b9aa2b9ee96e6024a061b0b4d4ee6fa8e`；安装助手实际回读的包哈希相同，新旧测试包证书一致。它是 Rust release + Android debug 外壳，versionName 0.1.0 / versionCode 1，**不是正式 v0.2.1 签名包**。
- 构建存在 Android 未使用常量及 Gradle 弃用提示，退出码均为 0；未为消除非阻断提示扩大本轮代码范围。
- 完整身份见 [artifacts.json](../testing/evidence/2026-10-28-release-readiness/artifacts.json)。

## 验证与验收

### Windows 实际宿主

- 本地 `native_acceptance.cjs --platform windows --cdp http://127.0.0.1:9261 ...`：**27 项通过**；[原始结果](../testing/evidence/2026-10-28-release-readiness/windows.json)。
- 使用真实 SQLite `BEGIN IMMEDIATE` 短暂阻塞写入，在列表已读、收藏、批量已读及 Reader 收藏写入后立即离页，再释放锁；原始结果以 `storageHeld` 记录这些场景，检查对应状态和返回范围。另检查工作区偏好保存后离页、返回的状态；该项原始结果未记录持锁断言，不计入受控延迟证据。不是 HTTP mock 或浏览器替代宿主。
- 目录实测：父级 6px、分支 5px、展开按钮靠右、文章缩进 8px、上下内边距 4px、点击高度至少 44px；大分支挂载数量有界，键盘展开正常。
- 三次打开缓存长文、图片加载、滚动及返回通过；位置断言包含 `0 → 0` 和 `650 → 650`，其中后者证明一次非零会话内位置恢复。无 uncaught WebView 错误。
- M/F 快捷键通过；另检查 Ctrl+M 不改变已读状态，见 [M 补充记录](../testing/evidence/2026-10-28-release-readiness/windows-keyboard.json)。
- 关闭并重新启动同一测试程序后，来源分组、未读筛选、标题搜索及结果范围保留：[重启前](../testing/evidence/2026-10-28-release-readiness/windows-prefs-before.json)、[重启后](../testing/evidence/2026-10-28-release-readiness/windows-prefs-after.json)。这不表示跨进程阅读位置持久化。

### Android 实机

- 本地 `native_acceptance.cjs --platform android --serial caa0c283 --cdp http://127.0.0.1:9262 ...`：**28 项通过**；[原始结果](../testing/evidence/2026-10-28-release-readiness/android.json)。
- CDP 触摸作用于实际 WebView；目录和来源弹窗用 Android 系统 Back 关闭，连续三次阅读返回不退出应用，文章及偏好写入、目录尺寸和长文位置恢复通过。Android 未人为延迟 SQLite 写入，不能等同于 Windows 的受控延迟场景。
- 冷进程重启保留分组、未读筛选、搜索和结果范围：[重启前](../testing/evidence/2026-10-28-release-readiness/android-prefs-before.json)、[重启后](../testing/evidence/2026-10-28-release-readiness/android-prefs-after.json)。该组合合法地返回空结果，核对的是重启前后同一范围。
- 已实际查看 ADB 物理屏幕截图；手机系统字体与桌面/Penpot 不同，不据此声称逐像素相等。
- 旧测试库含未发布 schema 0008，当前正式导入器明确拒绝此类开发库。用户明确回复“旧库不管了，你用新库就行了”；因此保留旧文件，新增由当前夹具生成的 `library.sqlite3`，没有放宽导入器或伪造迁移历史。
- 前后完整备份核对：**4 个旧库文件及 8 个 WAL/SHM，共 12 个文件字节一致**；新库 `integrity_check=ok`、6 来源 / 5000 文章 / 5000 正文，见 [数据记录](../testing/evidence/2026-10-28-release-readiness/android-data-retention.json)。

### 正式 v0.2.0 到当前候选的 Windows 升级

- 下载公开 Windows ZIP，SHA256 与 GitHub Release digest 一致；用实际 v0.2.0 程序创建单库，再向隔离库加入测试文章，并确认旧程序可读取。
- 停止旧程序后，以候选程序打开同一目录：新增 `is_archived` 字段，旧文章均未归档；5000 篇文章的全部旧字段、5000 篇正文及已读/收藏标记保留，完整性正常。
- 6 个订阅的身份及用户字段保留；启动刷新改变 `last_fetched_at`、`refresh_issued`、`refresh_diagnosed`、`updated_at`，原始差异保留在 [升级结果](../testing/evidence/2026-10-28-release-readiness/windows-upgrade.json)。没有将整表说成字节完全一致。
- 旧库设置表为空，因此本项不覆盖定制 v0.2.0 设置升级；两端本次保存偏好的重启验证另列于上文。

### Penpot

- 以新 MCP 会话读到正确文件 `cadc1646-c70c-8009-8008-d6f40c2d8fdc`，rev 370；55 张基准画板、55 个唯一 capture-key、16 张右侧箭头基准画板保留，`currentFile.validate()` 无错误。
- 已回读命名版本 `Right-side directory disclosure · badf7fae · 2026-10-27`。当前代码的目录尺寸变量与该版本的字面值等价，原生实测再次验证。
- 对一个桌面画板量测 4 个代表箭头、6 个文章行；缩进 8px、最小高度 44px，箭头尺寸标注 6px / 5px、右侧中心对齐。导出并查看桌面时间目录及手机来源目录；未发现本轮范围内需要修改的偏差。
- 后续全量尺寸请求遇到插件休眠，未重新执行全部 16 张画板的几何遍历。本次是代表抽查，不冒充全画板逐像素审计；未改设计内容。[结构与量测结果](../testing/evidence/2026-10-28-release-readiness/penpot.json)。

### 测试工具与边界

- 初期本地 harness 失败涉及旧的 5000 篇显示假设、原生 dialog 选择器、恢复滚动后的触摸命中、已滚动正文的手势坐标，以及重复运行前的阅读位置。按实际 DOM 与初始状态修正后再验，原失败记录保留在本机。
- 未重复运行完整 workspace / Web 套件：产品源码与已通过 PR #33 / main CI 的 a9608056 相同，见[合并记录](./2026-10-27-workspace-merge.md)。本轮补充原生覆盖，不把历史测试数量归入本轮执行。
- 自动审批曾拒绝组合启动命令，未给出具体原因；拆分为明确的夹具生成、复制身份核对和独立启动后通过，没有修改审批设置。
- 未覆盖真实系统 IME、Windows 前台性能、其他 Android 设备及 macOS/Linux 实机操作。

### 文档复核

- 本地 `docs_maintenance.py audit --label after`：311 份 Markdown、1244 个本地链接，缺失目标 0；`verify_archive.py`：147 份归档仅调整链接与历史标识，内容差异 0，缺失标题锚点 0。
- 11 份 JSON 均可解析，所有声明的验收状态为通过；`git diff --check` 通过。相对 main `a9608056` 的产品代码、构建脚本和 workflow 无差异。
- Claude 通过只读工具核对本轮文档和 11 份 JSON，结论为“没有阻断问题，这个文档 PR 可以交付”；会话 `a4ae8cec-3c97-45be-bad4-5b7f66c3b487`。
- 按建议补充升级后回退须恢复备份的限制，更新 Android 待验项与 Penpot 基准日期，区分 `dx build` 测试路径和正式 `dx bundle`，并收紧受控写入、滚动恢复及旧文件数量的证据表述。
- 此复核不代表 Claude 重新操作设备、验证在线 CI 或批准正式发布；最后上述文案调整由 Codex 校对。

## 结果与发布边界

本轮文档维护、已列原生交互、设计抽查与发布准备完成，未发现要求修改产品源码的新缺陷。[v0.2.1 发布草稿](../releases/v0.2.1-draft.md)已整理。

正式 APK/AAB 签名、versionCode 201、各平台发布包和最终 tag 源码身份仍须在获准发布后重新校验。Release 的手动入口同样会公开发布，不能当作无副作用的预检；本轮没有触发它。

## 给下一位 Agent 的备注

- 本机完整原始证据：`E:/gitclone/RSS-Reader/target/native-readiness-20261028/`；Penpot 与归档审计：`E:/gitclone/RSS-Reader/target/docs-readiness-20261028/`。
- 仓库保留上述 11 份精简 JSON。Windows 与 Android 测试包哈希不可替代未来正式发布包；新 tag 的构建标识也需重新确认。
- Windows 测试实例结束后关闭；手机保留已核验测试包与新库，旧双库及备份仍可查。
