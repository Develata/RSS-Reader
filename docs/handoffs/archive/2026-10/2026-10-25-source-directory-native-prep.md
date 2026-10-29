# 来源目录文章层级与来源筛选 Rust 草稿

- 日期：2026-10-25
- 作者 / Agent：Codex；Claude CLI 只读评审
- 分支：`feat/reading-workspace-20261024`
- 实施基线 HEAD：`d2e39e1a`
- 相关 commit：与本记录同次提交（提交前 commit: pending；用 `git log -1 -- docs/handoffs/2026-10-25-source-directory-native-prep.md` 定位）
- 相关 tag / release：N/A
- 状态：`validated`（实现及下列验证；Android 运行验收、正式性能基线仍待完成）

## 工作摘要

继续原生 Rust 界面前置整理，将来源筛选草稿规则独立为纯 Rust；按用户明确要求，来源目录从“来源 → 月份”扩展为“来源 → 月份 → 文章标题”。同时修复跨页月份锚点不一致的问题。此次不替换渲染器、不新增 crate，不改变数据库或业务层职责。

## 影响范围

- 模块：`components/entry_filters`、`pages/entries_page` 的分组、目录、文章卡片；`reading-workspace.css`；目录与主题选择器契约；浏览器回归脚本。
- 平台：共享前端涉及 Windows / Android / Web / Linux / macOS；实际运行验证覆盖 Windows WebView2 与 Web。
- 额外影响：增加隔离基线数据生成及 Windows 进程树测量脚本。没有用户日常数据库变更、依赖或发布变更。

## 关键变更

### 来源筛选草稿

- `SourceFilterDraft` 管理全选、部分选择、空选择、搜索、取消和来源集合变化；UI 保留输入、焦点与 indeterminate 显示适配。
- 应用空草稿被禁止；全选仍编码为空 URL 集合；取消后重开从已应用偏好初始化。
- 编辑期间新加入的来源不自动选入，已删除来源从草稿移除；最终应用再次核对可用来源。
- 7 个 Rust 用例与 9 个浏览器断言验证草稿边界。

### 来源目录文章层级

- 月份增加独立展开按钮，月份文字仍跳转月份；标题叶节点定位文章而非打开阅读页。
- 目录共享文章摘要的 `Arc`，展开后才挂载该月份的标题控件，不复制文章正文。
- Rust 跟随状态增加文章叶节点；DOM 只测量、报告输入与执行滚动。同页定位保留勾选，跨页清空勾选。
- 来源月份锚点改为来源 ID + 年月；时间日期锚点改为日期，不依赖某页第一篇文章 ID。已在 Windows 复现旧锚点跨页失配，修复后页面与目录命中同一个月份并展开标题。
- 保留原有跨页重置目录手动浏览状态的契约；已读/收藏更新不会无故打断目录手动浏览。

### 基线工具与评审

- `scripts/seed_reading_native.py` 只接受已迁移的空数据库对，写入 5000 篇 / 6 来源 / 50 每页、24 篇长正文与本地图片夹具；拒绝覆盖非空数据库，已验证拒绝路径。
- `scripts/measure_windows_process_tree.ps1` 校验预期可执行文件，累计自身及 WebView2 子进程的私有提交、私有工作集和工作集；明确普通工作集可能重复计算共享页，快照不等于峰值或泄漏结论。
- Claude 会话 `51477cf1-cd03-41fd-a103-d5ef52f20f36` 做只读代码评审；采纳稳定锚点、目录模型身份快速比较及带 key 的独立文章组件建议。Claude 没有代跑测试，评审不能替代运行验收。

## 验证与验收

### 自动化验证

- `cargo test -p rssr-app --locked -j 2`：138 单元 + 4 集成通过，1 个忽略。
- `cargo fmt --all --check`、任务范围 `git diff --check`：通过。
- `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings`：通过。
- `cargo build --release --locked -p rssr-app -j 2`：Windows release 通过。
- Dioxus Web debug 与 Android release Rust 构建、Android Gradle `assembleDebug`：通过；Android APK 尚未完成安装运行。
- Windows 实际 WebView2 冒烟：95 断言通过，0 页面错误。证据 `target/native-reading-20261025/candidate-final/smoke/assertions.json`。
- Web 全套：315 断言通过，1 项新目录测试对接近页底的滚动位置要求过严；修正测试目标后，重跑完整目录契约 130 项全部通过，0 页面错误。不可写成未经执行的“全套 316 全通过”。证据 `web-final/assertions.json` 与 `directory-final/assertions.json`。
- Python 语法、JS 模块语法、空库种子与非空库拒绝：通过。

### 手工及设备验收

- Windows 来源月份跨页：修复前主列表月份 ID 与全局目录不一致；修复后第 2 页仍展开 `entry-source-3-month-2026-10` 并显示 584 个该月标题。
- Android：检测到用户连接的 Android 15 真机；安装返回 `INSTALL_FAILED_USER_RESTRICTED: Install canceled by user`，等待用户允许 USB 安装后再继续。
- 用户通过 Escape 停止过一次计算机控制；该轮已立即停止。后续持久化检查通过独立的浏览器调试连接与隔离程序重启完成，见持久化交接。

## 结果

目录文章层级与纯 Rust 来源草稿已实现并经上述验证。截图：`target/native-reading-20261025/directory-final/directory-source-articles.png`。全部测量数据均来自独立夹具，未操作用户日常数据库。

## 风险与后续事项

- Windows 冒烟期间 Android 构建仍在运行；这些计时不能作为空闲机器性能基线。正式内存/响应时间对照仍待完成，不能宣称性能提升。
- Android 只有构建证据，没有安装、触摸、返回恢复或性能实机验收；Linux/macOS 未运行。
- Windows `dx build --platform desktop` 触发重复 VERSION 资源，改用项目现有 Cargo release 构建路径；本次未修改资源构建方案。
- 全局目录仍有大量文章摘要；本次只按展开月份挂载标题，尚未增加虚拟列表。

## 给下一位 Agent 的备注

- 当前 Windows 产物 SHA256：`BFA083FF6140AA64420C9D9AC730332FA5A3AFC341CEB561EDBF50C833234BF0`，源为 `d2e39e1a` 加本次前端补丁。
- Android 测试 APK SHA256：`46E1FFA14763A36F6448CCE2659DF64DE447D2A34F5873CAA0BE80BDE18286D4`，Rust 为 release 优化，Android 外壳为 debug。
- `target/native-reading-20261025/candidate-final/identity.json` 保留构建时 PID；后续重启 PID 以 `persistence-native-restart.json` 为准。
- 任务外 `.agents` / `.specify` 删除、旧交接修改及 Penpot 未跟踪文件均保留，不应随本任务提交。
