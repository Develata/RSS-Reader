# SQLite 迁移校验与历史换行兼容

原生索引库与正文库均通过 `rssr-infra/src/db/migration_compat.rs` 运行 SQLx 迁移。
SQLx 的 SHA-384 校验覆盖 SQL 文件的物理字节。历史 Windows 检出可能将同一脚本
变成 CRLF，其他构建使用 LF，因此语义相同的旧库也可能被严格校验拒绝。

## 兼容范围

- `.gitattributes` 将两个迁移目录的 SQL 统一为 LF；运行入口仍对嵌入脚本做 LF 规范化，
  避免已有工作区尚未重新检出时产生新的 CRLF 校验记录。
- 每个已执行版本只接受该脚本的 LF 或 CRLF 两种完整字节表示。不同版本可分别来自
  不同换行构建；不忽略空白、注释或任意 SQL 内容变更。
- 检查所有已执行版本后才允许运行待执行迁移。未知版本、损坏的校验值、未完成迁移
  继续报错，不通过重建数据库或清空历史来恢复。
- 只调整本次运行的内存中预期校验值，不修改 `_sqlx_migrations` 中原有的 checksum、
  installed_on、execution_time、description 或 success。新执行迁移记录使用 LF 校验值。
- 索引库与正文库均沿用 `NativeSqliteBackend` 的文件迁移锁；兼容检查与 SQLx 执行
  使用同一个连接。迁移的事务和错误处理继续交由 SQLx 完成。

## 边界与验证

此逻辑仅属于 SQLite adapter，不改变 application/domain 语义，也不适用于 BrowserStore。
它解决新版本打开历史换行构建数据库的问题，不扩展数据库 schema 的降级支持。

`migration_compat` 单测覆盖新库规范化、严格校验失败的复现、两种换行兼容与拒绝错误迁移。
`test_persistence_upgrade` 在磁盘上分别创建 LF、CRLF、按版本混用换行的旧索引库／正文库，
执行升级并关闭重开，核对历史迁移记录以及订阅、文章、正文、阅读／收藏／归档、设置与状态。

修改历史 SQL 时不得把兼容层扩展为无条件接受校验失败；正常 schema 变更仍需新增迁移。
