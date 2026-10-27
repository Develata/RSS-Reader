# 资料库存储模块说明

本目录提供所有本地宿主共用的 SQL 仓储：原生（桌面 / Android / CLI / 服务器）使用 `library.sqlite3`；Pages 使用页面线程上的私有内存 SQLite。两端均装索引、正文、设置、应用状态与刷新序号。设计依据见 `docs/design/v0.2-architecture.md` 的“存储”与“兼容性”两节。

## 职责边界

- `wasm_executor.rs`：页面线程 `thread_local` 连接、Send + Sync 索引句柄、有限准入、取消/截止与重载重置；不实现浏览器持久化。
- `options.rs`：两端执行器参数。
- `executor.rs`：连接归属与任务执行（专属线程、队列容量、准入时限、运行期限、取消与提交点）；打开与新建资料库。
- `connection.rs`：每个连接的统一设置（外键、busy timeout、`fold_for_search` SQL 函数、WAL）。
- `files.rs`：文件层面的事——身份探测（`immutable=1`）、创建锁、暂存文件与原子发布。
- `migrate.rs` + `migrations/`：基线 schema 与迁移器（应用 id、LF 校验和、单事务）。
- `feeds.rs` / `entries.rs` / `documents.rs` / `mutations.rs` / `refresh.rs`：各领域端口的 SQL 实现；`refresh.rs` 持有刷新序号 `I/P/D` 的规则。
- `workspace_edits.rs`：显式文章集合的批量标记 / 手动归档、保留订阅身份的编辑事务；地址变化在同一事务中失效旧刷新 ticket 并迁移已选来源地址。
- `queries.rs`：application 的聚合查询端口（`LibraryQueryPort`），每个方法一次读事务，只组合上面各文件已有的查询函数。
- `import.rs`：v0.1.x 旧库的单向导入（只读快照、迁移历史核对、整库映射、校验后发布）。
- `local.rs`：各平台数据目录规则与“打开或导入”入口；GUI、CLI 与界面偏好共用。
- `local/android_storage.rs`：Android 私有目录与旧位置的保留式迁移；校验、锁和完整快照都在平台适配边界内，不能改变共享仓储语义。
- `codec.rs` / `error.rs`：时间与 URL 编解码、错误分类。

## 关键原则

- 仓储层只处理存储与查询，不处理 UI 语义；网络请求与解析不在数据库事务内进行。
- 每个任务一个事务；任务内只做由输入规模与库快照约束的工作（执行器拦不住任务里的 Rust 循环）。
- 原生文件资料库只能经创建锁 + 暂存文件 + 原子发布出现；不得直接在正式路径上建库或覆盖已有文件。
- 打开已有文件前先确认身份；任何可能写入的操作都在确认之后。
- 共用数据目录的程序都必须经 `local::open_library_in` / `open_local_library` 打开：直接 `LibraryStore::open` 会在没有资料库时建空库而不查找旧库，从而让其他程序跳过导入。
- 程序退出前必须 `LibraryStore::close().await`（不能 await 的地方用 `close_blocking`）并等它返回，出错退出的路径也一样：只丢弃句柄不等连接关闭，进程退出可能截断最后一个连接关闭时的检查点，留下身份探测拒绝的主文件。
- 性能敏感路径（列表查询、刷新提交）优先：索引友好、避免全表扫描模拟导航、减少 clone 与分配；列表查询不读正文列。

## 修改约束

- 任何文章排序或筛选变更，都要同步考虑：文章页列表、阅读页上一 / 下一未读、搜索结果一致性；跑 `test_library_entry_state_and_search`。
- 改刷新提交规则（序号、校验器、诊断）时跑 `test_library_refresh_tickets` 与刷新相关的 `test_library_*` 契约测试。
- 改 WASM 执行器时跑 `wasm_memory_store`、共享 SQL 浏览器案例和 Pages 页面验收，记录 SQLite 版本并检查正式 Web 不带 SQLite。
- 改执行器或文件层时跑 `test_library_store`、`store::` 单元测试，并在 Linux（WSL）上跑一遍：POSIX 锁与路径语义只在 Unix 上成立。
- 改导入器时跑 `test_library_import`，并按 `docs/testing/native-upgrade-verification.md` 用官方旧版 CLI 跑升级验收脚本。
- `migrations/` 与 `migrations_content/`（仓库根目录）是冻结的 v0.1.x 历史，只供导入器核对校验和，不得修改。
- 不把 OPML / JSON 配置交换逻辑塞进仓储层；不把 Web `localStorage` 语义混进 SQLite 路径。
