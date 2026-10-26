# 大目录文章行窗口化

- 日期：2026-10-26
- 作者 / Agent：Codex
- 分支：feat/reading-workspace-20261024
- 起始 HEAD：ce9e4f05
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：validated（下述限定范围；不是发布全验）

## 工作摘要

上一轮 Android 实机展开同一来源月份会挂载 597 篇文章、1248 个目录元素。此次在保留文章层级和全局定位索引的前提下，为大文章分支增加可变高度窗口；先明确 Rust 与宿主的职责，再验证真实行为和性能。继续当前阅读分支，不迁入 v0.2。

## 影响范围

- 前端目录组件、可独立测试的 Rust 几何模型与 DOM 测量适配器。
- 共用此组件的 Web、Windows、Android。
- 不变更存储格式、业务命令、application/domain/infra 职责，不新增 crate 或依赖。

## 关键变更

- 完整目录引用仍保留；大于 80 篇的展开分支只挂载视口、预留区域及需要保留焦点的行。
- Rust 计算可变行高偏移、挂载范围、占位高度和未挂载文章位置；宿主只测量当前挂载行、回传视口、执行滚动和焦点动作。
- 几何缓存与展开/跟随状态分离，同一上下文折叠重开可复用；查询、分组、页面身份改变后失效。
- 时间为月份 → 日期 → 文章；来源为来源 → 月份 → 文章。分页仍使用完整结果的全局索引。
- 回归发现并修复了两处新窗口实现的焦点风险：定位前排队的测量不能在定位后覆盖新视口；行 ID 的 key 必须位于动态迭代器根部，避免范围移动重建仍在使用的按钮。修正后 Home/End、跨窗口 Tab 与变高测量保持焦点。
- 原目录通道与窗口通道共用 `transport::receive_message` 的解码错误恢复；坏消息只释放本次等待，后续正常输入仍可用。

## 验证与验收

- `cargo test -p rssr-app --locked`：159 单元 + 4 集成通过；1 项显式性能测试忽略。包含 5 项窗口几何测试。
- `cargo fmt --all --check`、`cargo clippy -p rssr-app --all-targets --locked -- -D warnings`、本轮范围 `git diff --check`：通过。
- `dx build --platform web --package rssr-app --locked` 与加 `--release` 的构建均产出可运行包。工具提示 dx 0.7.10 / Dioxus 0.7.9 不匹配；release 的可选 `wasm-opt` 退出 0xc0000409，dx 继续成功产出并用于真实浏览器验证。**不能声称 wasm-opt 成功**，未为此更改依赖或工具版本。
- `NODE_PATH=C:/Users/QQ/node_modules` 下，`node scripts/browser/rssr_directory_offline_regression.cjs`：150 项既有目录断言通过，无 page error。
- `node scripts/browser/rssr_directory_window_acceptance.cjs`：22 项通过，600 篇逻辑目录初始挂载 19 行；覆盖末篇、跨窗口 Tab、同页保留选择/跨页清空、全局第 12 页定位、宽度变化、隐藏恢复、主列表滚动恢复跟随、实际 406px 高标题及 3 种窗口通道坏消息。自定义高标题时挂载 7 行。
- `node scripts/browser/rssr_directory_transport_acceptance.cjs --offline-fixture true --artifact-dir target/directory-window-20261026/transport`：原目录通道 5 种坏消息及后续有效请求通过。
- 自动审批拒绝启动本地预览服务器；使用 Playwright 隔离无头 Chrome 内的静态文件/素材路由，不启动 HTTP 服务。debug WASM 通过 CDP 传输超过 Chrome 单消息上限，改用 6.7MB release WASM。没有改产品代码来绕过测试加载问题。

### 最新原生包

- Windows：`cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`。复制已有 5000 篇测试库到本轮隔离目录，后台 CDP 验证；原生目录/连续三次长文返回共 36 项通过，另一个来源目录专项 16 项通过（其中 2 项为新增的未挂载末篇/首篇焦点检查，其余为重复前置回归，不能相加宣称 52 个独立用例）。未激活窗口、操作系统鼠标键盘或采集无效的后台性能样本。
- Windows EXE SHA256：`EEA6A144DF80B8E93982AD745BF8E6A85F06B30BCB3DA68AC165D0C52CD18A80`，路径 `target/directory-window-20261026/windows/rssr-app.exe`。
- Android：`dx build --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false`；已有资源准备脚本 + Gradle `assembleDebug --no-daemon`，使用进程级 JDK 21。Rust release、可检查的 Android debug wrapper，不是发布签名包。
- APK SHA256：`3FFE04B4A00E88C7AB833931F6F7110D58D18377DA93AFEB136A5F73D61AB78A`，安装后读回相同。APK 内 `libmain.so` 与本次生成库相同，SHA256 `26A08FAFDC6B2F90A9870DE81F34E9075780D5E4A5FEE26F2AAFC5DF9CA931E2`。
- Xiaomi 12 / Android 15 / WebView 152：40 项通过，包含时间/来源文章层级、手动浏览、显隐/系统 Back、多选、同页/跨页定位、首末篇焦点、连续三次长文返回及本会话位置恢复。没有把 CDP 键盘输入称为真实输入法验收。
- 原生证据在 `windows-acceptance/`、`windows-window-focus/`、`android-acceptance/`。手机视觉证据以实际 `adb screencap` 的 `android-directory-physical.png` 为准。

### 同一手机上的前后测量

采用相同脚本、设备、5000 篇数据中的同一 597 篇来源月份。先安装已验基线 APK（源码 `34a63202`），再恢复安装本轮 APK，各记录 5 次展开/收起，均为已有行高缓存的重复展开。不是随机化基准，不涵盖冷启动、查询或整应用内存。

| 指标 | 基线 | 本轮 |
| --- | ---: | ---: |
| 逻辑文章数 | 597 | 597 |
| 实际挂载文章数 | 597 | 14 |
| 目录元素总数 | 1248 | 83 |
| 展开至首批目标行 DOM，中位 ms | 47.40 | 28.10 |
| 上项后两次 rAF，中位 ms | 90.80 | 37.90 |
| 含桥接/几何诊断完成等待，中位 ms | 152.00 | 121.20 |
| 展开区间 Layout，中位 ms | 23.12 | 1.59 |
| 展开区间 UpdateLayoutTree，中位 ms | 11.30 | 1.72 |
| 展开区间 FunctionCall，中位 ms | 18.97 | 15.68 |

- DOM 指标在基线表示完整分支，在本轮表示窗口行；两次 rAF 是呈现代理，不是屏幕实际显示时间。包含诊断等待的结束指标不是产品用户时延；trace 事件可嵌套，不能将各项直接相加。
- 收起 DOM 中位数 36.70 → 20.00ms，呈现代理 43.30 → 37.60ms；包含诊断等待的收起完成为 100.20 → 103.70ms，**没有所有指标都更快的结论**。
- 可确认：本样本实际挂载和样式/布局成本明显下降，完整文章导航未丢失。没有测量或声称内存下降，未优化查询或推断 SQL 耗时。
- 原始 20 次样本与 trace：`android-profile-baseline/`、`android-profile-candidate/`；汇总：`comparison.json`。本地 Python 汇总入口被自动审批拒绝，用户确认后仍受入口限制；改在现有 Node 会话中只读记录完成汇总，APK 库校验用只读 ZIP 流。没有更改审批配置。

### 手机数据保留

- 使用同包名覆盖安装，无卸载、清数据或修改手机安全设置；对照结束后已恢复本轮 APK。
- 安装前与最终停包备份 files/cache 双目录；只在电脑新的测试目录提取并以只读 SQLite 比较，未写回旧库。四个数据库完整性均 `ok`。
- 两处 5000 篇文章、5000 篇正文、已读/收藏/归档字段、迁移、同步/GC 逐行一致，shell prefs 相同。差异仅 files 的 `app_settings.updated_at` 和 `feeds.last_fetched_at/updated_at`；设置值、来源 URL/名称及其他字段一致。
- 已恢复 source 分组、05000 搜索并收起搜索栏；保留最新测试包。最终备份 SHA256 `DCC6DE5EFDB114032008ACC173628826F7F4BCB0F5CE7D646211F6CBAC450BA0`，前备份 `52F9B0F540BC8B09DA3B538B403E201CB103DD0A387B5E177A1E5BEC3B8E7418`。
- 记录：`phone-data-comparison.json`、`apk-library-identity.json`、安装收据目录。

## 当前状态与风险

- commit: pending；功能与上述原生范围验证完成，可提交当前阅读分支。
- 未验：真实系统 IME、Windows 前台性能、Linux/macOS、冷启动大目录、整应用内存及发布签名产物；不将本次结果当作原生渲染框架选型结论。
- 用户正在使用电脑；所有浏览器检查采用无头 Chrome，原生 Windows 只允许后台 CDP，不使用桌面鼠标键盘或激活窗口。真实输入法不在本轮后台验证范围。
- 保留原有不相关删除、Penpot 文档与附件。
- 后续优先检查冷展开和滚动期间的实际帧成本；只有测量支持时再调整预留范围、测量批次和宿主往返，不扩大业务层职责或盲目增加另一套数据状态。

## 给下一位 Agent 的备注

- 入口：directory/window.rs、virtual_articles.rs、window_adapter.js。
- 对照基线：2026-10-26-native-directory-validation.md；证据在 target/native-validation-20261026/。
- 本轮临时证据目录：target/directory-window-20261026/。
