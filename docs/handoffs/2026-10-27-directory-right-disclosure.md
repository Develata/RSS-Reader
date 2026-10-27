# 目录箭头移至右侧与文章缩进收紧

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 修改前 HEAD：`d2054f91`
- 相关 commit：代码与契约 `badf7fae`；最终 Penpot 记录为本文件最新提交，准确提交号见 `git log -1 -- docs/handoffs/2026-10-27-directory-right-disclosure.md`。
- 相关 tag / release：N/A；未 push、未发布。
- 状态：`validated`（代码及 Penpot 局部同步）。

## 工作摘要

用户指出上一版文章左侧仍有过宽空白，并提供右侧展开箭头的目录作为参考。本次将月份、来源及文章分支的箭头统一移到右侧，文章容器左缩进从 38px 缩为 8px。撤销上一版“文章与日期文字齐平”的布局决定，改为文章比日期文字多 8px 的浅层缩进；不改变定位与展开的职责。

## 影响范围

- `assets/styles/reading-workspace.css`：目录展开控件、父级标题行、文章缩进。
- `crates/rssr-app/src/pages/entries_page/directory/view.rs`：日期／月份的定位按钮先于展开按钮，DOM、视觉与 Tab 顺序一致。
- 共用 UI 影响 Web、桌面、Android；实际运行验收为 Windows 浏览器 Web。
- 更新目录设计契约、主题作者参考及 Penpot 当前画板。历史稿和其他任务的文件保持原状。

## 关键变更

- 父级 6px 箭头定位到右边，与分支 5px 箭头中心对齐；计数保留在箭头左侧。
- 父级行与分支定位按钮最小高度统一为 44px，独立展开按钮仍为 44×44px。
- 文章左侧空白相对上一版减少 30px；行内 padding 仍为 4px 8px，最小行高 44px，长标题最多两行。
- 目录事件、自动跟随、按需挂载、虚拟列表、文章选择和跨页语义未改。

## 验证与验收

- `dx build --platform web --package rssr-app --locked`：成功，50.67 秒；既有 dx 0.7.10 / Dioxus 0.7.9 提示未阻止构建。
- `cargo fmt --all --check`、`git diff --check`：通过。
- 隔离端口 18104，72 篇 `directory-contract` 样例；没有使用用户真实订阅。
- 1200×936 默认桌面：父级标题 x=952.5、日期标题 x=958.5、文章标题 x=966.5；相比上一版文章 x=996.5 减少 30px。父级与分支箭头中心均在行右侧 22px 处，数量对齐。短标题行 44px，Charlie / Foxtrot 双行 46.375px；目录总高 714.75px，无页面横向溢出。
- 分支按钮鼠标收起后文章节点 6→0、Enter 展开后 0→6；日期定位按钮 Tab 后聚焦右侧分支按钮，焦点轮廓可见。点击文章仍定位到 `/entries#entry-card-71`。
- 父级月份鼠标收起、Space 展开：`aria-expanded` false→true，文章节点 0→6。
- 390×844 手机：浮层宽 220px，文章标题 x=173，右侧按钮 x=293、44×44px、5px 箭头；无页面横向溢出。来源分组中的六个月份分支也使用右侧按钮，文章行高 44px。
- 900×900 中等宽度：Amethyst 主题目录浮层宽 220px，无页面横向溢出，DOM 顺序为定位标签→展开按钮。
- 深色桌面：设置成功保存，`.theme-dark`，背景 `rgb(20,20,20)`，目录宽 200px，无横向溢出。样例原归档阈值为 0，首次保存被验证拒绝；仅将隔离样例改为 12 月后成功保存，没有更改产品校验。
- Amethyst 桌面实际字体与默认不同，六个样例标题均为双行，行高 46.375px，目录总高 724.25px；Penpot 按实测换行保留。
- 浏览器相关错误日志为空。未运行全 workspace 测试；未新增只复述样式值的测试。

## Penpot 同步与验证

- 修改前命名版本：`Before right-side directory disclosure · 2026-10-27`。
- 16 张当前画板：86 个父级箭头、43 个分支箭头移到右侧，69 个文章条目使用 8px 容器缩进；原画板 ID、分类与主题保留。
- 导出发现手机标题残留旧换行缓存，已逐页面切换文本增长方式重新排版；最终几何检查确认文字宽度没有超过标题容器，单行／双行与实测相符。
- 默认桌面、手机来源目录、搜索目录和 Amethyst 桌面实际导出复核通过。全量几何检查无问题，`currentFile.validate()` 返回空错误列表，当前画板范围没有遗漏的旧左侧父级箭头。
- 最终 Penpot rev 369，55 张画板与 55 个唯一 capture-key 保留；16 张相关画板新增 `directory-right-source=badf7fae`，00 使用说明更新为右侧箭头与 8px 缩进。
- 命名版本 `Right-side directory disclosure · badf7fae · 2026-10-27` 已保存并通过 `findVersions()` 回读，创建时间 `2026-10-27T13:05:58.998Z`，非自动保存版本。

## 风险与后续事项

- 未重新运行 Android / Windows WebView2 实机验收；响应式浏览器检查不等于原生平台验收。
- 未重验超过 80 条的虚拟分支；其布局测量与行为算法未改。
- Penpot 保留原生可编辑图层；字体渲染与抗锯齿可能与浏览器略有差异，静态画板不是运行时交互验收。
- 整体设计基准仍为 `34a63202`，本次仅更新目录布局。

## 给下一位 Agent 的备注

- 证据目录：`C:/Users/QQ/.codex/visualizations/2026/10/26/01a1779a-f271-7ec2-ab70-c1ef756f9ffb`。
- 浏览器证据：`directory-right-desktop.png`、`directory-right-mobile.png`、`directory-right-mobile-source.png`、`directory-right-dark.png`、`directory-right-desktop-geometry.json`、`directory-right-amethyst-geometry.json`。
- Penpot 审计与导出：`penpot-directory-right-audit.json`、`penpot-directory-right-desktop.png`、`penpot-directory-right-mobile.png`、`penpot-directory-right-amethyst.png`；保存版本、全部目标几何及文字回读已记录。
- 本记录取代此前箭头与间距记录中的“标题与父级标签左对齐”和“38px 文章容器缩进”；5px 分支箭头和 44px 点击区域继续适用。
