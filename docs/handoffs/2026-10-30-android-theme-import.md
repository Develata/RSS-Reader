# PR #48 整合与 Android 主题文件导入

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/android-theme-import`
- 产品基线：`ef04ded9955699e1175b3904d470c2e9046edf9f`
- 当前产品提交：`bfa6fda12412133e334b3566f7a127c1a48d6a62`
- 相关 commit：BOM 修复 `58501e56cbaf44302f6be541d4bdbf96ef438bb5`；Android 适配 `bfa6fda12412133e334b3566f7a127c1a48d6a62`
- 远端评审：[PR #49](https://github.com/Develata/RSS-Reader/pull/49)，尚未合并；Checks 按该 PR 的当前 head 核对
- 相关 tag / release：无；未发布
- 状态：`validated`（本地和实际宿主）

## 工作摘要

按用户批准的推进顺序，先完成密度退役 / `.agents` 清理的 PR #48 整合，再接入 Android 系统主题文件导入。继续复用 Settings 现有导入身份、预览、逐字段保存与 `stored → submitted → pending`；不实现独立原生设置面板，不在本批删除 0.2.x 兼容字段。

## 影响范围

- 模块：`android/MainActivity.kt`、Settings `themes/theme_file_io`、私有 `theme_android_import` 及 JNI bridge。
- 平台：新增 Android OpenDocument 导入；桌面/Web 文件读写行为保持。另有独立的首 BOM 呈现修复，见下节；未更换 Dioxus/WebView。
- 验收工具：`scripts/android/theme-import/` 的仅测试包 DocumentsProvider，以及 `scripts/browser/rssr_theme_import_android.mjs`。不进入产品 manifest，不新增依赖、crate、数据库迁移或存储权限。
- 继续复用 `E:/gitclone/RSS-Reader-worktrees/native-back-policy`；主 checkout 的其他任务不在本批范围。

## 关键变更

- 宿主只负责选择、按实际字节限量读取和 JNI 交付；domain 的 64 KiB 上限传入宿主，JNI 复制前再次检查，UTF-8 严格解码复用原生路径，保留 BOM 与 CRLF。
- 普通 Rust 模块管理唯一 waiter 与不回绕的请求编号。忙碌、完成、取消和旧回调用事件序列测试；等待者 Drop 释放自己的槽位并通知宿主。
- ActivityResult 不携带调用编号，因此取消等待不提前释放物理选择器/读取占用；新请求报告忙碌，直到旧选择器或读取结束。阻塞的文档提供器不保证能被强制中断，避免谎报 I/O 已停止或并发启动更多读取。Activity 销毁结束其请求。
- Settings 原规则继续决定迟到结果能否更新草稿、提示和预览；导入不自动保存。取消与错误保持原草稿、已应用样式和存储。
- 系统 API 依据：[访问文档](https://developer.android.com/training/data-storage/shared/documents-files)、[Activity Result 生命周期](https://developer.android.com/training/basics/intents/result)。受控文档提供器按[官方测试所用接口](https://developer.android.com/guide/topics/providers/create-document-provider)注册，仅为验收提供错误和延迟。

## 验证与验收

- `cargo test -p rssr-app --locked`：391 通过、1 忽略，其中新增传输事件序列 4 项；既有导入身份、离页取消、CSS 原文与错误测试继续通过。
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- Android `cargo check`：补齐本进程 NDK CC / AR / linker 后通过；首次缺编译器路径失败保留在日志。既有 `LOCAL_DATA_DIR_NAME` 未使用警告未改。
- `dx build --locked --platform android --package rssr-app --target aarch64-linux-android --debug-symbols false --verbose`：通过，JDK 21 / NDK 28.2 / 独立全新 target；随后仅测试包加入 DocumentsProvider，再由 Gradle assembleDebug 通过。
- Android 15 真机 `caa0c283` / WebView `156.0.8078.47`，独立包实际验收 **10/10**：系统返回取消；UTF-8 / BOM / CRLF 原文导入仅预览；无效 UTF-8、65537 字节、无效 CSS、提供器读取失败均保持草稿/预览/存储；65536 字节可导入；后续编辑丢弃迟到结果；离页释放 waiter、物理读取仍忙碌、完成后重试成功；明确保存后原文入库并重开。`android-05/acceptance/acceptance.json` 是最终回执。
- Windows 实际 WebView2 `156.0.4314.50`，首 BOM 呈现及字符串传输/保存/重开 **13/13**；0 页面异常、0 console error。EXE SHA256 `7b2d2bd0ac8ffe9522e6bc3db2e61ff3d7b341624bec22dd8d27e95add7f7f47`，隔离 5000 篇 / 6 localhost 来源，回执 `windows-bom-01/acceptance-03/acceptance.json`。输入为 CDP，不代表物理键盘/IME。

### 独立发现的既有缺陷与脚本调整

- Android 首次导入 BOM CSS 后，已应用 style 保留原文但第一条规则不生效；旧 PR #48 产品 APK 可在原有手动预览入口复现。单独修正 `<style>` 的开头 BOM 呈现，不改变存储原文，见[复现与回归](./2026-10-30-css-leading-bom-render.md)，不把它称为等价提取。
- `android-01` 在 DocumentsUI 默认大网格中未找到屏幕外的文件；脚本改为实际列表视图。`android-02` 启动命中上次失败留下的选择器任务，无测试进程；已验证其 `launchedFromPackage` 并关闭，未发现应用崩溃证据。脚本现在在失败收尾时关闭属于本次测试包的选择器。
- `android-03/04` 保留 BOM 失效及排除等待不足的证据；最终包重新构建。Windows 首轮的 8 项呈现/原文检查通过后，旧“至少两批以 BOM 开头”的传输探针失效；现在明确验证连续 BOM 用例仍经过真实字符串表。第二次复跑因仍在设置页未满足启动前提退出，回首页后第三次 13/13；这些都保留失败回执，不修改产品状态绕过断言。

## 产物与证据

紧凑结果见[本批回执](../testing/evidence/2026-10-30-android-theme-import/receipt.json)；BOM 复现与修正另有[独立回执](../testing/evidence/2026-10-30-css-leading-bom-render/receipt.json)。

- 完整证据：`target/android-theme-import-20261030/`；最终产品基线加 `final-source.patch` SHA256 `750d754c1399cc56d08e08ac4c7bddbcaffc64f19d427d54d524be0df72a277e`。补丁快照含当时文档；最终产品身份另以逐文件清单核对，不把后续文档更新写成重新构建。
- `final-product-files.json` 核对 458 个产品输入；生成 MainActivity 只替换测试包 BuildConfig 引用；`.d` 指向 `android-source-final`，使用全新 `android-target-final`。
- 最终不带提供器的测试 APK SHA256：`558449cd20b52f0c00088d7fa6e848418245b24f719739c568f45f8dcd0629ea`；带提供器的验收 APK：`80723aa721f43084a06338edc5e4e85b1e03c20dc9f07581125160b538116352`。二者与构建输出的 `libmain.so` 均为 `e5a7860d1027404842d3671ef71e76999a5d86a221754041509d7bd66e02e320`；fixture 仅注册在验收包 manifest。
- 独立包旧 APK / files 已备份校验；所有原资料库文件完成恢复和逐文件哈希核对，原资料库 SHA256 仍为 `13a5449fa88b4fd9332670ca7bff4834c9ea740e906e825751ad48a487ebca0a`。日常包 SHA256 仍为 `32111b52c2396e53cd7afba5578f2b0b9aa2b9ee96e6024a061b0b4d4ee6fa8e`，原 forward/reverse 不变；仅测试包保留新 APK 和测试证据目录。
- 主 checkout 原 40 条状态、index、42 个文件/缺失路径一致；另一任务新增的 JavaScript 审计交接也保留，未纳入提交。
- PR #48 head 和合并 main 的 CI 均 36/36；main 的 Pages / Docker workflow 成功。这里只证明整合基线，不替代本批代码或容器手工验收。新分支的远端检查以 GitHub 当前 head 为准。

## 结果、风险与后续

实现、源码复核、普通 Rust 和实际宿主验收已完成，未发现未处理的阻塞项；两笔产品提交均已推送至 `origin/codex/android-theme-import`，PR #49 已建立。后续回执只更新文档，不改变本轮验收的产品输入。本批不宣称正式签名升级、真实云端文档提供器、Activity 重建/进程回收全生命周期、系统 IME / 辅助技术、macOS/Linux GUI 或性能验收；Web GUI 未本地重跑。测试提供器的错误/延迟是受控故障，真实系统选择器与 JNI 确有运行。

独立原生设置面板继续只作为设计候选；0.3.x 密度参数删除另按[维护待办](../maintenance-backlog.md)推进。当前长期规则见[原生适配契约](../design/native-control-adapter-contracts.md)。
