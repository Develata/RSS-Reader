# v0.1.x 迁移（冻结）

本目录与 `../migrations_content/` 是 v0.1.0–v0.1.23 由 sqlx 应用的历史迁移，**冻结、不得修改**：

- v0.2 起原生端使用单个资料库 `library.sqlite3`，其基线 schema 在 `crates/rssr-infra/src/store/migrations/`；
- 这里的文件只供 `rssr_infra::store::import` 核对旧库的迁移记录（SHA-384，LF 或 CRLF）与测试构造 v0.1.x 夹具；改动任何一个字节都会让真实旧库的导入被拒绝；
- `0008`、`0009` 从未发布，只用于确认开发版程序建立的库会被拒绝。
