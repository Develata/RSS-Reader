# SQLite 迁移校验与历史换行兼容

当前 v0.2 使用 `library.sqlite3` 与 rusqlite。迁移入口是
`crates/rssr-infra/src/store/migrate.rs`；v0.1.x 双库只通过 `store/import.rs`
只读导入，原库及其 `_sqlx_migrations` 记录保留。

## 当前资料库

- `store/migrations/0001_baseline.sql` 是已发布基线，不改写其内容或校验历史。
- `0002_manual_archive.sql` 增加独立手动归档状态，已有文章默认未手动归档；
  自动归档仍由时间查询决定，关闭自动归档不会丢失手动归档状态。
- SHA-384 校验前将 SQL 换行为 LF，使 Windows / Unix 一致；不忽略 SQL、空白或注释修改。
- 历史必须是已知迁移的连续前缀；未知版本、缺失版本和校验不符拒绝打开。
  待执行迁移及其历史在单个事务中提交，失败回滚。

## v0.1.x 导入

- 根目录 `migrations/` 与 `migrations_content/` 冻结为历史，不添加新功能迁移。
- 导入器仅接受已发布历史及各脚本的 LF / CRLF 两种完整校验值，拒绝未发布的
  开发 schema、损坏记录和未成功迁移，不修改原库的校验记录。
- 在暂存资料库中映射并校验订阅、文章、正文、标记、配置与刷新序号，然后原子发布。
  不以清空资料库或绕过校验来修复错误。

## 验证入口

`store::migrate` 单测覆盖换行一致、非法历史、事务回滚和已发布基线升级；
`test_library_baseline_schema` 核对冻结基线及增量迁移；
`test_library_import` 核对只读旧库导入；`test_library_entry_state_and_search`
核对手动 / 自动归档组合、事务与重开持久化。

平台路径差异仅在 infra adapter 内处理；application/domain 共用同一仓储语义。
