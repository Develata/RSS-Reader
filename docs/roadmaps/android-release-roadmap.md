# Android 安装包落地清单

## 目标

为 `rssr-app` 增加 Android 交付能力，分阶段实现：

1. 本地可构建 Debug APK
2. 真机/模拟器可启动并完成核心回归
3. GitHub Actions 可自动构建 Release APK / AAB
4. GitHub Release 可附带 Android 安装包资产

当前状态：
- 共享 Rust 业务逻辑已存在，可复用
- Desktop / Web / CLI 已完成
- Android Debug APK 已可本地构建，且仓库已具备 Android target smoke check
- GitHub Release 已能发布 Android debug APK
- Android 正式签名 APK / AAB 仍依赖 keystore secrets 与最终真机验收

## 当前缺口

### 代码与工程

- 仓库中没有长期维护的 Android 宿主工程目录
  - 当前主要依赖 Dioxus 在打包阶段生成 Android 工程骨架
  - 这降低了仓库复杂度，但也意味着对生成模板和打包链的稳定性更敏感
- 缺少 Android 平台入口与 Dioxus 移动端接线
- 缺少 Android 数据库默认路径策略验证
- 缺少 Android 文件导入导出与系统分享/文件选择方案

### 发布与签名

- 缺少 APK / AAB 构建步骤
- 缺少 keystore 管理方案
- 缺少 GitHub Secrets 约定
- 缺少 Android Release workflow
- 缺少 Android 产物上传到 GitHub Release 的逻辑

### 验证与文档

- 缺少 Android 手工回归清单
- 缺少 Android 环境准备说明
- 缺少 Android 已知限制说明

## 分阶段实施

### Phase 1：跑通 Android Debug 构建

目标：
- 产出可安装的 Debug APK
- App 至少能启动到首页

任务：
- 新建 Android 宿主工程目录
- 接入 Dioxus Android 平台构建链
- 配置 Rust Android target
  - `aarch64-linux-android`
  - 如需要再补 `x86_64-linux-android` 供模拟器使用
- 配置 NDK / SDK / Gradle 构建依赖
- 确认 `rssr-app` 在 Android target 下可编译
- 确认应用启动不因本地数据库路径失败

完成标准：
- 本地能成功构建 Debug APK
- APK 能安装到模拟器或真机
- 启动后不崩溃

当前进度：
- 已完成

### Phase 2：补齐 Android 运行时适配

目标：
- 核心使用路径在 Android 上可用

任务：
- 验证 SQLite 在 Android 沙箱中的默认路径
- 适配文件导入导出
  - JSON
  - OPML
  - CSS 主题文件
- 评估 WebDAV 在 Android 网络权限下的表现
- 检查移动端布局
  - 导航
  - 阅读页
  - 设置页
  - 主题页交互
- 检查触控交互与滚动体验
- 检查生命周期恢复
  - 应用切后台再恢复
  - 进程重启后本地数据保留

完成标准：
- 可完成订阅、刷新、阅读、主题切换、配置导入导出
- 小屏交互无明显阻塞

### Phase 3：建立 Android 手工回归

目标：
- 有一套独立于桌面/Web 的 Android 验收流程

任务：
- 编写 Android 手工回归清单
- 覆盖以下路径：
  - 启动与首次初始化
  - 添加/删除订阅
  - 刷新与阅读
  - 已读/收藏
  - 主题切换与自定义 CSS
  - JSON / OPML 导入导出
  - WebDAV 上传下载
- 标注 Android 特有风险：
  - 文件权限
  - 后台恢复
  - 小屏布局

完成标准：
- 有可重复执行的 Android 回归文档

### Phase 4：接入 Android Release 打包

目标：
- 本地可构建 Release APK / AAB

任务：
- 建立 Release build variant
- 选择发布产物策略
  - `apk`
  - `aab`
  - 或两者都发
- 配置签名流程
  - keystore
  - alias
  - store password
  - key password
- 记录本地打包命令

完成标准：
- 本地可稳定产出签名后的 Release APK 或 AAB

当前进度：
- 已部分完成：GitHub workflow 已支持在 secrets 存在时构建 release APK / AAB
- 仍需实际 keystore、真实签名验证和产物安装验收

### Phase 5：接入 GitHub 自动发布

目标：
- GitHub Actions 自动生成 Android 安装包资产

任务：
- 新增 Android release job
- 配置以下 GitHub Secrets：
  - `ANDROID_KEYSTORE_BASE64`
  - `ANDROID_KEYSTORE_PASSWORD`
  - `ANDROID_KEY_ALIAS`
  - `ANDROID_KEY_PASSWORD`
- 在 CI 中恢复 keystore 文件
- 构建签名 APK / AAB
- 上传为 GitHub Release 附件
- 在 README 中补 Android 下载与安装说明

完成标准：
- 打 tag 后，GitHub Release 自动附带 Android 产物

当前进度：
- 已部分完成：debug APK 已进入 GitHub Release 产物
- 若配置 Android signing secrets，可额外发布 release APK / AAB
- 仍需补充正式签名产物的验收记录

## 建议的 GitHub Release 产物策略

优先建议：
- `RSS-Reader-android-arm64-v8a-debug.apk`
- `RSS-Reader-android-arm64-v8a-release.aab`

可选：
- `RSS-Reader-android-x86_64-debug.apk`
  - 主要供模拟器或开发环境使用
  - 不一定需要面向普通用户发布

## 建议的最小交付顺序

1. 先跑通本地 Debug APK
2. 再完成 Android UI / 文件 / 数据路径适配
3. 再补 Android 手工回归
4. 最后接 GitHub Release 自动签名与发包

## 风险提示

### 高风险项

- Android 文件导入导出与桌面交互模型不同
- Dioxus Android 构建链比 Desktop / Web 更脆弱
- 签名与 GitHub Secrets 容易成为发布阻塞点

### 中风险项

- SQLite 默认路径在 Android 上需要实机验证
- 小屏 UI 可能需要额外压缩和改版
- WebDAV / 网络权限在移动端可能有额外限制

## 当前结论

Android 已经从“仅有架构目标”推进到：

- 本地 Debug APK 可构建
- GitHub Release 可附带 debug APK
- 若 secrets 齐备，可进一步构建 release APK / AAB

当前更务实的剩余重点是：

- 真机/模拟器安装与启动回归
- Android 文件导入导出体验补齐
- keystore 签名链的真实验收
- release APK / AAB 的最终发布验证
