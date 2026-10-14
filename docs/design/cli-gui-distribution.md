# GUI 与 CLI 的分发形式（已确认设计，Scoop 包待实施）

- 决定日期：2026-10-13（Develata 确认）
- 背景：审计遗留问题“Scoop 分发时 GUI 与 CLI 是否共用数据库”，以及“能否把 CLI 并入 GUI”。设计时分别由 Claude 与 Codex（gpt-6-astra）独立分析，结论一致。

## 决定

**GUI 与 CLI 保持两个可执行文件、一套业务代码，不合并成一个二进制。** Windows 上以一个包同时分发两者，并让它们共用同一份数据。

这与[功能设计哲学](./functional-design-philosophy.md)的“GUI 和 CLI 是同一产品能力的不同入口”一致：合一的是业务规则（application / infra），不是可执行文件。

## 理由

1. **没有图形界面的 Linux 也要能运行 CLI。** 在 Linux 上合并后，程序会链接 GTK / WebKitGTK 等 GUI 共享库；动态链接器在执行 `main` 之前就要求这些库存在，CLI 分支不创建窗口也无济于事。服务器、容器和无桌面环境里供 agent 使用的 CLI 不应依赖它们。
2. **Windows 的“窗口程序 / 控制台程序”是文件属性。** PE 头里的 `Subsystem` 字段（2 = 窗口，3 = 控制台）在程序运行前就决定了启动方式，运行后检查参数也改不了：
   - 窗口程序：双击不弹控制台，但在 cmd / PowerShell 里直接运行时 shell 不等待它结束，拿不到输出和退出码。
   - 控制台程序：shell 会等待、能取得输出和退出码，但双击时会弹出控制台窗口。
   - 唯一干净的单文件做法是控制台程序加清单项 `consoleAllocationPolicy=detached`，但它要求 Windows 11 24H2（build 26100）或更新版本；更早的 Windows 会闪出控制台窗口。见 [Console Allocation Policy](https://learn.microsoft.com/en-us/windows/console/console-allocation-policy)。
   - 其余做法（窗口程序里 `AttachConsole`、控制台程序里 `FreeConsole` 或隐藏窗口）都无法同时满足“双击不闪窗”和“shell 等待它结束”。
3. **CLI 保持轻量。** 现在的 release 版 CLI 跑 `list-feeds` 时峰值工作集约 8 MiB、耗时约 35 ms（2026-10-12 在 Windows 上实测）。合并后 GUI 依赖的 DLL 会在进程启动时载入，部分 GUI 库在 `main` 之前就会执行初始化，这些开销都没有必要。

## 不变量

- 业务规则只有一份：GUI 与 CLI 都直接调用 application 层；平台差异只在入口和 infra。
- 数据身份由数据目录决定，不由当前工作目录、父进程、终端类型或安装版本决定。
- CLI 是独立完成的进程，不要求 GUI 正在运行：结果写 stdout，诊断写 stderr，结束时返回退出码。
- 升级只替换程序，不替换用户数据。

## 打包与数据目录

| 分发形式 | 布局 |
|---|---|
| Windows zip / Scoop | `RSS-Reader.exe` 与 `rssr-cli.exe` 放在同一目录，共用旁边的 `RSS-Reader/` 数据目录 |
| Linux `.deb` | 两个程序都装到 `/usr/bin`，共用 `$XDG_DATA_HOME/rss-reader/`（未设置时为 `~/.local/share/rss-reader/`） |
| Linux 便携包 | 两个程序同目录，共用旁边的 `RSS-Reader/` |
| macOS | v0.2 起为 `~/Library/Application Support/RSS-Reader/`（签名后的应用包视为只读）；v0.1.x 的数据在应用包内，替换应用包会删除它，升级前由用户把旧目录中的文件（两个旧库及可能存在的 `-wal` / `-shm`）复制到新目录，任一新版首次启动时导入 |

计划中的 Scoop 清单关键字段（完整清单还需版本、下载地址与校验和）：

```json
{
  "bin": [["rssr-cli.exe", "rssr"]],
  "shortcuts": [["RSS-Reader.exe", "RSS-Reader"]],
  "persist": "RSS-Reader"
}
```

- `bin` 只给控制台程序 `rssr-cli.exe` 建命令 `rssr`。Scoop 会给窗口程序生成窗口类型的 shim，不要把 `RSS-Reader.exe` 当成命令行入口暴露。
- `shortcuts` 给 GUI 建开始菜单快捷方式，指向真实的 `RSS-Reader.exe`。
- `persist` 持久化**整个** `RSS-Reader/` 目录：数据库（v0.1.x 为索引库与正文库两个文件，v0.2 起为一个 `library.sqlite3`）、`-wal` / `-shm`、迁移锁与界面偏好必须一起保留，只持久化单个数据库文件会遗漏其余状态。Scoop 用目录联接把它接到 `persist/` 下，升级换版本目录时数据不丢。

## 共用数据库的约束

- SQLite WAL 允许跨进程读写并行，但每个数据库同一时刻只有一个写者；设置的 30 秒 `busy_timeout` 是等锁时长，不是整条命令的超时。
- 迁移锁只串行化迁移，不保护已经在运行的旧版本进程。升级前应退出所有使用同一数据目录的 GUI 与 CLI。
- 刷新目前只在同一进程内协调；GUI 自动刷新与 agent 用 CLI 刷新同一订阅时可能交错。跨进程刷新协调（持久化抓取序号与条件发布）在 v0.2 阶段 A 实施，见 [v0.2 架构](./v0.2-architecture.md#存储)。
- 打包的 SQLite 需不低于 3.51.3：更早版本有官方确认的 WAL-reset 缺陷，触发条件正是同一库文件上多个连接同时写入或做检查点（见 [SQLite WAL](https://sqlite.org/wal.html#walreset)）。现由 `rusqlite 0.40.2`（`libsqlite3-sys 0.38.2`，SQLite 3.53.2）满足，测试 `bundled_sqlite_has_the_wal_reset_fix` 守住这一基线。

## 服务器上的 CLI 与 Web 部署

- v0.1.x：Web 部署（`rssr-web`）托管静态 Web 应用、登录与 `/feed-proxy`，文章库在各自的浏览器里，与服务器上 CLI 的数据库互不相通。
- v0.2 起（已决定，见 [v0.2 架构](./v0.2-architecture.md)）：Web 是服务器端阅读器，资料库保存在服务器上；服务器上的 CLI 用于自动化（导入导出、供 agent 调用），通过同一 `--data-dir` / 环境变量与 Web 共用这份资料库。定时刷新由服务器进程负责。

## 发布前须在真实 Windows 上验收

- 发布文件的 `Subsystem`、清单与导入的 DLL 符合设计（用 `DUMPBIN` 检查实际发布的文件，不用 debug 构建代替）。
- 双击与开始菜单快捷方式启动 GUI：无多余控制台窗口。
- CLI：在 cmd、Windows PowerShell 5.1、PowerShell 7 中，于 conhost 与 Windows Terminal 下验证等待与退出码；stdout / stderr 的分开与合并重定向、管道、中文与含空格参数。
- Agent 通过管道调用：直接调用 exe 与经 Scoop shim 调用两条路径。
- Scoop 安装、更新、清理旧版本、卸载后重装：两个入口始终使用同一对数据库，数据不丢。
- GUI 运行时并发执行 CLI：写锁等待、迁移与升级时的行为。
