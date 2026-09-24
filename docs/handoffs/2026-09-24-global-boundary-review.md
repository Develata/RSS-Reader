# 全局检查：SQLite 身份、Web 网络与回跳、Debian 输出边界

- 日期：2026-09-24
- 作者 / Agent：Codex
- 分支：main
- 审查基线：b61aded，开始时工作树干净
- 相关 commit：4a8b70a（SQLite）、1ad4297（Web）、145eccd（Debian）；本文与覆盖映射随文档提交交付
- 相关 tag / release：无新 tag；未修改已发布 v0.1.17
- 状态：`validated`（以下自动化范围；不代表跨平台实机验收）

## 工作摘要

沿存储路径、刷新任务生命周期、Web 认证/代理、打包及 CI 聚合失败路径进行检查，复现并修复四项边界错误。保持既有 application/domain 与 host/infra 分层，没有新增依赖、crate 或平台业务分叉。

## 影响范围

- `rssr-infra::db` / `sqlite_native`：使用 SQLite 的 native 平台，实际运行验证在 Linux。
- `rssr-web::proxy` / `auth`：Web host / Docker。
- `scripts/prepare_linux_deb.sh`：Linux 发布包重写工具。
- Web 部署说明与主线验收覆盖矩阵；未修改 workflow、UI、版本或发布状态。

## 关键变更

### Web 请求与回跳边界

- 请求客户端原本默认继承环境代理，代理端可能自行解析目标主机，绕过经校验后固定的 IP。固定 IP 的客户端显式禁用环境代理；部署文档说明服务器需要直接访问公开 feed。
- 隔离子进程设置六种大小写环境代理变量，使用伪域名 + 固定 localhost 地址请求实际 HTTP fixture。修复前错误连接 `127.0.0.1:0`；修复后返回 200。只在测试内用 localhost，不放宽生产地址校验。
- 登录回跳原本只检查 `/` / `//` 前缀。`/\evil.example` 经 URL 解析成为外站地址；控制字符还可能生成无效 Location。现在拒绝反斜杠及 ASCII 控制字符，保留正常路径与编码后的中文查询。
- 回归测试实际解析回跳 origin，并检查 HTTP header 可用性；修复前外站 origin 断言失败，修复后通过。

### SQLite 路径与内存库身份

- 统一 URL 内存库分类：检查数据库部分及解码后的精确查询键值，不再对子串 `mode=memory` 进行全 URL 搜索。正文库 URL 推导复用同一规则。
- 显式文件路径不再交给 URL 分类器；复用文件库连接数常量，保持 WAL 与既有连接池容量。
- 修复前，包含 `mode=memory.db` 的真实文件路径使索引与正文库指向同一文件；`sqlite://:memory:` 的正文 URL 又被错误加上文件后缀。修复后中文/空格目录中的实际 SQLite 测试确认两个文件分离且 WAL 生效。
- Unix 文件名包含 `?mode=memory` 时仍按文件处理；内存库连接数保持保守的 1。
- 无自动数据迁移；曾经使用异常自定义路径的部署，应备份并检查其旧索引/正文布局，本轮未验证历史异常布局的数据恢复。

### Debian 工具输出身份

- 拒绝与输入同 inode 的输出（路径别名、硬链接、符号链接），并拒绝目录输出。
- 修复前 `./original.deb` 被接受并覆盖输入；修复后拒绝这些输入，原包 Version 保持不变，目录中不遗留误放置的 `repacked.deb`。

## 验证与验收

### 自动化验证

以下均通过：

- `cargo test --locked --workspace --exclude rssr-app`：覆盖 domain/application/infra/CLI/Web，0 失败；既有 1 项 ignored 保持未执行。此命令执行于新增登录回跳测试前，之后单独重跑完整 Web 测试。
- `cargo test --locked -p rssr-web`：最终 19 项通过，包含回跳和环境代理回归测试。
- `cargo clippy --locked --workspace --exclude rssr-app --all-targets -- -D warnings`，最终 auth 修改后另跑 `cargo clippy --locked -p rssr-web --all-targets -- -D warnings`。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`。
- `cargo fmt --all --check`。
- `bash scripts/test_prepare_linux_deb.sh`。
- `shellcheck scripts/prepare_linux_deb.sh scripts/test_prepare_linux_deb.sh`。
- `/home/deve/go/bin/actionlint`。
- `git diff --check`。

上述四类回归均先复现失败再修复，没有用旧 release 产物证明新实现有效。测试日志在 `/tmp/rssr-review-20260924-{tests,web-tests,clippy,wasm}.log`，属于本机临时证据。

### CI 与手工验收

- 只读核对基线 b61aded 的 CI `35921066108` 和 Docker `35921066184` 均成功；这不证明本轮新提交的远端 CI 已通过。
- 检查现有模块矩阵、`fail-fast: false` 与汇总依赖状态判定；新 Rust 测试沿用现有模块 job，Debian 回归沿用 tools job，没有另建孤立验收链。
- 未执行完整 native workspace GUI 测试：本环境 `pkg-config` 未找到 GTK3/WebKit2GTK 4.1 开发依赖。
- 未执行 Windows/Android 实机验收、Web release 构建、浏览器视觉 smoke 或公开互联网 feed 端到端 smoke。本轮未修改 UI；网络回归使用实际本地 HTTP fixture，不能替代部署网络验证。
- Android Rust target 未安装，本轮未安装 SDK 或依赖。

## 结果与风险

- 已完成本轮可复现问题的实现、回归与本地分批提交；未 push、未 tag、未发布。
- 依赖出站环境代理的 Web host 部署需要调整网络配置；保持安全目标校验优先，不能静默依赖代理端 DNS。
- 没有性能基准测量，不宣称性能提升；保持现有复杂度，改动限于身份分类与安全边界。
- 本轮检查未覆盖全部用户路径，也没有宣称无剩余问题。

## 给下一位 Agent 的备注

- 首先查看上述三个代码提交与 `docs/testing/mainline-validation-matrix.md`。
- 下一步如准备发布，先获得本轮提交的远端 CI 结果，并在具备原生依赖的平台验证新构建；不能复用基线 CI 或旧发布收据。
