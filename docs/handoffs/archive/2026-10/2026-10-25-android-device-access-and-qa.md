# Android 安装自动化与实机验收

- 日期：2026-10-25
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 检查基线 HEAD：`1448539a`
- 相关 commit：本记录与补丁同次提交（提交前 commit: pending；用 `git log -1 -- docs/handoffs/2026-10-25-android-device-access-and-qa.md` 定位）
- 相关 tag / release：N/A
- 状态：`validated`（安装工具与下列通过项；Android 整体验收未通过）

## 工作摘要

用户允许 USB 安装后，要求减少每次手动确认。检查已连接的小米 12，沿用已授权的 ADB，实现仅处理本项目测试包的安装助手；无需额外手机插件或 root。继续完成来源目录、来源筛选与升级保留检查，并修复目录定位新增 WebView 历史条目的问题。实机同时发现系统返回和数据库缓存目录两个未解决项，不能据此宣布 Android 发布就绪。

## 影响范围

- 模块：`scripts/install_android_test.ps1`；文章目录 DOM 适配层；浏览器目录回归及设计契约。
- 平台：Windows 上的 ADB 工具链、Android 15 / HyperOS `OS3.0.3.0.VLCCNXM` 实机、Chrome Web。
- 额外影响：没有改变手机安全设置，没有安装 Shizuku / scrcpy，没有清空应用数据、卸载应用、解锁引导程序或修改数据库路径。
- 手机原先没有本项目应用；本次数据来自 UI 添加的 6 个本地测试订阅，共 5000 篇文章。没有把 Windows 数据库复制到手机。

## 关键变更

### 限定范围的安装助手

- 显式指定设备、APK 与预期 SHA256；安装前核对哈希、包名 `io.github.develata.rssreader` 和应用名 `RSS-Reader`，使用 `adb install -r` 保留数据。
- 只在本助手启动的安装进程尚未结束时，识别 `com.miui.permcenter.install.AdbInstallActivity` 的“USB安装提示”、RSS-Reader 名称和对应安装说明。仅点击该视图中唯一且启用的“继续安装”，点击坐标从当前 UI 树取得，点击前再检查前台窗口。
- 不选“拒绝后下次安装不再提示”（它不是永久允许），不自动处理其他应用或权限对话框。
- 保存安装日志和收据，并核对手机上实际 `base.apk` 的 SHA256。保留 ADB 的原有电脑信任关系；锁屏、撤销授权或改变 ROM 的提示结构仍可能需要用户操作。

调用示例：

```powershell
./scripts/install_android_test.ps1 `
  -DeviceSerial caa0c283 `
  -Apk target/native-reading-20261025/android/rssr-back-fix.apk `
  -ExpectedSha256 D26FEDA8D4A05001D258C854BFFC81F5E9E7E925E47F9BBDD46EB7B199E8ADC2
```

### 目录锚点历史修正

- 将直接写入 `location.hash` 改为 `history.replaceState(history.state, '', hash)`；同页和跨列表页定位仍更新锚点并滚动，但不新增 WebView 历史条目或丢失路由状态。
- 新增两条历史长度与状态保持断言。
- 实测测试在平滑滚动最后一帧（距终点 1px）立即发滚轮时，输入会被前一次动画消耗。测试先等待滚动稳定，再计算滚轮距离；仍按原来的 2px 精度检查最终文章位置，没有放宽定位标准。

## 验证与验收

### 安装与产物

- 原始测试包：`target/native-reading-20261025/android/rssr-test.apk`，SHA256 `46E1FFA14763A36F6448CCE2659DF64DE447D2A34F5873CAA0BE80BDE18286D4`。
- 目录历史补丁测试包：`target/native-reading-20261025/android/rssr-back-fix.apk`，SHA256 `D26FEDA8D4A05001D258C854BFFC81F5E9E7E925E47F9BBDD46EB7B199E8ADC2`；最后恢复安装的是此包。构建后仅调整了适配层注释措辞，没有再改变运行逻辑。
- 这些是 ARM64、Rust release 优化、可调试 Android 外壳的测试包；脚手架 versionName 为 `0.1.0`，不是正式发布产物。
- 第一次被限制的安装：读取真实提示 XML 后，ADB 点击“继续安装”成功。随后助手多次覆盖安装均成功，设备包哈希相符；这些重装没有再出现提示，因此**助手自动确认分支尚未单独完成实机端到端验收**。
- 提示解析器用实际 XML 回放：正确提示识别、错误窗口、其他应用名、不同按钮、禁用按钮及畸形 XML，共 6 项通过；错误预期 APK 哈希在调用 ADB 前被拒绝。

### 构建和浏览器回归

- `dx build --locked --platform web --package rssr-app`：通过。
- `dx build --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false`：通过；重新运行 Android bundle 准备脚本及 Gradle `assembleDebug` 后打包通过。
- dx `0.7.10` 与工作区 Dioxus `0.7.9` 的版本提示仍存在；两端构建最终退出码均为 0。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:9244 --static-base http://127.0.0.1:8112 --artifact-dir target/native-reading-20261025/back-fix-directory-final --directory-only true`：**132 项通过，0 控制台错误**。
- `cargo fmt --all --check`、目录测试脚本语法、适配层 `AsyncFunction` 语法与本次 diff 检查：通过。适配层是嵌入 async 函数的脚本，不适用按 CommonJS 文件执行 `node --check`。
- 本轮没有重跑 Windows 原生完整验收、完整 workspace 测试或正式内存性能对比；此前 Windows/Web 结果见同日其他交接，不能替代本次 Android 验收。

### 实际手机 UI 验收

- 原包 15 项通过：目录浏览和同页文章定位保持选择，跨页定位及清空选择，月份展开文章标题，来源草稿混合状态/取消/空选择/应用/省略号/全选，以及长文 ADB 触屏滚动。
- 新包 6 项通过：覆盖升级保留来源、文章计数、已读/收藏/归档、自定义名称和 URL、搜索/筛选/分组/页大小；同页及跨页目录跳转不增加 WebView 历史。
- 升级后 UI 明确显示 6 个订阅、5000 篇文章、4999 未读、1 已归档；测试来源自定义名和带查询参数的 URL 保留。
- 这些证明普通覆盖升级的保留，不证明清缓存、卸载、断电或任意时刻杀进程后的保留。阅读位置原本仅会话内保存，未新增跨进程语义。

## 结果与风险

1. **P1：Android 系统返回仍未通过。** 新包无多余锚点历史后，从阅读页按系统返回会退出应用；进程日志另出现 `FORTIFY: pthread_mutex_lock called on a destroyed mutex`。用原包重新安装、从全新 WebView（history 长度 1）直接打开文章，也复现返回桌面，证明“阅读页系统返回直接退出”并非此次锚点修改才出现。原包对照没有观察到同一 mutex 日志，不断言两者根因相同。当前 Tao hook 不能代替对实际 Activity 返回事件桥接的验收；系统返回后的阅读位置恢复仍未验收。
2. **P1：Android 数据库存放在 cache。** 实际日志与 `run-as` 路径是 `/data/user/0/io.github.develata.rssreader/cache/RSS-Reader/`。`sqlite_native.rs::android_data_base_dir` 在没有 HOME 时回退 `temp_dir`，该行为不是此次改版新增。Android 可以回收应用缓存，普通重启/升级成功不能证明长期安全；官方说明见 <https://developer.android.com/training/data-storage/app-specific>。本轮未清缓存、移动或删除数据库。
3. 后续修正数据目录需同时考虑索引库、正文库、WAL 和 shell 偏好；应先设计只在 host/infra 适配边界使用应用 files 目录的方案，验证旧数据完整性、已有目标目录、失败恢复及保留旧备份，再依仓库高风险迁移规则确认实施。不能仅换路径后让原有用户看见空库。
4. 安装助手的可交付范围已经完成；整套 Android 行为验收与发布仍受上述问题阻塞。未取得、也未尝试绕过 Android 的 root、屏幕锁或全局权限边界。

## 给下一位 Agent 的备注

- 证据根目录：`target/native-reading-20261025/android-device/`。
- `install-confirmation.xml`：实际小米提示；`final-install/receipt.json`：最终安装身份；`device-qa-receipt.json`：按包区分的 21 项通过及系统返回失败；`system-back-baseline.log` / `system-back-failure.log`：对照日志。
- 浏览器证据：`target/native-reading-20261025/back-fix-directory-final/assertions.json`。
- 测试订阅依赖电脑 `127.0.0.1:8111` 的夹具服务及 `adb reverse tcp:8111 tcp:8111`。连接被撤销或反向转发消失后刷新失败不代表用户订阅服务损坏。
- 任务外 `.agents/.specify` 删除、旧交接修改及 Penpot/Figma 未跟踪文件保持原状；没有 push、tag 或发布。
