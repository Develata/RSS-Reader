# 目录展开箭头与条目间距修正

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 修改前 HEAD：`6b3aa3b0da92e50e4dd018b71d37d2503cee2c24`
- 相关 commit：箭头 `119dc73f`；间距补充为本记录与 CSS 的同一提交，准确提交号见本文件 `git log -1`。
- 相关 tag / release：N/A；未发布、未 push。
- 状态：`validated`（代码）；Penpot 同步待服务恢复。

## 工作摘要

用户指出日期展开箭头大于月份箭头，并明确要求改为 5px。根因是文章分支展开按钮复用了 20×20px 工具栏 SVG，而父级月份使用 6px CSS 箭头。

后续用户要求控制目录间距。将文章条目上下内边距从 8px 改为 4px，最小点击高度从 40px 改为 44px；保持 1.5 倍文字行距和两行截断。文章容器左缩进从 22px 改为 38px，使文章文字（38+8）与日期标签（44px 按钮+2px 内边距）左边缘一致，避免子文章向父级文字左侧突出。

## 影响范围

- `assets/styles/reading-workspace.css`：共用的文章分支展开按钮，涵盖时间分组的日期和来源分组的月份。
- Web、桌面与 Android 共用 CSS；本轮实际验收为 Windows 浏览器 Web。
- 不修改目录展开、定位、自动跟随或键盘语义，也不改变其他工具栏图标。

## 关键变更

- 局部隐藏原分支按钮 SVG，使用与父级月份相同的边框箭头画法；绘制盒为 5×5 CSS px，父级仍为 6×6px。
- `data-open` 控制展开与收起方向；按钮继续为 44×44px，保留原生焦点、标签和 ARIA 展开状态。
- 5px 指旋转前的 CSS 绘制盒，与父级 6px 采用相同计量方式；旋转后的外接框不等于 5px。

## 验证与验收

- `dx build --platform web --package rssr-app --locked`：成功，60.05 秒。存在既有 dx 0.7.10 / Dioxus 0.7.9 版本提示，未阻止构建。
- 隔离样例 `127.0.0.1:18102`，`directory-contract`，72 篇文章；未使用用户真实订阅。
- 桌面 1200×936：计算样式确认日期 5×5、月份 6×6、按钮 44×44，原 SVG 为 `display:none`。
- 实际鼠标点击日期按钮后 `aria-expanded=false`，键盘 Enter 后恢复 `true`；焦点环可见。
- 手机 390×844：打开目录后按钮为 44×44、箭头为 5×5，无水平页面溢出。
- 深色手机：保存设置成功，实际 `.theme-dark`、背景 `rgb(20,20,20)`；箭头仍为 5×5，继承深色前景。浏览器错误日志为空。
- 来源分组：月份分支复用相同控件，确认箭头 5px、按钮宽 44px。
- `git diff --check`：通过。仅改局部 CSS，没有新增镜像实现的自动化测试，也未运行全 Rust workspace 测试。

### 间距补充验证

- 基于 `119dc73f`，`dx build --platform web --package rssr-app --locked` 再次成功，47.00 秒；同一既有版本提示。
- 隔离样例端口 18103。构建期间首次加载未完成，构建完成后重载，实际内容已显示后才测量。
- 桌面 1200×936：日期文字与六个子文章文字均为 x=996.5px；六个双行条目高度均为 46.375px（此前约 54.375px），上下 padding 为 4px、文字行距 19.2px，无水平页面溢出。
- 手机 390×844：日期与子文章文字均为 x=203px，双行条目 46.375px，无水平页面溢出。
- 手机点击收起后文章节点为 0，键盘 Enter 展开后为 6；错误日志为空。
- 目录虚拟列表按 DOM 实测行高处理，未改其算法；本轮没有重新验收大于 80 条的虚拟分支或全部主题组合。
- 新截图：`directory-spacing-desktop.png`、`directory-spacing-mobile.png`，位于下述证据目录。

## 结果与待跟进

- 代码修正与浏览器验证完成；重新构建/部署后可使用新样式。
- Penpot MCP 两次返回 HTTP 522；普通浏览器编辑页导航也超时。备用连接要求重新认证，未修改认证配置。因此本轮没有写入 Penpot，不得声称设计稿已同步。
- Penpot 恢复后，同步当前基准画板中的 `entry-directory-branch-toggle` 图形，以及文章条目 4px 上下内边距、至少 44px 高度、与日期文字对齐的缩进。保留状态、历史页和原有交互；重新导出并保存命名版本。
- 其他任务的 dirty / untracked / deleted 文件保持不动。

## 给下一位 Agent 的备注

- 证据目录：`C:/Users/QQ/.codex/visualizations/2026/10/26/01a1779a-f271-7ec2-ab70-c1ef756f9ffb`。
- 截图：`directory-arrow-5px-desktop.png`、`directory-arrow-5px-mobile.png`、`directory-arrow-5px-dark-mobile.png`、`directory-arrow-5px-desktop-final.png`（最后一张为深色）。
- Penpot 文件和既有画板清单见 `docs/design/penpot-code-baseline.md` 及证据目录的 `penpot-final-audit.json`；不要将本次局部修正标记为全文件重新对齐最新 HEAD。
