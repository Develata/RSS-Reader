# 目录窗口化交互复查

- 日期：2026-10-26
- 作者 / Agent：Codex
- 分支：feat/reading-workspace-20261024
- 当前 HEAD：d0b092b0（修复前）
- 相关 commit：pending
- 相关 tag / release：无
- 状态：validated（等待提交）

## 工作摘要与背景

复查目录窗口化对定位、多选和滚动恢复的影响。继续当前阅读分支，不迁入 v0.2。先在隔离的无头 Chrome 复现，再按现有 Rust 控制器和宿主适配边界修复。

## 受影响模块与平台

- rssr-app 前端目录输入与窗口适配器，共用于 Web、Windows、Android。
- 不更改业务命令、存储和完整目录投影。

## 关键变更

- 目录内 Tab / Shift+Tab 浏览进入手动模式，收起重开保留浏览位置；目录外 Tab 不触发主列表跟随。
- 自动揭示只挂载目标行，实际自动对齐由目录控制器统一执行，避免窗口通道迟到的回复覆盖新的手动滚动。键盘显式定位仍由窗口适配器执行焦点与滚动。
- 查询缩短投影时，窗口模型可能在 epoch 重置前短暂保留旧范围；失效行改为跳过，避免无 key 的空 VNode 混入键控迭代器。此修复与小目录已有的引用过滤保持一致，不新增另一份业务状态。
- 新增 `rssr_directory_window_review.cjs`，复用离线 Chrome fixture，提供两来源大目录与隐藏归档的测试设置。设计契约补充滚动职责和过渡渲染规则。

## 验证与验收

- 修复前新回归复现两项失败：Tab 后收起重开 scrollTop 1459 → 0；延迟自动回复在手动滚动后将 scrollTop 555 → 1056。两次已选文章 600 均保留。
- 隐藏状态调整窗口宽度后，同一逻辑首行 582 与选择状态保留；像素位置因换行改变，并非丢失恢复。
- 另外复现 P1：600 篇来源目录滚动到末篇后，在不显示归档的视图里归档主列表选中的两篇，Dioxus 0.7.9 的 keyed diff 在 `iterator.rs:287` 对缺失 key unwrap，Web 出现 `RuntimeError: unreachable`，界面停止更新。修复后目录变为 598 篇，仅这两篇移出列表，选择清空，空选择的批量操作禁用。
- 新回归 8 个场景全部通过：Tab 收起重开、延迟自动定位与手动滚动交错、隐藏时改变宽度、反向 Tab 与目录外 Tab、已读/收藏更新、批量已读、批量归档、两个大分支中部位置恢复。后者比较实际文章 ID，并忽略视口边缘不足 2px 的截断片段；不要求变宽换行后像素 scrollTop 相同。
- `cargo test -p rssr-app --locked`：159 单元 + 4 集成通过，1 项显式性能测试忽略。`cargo clippy -p rssr-app --all-targets --locked -- -D warnings`、`cargo fmt --all --check`、本次文件 `git diff --check` 通过。
- `dx build --platform web --package rssr-app --locked --release` 产出本轮可运行包。工具仍提示 dx 0.7.10 / Dioxus 0.7.9 不匹配，可选 wasm-opt 仍退出 0xc0000409，dx 继续完成；不将其记为优化器成功，未更改工具或依赖。
- 最终源码的无头 Chrome 回归（`NODE_PATH=C:/Users/QQ/node_modules`）：
  - `node scripts/browser/rssr_directory_offline_regression.cjs`：150 项通过。
  - `node scripts/browser/rssr_directory_window_acceptance.cjs`：22 项通过。
  - `node scripts/browser/rssr_directory_window_review.cjs`：上述 8 场景通过。
  - `node scripts/browser/rssr_directory_transport_acceptance.cjs --offline-fixture true --artifact-dir target/directory-window-review-20261026/transport`：5 种坏消息及后续正常请求通过。
- 本轮所有 Web 测试使用 Playwright 路由提供隔离素材，不启动 HTTP 服务器，不控制用户浏览器。证据目录 `target/directory-window-review-20261026/`，包含 before/review.json、before-archive-fix.json、archive-panic-before.json、after/review.json、window/、contracts/、transport/。

### 最新原生包

- Windows：`cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`。复制 5000 篇测试库到本轮独立目录，通过 CDP 验收。
- 原生目录/连续三次长文返回 38 项通过；另补 Tab 手动浏览、收起重开及选择保留，scrollTop 1440 → 1440。记录 windows-acceptance/acceptance.json、windows-extra.json。延迟消息的精确注入在 Chrome 验证；原生尝试改变了来源分支，未产生待测揭示回复，不能算原生故障注入通过。
- Windows EXE SHA256：`E52314555B10BECD2E7EF954EB8B47AFE86F510B4916443F944EA8D04DEDF688`，`windows/rssr-app.exe`。
- Android：`dx build --locked --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false`，`python scripts/prepare_android_bundle.py target/dx/rssr-app/release/android/app/app/src/main`，JDK 21 下 Gradle `assembleDebug --no-daemon` 通过。Rust release + 可调试 Android wrapper，不是发布签名包。
- Xiaomi 12 / Android 15 / WebView 152：40 项目录/连续返回回归通过；另补 Tab 浏览 → 系统 Back → 重新展开，scrollTop 1297.8182 → 1297.8182，手动状态与勾选保留。记录 android-acceptance/、android-extra.json。
- APK SHA256：`BA101292556049C8DA3C76EC1080EA90AEEC285D4C669BB48E6858842CDB5B27`，安装后读回相同。APK 内 libmain.so 与生成库 SHA256 均为 `A319C625BA21275A6B5CE6841F97F72DD9B5C31D74EA5A9CF77199FF10FFA109`。记录 android-install/、apk-library-identity.json。
- 上述二进制在提交前由相同生产源码构建；源码提交与产物哈希分开记录。

### 数据保留

- 手机覆盖安装前、最终停包后分别备份 files/cache 数据；没有卸载、清数据或写回旧数据库。前备份 SHA256 `DCC6DE5EFDB114032008ACC173628826F7F4BCB0F5CE7D646211F6CBAC450BA0`，后备份 `ACB7EAA39DEC15DF7D8EC3929BE5B487A653244420F093CCE17717E10F9CAE96`。
- 提取到电脑独立目录后只读 SQLite 比较，四库完整性均为 ok；两处 5000 篇文章及已读/收藏/归档字段、5000 篇正文、迁移/同步/GC 数据逐行一致，shell prefs 相同。差异仅 files 库 app_settings.updated_at、feeds.last_fetched_at/updated_at，设置值、来源名称和 URL 不变。
- 已恢复 source 分组、05000 搜索并收起搜索栏；最新包保留在手机。记录 phone-data-comparison.json。

## 当前状态、风险、待跟进项

- commit: pending
- 保留所有任务外删除和 Penpot 文档；不改 C 盘旧工作区。
- 仅使用无头 Chrome 或原生宿主调试接口；不操作用户桌面鼠标键盘。
- 同页定位/目录浏览仍保留选择；跨页/筛选/视图切换仍按已有约定清空，已读与归档仍只处理已选文章。归档移除条目后目录按新投影重置，不能继续保留指向已移除文章的旧窗口。
- 本轮不新增性能结论，未验证真实系统输入法、Linux/macOS、前台 Windows 帧耗时或发布签名包。浏览器故障注入不等于原生消息故障注入；CDP 键盘不等于真实 IME。
- 测试 Chrome、Windows 测试进程及 Android CDP 转发均已关闭；手机已停包。未留下预览服务。

## 相关提交与 worktree

- 基线 d0b092b0，功能基线 848fe6f7；E:\gitclone\RSS-Reader，feat/reading-workspace-20261024。

## 给下一位 Agent 的备注

- 优先使用新增复现脚本验证交错输入和数据缩短，再看 window_adapter.js 与 virtual_articles.rs。完整目录索引及 Rust 几何模型此次未改。
- 原生复测入口沿用 `target/directory-window-20261026/native-directory.cjs --platform windows|android --cdp <endpoint> --out <directory>`，Android 另传 `--serial caa0c283`。必须匹配本轮安装包和隔离素材，勿把旧包通过记录当作新包结果。
