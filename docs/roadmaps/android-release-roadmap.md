# Android 构建与验收状态

这份文档记录 Android 交付链已经做到哪里，以及仍需真机确认什么。历史分阶段工作见 [`2026-07-27-android-release-signing.md`](../handoffs/2026-07-27-android-release-signing.md)；当前发布证据见 [`v0.1.15` 交接记录](../handoffs/2026-09-22-v0.1.15-release.md)。

## 当前状态（2026-09-23）

| 能力 | 状态 | 证据边界 |
| --- | --- | --- |
| 共享 Rust 订阅 / 阅读逻辑 | 已实现 | UI、application 与其他端复用；平台差异留在 adapter / host capability |
| Android debug 构建与自动化 smoke | 已接入 | CI 检查 ARM64 构建、APK 资源、ABI 与版本信息 |
| 正式签名 APK / AAB | 已发布 | `v0.1.15` 含 `RSS-Reader-android-arm64-v8a-release.apk` 和 `.aab`；签名和 tag 版本在发布 workflow 验证 |
| 真机交互 | 待验收 | 构建与浏览器触摸模拟不能证明 Android 系统返回、长按选择、下拉刷新或图片缩放 |

打 tag 的发布流程要求 Android signing secrets 齐全，否则 `build-android` 失败；手动 workflow dispatch 可用于不带长期签名的 debug 验证。正式签名包与临时 debug 签名包不能互相覆盖安装。卸载会清除本地订阅、文章和图片缓存；卸载前先导出配置，但配置导出本身不包含文章库和已读状态。`versionCode` / `versionName` 从发布 tag 派生，不读取 workspace `Cargo.toml` 的版本号。

## 本地构建 debug APK

需要 Rust Android target、JDK 21、Android SDK / NDK、platform tools、Gradle 所需 Android 平台与 build-tools，以及 Dioxus CLI `0.7.9`。请先按本机 Android SDK 安装位置配置 `JAVA_HOME`、`ANDROID_SDK_ROOT` 与 `ANDROID_NDK_HOME`，再运行：

```bash
rustup target add aarch64-linux-android
cargo check --locked -p rssr-app --target aarch64-linux-android
dx bundle --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false
python3 scripts/prepare_android_bundle.py target/dx/rssr-app/release/android/app/app/src/main
(cd target/dx/rssr-app/release/android/app && ./gradlew assembleDebug --no-daemon --console=plain)
```

输出位于 `target/dx/rssr-app/release/android/app/app/build/outputs/apk/debug/app-debug.apk`。`prepare_android_bundle.py` 修改生成的 Android 工程中的图标、应用名与 SDK 配置；它之后必须重新执行 Gradle 打包，才能把修改写进 APK。本地 debug 包由本地调试签名，不等于 GitHub Release 的长期签名包。

正式发布的签名 secret 名称及构建断言以 [release workflow](../../.github/workflows/release.yml) 为准；不要把 keystore、密码或签名输出写入仓库。

## 待完成的设备验收

获得 Android 真机后，使用**本次待验源码构建的包**或明确版本的 Release APK，在隔离测试数据上验证：

1. 安装 / 升级、首次启动、SQLite 数据路径与应用名 / 图标。
2. 添加订阅、刷新、阅读、已读 / 收藏、切后台再恢复及进程重启后的数据保留。
3. 系统返回键与 Reader 图片查看器的关闭顺序；文章首页下拉刷新只在顶部触发，且不会影响正文长按选择。
4. 360px 级窄视口来源名、搜索、分页、safe area、图片点击放大与正文选择手柄。
5. JSON / OPML、主题文件导入导出、WebDAV，以及失败重试和权限拒绝路径。

记录设备型号、Android 版本、APK 来源和签名身份，以及实际操作结果。当前 [手工回归清单](../testing/manual-regression.md) 与 [主线验证矩阵](../testing/mainline-validation-matrix.md) 提供通用路径；**没有真机结果前，不应把 Android 标为完整体验验收通过**。
