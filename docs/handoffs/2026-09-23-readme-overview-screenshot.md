# README 实际界面预览图

- 日期：2026-09-23
- 作者 / Agent：Codex
- 分支：`main`
- 当前 HEAD（任务基线）：`46ee54a`
- 相关 commit：`commit: pending`（撰写时；最终提交以 Git 历史为准）
- 相关 tag / release：未打 tag，未发布新版
- 状态：`validated`

## 工作摘要

在中英文 README 徽章下增加一张可直接看到 RSS-Reader 阅读首页的真实浏览器截图，回应读者进入仓库时缺少产品界面预览的问题。

## 影响范围

- 模块：`README.md`、`docs/README.en.md`、`assets/readme/rss-reader-overview.png`。
- 平台：展示的是 Web 版；产品代码及其它平台未改动。
- 额外影响：仅仓库文档和图片资产。

## 关键变更

- 使用当前工作树的 Web release bundle、独立 Chrome profile 和仓库已有的静态 Web 认证 helper，在真实浏览器中渲染文章首页。
- 以隔离的本地演示状态替换 `reader-demo` 的条目，使用三个演示来源与六篇演示文章；没有读取或展示用户的订阅、文章库和凭据。截图为 1365×900 CSS px、DPR 2 的原始浏览器 PNG，未进行拼接或绘制 UI。
- 两份 README 都在图片下明确说明内容是演示数据。选择文章首页作为主图，可直接看到导航、文章卡片和时间目录；没有把阅读页额外塞进首页。

## 验证与验收

### 自动化验证

- `dx build --platform web --package rssr-app --release --locked --debug-symbols false`：命令以 0 退出并生成 bundle；本机 `dx 0.7.10` 与项目 Dioxus `0.7.9` 不一致，构建日志报告兼容性警告，见风险。
- CDP 浏览器截图检查：文章首页渲染 6 张卡片，首篇标题正确；1365×900 视口没有横向溢出，也没有浏览器运行时异常。
- `identify`：提交图片为 2730×1800 PNG，大小约 652 KB。
- `git diff --check`、`cargo fmt --all --check`、README 图片相对路径、PNG 签名与尺寸检查：均通过。

### 手工验收

- 已目视检查文章首页截图：R、搜索、S、设置、文章列表和目录均可见，未出现调试弹窗或加载占位。
- 同次运行的阅读页也渲染成功并经目视检查，但没有加入 README，以保持首页简洁。

## 结果

- README 首屏有真实运行画面；截图所用文章是演示内容，不代表远端 feed 或发布产物的实机状态。

## 风险与后续事项

- 图片展示当前 `main` 的 Web 版，不代表 `v0.1.15` 下载附件或 Windows/Android/macOS 外观完全相同；之后 UI 大改需更新截图。
- 本机 `dx` 比项目锁定的 0.7.9 新一小版；此次完成了浏览器渲染验收，但若需精确复现发布构建，应使用 CI 的锁定工具版本。
- 未执行全量 Rust 测试；本次没有产品代码变更。

## 给下一位 Agent 的备注

- 现有 `scripts/run_web_spa_regression_server.sh` 支持 `--seed reader-demo`，可用隔离浏览器状态再次截图；不要使用个人日常 profile 或把用户订阅数据提交到仓库。
