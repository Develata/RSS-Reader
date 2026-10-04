# 原生旧库升级验收

从空库运行最新 migrations 只能证明建库，不能证明旧版本升级。`scripts/verify_release_upgrade.py` 使用两个明确指定的真实 CLI，在新建目录中验证 v0.1.21 的 migrations 1–5 升到 1–7。它不接受已有数据库目录，也不读取日常安装的数据。

## 准备与执行

1. 从 v0.1.21 正式 Release 下载当前平台的 CLI 包，核对 ZIP/tar 的 SHA-256 与 Release API 的 `digest`；保留下载包和核对结果。
2. 从待验固定提交执行 `cargo build --locked --release -p rssr-cli`，按发布流程打包后解压。记录源码 SHA、Cargo.lock 哈希、编译器、包和可执行文件哈希。
3. 用解压后的两个 CLI 执行下列命令。`--output-dir` 必须尚不存在；Windows 优先使用 E 盘任务目录。不要用 `python -O`，验收断言必须启用。

```powershell
python -X utf8 scripts/verify_release_upgrade.py `
  --legacy-cli target/upgrade/legacy/rssr-cli.exe `
  --candidate-cli target/upgrade/candidate/rssr-cli.exe `
  --output-dir target/upgrade/run-1
```

脚本只监听随机 loopback 端口提供合成 RSS；订阅、索引、正文、已读、设置与自定义 CSS 由旧 CLI 创建。旧 CLI 无收藏命令，因此仅收藏标记通过旧 schema 的 SQL 夹具写入；这一点写入报告，不冒充旧 GUI 全流程。

## 断言与证据

- 旧库确实只有 migrations 1–5，包含 3 个订阅、144 篇正文、48 个已读、20 个收藏，以及非默认设置和自定义 CSS。
- 0006/0007 升级后比较所有旧有字段与正文，检查 integrity / foreign keys、generation 默认值和清理队列状态。
- 三次独立启动候选 CLI 后数据与 CSS 不变。
- 分别在 SQLx 写入迁移历史时用 SQLite trigger 注入失败，验证该次 migration 的 DDL 和历史记录一起回滚；移除故障后重试成功。0007 失败时保留已提交的 0006。
- 旧库持有写锁时强制终止候选进程，再启动可完成升级。此项不等于验证断电、磁盘损坏或正在刷盘时的崩溃恢复。
- 旧 CLI 拒绝已升级的迁移历史；恢复升级前的两个原始库后旧 CLI 可正常打开。
- 原始库全程保留，最终哈希与创建时一致；失败/重试目录、命令参数与 stdout/stderr 都不清理。

`summary.json` 是本轮结果，`commands.json` 是调用证据，`legacy-snapshot.json` 是旧数据基准。原始库位于 `legacy-original/`，其它场景各自使用副本。脚本不证明二进制来源，调用者仍须完成前述官方包核验。

## 仍需独立完成的发布门禁

升级脚本不能替代 Windows GUI、Android 实机、签名和最终多平台发布包验收。GUI 测试要另复制原始库到隔离安装，先用旧版启动，关闭后覆盖该测试安装中的程序，再验证新版和重启；不修改 `legacy-original/`。Android 必须在安全隔离应用/数据上验证保留数据升级，不能卸载用户应用来获得“干净通过”。降级操作边界见[数据备份](../user-guide.md#数据备份)。
