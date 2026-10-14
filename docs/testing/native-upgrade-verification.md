# 原生旧库升级验收

v0.2 不再原地迁移旧库，而是把 v0.1.x 库单向导入新的 `library.sqlite3`（[v0.2 架构](../design/v0.2-architecture.md#兼容性断代)）。从空库建库只能证明新建，不能证明导入。`scripts/verify_release_upgrade.py` 使用两个明确指定的真实 CLI：官方旧版用 `--database-url` 建一份有数据的旧库，候选版用 `--data-dir` 在副本上导入，再逐列比对。旧版应取最新的正式发布，并加测一个只有迁移 1–5 的版本（v0.1.21；v0.1.22 起为 1–7）。脚本不接受已有输出目录，也不读取日常安装的数据。

v0.1.x 之间原地升级（迁移 0006 / 0007 的注入失败、回滚与降级探针）的旧版脚本见 tag `v0.1.23` 的同名文件。

## 准备与执行

1. 从正式 Release 下载当前平台的 CLI 包，核对 ZIP/tar 的 SHA-256 与 Release API 的 `digest`；保留下载包和核对结果。
2. 从待验固定提交执行 `cargo build --locked --release -p rssr-cli`（发布前用按发布流程打包后解压出的 CLI）。记录源码 SHA、Cargo.lock 哈希、编译器和可执行文件哈希。
3. 用两个 CLI 执行下列命令。`--output-dir` 必须尚不存在；Windows 优先使用 E 盘任务目录。不要用 `python -O`，验收断言必须启用。

```powershell
python -X utf8 scripts/verify_release_upgrade.py `
  --legacy-cli target/upgrade/legacy-v0.1.23/rssr-cli.exe `
  --candidate-cli target/upgrade/candidate/rssr-cli.exe `
  --output-dir target/upgrade/import-run-1
```

脚本只监听随机 loopback 端口提供合成 RSS；订阅、索引、正文、已读、设置与自定义 CSS 由旧 CLI 创建。旧 CLI 无收藏命令，因此仅收藏标记通过旧 schema 的 SQL 夹具写入；这一点写入报告，不冒充旧 GUI 全流程。候选进程的环境中去掉 `RSSR_DATA_DIR`，数据目录只由命令行给出。

## 断言与证据

- 旧库的迁移历史是连续的 1–k（k 记入 `summary.json` 的 `legacy_migrations`），包含 3 个订阅、144 篇正文、48 个已读、20 个收藏，以及非默认设置和自定义 CSS；快照 `legacy-snapshot.json` 记录 feeds / entries / app_settings / entry_contents 的每行每列、`sqlite_sequence` 与迁移历史。
- 候选版导入后：完整性与外键检查通过，应用 id 与 WAL 模式正确；订阅的每个旧列（含 `generation`）不变，`custom_title` 为空，三个刷新序号为 0；文章除 `has_content` 外每个旧列不变，`has_content` 与正文是否存在一致，`title_search` 为折叠标题；正文按存活文章保留且去掉冗余的 `feed_id`；设置与应用状态的 JSON 和 `updated_at` 不变，不多出别的文档；订阅与文章的 AUTOINCREMENT 高水位不变；没有遗留暂存文件；两个旧文件的哈希不变。
- 三次独立启动候选 CLI 后数据与 CSS 不变；导入只发生一次（之后改动旧文件不会再被读入）。
- 迁移历史里多一条未知迁移（在已发布范围内校验和不符，或超出已发布范围）时拒绝导入，不创建资料库、不留暂存文件；删掉该记录后导入成功。
- 被强杀的导入留下的暂存文件（`library.sqlite3.importing` 等）不妨碍下次导入，并被清理。
- 另一方持有创建锁（`library.sqlite3.create-lock`）时候选进程等待；此时强制终止它不留下资料库或暂存文件，释放锁后再启动可完成导入。此项不等于验证断电、磁盘损坏或正在刷盘时的崩溃恢复。
- 导入后旧 CLI 仍能打开未被改动的旧文件；原始库全程保留，最终哈希与创建时一致；各场景目录、命令参数与 stdout/stderr 都不清理。

`summary.json` 是本轮结果，`commands.json` 是调用证据，`legacy-snapshot.json` 是旧数据基准。原始库位于 `legacy-original/`，其它场景各自使用副本。脚本不证明二进制来源，调用者仍须完成前述官方包核验。

覆盖范围：真实发布版只验证了它们写出的双库布局；v0.1.0–v0.1.6 的单库布局（正文在索引库内）、v0.1.2–v0.1.6 的 `last_opened_feed_id` 与 v0.1.6 的列表偏好由 `crates/rssr-infra/tests/test_library_import.rs` 用已发布迁移文件构造的夹具覆盖，未用这些版本的官方程序实测。

## 仍需独立完成的发布门禁

升级脚本不能替代 Windows GUI、Android 实机、签名和最终多平台发布包验收。GUI 测试要另复制原始库到隔离安装，先用旧版启动，关闭后覆盖该测试安装中的程序，再验证新版首次启动的导入和重启；不修改 `legacy-original/`。macOS 还须验证从应用包旁的旧目录导入到 `~/Library/Application Support/RSS-Reader/`。Android 必须在安全隔离应用/数据上验证保留数据升级（含应用私有目录中的创建锁与原子发布），不能卸载用户应用来获得“干净通过”。降级操作边界见[数据备份](../user-guide.md#数据备份)。
