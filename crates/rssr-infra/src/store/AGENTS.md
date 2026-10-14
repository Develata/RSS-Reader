# 资料库存储模块说明

本目录是原生端（桌面 / Android / CLI）唯一的本地存储：一个 SQLite 资料库 `library.sqlite3`（rusqlite），装索引、正文、设置、应用状态与刷新序号。设计依据见 `docs/design/v0.2-architecture.md` 的“存储”与“兼容性”两节。

## 职责边界

- `executor.rs`：连接归属与任务执行（专属线程、队列容量、准入时限、运行期限、取消与提交点）；打开与新建资料库。
- `connection.rs`：每个连接的统一设置（外键、busy timeout、`fold_for_search` SQL 函数、WAL）。
- `files.rs`：文件层面的事——身份探测（`immutable=1`）、创建锁、暂存文件与原子发布。
- `migrate.rs` + `migrations/`：基线 schema 与迁移器（应用 id、LF 校验和、单事务）。
- `feeds.rs` / `entries.rs` / `documents.rs` / `mutations.rs` / `refresh.rs`：各领域端口的 SQL 实现；`refresh.rs` 持有刷新序号 `I/P/D` 的规则。
- `import.rs`：v0.1.x 旧库的单向导入（只读快照、迁移历史核对、整库映射、校验后发布）。
- `local.rs`：各平台数据目录规则与“打开或导入”入口；GUI、CLI 与界面偏好共用。
- `codec.rs` / `error.rs`：时间与 URL 编解码、错误分类。

## 关键原则

- 仓储层只处理存储与查询，不处理 UI 语义；网络请求与解析不在数据库事务内进行。
- 每个任务一个事务；任务内只做由输入规模与库快照约束的工作（执行器拦不住任务里的 Rust 循环）。
- 新资料库只能经创建锁 + 暂存文件 + 原子发布出现；不得直接在正式路径上建库或覆盖已有文件。
- 打开已有文件前先确认身份；任何可能写入的操作都在确认之后。
- 性能敏感路径（列表查询、刷新提交）优先：索引友好、避免全表扫描模拟导航、减少 clone 与分配；列表查询不读正文列。

## 修改约束

- 任何文章排序或筛选变更，都要同步考虑：文章页列表、阅读页上一 / 下一未读、搜索结果一致性；跑 `test_library_entry_state_and_search`。
- 改刷新提交规则（序号、校验器、诊断）时跑 `test_library_refresh_tickets` 与刷新相关的 `test_library_*` 契约测试。
- 改执行器或文件层时跑 `test_library_store`、`store::` 单元测试，并在 Linux（WSL）上跑一遍：POSIX 锁与路径语义只在 Unix 上成立。
- 改导入器时跑 `test_library_import`，并按 `docs/testing/native-upgrade-verification.md` 用官方旧版 CLI 跑升级验收脚本。
- `migrations/` 与 `migrations_content/`（仓库根目录）是冻结的 v0.1.x 历史，只供导入器核对校验和，不得修改。
- 不把 OPML / JSON 配置交换逻辑塞进仓储层；不把 Web `localStorage` 语义混进 SQLite 路径。
