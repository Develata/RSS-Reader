# Android 系统返回与持久数据目录修复

- 日期：2026-10-25（America/New_York；手机日志使用 UTC+08）
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 修复基线 HEAD：`f5034965`
- 相关 commit：修复与本记录同次提交；提交前 `commit: pending`，最终提交可由本文件的 Git 历史定位。
- 推送授权：用户明确要求“修复以后再推送”；目标 `origin/feat/reading-workspace-20261024`，不合并 main、不打 tag、不发布。
- 状态：`validated`（本轮修复范围；不是完整 Android 发布验收）

## 工作摘要

修复上一轮实际手机检查确认的两个问题：阅读页系统返回直接退出，以及索引库、正文库和界面偏好落在可被回收的缓存目录。Android 外壳只转发输入，Rust 继续控制弹层与路由；存储位置与迁移留在 infra，没有改变 domain / application 的统一语义、数据库 schema 或 Windows/Web 的数据路径。

## 影响范围

- Android 外壳 `android/MainActivity.kt`、Rust `use_mobile_back_navigation` 及其 `android_bridge`。
- infra `sqlite_native::android_storage`、Android 专用 JNI/NDK 依赖声明；复用锁文件已有版本，没有新增 crate 或更新第三方版本。
- 持久化升级测试的 Windows 文件清理时序；用户指南和前端交互契约。
- 设备：小米 12 / Android 15 / HyperOS `OS3.0.3.0.VLCCNXM`；包名 `io.github.develata.rssreader`。

## 关键变更

### 系统返回

- 原 Tao `KeyboardInput` / `CloseRequested` hook 不能可靠收到当前 Wry Android Activity 的系统返回。新实现由 AndroidX `OnBackPressedDispatcher` 经 JNI 转发到容量 1 的 Rust 输入队列；关闭 Wry 的 WebView 历史返回分支。
- Rust 页面任务先处理图片、来源筛选、目录和来源编辑的 dismiss，再调用现有 history / fallback 规则。Java 线程不接触 Dioxus 状态。
- 已有阅读位置采集仍在导航前执行；重复输入合并，旧页面清理用监听代次校验，不会注销新页面。
- 根页无可返回目标时 `moveTaskToBack`，不销毁 Activity；移除此前隐藏窗口后延迟重新显示/聚焦的补偿逻辑。

### 持久目录迁移

- 使用 Android `Context.getFilesDir()/RSS-Reader`；旧 HOME / temp 回退仅用于寻找升级前目录。
- 在任何数据库或 `shell-prefs.json` 打开前完成初始化；进程内锁与文件锁串行化启动。完成后的目录在进程内缓存。
- 复制两个数据库、WAL、界面偏好及其他常规文件，跳过可重建的 SQLite SHM 与迁移锁。拒绝符号链接和非常规文件。
- 源文件前后与副本均做 SHA256 对照；文件刷盘、写入完成标记，再将整个暂存目录原子重命名到目标并刷盘父目录。旧目录不写入、不删除。
- 目标存在但无完成标记时停止，不覆盖另一份库或显示空库。只有旧 HOME 本来就是同一个持久目录时才原位接纳。
- 中断暂存不当作有效库，重试重新建立完整副本；完成标记使后续启动只读新目录，避免旧缓存覆盖新数据。中断暂存副本保留，未自动批量删除。
- Windows 持久化测试曾在数据断言全部通过后因 `remove_dir_all` 遇到错误 32 失败。仅对短暂 sharing/lock violation 做最多 2 秒重试，其他错误和超时仍失败；未放宽数据断言。

## 验证与验收

### 自动化检查

- `cargo check -p rssr-app --offline`：通过；锁文件仅增加 infra 对已存在 JNI/NDK 包的引用。
- `cargo test -p rssr-infra --locked android_storage`：6 项通过，覆盖双库未 checkpoint 的 WAL 恢复、原始文件保留、冲突目标、中断暂存、并发发布、新装、已有持久 HOME 与缺失索引库保护。Unix 符号链接单测在 Windows 本机未运行。
- `cargo test -p rssr-app --locked android_bridge`：2 项通过，覆盖旧页面迟到清理和忙碌输入合并。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过；清理测试调整后额外跑该测试目标的 Clippy，通过。
- `cargo test --workspace --locked`：最终 **432 passed / 0 failed / 2 ignored**；2 项为明确标记的性能探针。第一次运行仅上述 Windows 测试清理失败，修正后完整重跑通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`、`cargo fmt --all --check` 和本次 `git diff --check`：通过。
- `dx build --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false`：使用 JDK 21 编译及打包通过。既有 dx 0.7.10 / Dioxus 0.7.9 版本提示仍存在。

### 精确测试包与打包边界

- APK：`target/android-repair-20261025/rssr-repair-debug.apk`。
- SHA256：`4D2402738646F6F43E883801C111F924FE3ACAA1C7F6AD0B5E184B3E7E64DD39`。
- ABI `arm64-v8a`、minSdk 24、targetSdk 34、versionName 0.1.0；可调试外壳、Rust release 优化，不是正式发布包。
- 签名证书 SHA256：`0672a3442f32db36b7ea98aa9204b67b5dc540ecdaa47a1d64c0182845be450a`，与上一轮已安装包一致；`adb install -r` 成功，没有卸载或清数据。
- 自动审批拒绝了 `python scripts/prepare_android_bundle.py ...`，仅给出“需要批准”，当前会话无法发起批准。未换执行器或改写资源来绕过；直接验收 dx 已经产生的同包名、同签名 APK。因此测试包显示名为 **RssrApp**、使用脚手架图标；正式资源准备在本机未验证，原资源脚本及发布流程未修改。

### 真实手机检查

- 14 项 UI 检查通过：迁移后列表偏好/搜索/状态/名称、完整正文、阅读页系统返回、停止滚动后的阅读位置恢复、图片查看器/来源筛选/目录/来源编辑关闭优先级、编辑取消不落盘、订阅页返回、根页退后台及三次正常图标启动恢复、冷启动、新偏好不被旧缓存覆盖，以及 Android 边缘返回手势。
- 实际日志确认两个数据库从 `/data/user/0/io.github.develata.rssreader/files/RSS-Reader/` 打开；原 cache 目录保留。
- 安装前停止应用并备份旧目录；升级后再次停止应用，读取新目录副本做全量业务字段对照：**6 个订阅、5000 篇文章、5000 条正文、所有阅读/收藏/归档状态、设置和搜索偏好一致**。忽略自动刷新时间等运行记录，没有忽略文章内容或状态位。
- 新旧库各自 `PRAGMA integrity_check` 均为 `ok`；原缓存目录文件与安装前归档逐字节 SHA256 一致。
- 将新目录搜索词改为 `04994`，旧缓存仍为 `05000`；重启后恢复 `04994`，证明未重新导入旧缓存。验收后恢复测试搜索词 `05000`。
- 没有清除真实手机缓存来做破坏性测试；删除旧缓存后的可用性由隔离目录测试覆盖。

## 结果与风险

- 上一轮两个 P1 的正常使用路径已修复并在实际手机通过。本轮可提交并按用户授权推送；没有把局部检查描述成正式发布或所有 Android 场景验收。
- 非正常调试入口仍有上游限制：混用裸 `am start -n` 和 launcher Intent 曾创建第二个 Activity，触发 `ndk-context` 重复初始化断言。本次未增加多 Activity 支持；冷启动/恢复统一使用下述 MAIN/LAUNCHER 参数。正常 launcher 任务复用连续三轮进程不变、页面仍可操作。
- Android WebView 的 `document.hidden` 没有反映任务退后台状态，因此后台验收使用实际前台 Activity、PID 和恢复后的 UI，未把该 JS 值当作系统可见性依据。
- 未做系统强制清缓存、断电、损坏存储、正式签名升级、多机型或所有历史版本迁移；SQLite WAL 恢复和中断重试有隔离测试。
- 未改同步/凭据语义，未执行远端 WebDAV 或完整主题 SAF 验收。生产目录迁移不回收保留副本，也不自动合并降级旧版后在缓存里产生的新修改。
- 任务外 `.agents/.specify` 删除、旧交接修改和未跟踪设计文件保留，不纳入本次提交。

## 给下一位 Agent 的备注

- 本轮原始证据：`target/android-repair-20261025/`，包括 `before-cache.tar`、`after-files.tar`、`retained-cache.tar`、`migration-receipt.json`、`ui-receipt.json`、`root-back-resume.json` 和进程日志。归档含完整测试数据库，不提交到 Git。
- 旧归档 SHA256：`bb16dd92893e492d5aac1ea267039f5f32653e69bfbcca587ede57163ba57e05`；迁移后归档 SHA256：`66d4f2112e39cfe604b41df3339488feab9608e9bd66b12b43669f9eab832104`。
- 正常启动/恢复：`adb -s caa0c283 shell am start -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -f 0x10200000 -n io.github.develata.rssreader/dev.dioxus.main.MainActivity`。不要在存活进程中混用不同根 Intent。
- 旧 PID 32048 的混合启动探测日志有上游断言；正常启动与三轮恢复使用 PID 714，后续冷启动使用 7583 / 8615。比较日志时不要混淆进程。
- CI 当前仅在 main push、pull_request 或手动 dispatch 运行；本次新分支 push 本身不代表 CI 已跑过。
