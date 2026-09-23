# README 与使用、部署文档整理

- 日期：2026-09-23
- 作者 / Agent：Codex
- 分支：`main`
- 当前 HEAD（任务基线）：`2038eeb304fd697eaf67a1985ad3c0e5c8059122`
- 相关 commit：`commit: pending`（记录撰写时；提交号以本文件 Git 历史为准）
- 相关 tag / release：公开基线 `v0.1.15`；本次未打 tag 或发布新版
- 状态：`validated`（文档和配置静态验证；设备及镜像构建未运行）

## 工作摘要

参考 StickyMD README 的任务优先层次，把 RSS-Reader 首页从长篇部署模板整理成下载、快速阅读、数据边界和开发入口；把仍需保留的使用与部署细节移入专门文档，并校正已发布产物及 Android 验收状态。

## 影响范围

- 模块：根 README、英文 README、`docs/` 索引 / 使用 / Web 部署 / Android 路线图 / 交接记录、贡献与开发指引、`.dockerignore`。
- 平台：Windows、Linux、macOS、Web、Android 的文档；Docker 源码构建上下文排除本地 `.env`。
- 额外影响：未修改 Rust 产品代码、CI workflow、发布权限或已有历史交接结论。

## 关键变更

### 下载与使用信息

- 核对公开 `v0.1.15` 的 11 个实际附件，修正 Linux 应用包名为 `.deb`，区分 Android 可安装的 APK 与商店 AAB。README 不再把 Android 签名包写成待配置 secrets；明确 Android/macOS 实机仍未验收，也明确 `main` 后续修复尚未进入旧 Release。
- 将 R / 搜索 / 阅读 / 主题 / CLI / 本地存储等日常路径收束到 README，细节移入 `docs/user-guide.md`；英文 README 同步下载身份和主要使用边界。
- 重新核对 `v0.1.15` Linux `.deb` 内容：可执行文件安装在 `usr/bin/rssr-app`，而 `NativeSqliteBackend::from_default_location()` 使用可执行文件目录下的 `RSS-Reader/`。普通账户通常无权在 `/usr/bin` 创建该目录。README 已明确说明包虽发布，但普通用户启动尚无可用性证明；后续需单独设计数据目录与迁移。

### 部署与链接

- Web 登录、Compose、生产 cookie、认证状态及本地代理验证集中到 `docs/deployment/web.md`。正式部署的 Argon2 哈希在 Compose `.env` 中需以单引号保存 `$`；本地 Compose 解析已验证。
- `.dockerignore` 新增 `.env` / `.env.*`，避免源码构建镜像的 `COPY . .` 把本地凭据带进构建上下文。
- Android 路线图从旧的“待接入发布”改为已签名发布与待真机验收的事实状态；当前维护文档中的旧主机绝对链接改为相对路径，交接历史文件保持原样。

## 验证与验收

### 自动化验证

- `gh release view v0.1.15 --json assets`：核对 README 中文 / 英文列出的 6 个应用附件均存在；公开 Release 另含 CLI 与 AAB，总计 11 个构建资产。
- 在隔离临时目录下载 `RSS-Reader-linux-x86_64.deb` 并执行 `dpkg-deb -c`：确认 `usr/bin/rssr-app`；对照当前源码数据目录计算，识别写权限风险。未在普通用户会话实际安装运行。
- Python 本地 Markdown 路径检查：当前 README、开发 / 设计 / 测试 / 路线图 / 部署等 46 份维护文档的 254 个相对链接目标均存在；未检查外部站点实时可达性或 GitHub 锚点渲染。
- `docker compose config --quiet`：通过；隔离 `.env` 下 `config --environment` 确认单引号 Argon2 示例中的 `$` 被原样读取。未实际构建或启动镜像。
- Docker 隔离临时上下文 `FROM scratch` 构建：普通文件可复制到输出目录，`COPY .env` 按预期失败，确认新增 `.dockerignore` 规则确实从构建上下文排除本地凭据；没有构建完整 RSS-Reader 镜像。
- `cargo metadata --locked --no-deps`：确认 6 个 workspace crate 与 Dioxus `0.7.9` 约束。
- `git diff --check`、`cargo fmt --all --check`：通过。

### 手工验收

- 按下载 → 快速使用 → 数据与限制 → 开发 / 部署顺序复读中文与英文 README，核查所有新增文档入口。
- 未运行 Web / Windows / Android / macOS UI 或 Docker 镜像构建；本次没有修改对应运行时代码。前一条产品修复提交的验证见 `2026-09-23-home-search-context-menu.md`，不能由本次文档检查替代。

## 结果

- 文档和构建上下文规则可提交；原有 `v0.1.15` 标签与 Release 未改变。
- `README.md` 不再把已知 Linux `.deb` 风险表述为“安装即可使用”。

## 风险与后续事项

- Linux `.deb` 安装后普通用户写权限问题需单独修复；改变原生数据目录前必须考虑已有 portable 目录和既有数据库迁移，不能只改一个默认路径。
- Docker `.env` 忽略规则已用隔离最小构建验证；完整产品镜像、外部链接与 Android/macOS 设备行为仍未本轮复验。
- 英文 README 提供主要流程；详细使用和部署文档当前以中文为主。

## 给下一位 Agent 的备注

- 使用者入口是 `README.md` / `docs/README.en.md`；具体操作见 `docs/user-guide.md` 与 `docs/deployment/web.md`。
- Linux 数据路径问题从 `crates/rssr-infra/src/db/sqlite_native.rs` 的 `local_data_base_dir()`、`NativeSqliteBackend::from_default_location()` 和 `.deb` 的 `usr/bin/rssr-app` 一起分析；不要让包存在或 CI 结构检查冒充运行成功。
