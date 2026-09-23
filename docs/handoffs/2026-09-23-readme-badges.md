# README 发布、CI、许可证与平台徽章

- 日期：2026-09-23
- 作者 / Agent：Codex
- 分支：`main`
- 当前 HEAD（任务基线）：`f1145d946d522862969e1f28c992fc09bdd5de79`
- 相关 commit：`commit: pending`（记录撰写时；提交号以本文件 Git 历史为准）
- 相关 tag / release：公开最新版本 `v0.1.15`；本次不打 tag 或发布
- 状态：`validated`（README 链接与徽章端点）

## 工作摘要

补上前次精简 README 时遗漏的发布、CI、MIT 许可证和平台概览徽章，使首页与 StickyMD 的信息入口保持同类可读性，同时避免把跨端产品误标成仅支持 Windows 11。

## 影响范围

- 模块：根 `README.md`、`docs/README.en.md`、本交接记录。
- 平台：仅 GitHub 文档呈现；没有改动产品、CI 工作流或发布产物。

## 关键变更

- 中英文 README 顶部均加入动态 Latest release、`main` 分支 CI、MIT License 徽章。
- 第四枚静态徽章标记 Windows、Linux、macOS、Android、Web 为 Release targets，并链接到各语言的下载表；该徽章不代表每个平台已完成实机验收。Linux `.deb` 写权限风险和 Android/macOS 验收缺口仍在表格下明确披露。

## 验证与验收

### 自动化验证

- GitHub API：仓库默认分支为 `main`、许可证 SPDX 为 `MIT`，最近一次 `ci.yml` 在该分支完成且成功。
- HTTP GET：两枚 shields.io 动态徽章、GitHub Actions CI SVG 和静态 Targets SVG 均返回 200 与 SVG 内容类型。
- `git diff --check`、`cargo fmt --all --check`、README 徽章标签 / 链接检查：均通过。

### 手工验收

- 对照 StickyMD README 源码确认徽章排列和链接语义；未运行产品 UI，本次没有产品代码变更。

## 结果

- 首页读者可以直接看到发布版本、主线 CI、许可证和跨平台发布范围。

## 风险与后续事项

- 徽章是 GitHub/第三方服务动态图片；图像服务不可用时仍可通过旁边的文本下载和文档链接进入对应页面。
- Targets 是发布目标，不是设备可用性认证；Linux 安装包限制及 Android/macOS 实机缺口仍须按 README 正文理解。

## 给下一位 Agent 的备注

- 徽章位于两个 README 的标题和导语之间；不要把 CI 徽章解释成 Release 产物或设备交互已验收。
