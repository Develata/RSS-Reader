# 订阅与设置页顶部层级统一

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/secondary-page-surfaces`
- 基线 HEAD：`6a201d5a83efa730c48a966e002d2f76025cfcda`
- 相关 commit：代码 `4167a08c65c1fde28c6af9c431a749a1e075a601`；本次后续文档提交记录设计版本。
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

用户指出订阅与设置页仍由旧的整页大面板包住导航，与文章页的独立导航、内容面板层级不一致。本次沿用已授权的独立分支、PR、检查通过后合并及 Penpot 同步流程。

## 影响范围

- `rssr-app` 订阅页、设置页、共享样式及已有浏览器验收选择器。
- 共享前端；实际浏览器验收为 Windows Chrome。原生设备本轮未重新实测。
- 复用干净的隔离工作区 `E:\gitclone\RSS-Reader-directory-ui-20261029`，从最新 main 新建分支；原工作区的其他改动未触碰。

## 关键变更

- 两页外层去除旧面板背景、描边、阴影和内边距；导航保持页面直属，下方用 `page-main` 承载原有内容。
- 主面板沿用文章页的主题表面、圆角、桌面 16px / 手机 12px 内边距；标题行至少 44px，设置页 GitHub 入口保持同排。
- 修正统计区及状态提示依赖旧直属关系的选择器；业务命令、表单和弹窗不变。
- Penpot 同步当前订阅、设置、编辑与错误状态示例；历史稿保留。

## 验证与验收

- `cargo fmt --all --check`、`git diff --check`：通过。
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过；颜色契约 3 项测试通过。
- dx 0.7.9 Web release、`rssr-web` 与真实服务器 fixture 构建：通过。
- 真实 Chrome：默认浅/深色及四套内置主题 × feeds/settings × 390/600/760/1000/1200 共 60 组；搜索、收起展开、滚动、44px 标题行、无横向溢出及页面错误检查通过。另有 compact 密度下三页 × 两宽度共 6 组，几何一致。
- 既有 workspace 回归：18 个功能段及无页面错误检查通过，涵盖订阅编辑、设置偏好、目录与主题。最后仅修正标题行 display/padding 后，重建并重跑上述 66 组视觉/交互检查。
- 独立只读复核：未发现阻断问题；确认 Atlas 导航直属关系及统计/状态选择器适配。
- Penpot：更新 11 张现有可编辑画板，保持历史页、画板 ID 和 capture-key；4 张完整页面按实际 DOM 几何同步，另外 7 张编辑/空白/错误状态同步顶部结构，保留原状态内容。11 处导航与主面板间隔均为 6px；`File.validate()` 无错误，订阅桌面/手机、设置手机及空白状态导出复核通过。
- 已保存并回读命名版本 `Secondary page surfaces · 4167a08c · 2026-10-29`；文件及画板元数据注明代码提交。订阅演示 URL 随本轮本地样例同步，保留原计数与其他说明文本。
- 证据目录：`C:\Users\QQ\Documents\Codex\2026-10-29\secondary-page-surfaces`。

## 结果

- 代码、设计与本地验收完成；PR 及远端 CI 将随提交完成。合并前检查最新提交的 CI 和审查结果。

## 风险与后续事项

- 自定义 CSS 的任意深层直属选择器不保证兼容；稳定入口及新增 `page-main` 已记录在主题作者文档。
- 共享前端的浏览器验收不等同于 Android 或桌面原生宿主实测。

## 给下一位 Agent 的备注

- 先读 `theme-author-selector-reference.md`；Atlas 依赖导航仍是页面直属，本次保持该结构。
- Penpot 文件 `cadc1646-c70c-8009-8008-d6f40c2d8fdc`；修改前命名版本为 `Before secondary-page surfaces · 2026-10-29`。
