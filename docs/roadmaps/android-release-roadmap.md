# Android 构建与验收状态

这份文档记录 Android 交付链已经做到哪里，以及仍需真机确认什么。历史签名工作见 [2026-07-27 记录](../handoffs/archive/2026-07/2026-07-27-android-release-signing.md)；当前发布证据见 [v0.2.1 交接记录](../handoffs/2026-10-28-v0.2.1-release.md)。主线的新改动与正式发布分开记录，见[维护状态](../current-status.md)。

## 当前发布状态（2026-10-28，v0.2.1 `dbfbf896`）

| 能力 | 状态 | 证据边界 |
| --- | --- | --- |
| 共享 Rust 订阅 / 阅读逻辑 | 已实现 | UI、application 与其他端复用；平台差异留在 adapter / host capability |
| Android debug 构建与自动化 smoke | 已接入 | CI 检查 ARM64 构建、APK 资源、ABI 与版本信息 |
| 正式签名 APK / AAB | v0.2.1 已发布 | versionName 0.2.1 / versionCode 201；APK 与 AAB 验签通过，证书与 v0.2.0 相同，公开附件与 CI 文件 SHA256 相符 |
| 正式 APK 覆盖安装与初始化 | Android 16 隔离模拟器通过 | v0.2.0 → v0.2.1 未卸载覆盖安装、appId / 首次安装时间保留；新持久目录初始化及预置 Windows v0.2.0 单库夹具对照通过。ARM64 使用模拟器 native bridge；不等于真实旧 Android 数据搬迁或真机签名升级 |
| 真机交互 | 2026-10-28 当前产品代码的 28 项专项通过 | Xiaomi 12 / Android 15，含系统返回、写入与筛选恢复；另补冷进程偏好保留。仅适用于记录的设备、`67e1c2c2` debug 签名包与场景，见[本轮验收](../handoffs/2026-10-28-release-readiness.md) |

2026-10-25 补充：小米 12 / Android 15 已完成系统返回与边缘手势、弹层关闭顺序、后台恢复，以及缓存目录迁入持久目录后的数据对照与重启检查，见[修复交接](../handoffs/archive/2026-10/2026-10-25-android-back-and-persistent-storage.md)。2026-10-26 又完成目录和阅读返回专项，见[原生目录记录](../handoffs/archive/2026-10/2026-10-26-native-directory-validation.md)。这些结果不替代最新候选、其他设备或正式签名包的验收。

打 tag 和手动触发 Release 均要求 Android signing secrets 齐全，否则 `build-android` 失败；两种入口都会发布，不能将手动入口当作 debug 试跑。临时 debug 验证使用下面的本地构建流程或对应 CI。正式签名包与临时 debug 签名包不能互相覆盖安装。卸载会清除本地订阅、文章和图片缓存；卸载前先导出配置，但配置导出本身不包含文章库和已读状态。`versionCode` / `versionName` 从发布 tag 派生，不读取 workspace `Cargo.toml` 的版本号。

## 本地构建 debug APK

需要 Rust Android target、JDK 21、Android SDK（platform 34、build-tools 34.0.0）、NDK 27.3.13750724 与 platform-tools，以及 Dioxus CLI `0.7.9`。请先按本机 Android SDK 安装位置配置 `JAVA_HOME`、`ANDROID_SDK_ROOT` 与 `ANDROID_NDK_HOME`，再运行：

```bash
rustup target add aarch64-linux-android
cargo check --locked -p rssr-app --target aarch64-linux-android
dx bundle --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false
python3 scripts/prepare_android_bundle.py target/dx/rssr-app/release/android/app/app/src/main
(cd target/dx/rssr-app/release/android/app && ./gradlew assembleDebug --no-daemon --console=plain)
```

输出位于 `target/dx/rssr-app/release/android/app/app/build/outputs/apk/debug/app-debug.apk`。`prepare_android_bundle.py` 修改生成的 Android 工程中的图标、应用名与 SDK 配置；它之后必须重新执行 Gradle 打包，才能把修改写进 APK。本地 debug 包由本地调试签名，不等于 GitHub Release 的长期签名包。

2026-10-28 真机专项实际使用 `dx build` 生成工程、NDK 28.2.13676358，再执行上述准备脚本和 `assembleDebug`；这是测试包路径。v0.2.1 正式工作流已另行通过，正式产物身份、模拟器验收和真机 debug 专项分别记录，不能互相代替覆盖范围。

正式发布的签名 secret 名称及构建断言以 [release workflow](../../.github/workflows/release.yml) 为准；不要把 keystore、密码或签名输出写入仓库。

## 待完成的设备验收

本轮已验证 Xiaomi 12 上的目录、系统返回、文章写入及冷进程偏好保留；完整范围见上方专项记录。后续使用**实际发布候选的包**，在隔离测试数据上补充：

1. 正式签名包在真机上的覆盖升级、首次启动、SQLite 数据路径与应用名 / 图标；v0.2.1 模拟器覆盖安装已通过，但真实旧 Android 数据搬迁与真机签名升级仍未验收。
2. 真实远端订阅的添加与刷新、后台恢复及数据保留；核对实际发布包仍符合本轮已验证的阅读与写入行为。
3. Reader 图片查看器的系统返回关闭顺序、正文长按选择，以及文章首页下拉刷新只在顶部触发。
4. 360px 级窄视口来源名、搜索、分页、safe area、图片点击放大与正文选择手柄。
5. JSON / OPML、主题文件 SAF 导出、WebDAV（CSS 文件导入当前未实现），以及失败重试和权限拒绝路径。

记录设备型号、Android 版本、APK 来源和签名身份，以及实际操作结果。当前 [手工回归清单](../testing/manual-regression.md) 与 [主线验证矩阵](../testing/mainline-validation-matrix.md) 提供通用路径；已有专项通过不等于上述未覆盖场景、其他设备或正式包已完成验收。
