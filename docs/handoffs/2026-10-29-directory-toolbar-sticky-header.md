# 目录入口、固定标题与 Penpot 同步

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/directory-toolbar-sticky-header`
- 基线 HEAD：`6e99706ce90026c70c9538eb35faf572f02781fd`
- 相关 commit：代码 `9e20b56a3e151e1b3c5ca63268d02165470b8d53`；设计版本记录为本次后续文档提交。
- PR：[#39](https://github.com/Develata/RSS-Reader/pull/39)，目标 `main`。
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

按用户确认的方案，将目录入口移至文章主面板右上角；目录标题及收起按钮整行固定。用户同时授权新分支、推送、PR、检查通过后合并，并要求同步修复 Penpot 平板示例。

## 影响范围

- 模块：`rssr-app` 文章工具栏、目录宿主测量、工作区样式及浏览器回归。
- 平台：共享前端；本轮实际运行验收为 Windows Chrome 的真实 Web 构建。
- 不改变后端、存储接口、命令或文章身份／选择／分页规则。
- 独立工作区：`E:\gitclone\RSS-Reader-directory-ui-20261029`。原工作区 `E:\gitclone\RSS-Reader` 的其他任务改动保持原样。App 工作区创建超时后使用 Git CLI 建立隔离工作区。

## 关键变更

- 右侧同排为批量已读、批量归档、列表选项、目录；保持 44px 点击区域，移除手机左下目录入口。工具栏按实际面板宽度布局，兼容 Atlas 侧栏，长计数只在标题内省略。
- 保留 rail 滚动容器，64px 标题行内部吸顶，背景由主题变量派生为不透明色；固定区域纳入跟随、虚拟窗口和键盘定位测量。
- End／Home 定位后，新挂载条目的行高可能再次变化。仅在该次键盘定位的测量稳定前保持焦点可见；指针、触摸或滚轮输入取消保持。
- 颜色契约检测允许从 `var(...)` 派生的相对颜色，仍拒绝硬编码颜色；补上相应检测用例。
- Penpot 当前规范修改 28 张现有画板，16 个目录标题；新增两张滚动后的可编辑示例。现共 57 张唯一 capture-key 画板、30 组工具栏、18 个固定标题。误改的平板按钮恢复正确父层级、44px 区域和图标位置。历史 90–94 页不改。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过。
- `cargo test -p rssr-app --locked`：290 通过、1 个既有忽略项。
- `cargo build -p rssr-web --locked` 及真实服务器 fixture 构建：通过。
- dx 0.7.9 `build --platform web --package rssr-app --release --locked --debug-symbols false`：通过，确认完整优化完成；不是仅检查退出码。初次误用本机默认 dx 0.7.10 的结果未计入验收。
- 工作区回归：通过；新增五套主题、35 组尺寸的工具栏及固定标题底色断言，最终布局及长计数补充复测通过。
- 目录回归：150 项通过、无 console error。
- 600 篇虚拟目录：25 项通过；End／Home 焦点位于固定标题下，保持有界挂载；覆盖来源／时间、关闭重开、选择、自动跟随、异常消息。
- 虚拟目录边界：11 项通过；覆盖旧回复、筛选／分组／分页切换和阅读返回。
- 真实浏览器视觉矩阵：浅色、深色及四套主题 × 360/390/600/601/760/1000/1001/1200，共 48 组通过，保存 36 张截图及几何 JSON；最终布局及长计数补充复测通过。
- Penpot 几何检查及 `File.validate()`：通过；57 个 capture-key 唯一，代表平板、桌面滚动、浮层滚动及 Atlas 主题画板导出复核通过。文件 rev 386，命名版本 `Directory toolbar and sticky heading · 9e20b56a · 2026-10-29` 已保存并回读，00 使用说明及图层元数据注明代码提交。

### 证据与实际发现

- 本地证据：`C:\Users\QQ\Documents\Codex\2026-10-29\directory-toolbar-sticky-header`，包含构建日志、Rust 日志、浏览器截图、`visual-qa.json`、各目录回归输出及 Penpot 导出。
- 实际发现并修复：Atlas 1001px 时操作组换行；玻璃主题标题透明；End 后变高的虚拟行挤出视口。新增断言捕获这些问题。
- 中途一次批量操作验收因 CSS 修改后只重建页面，导致服务端版本检查拒绝写入；随后从同一工作区重建两端，工作区全套通过。没有关闭或绕过版本检查。

## 结果

- 代码与设计同步已实现并通过本地验收，分支已推送并创建 PR #39。远端 CI 和审查结果以该 PR 的提交检查记录为准；用户已授权必要检查通过后合并。

## 风险与后续事项

- 浏览器响应式检查不等于 Android 触摸、Windows WebView2 或 macOS/Linux 原生设备实测；这些本轮未重跑。
- Penpot 为可编辑状态快照，新增 1200×720、760×720 画板展示原目录滚动 146px 后的固定标题，不模拟完整应用的跟随状态机。
- 自定义主题及任意字号组合未穷举；既有大字号虚拟列表回归通过。

## 给下一位 Agent 的备注

- 从目录模块 README 与 `entries-directory-follow.md` 阅读定位和跟随约束。
- 浏览器脚本共用 8119 端口，必须串行；页面与服务端必须来自同一源码状态。使用 dx 0.7.9。
- Penpot 文件 `cadc1646-c70c-8009-8008-d6f40c2d8fdc`；新增桌面画板 `863bc8b9-41ce-80de-8008-dd1464812e56`、平板浮层 `863bc8b9-41ce-80de-8008-dd1481240a83`。修改前命名版本为 `Before directory toolbar and sticky heading · 2026-10-29`。
